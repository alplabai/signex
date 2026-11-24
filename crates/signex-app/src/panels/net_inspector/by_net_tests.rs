//! Mockup row 58: a schematic By Net row shows Highlight while the pointer
//! is over it, as a board row has it; the press highlights the net and does
//! not fold or unfold the row.

use super::*;
use crate::layout_probe::{lay_out_texts, messages_on_click};

const WINDOW: iced::Size = iced::Size::new(1200.0, 800.0);

fn net(name: &str) -> NetRow {
    NetRow {
        name: name.to_string(),
        class: None,
        pins: 2,
        expanded: false,
        members: Vec::new(),
        more: 0,
    }
}

fn view_hovering(hovered: Option<&str>) -> NetInspectorView {
    NetInspectorView {
        nets: vec![net("GND"), net("VOUT_3V3")],
        total: 2,
        hovered_net: hovered.map(str::to_string),
        ..NetInspectorView::default()
    }
}

fn texts(ctx: &PanelContext, view: &NetInspectorView) -> Vec<String> {
    lay_out_texts(list(ctx, view), WINDOW)
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

#[test]
fn the_hovered_net_row_shows_highlight_and_its_press_highlights_the_net() {
    let (app, _boot) = crate::app::Signex::new();
    let ctx = app.document_state.panel_ctx.clone();
    let quiet = view_hovering(None);
    assert!(
        !texts(&ctx, &quiet).iter().any(|t| t == "Highlight"),
        "no row is hovered"
    );
    let hovered = view_hovering(Some("VOUT_3V3"));
    let shown = texts(&ctx, &hovered);
    assert_eq!(
        shown.iter().filter(|t| *t == "Highlight").count(),
        1,
        "only the hovered row: {shown:?}"
    );
    // The probe's fresh tree sees the pointer enter the row first; the press
    // itself sends only the Highlight, not the row's fold.
    let pressed = messages_on_click(list(&ctx, &hovered), WINDOW, "Highlight");
    assert!(
        matches!(
            pressed.as_slice(),
            [
                PanelMsg::NetInspector(NetInspectorMsg::NetRowEntered(entered)),
                PanelMsg::NetInspector(NetInspectorMsg::NetHighlighted(net)),
            ] if entered == "VOUT_3V3" && net == "VOUT_3V3"
        ),
        "{pressed:?}"
    );
}

#[test]
fn a_press_on_the_net_name_still_folds_the_row() {
    let (app, _boot) = crate::app::Signex::new();
    let ctx = app.document_state.panel_ctx.clone();
    let view = view_hovering(None);
    let pressed = messages_on_click(list(&ctx, &view), WINDOW, "GND");
    assert!(
        matches!(
            pressed.as_slice(),
            [
                PanelMsg::NetInspector(NetInspectorMsg::NetRowEntered(entered)),
                PanelMsg::NetInspector(NetInspectorMsg::NetToggled(net)),
            ] if entered == "GND" && net == "GND"
        ),
        "{pressed:?}"
    );
}
