//! #1280: the harness rules, and the rules that must read a harness as a
//! connection (ports, typed sheet entries and wires into entries).

use std::sync::Arc;

use signex_types::schematic::{
    ChildSheet, FillType, HarnessWire, Label, LabelType, Point, SchematicSheet, SelectedKind,
    SheetPin, Wire,
};
use uuid::Uuid;

use crate::context::ErcContext;
use crate::fixtures::{harness_connector, harness_definition};
use crate::pin_matrix::ErcConfig;
use crate::{RuleKind, engine};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn found(ctx: &ErcContext, kind: RuleKind) -> Vec<crate::Diagnostic> {
    engine::run_all(ctx)
        .into_iter()
        .filter(|d| d.rule_kind == kind)
        .collect()
}

/// The messages of `kind` with the project's harness `definitions`.
fn found_with_definitions(
    ctx: &ErcContext,
    definitions: &[signex_types::schematic::HarnessDefinition],
    kind: RuleKind,
) -> Vec<String> {
    let config = ErcConfig::default().with_harness_definitions(definitions);
    engine::run_all_with_config(ctx, &config)
        .into_iter()
        .filter(|d| d.rule_kind == kind)
        .map(|d| d.message)
        .collect()
}

fn harness_wire(a: Point, b: Point) -> HarnessWire {
    HarnessWire {
        uuid: Uuid::now_v7(),
        start: a,
        end: b,
        locked: false,
    }
}

/// `mcu` with a connector of `names` whose harness runs to the typed entry
/// `SPI_BUS` of the sheet symbol `sensor`.
fn parent(names: &[&str]) -> SchematicSheet {
    let mut s = SchematicSheet::default();
    let c = harness_connector("SPI_BUS", pt(10.0, 10.0), names);
    let to = pt(60.0, 20.0);
    s.harness.wires.push(harness_wire(c.tip_point(), to));
    let pin = SheetPin {
        uuid: Uuid::now_v7(),
        name: "SPI_BUS".into(),
        direction: String::new(),
        position: to,
        rotation: 0.0,
        auto_generated: false,
        user_moved: false,
    };
    s.harness.entry_types.insert(pin.uuid, "SPI_BUS".into());
    s.child_sheets.push(ChildSheet {
        uuid: Uuid::now_v7(),
        name: "sensor".into(),
        filename: "sensor.snxsch".into(),
        position: pt(60.0, 10.0),
        size: (20.0, 20.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: vec![pin],
        instances: Vec::new(),
    });
    s.harness.connectors.push(c);
    s
}

/// `sensor` with a port `SPI_BUS` on a harness to a connector of `names`.
fn child(names: &[&str]) -> SchematicSheet {
    let mut s = SchematicSheet::default();
    let c = harness_connector("SPI_BUS", pt(30.0, 10.0), names);
    let port = pt(5.0, 5.0);
    s.harness.wires.push(harness_wire(port, c.tip_point()));
    s.labels
        .push(Label::new("SPI_BUS", port, LabelType::Hierarchical));
    s.harness.connectors.push(c);
    s
}

fn with_child(parent: &SchematicSheet, child: &SchematicSheet) -> ErcContext {
    let mut ctx = ErcContext::from_snapshot(parent);
    ctx.children.insert(
        "sensor.snxsch".into(),
        Arc::new(ErcContext::from_snapshot(child)),
    );
    ctx
}

#[test]
fn an_entry_the_far_side_lacks_is_named_with_the_one_expected() {
    let ctx = with_child(&parent(&["SCK", "CS"]), &child(&["SCK", "CS_ADC"]));
    let hits = found(&ctx, RuleKind::HarnessEntryMismatch);
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(
        hits[0].message,
        "sensor expects entry CS_ADC; this harness has CS"
    );
    let cs = &ctx.harness.connectors[0].entries[1];
    let primary = hits[0].primary.expect("on the entry");
    assert_eq!(primary.uuid, cs.uuid);
    assert_eq!(primary.kind, SelectedKind::HarnessEntry);
}

#[test]
fn matching_sides_report_nothing() {
    let ctx = with_child(&parent(&["SCK", "CS"]), &child(&["CS", "SCK"]));
    assert!(found(&ctx, RuleKind::HarnessEntryMismatch).is_empty());
}

#[test]
fn an_entry_the_definition_does_not_list_is_reported() {
    let ctx = ErcContext::from_snapshot(&child(&["SCK", "CS", "IRQ"]));
    let messages = found_with_definitions(
        &ctx,
        &[harness_definition("SPI_BUS", &["SCK", "CS"])],
        RuleKind::HarnessEntryMismatch,
    );
    assert_eq!(messages, ["SPI_BUS defines no entry IRQ"]);
}

#[test]
fn a_definition_entry_missing_on_the_connector_names_both() {
    let ctx = ErcContext::from_snapshot(&child(&["SCK", "CS"]));
    let messages = found_with_definitions(
        &ctx,
        &[harness_definition("SPI_BUS", &["SCK", "CS_ADC"])],
        RuleKind::HarnessEntryMismatch,
    );
    assert_eq!(
        messages,
        ["SPI_BUS expects entry CS_ADC; this harness has CS"]
    );
    // Without definitions the check is skipped.
    let bare = ErcContext::from_snapshot(&child(&["SCK", "CS"]));
    assert!(found(&bare, RuleKind::HarnessEntryMismatch).is_empty());
}

#[test]
fn two_types_on_one_harness_conflict() {
    let ctx = crate::fixtures::context_for(RuleKind::HarnessTypeConflict);
    let hits = found(&ctx, RuleKind::HarnessTypeConflict);
    assert_eq!(hits.len(), 1);
    assert_eq!(
        hits[0].message,
        "Harness types SPI_BUS and UART meet on one harness"
    );
    assert!(hits[0].peer.is_some());
}

#[test]
fn a_port_and_a_typed_entry_on_a_harness_are_connected() {
    let ctx = with_child(&parent(&["SCK"]), &child(&["SCK"]));
    assert!(found(&ctx, RuleKind::SheetPinUnconnected).is_empty());
    let child_ctx = ErcContext::from_snapshot(&child(&["SCK"]));
    assert!(found(&child_ctx, RuleKind::HierPortDisconnected).is_empty());
}

#[test]
fn a_wire_into_a_harness_entry_does_not_dangle() {
    let mut s = child(&["SCK"]);
    let at = s.harness.connectors[0].entry_point(0);
    s.wires.push(Wire {
        uuid: Uuid::now_v7(),
        start: pt(at.x - 10.0, at.y),
        end: at,
        ..Default::default()
    });
    let ctx = ErcContext::from_snapshot(&s);
    let dangling: Vec<_> = found(&ctx, RuleKind::DanglingWire)
        .into_iter()
        .filter(|d| (d.location.x - at.x).abs() < 1e-6)
        .collect();
    assert!(dangling.is_empty(), "{dangling:?}");
}
