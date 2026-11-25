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

/// #2743: an extra entry here, with nothing missing to pair it with.
#[test]
fn an_entry_only_the_parent_has_is_named() {
    let ctx = with_child(&parent(&["SCK", "CS", "CS2"]), &child(&["SCK", "CS"]));
    let hits = found(&ctx, RuleKind::HarnessEntryMismatch);
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(hits[0].message, "sensor has no entry CS2");
    let cs2 = &ctx.harness.connectors[0].entries[2];
    let primary = hits[0].primary.expect("on the entry");
    assert_eq!(primary.uuid, cs2.uuid);
    assert_eq!(primary.kind, SelectedKind::HarnessEntry);
    assert_eq!(hits[0].tag.as_deref(), Some("extra:sensor:CS2"));
}

#[test]
fn an_entry_only_the_child_has_is_named() {
    let ctx = with_child(&parent(&["SCK", "CS"]), &child(&["SCK", "CS", "CS2"]));
    let hits = found(&ctx, RuleKind::HarnessEntryMismatch);
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(
        hits[0].message,
        "sensor expects entry CS2; this harness lacks it"
    );
}

/// `sensor` passing the harness on (#2657): a port `SPI_BUS` on a harness
/// wire to the typed entry `SPI_BUS` of the sheet symbol `inner`, no
/// connector.
fn pass_through() -> SchematicSheet {
    let mut s = parent(&[]);
    s.harness.connectors.clear();
    let port = s.harness.wires[0].start;
    s.labels
        .push(Label::new("SPI_BUS", port, LabelType::Hierarchical));
    s.child_sheets[0].name = "inner".into();
    s.child_sheets[0].filename = "inner.snxsch".into();
    s
}

/// `parent` -> `sensor` (passing the harness on) -> `inner`, when given.
fn through_sensor(parent: &SchematicSheet, inner: Option<&SchematicSheet>) -> ErcContext {
    let mut sensor = ErcContext::from_snapshot(&pass_through());
    if let Some(inner) = inner {
        sensor.children.insert(
            "inner.snxsch".into(),
            Arc::new(ErcContext::from_snapshot(inner)),
        );
    }
    let mut ctx = ErcContext::from_snapshot(parent);
    ctx.children
        .insert("sensor.snxsch".into(), Arc::new(sensor));
    ctx
}

#[test]
fn a_harness_carried_through_a_child_is_matched_against_the_grandchild() {
    let ctx = through_sensor(&parent(&["SCK", "CS"]), Some(&child(&["CS", "SCK"])));
    assert!(found(&ctx, RuleKind::HarnessEntryMismatch).is_empty());
}

#[test]
fn a_carried_harness_names_the_entry_the_grandchild_expects() {
    let ctx = through_sensor(&parent(&["SCK", "CS"]), Some(&child(&["SCK", "CS_ADC"])));
    let messages: Vec<String> = found(&ctx, RuleKind::HarnessEntryMismatch)
        .into_iter()
        .map(|d| d.message)
        .collect();
    assert_eq!(
        messages,
        ["sensor expects entry CS_ADC; this harness has CS"]
    );
}

#[test]
fn matching_nested_entries_carried_through_a_child_report_nothing() {
    let mut near = parent(&["UART"]);
    nest_uart(&mut near, &["TX", "RX"]);
    let mut far = child(&["UART"]);
    nest_uart(&mut far, &["RX", "TX"]);
    let hits = found(
        &through_sensor(&near, Some(&far)),
        RuleKind::HarnessEntryMismatch,
    );
    assert!(hits.is_empty(), "{hits:?}");
}

/// The 2026-11-23 rule, kept where the carry reaches no connector: the
/// signals stop at the child, so each entry is reported.
#[test]
fn a_carry_that_reaches_no_connector_reports_every_entry() {
    let ctx = through_sensor(&parent(&["SCK", "CS"]), None);
    let mut messages: Vec<String> = found(&ctx, RuleKind::HarnessEntryMismatch)
        .into_iter()
        .map(|d| d.message)
        .collect();
    messages.sort();
    assert_eq!(
        messages,
        ["sensor has no entry CS", "sensor has no entry SCK"]
    );
}

/// `sheet`'s connector gets a `UART` connector of `names` nested at entry 0.
fn nest_uart(sheet: &mut SchematicSheet, names: &[&str]) {
    let at = sheet.harness.connectors[0].entry_point(0);
    let inner = harness_connector("UART", pt(at.x - 40.0, at.y + 40.0), names);
    sheet
        .harness
        .wires
        .push(harness_wire(inner.tip_point(), at));
    sheet.harness.connectors.push(inner);
}

#[test]
fn a_nested_entry_only_the_parent_has_is_named_by_its_path() {
    let mut near = parent(&["UART"]);
    nest_uart(&mut near, &["TX", "RX"]);
    let mut far = child(&["UART"]);
    nest_uart(&mut far, &["TX"]);
    let hits = found(&with_child(&near, &far), RuleKind::HarnessEntryMismatch);
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(hits[0].message, "sensor has no entry UART.RX");
}

#[test]
fn a_nested_entry_only_the_child_has_is_named_by_its_path() {
    let mut near = parent(&["UART"]);
    nest_uart(&mut near, &["TX"]);
    let mut far = child(&["UART"]);
    nest_uart(&mut far, &["TX", "RX"]);
    let hits = found(&with_child(&near, &far), RuleKind::HarnessEntryMismatch);
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(
        hits[0].message,
        "sensor expects entry UART.RX; this harness lacks it"
    );
}

/// A rename of the top entry would move every leaf under it, so a nested
/// difference is never offered as one.
#[test]
fn a_nested_difference_is_never_paired_as_a_rename() {
    let mut near = parent(&["UART"]);
    nest_uart(&mut near, &["TX", "RTS"]);
    let mut far = child(&["UART"]);
    nest_uart(&mut far, &["TX", "RX"]);
    let hits = found(&with_child(&near, &far), RuleKind::HarnessEntryMismatch);
    let mut messages: Vec<&str> = hits.iter().map(|d| d.message.as_str()).collect();
    messages.sort_unstable();
    assert_eq!(
        messages,
        [
            "sensor expects entry UART.RX; this harness lacks it",
            "sensor has no entry UART.RTS",
        ]
    );
}

#[test]
fn matching_nested_entries_report_nothing() {
    let mut near = parent(&["UART"]);
    nest_uart(&mut near, &["TX", "RX"]);
    let mut far = child(&["UART"]);
    nest_uart(&mut far, &["RX", "TX"]);
    let hits = found(&with_child(&near, &far), RuleKind::HarnessEntryMismatch);
    assert!(hits.is_empty(), "{hits:?}");
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
