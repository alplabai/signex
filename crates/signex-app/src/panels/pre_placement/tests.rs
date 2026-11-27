//! The pre-placement form as laid out (#2532): its rows come from the
//! catalog, the Location fields are editable, Rotation is a 4-step control
//! and the justification grid names its anchor.

use super::super::{PanelContext, PanelKind, PanelMsg, view_panel};
use super::*;
use crate::layout_probe::{LaidOutText, lay_out_texts, messages_on_click};
use signex_types::schematic::{HAlign, VAlign, WireWidthPreset};

const WINDOW: iced::Size = iced::Size::new(360.0, 6000.0);

/// A schematic panel context with the Tab form of `kind` open.
fn form(kind: PrePlacementKind, tool: &str) -> PanelContext {
    let (app, _boot) = crate::app::Signex::new();
    let mut ctx = app.document_view.panel_ctx.clone();
    ctx.has_schematic = true;
    ctx.pre_placement = Some(PrePlacementData::new(
        tool.to_owned(),
        kind,
        "SPI_CLK".to_owned(),
        101.6,
        76.2,
    ));
    ctx
}

fn texts(ctx: &PanelContext) -> Vec<String> {
    lay_out_texts(view_panel(PanelKind::Properties, ctx), WINDOW)
        .into_iter()
        .map(|t: LaidOutText| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

fn has(shown: &[String], text: &str) -> bool {
    shown.iter().any(|t| t == text)
}

#[test]
fn the_net_label_form_shows_its_header_caption_sections_and_rows() {
    let shown = texts(&form(PrePlacementKind::NetLabel, "Net Label"));
    for label in [
        "Place Net Label",
        "Applies to the next object placed",
        "Enter",
        "Esc",
        "back to placing",
        "LOCATION",
        "PROPERTIES",
        "Position",
        "X",
        "Y",
        "Rotation",
        "0°",
        "90°",
        "180°",
        "270°",
        "Net Name",
        "Font",
        "Font Size",
        "Justification",
    ] {
        assert!(has(&shown, label), "{label:?} missing from {shown:#?}");
    }
    assert!(!has(&shown, "(X/Y)"), "the read-only readout is gone");
}

#[test]
fn the_wire_form_has_start_end_and_a_read_only_length() {
    let shown = texts(&form(PrePlacementKind::Wire, "Wire"));
    for label in [
        "Start",
        "End",
        "Length",
        "0.000",
        "Length is derived from Start and End",
        "Smallest",
        "Small",
        "Medium",
        "Large",
    ] {
        assert!(has(&shown, label), "{label:?} missing from {shown:#?}");
    }
    assert!(!has(&shown, "Position"), "{shown:#?}");
}

/// The Location fields and the Length show the panel's unit, as the placed
/// rows do: a panel in mils shows the cursor in mils with "mil" beside it.
#[test]
fn the_location_fields_show_the_panels_unit() {
    use signex_types::coord::Unit;
    let mut ctx = form(PrePlacementKind::Wire, "Wire");
    ctx.unit = Unit::Mil;
    let pp = ctx.pre_placement.clone().expect("form");
    let ([x, ..], _) = location_texts(&pp, &ctx);
    assert_eq!(x, LengthFormat::new(&ctx.i18n, Unit::Mil).field_text(101.6));
    let shown = texts(&ctx);
    let mils = shown.iter().filter(|t| *t == "mil").count();
    assert!(mils >= 5, "four coordinates and the Length: {shown:#?}");
}

#[test]
fn a_tool_without_options_shows_the_card_and_no_rotation() {
    let shown = texts(&form(PrePlacementKind::Other, "Junction"));
    assert!(has(&shown, "No other options"), "{shown:#?}");
    assert!(
        has(
            &shown,
            "Set the position above, or click on the sheet to place."
        ),
        "{shown:#?}"
    );
    assert!(has(&shown, "Position"), "{shown:#?}");
    assert!(!has(&shown, "Rotation"), "{shown:#?}");
}

#[test]
fn the_justification_grid_names_its_anchor() {
    let mut ctx = form(PrePlacementKind::NetLabel, "Net Label");
    if let Some(pp) = ctx.pre_placement.as_mut() {
        pp.justify_h = HAlign::Left;
        pp.justify_v = VAlign::Bottom;
    }
    assert!(has(&texts(&ctx), "Bottom left"));
}

#[test]
fn a_rotation_step_hands_back_its_degrees() {
    let ctx = form(PrePlacementKind::NetLabel, "Net Label");
    // The step number is a message argument, so it sits in bidi isolation marks.
    let label = "\u{2068}90\u{2069}°";
    let picked: Vec<f64> =
        messages_on_click(view_panel(PanelKind::Properties, &ctx), WINDOW, label)
            .into_iter()
            .filter_map(|m| match m {
                PanelMsg::SetPrePlacementRotation(deg) => Some(deg),
                _ => None,
            })
            .collect();
    assert_eq!(picked, [90.0]);
}

#[test]
fn a_width_preset_hands_back_its_preset() {
    let ctx = form(PrePlacementKind::Wire, "Wire");
    let picked: Vec<WireWidthPreset> =
        messages_on_click(view_panel(PanelKind::Properties, &ctx), WINDOW, "Large")
            .into_iter()
            .filter_map(|m| match m {
                PanelMsg::SetPrePlacementWireWidth(p) => Some(p),
                _ => None,
            })
            .collect();
    assert_eq!(picked, [WireWidthPreset::Large]);
}

#[test]
fn the_ok_button_confirms() {
    let ctx = form(PrePlacementKind::Other, "Junction");
    let sent = messages_on_click(view_panel(PanelKind::Properties, &ctx), WINDOW, "OK");
    assert!(
        sent.iter()
            .any(|m| matches!(m, PanelMsg::ConfirmPrePlacement)),
        "{sent:?}"
    );
}

#[test]
fn only_tools_without_text_shape_or_wire_options_get_the_card() {
    for kind in [
        PrePlacementKind::NoConnect,
        PrePlacementKind::BusEntry,
        PrePlacementKind::Bus,
        PrePlacementKind::Other,
    ] {
        assert!(has_no_options(kind), "{kind:?}");
    }
    for kind in [
        PrePlacementKind::Wire,
        PrePlacementKind::NetLabel,
        PrePlacementKind::PowerPort,
        PrePlacementKind::Component,
        PrePlacementKind::Rectangle,
    ] {
        assert!(!has_no_options(kind), "{kind:?}");
    }
}
