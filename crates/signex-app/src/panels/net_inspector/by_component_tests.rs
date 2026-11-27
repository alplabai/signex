//! The By Component pin rows: each pin shows its net, its pin name and the
//! sheet its unit is on.

use signex_net::{NetAnchor, SheetKey};
use signex_types::schematic::InstancePath;

use super::*;
use crate::layout_probe::lay_out_texts;

const WINDOW: iced::Size = iced::Size::new(2000.0, 800.0);

fn pin(number: &str, name: &str, net: Option<&str>, sheet: &str) -> ComponentPinRow {
    ComponentPinRow {
        pin: number.to_string(),
        name: name.to_string(),
        net: net.map(str::to_string),
        sheet: sheet.to_string(),
        anchor: NetAnchor::new(
            SheetKey::new(sheet),
            uuid::Uuid::now_v7(),
            InstancePath::root(),
        ),
    }
}

/// `U1` drawn as two units, unit A on the Power sheet and unit B on the
/// Logic sheet, expanded.
fn two_sheet_part() -> NetInspectorView {
    NetInspectorView {
        tab: crate::panels::net_inspector::NetInspectorTab::ByComponent,
        components: vec![ComponentRow {
            key: "U1".into(),
            designator: "U1".into(),
            value: "LM358".into(),
            footprint: "SOIC-8".into(),
            pins: 3,
            expanded: true,
            pin_rows: vec![
                pin("1", "OUTA", Some("VSENSE"), "Power"),
                pin("5", "INB+", Some("LOGIC_IN"), "Logic"),
                pin("7", "OUTB", None, "Logic"),
            ],
            more: 0,
        }],
        component_total: 1,
        ..NetInspectorView::default()
    }
}

fn texts(view: &NetInspectorView) -> Vec<String> {
    let (app, _boot) = crate::app::Signex::new();
    let ctx = app.document_view.panel_ctx.clone();
    lay_out_texts(list(&ctx, view), WINDOW)
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

#[test]
fn a_two_sheet_multi_unit_part_shows_each_pins_sheet_and_name() {
    let shown = texts(&two_sheet_part());
    assert!(shown.iter().any(|t| t == "Sheet"), "the heading: {shown:?}");
    // Each pin row reads number, net, pin name, sheet in that order.
    for row in [
        ["1", "VSENSE", "OUTA", "Power"],
        ["5", "LOGIC_IN", "INB+", "Logic"],
        ["7", "— unconnected", "OUTB", "Logic"],
    ] {
        assert!(
            shown.windows(4).any(|cells| cells == row),
            "the row {row:?} in {shown:?}"
        );
    }
}
