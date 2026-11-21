//! The Pad Properties header and form (#2262, mockup row 125): the selected pad, its
//! kind and its place among the footprint's pads; what each pad kind shows
//! and how its enable headers write the pad.

use std::path::PathBuf;

use super::pad::selected_pad_heading;
use crate::app::{FootprintDocument, Message, Signex, TabInfo, TabKind};
use crate::library::editor::footprint::{PadSide, PadStackTab};
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use crate::panels::{FootprintEditorPanelContext, PanelMsg};
use signex_library::{Footprint, FootprintFile, Pad, PadKind};

const PATH: &str = "pad-heading.snxfpt";

/// One footprint edit, sent through `Signex::update` as the canvas sends it.
fn edit(app: &mut Signex, msg: FootprintEditorMsg) {
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: PathBuf::from(PATH),
        msg: PrimitiveEdit::Footprint(msg),
    }));
}

/// One Properties panel message, sent through `Signex::update`.
fn panel(app: &mut Signex, msg: PanelMsg) {
    let _ = app.update(Message::Dock(crate::dock::DockMessage::Panel(msg)));
}

fn panel_ctx(app: &Signex) -> FootprintEditorPanelContext {
    app.document_state
        .panel_ctx
        .footprint_editor
        .clone()
        .expect("the footprint editor panel context is built")
}

/// An app with a footprint editor tab open on eight saved pads of `kind`,
/// nothing selected.
fn app_with_eight(kind: PadKind) -> Signex {
    let path = PathBuf::from(PATH);
    // Pads go in through the library primitive, as a loaded file brings
    // them, so the test does not reach into the editor's internals.
    let mut footprint = Footprint::empty("SOIC-8");
    footprint.pads = (1..=8)
        .map(|n| Pad {
            number: n.to_string(),
            size: [1.0, 1.0],
            position: [f64::from(n), 0.0],
            kind,
            ..Pad::default()
        })
        .collect();
    let file = FootprintFile::from_footprint(footprint);
    let editor = FootprintDocument::new(path.clone(), file);
    let (mut app, _task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: "SOIC-8".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;
    app
}

fn app_with_eight_pads() -> Signex {
    app_with_eight(PadKind::Smd)
}

/// The panel context of the eight-pad editor with pad 3 selected.
fn ctx_with_pad_3_selected() -> FootprintEditorPanelContext {
    let mut app = app_with_eight_pads();
    edit(&mut app, FootprintEditorMsg::SelectPad(Some(2)));
    panel_ctx(&app)
}

/// The header lines, with the isolation marks Fluent puts around arguments
/// removed: (title, kind, place).
fn shown(ctx: &FootprintEditorPanelContext) -> Option<(String, String, String)> {
    let i18n = signex_i18n::I18n::en_us();
    let plain = |s: String| s.replace(['\u{2068}', '\u{2069}'], "");
    selected_pad_heading(ctx, &i18n).map(|h| {
        (
            plain(h.title.to_string()),
            plain(h.kind.to_string()),
            plain(h.place.to_string()),
        )
    })
}

fn heading(title: &str, kind: &str, place: &str) -> Option<(String, String, String)> {
    Some((title.into(), kind.into(), place.into()))
}

#[test]
fn a_selected_pad_names_its_number_kind_and_place_among_the_pads() {
    let ctx = ctx_with_pad_3_selected();
    assert_eq!(shown(&ctx), heading("Pad 3", "SMD", "3 of 8 pads"));
}

/// A pad placed and not saved yet counts: the header numbers it among the
/// editor's pads, not the eight the file holds.
#[test]
fn an_unsaved_new_pad_is_counted_among_the_pads() {
    let mut app = app_with_eight_pads();
    edit(
        &mut app,
        FootprintEditorMsg::AddPad {
            x_mm: 20.0,
            y_mm: 0.0,
        },
    );
    edit(&mut app, FootprintEditorMsg::SelectPad(Some(8)));
    let ctx = panel_ctx(&app);
    assert_eq!(ctx.editor_pad_count, 9);
    let number = ctx
        .selected_pad
        .as_ref()
        .expect("the new pad is selected")
        .number
        .clone();
    assert_eq!(
        shown(&ctx),
        heading(&format!("Pad {number}"), "SMD", "9 of 9 pads")
    );
}

#[test]
fn a_footprint_with_one_pad_says_pad_not_pads() {
    let mut ctx = ctx_with_pad_3_selected();
    ctx.editor_pad_count = 1;
    ctx.selected_pad.as_mut().expect("pad 3 is selected").idx = 0;
    assert_eq!(shown(&ctx), heading("Pad 3", "SMD", "1 of 1 pad"));
}

#[test]
fn no_selected_pad_keeps_the_footprint_header() {
    let mut ctx = ctx_with_pad_3_selected();
    ctx.selected_pad = None;
    assert_eq!(shown(&ctx), None);
}

#[test]
fn a_pad_index_past_the_pad_count_keeps_the_footprint_header() {
    let mut ctx = ctx_with_pad_3_selected();
    ctx.editor_pad_count = 2;
    assert_eq!(shown(&ctx), None);
}

#[test]
fn a_running_placement_keeps_the_footprint_header() {
    let mut ctx = ctx_with_pad_3_selected();
    ctx.placement_active = true;
    assert_eq!(shown(&ctx), None);
    ctx.placement_active = false;
    ctx.placement_paused = true;
    assert_eq!(shown(&ctx), None);
}

const DOCK: iced::Size = iced::Size::new(260.0, 4000.0);

/// The Pad Properties view of `app`.
fn view(app: &Signex) -> iced::Element<'_, PanelMsg> {
    let ctx = &app.document_state.panel_ctx;
    let fp = ctx.footprint_editor.as_ref().expect("footprint context");
    let palette = crate::panels::PanelPalette::from_tokens(&ctx.tokens);
    let i18n = signex_i18n::I18n::en_us();
    super::view_footprint_editor_properties(
        fp,
        palette,
        &ctx.tokens,
        &ctx.collapsed_sections,
        signex_types::coord::Unit::Mm,
        &i18n,
    )
}

/// The texts of the Pad Properties view laid out at a dock's width, with
/// the isolation marks Fluent puts around arguments removed.
fn pad_texts(app: &Signex) -> Vec<String> {
    crate::layout_probe::lay_out_texts(view(app), DOCK)
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

fn assert_shows(texts: &[String], shown: &[&str], hidden: &[&str]) {
    for label in shown {
        assert!(
            texts.iter().any(|t| t == label),
            "{label:?} missing: {texts:#?}"
        );
    }
    for label in hidden {
        assert!(
            !texts.iter().any(|t| t == label),
            "{label:?} shown: {texts:#?}"
        );
    }
}

/// Clicks the text `label` (read as `pad_texts` reads it, without the
/// isolation marks) and sends the first message `pick` keeps.
fn click(app: &mut Signex, label: &str, pick: impl Fn(&PanelMsg) -> bool) {
    let at = crate::layout_probe::lay_out_texts(view(app), DOCK)
        .into_iter()
        .find(|t| t.text.replace(['\u{2068}', '\u{2069}'], "") == label)
        .unwrap_or_else(|| panic!("no text {label:?} in the pad form"))
        .bounds
        .center();
    let msg = crate::layout_probe::messages_on_click_at(view(app), DOCK, None, at)
        .into_iter()
        .find(|m| pick(m))
        .unwrap_or_else(|| panic!("a click on {label:?} sends the expected message"));
    panel(app, msg);
}

fn selected_pad_3(app: &Signex) -> &crate::library::editor::footprint::EditorPad {
    &app.document_state.footprint_editors[&PathBuf::from(PATH)]
        .state
        .pads[2]
}

/// Mockup rows 97 and 125: an SMD pad shows only its own layer. No pad stack
/// mode, no inner or bottom rows; that layer's paste and mask as enable
/// headers over their expansion; the read-only position as one X / Y row.
#[test]
fn an_smd_pad_shows_its_own_layer_and_no_pad_stack() {
    let mut app = app_with_eight_pads();
    edit(&mut app, FootprintEditorMsg::SelectPad(Some(2)));
    let texts = pad_texts(&app);
    assert_shows(
        &texts,
        &[
            "Pad 3",
            "SMD",
            "3 of 8 pads",
            "Edit in Sketch ▸",
            "Pad · Top Layer",
            "Size X / Y",
            "Copper offset",
            "Top Paste",
            "Top Solder mask",
            "opening",
            "Expansion",
            "Electrical Type",
            "X / Y (mm)",
            "3.000",
            "0.000",
            "Pad Features",
        ],
        &[
            "Pad Stack",
            "Simple",
            "Top-Middle-Bottom",
            "Full Stack",
            "Bottom Paste",
            "Bottom Solder mask",
            "Inner",
            "Hole Details",
            "PASTE",
            "SOLDER",
            // Pad Features starts closed.
            "Top Assembly",
        ],
    );
}

/// The Top Paste header is the pad's paste switch.
#[test]
fn the_top_paste_header_switches_the_pads_paste() {
    let mut app = app_with_eight_pads();
    edit(&mut app, FootprintEditorMsg::SelectPad(Some(2)));
    let before = selected_pad_3(&app).stack.paste_enabled_top;
    click(&mut app, "Top Paste", |m| {
        matches!(
            m,
            PanelMsg::FpEditorToggleSelectedPadPasteEnabledTop { idx: 2, .. }
        )
    });
    assert_ne!(selected_pad_3(&app).stack.paste_enabled_top, before);
    assert_shows(&pad_texts(&app), &["off"], &[]);
}

/// The solder mask header is on while the mask opens over the pad: clearing
/// it tents the pad, the inverse of the stored flag.
#[test]
fn the_solder_mask_header_tents_the_pad_when_cleared() {
    let mut app = app_with_eight_pads();
    edit(&mut app, FootprintEditorMsg::SelectPad(Some(2)));
    assert!(!selected_pad_3(&app).stack.mask_tented_top);
    click(&mut app, "Top Solder mask", |m| {
        matches!(
            m,
            PanelMsg::FpEditorToggleSelectedPadMaskTentedTop { idx: 2, .. }
        )
    });
    assert!(
        selected_pad_3(&app).stack.mask_tented_top,
        "cleared = tented"
    );
    assert_shows(&pad_texts(&app), &["tented"], &["opening"]);
}

/// A through-hole pad is Multi-Layer with a pad stack, hole details and a
/// thermal relief; both mask layers and no paste.
#[test]
fn a_through_hole_pad_shows_multi_layer_its_pad_stack_and_no_paste() {
    let mut app = app_with_eight_pads();
    edit(&mut app, FootprintEditorMsg::SelectPad(Some(2)));
    panel(
        &mut app,
        PanelMsg::FpEditorSetSelectedPadSide {
            idx: 2,
            side: PadSide::All,
        },
    );
    let texts = pad_texts(&app);
    assert_shows(
        &texts,
        &[
            "Through-hole",
            "Multi-Layer",
            "Pad Stack",
            "Simple",
            "Top-Middle-Bottom",
            "Full Stack",
            "Layers",
            "Hole Details",
            "Plated",
            "Thermal relief",
            "Air gap",
            "Spoke width",
            "Spokes",
            "Top Solder mask",
            "Bottom Solder mask",
            "No paste on through-hole pads and holes.",
            "Copper offset",
            "Electrical Type",
        ],
        &["Top Paste", "Bottom Paste", "Pad · Top Layer"],
    );
}

/// Full Stack names the inner copper by position, L1 being Top and Ln
/// Bottom; Top-Middle-Bottom calls it Inner.
#[test]
fn the_full_stack_names_inner_copper_by_position() {
    let mut app = app_with_eight_pads();
    edit(&mut app, FootprintEditorMsg::SelectPad(Some(2)));
    panel(
        &mut app,
        PanelMsg::FpEditorSetSelectedPadSide {
            idx: 2,
            side: PadSide::All,
        },
    );
    panel(
        &mut app,
        PanelMsg::FpEditorSetPadStackTab(PadStackTab::TopMiddleBottom),
    );
    assert_shows(
        &pad_texts(&app),
        &["Top Layer", "Inner", "Bottom Layer"],
        &["L2…Ln−1"],
    );
    panel(
        &mut app,
        PanelMsg::FpEditorSetPadStackTab(PadStackTab::FullStack),
    );
    assert_shows(
        &pad_texts(&app),
        &["Top Layer", "L2…Ln−1", "Bottom Layer"],
        &["Inner"],
    );
}

/// A non-plated hole has no copper: no pad stack, copper offset, electrical
/// type, thermal relief or paste; Plated is shown cleared.
#[test]
fn a_non_plated_hole_hides_what_needs_copper() {
    let mut app = app_with_eight(PadKind::NptHole);
    edit(&mut app, FootprintEditorMsg::SelectPad(Some(2)));
    assert_shows(
        &pad_texts(&app),
        &[
            "NPTH",
            "Hole Details",
            "Plated",
            "Top Solder mask",
            "Bottom Solder mask",
        ],
        &[
            "Electrical Type",
            "Copper offset",
            "Pad Stack",
            "Pad · Top Layer",
            "Thermal relief",
            "Top Paste",
        ],
    );
}

/// A size the pad does not take keeps the stored size and shows why.
#[test]
fn a_refused_size_shows_its_reason_and_keeps_the_pad() {
    let mut app = app_with_eight_pads();
    edit(&mut app, FootprintEditorMsg::SelectPad(Some(2)));
    let size = |app: &Signex| selected_pad_3(app).size_mm;
    let before = size(&app);
    panel(
        &mut app,
        PanelMsg::FpEditorSetSelectedPadSizeX {
            idx: 2,
            value: "1,5".into(),
        },
    );
    assert_eq!(size(&app), before);
    assert_shows(&pad_texts(&app), &["Enter a number"], &[]);
    panel(
        &mut app,
        PanelMsg::FpEditorSetSelectedPadSizeX {
            idx: 2,
            value: "0".into(),
        },
    );
    assert_eq!(size(&app), before);
    assert_shows(
        &pad_texts(&app),
        &["Must be greater than 0"],
        &["Enter a number"],
    );
    panel(
        &mut app,
        PanelMsg::FpEditorSetSelectedPadSizeX {
            idx: 2,
            value: "2".into(),
        },
    );
    assert_eq!(size(&app).0, 2.0);
    assert_shows(
        &pad_texts(&app),
        &[],
        &["Must be greater than 0", "Enter a number"],
    );
}

/// Pad Features opens on its header and lists every shipped control.
#[test]
fn pad_features_open_on_their_header() {
    let mut app = app_with_eight_pads();
    edit(&mut app, FootprintEditorMsg::SelectPad(Some(2)));
    click(
        &mut app,
        "Pad Features",
        |m| matches!(m, PanelMsg::ToggleSection(key) if key == "fp_pad_features_open"),
    );
    assert_shows(
        &pad_texts(&app),
        &[
            "Top Side",
            "Bottom Side",
            "Testpoint",
            "Top Assembly",
            "Top Fab",
            "Bottom Assembly",
            "Bottom Fab",
        ],
        &[],
    );
}

/// The paste and solder mask sections of the pad form name their layers as
/// the Layers panel does: a paste section is the layer's catalog name, a
/// solder mask section the catalog name with "mask"; the copper table's
/// outer rows are the copper layers' catalog names.
#[test]
fn the_per_side_rows_use_the_layer_catalog_names() {
    use crate::pcb_canvas::layer_style::layer_name;
    use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer as T};
    let i18n = signex_i18n::I18n::en_us();
    let name = |layer: T| layer_name(&i18n, PcbLayer::Technical(layer)).into_string();
    let mask = |layer: T| format!("{} mask", name(layer));

    let mut app = app_with_eight_pads();
    edit(&mut app, FootprintEditorMsg::SelectPad(Some(2)));
    let (top_paste, top_mask) = (name(T::PasteTop), mask(T::SolderMaskTop));
    let (bottom_paste, bottom_mask) = (name(T::PasteBottom), mask(T::SolderMaskBottom));
    let (top_paste, top_mask) = (top_paste.as_str(), top_mask.as_str());
    let (bottom_paste, bottom_mask) = (bottom_paste.as_str(), bottom_mask.as_str());
    assert_shows(
        &pad_texts(&app),
        &[top_paste, top_mask],
        &[bottom_paste, bottom_mask],
    );

    let side = |side| PanelMsg::FpEditorSetSelectedPadSide { idx: 2, side };
    panel(&mut app, side(PadSide::Bottom));
    assert_shows(
        &pad_texts(&app),
        &[bottom_paste, bottom_mask],
        &[top_paste, top_mask],
    );

    // A through-hole pad shows both mask layers and no paste.
    panel(&mut app, side(PadSide::All));
    assert_shows(
        &pad_texts(&app),
        &[top_mask, bottom_mask],
        &[top_paste, bottom_paste],
    );

    // Its copper table names the outer copper as the Layers panel does.
    let copper = |layer| layer_name(&i18n, PcbLayer::Copper(layer)).into_string();
    let (top, bottom) = (copper(CopperLayer::Top), copper(CopperLayer::Bottom));
    panel(
        &mut app,
        PanelMsg::FpEditorSetPadStackTab(PadStackTab::TopMiddleBottom),
    );
    assert_shows(&pad_texts(&app), &[top.as_str(), bottom.as_str()], &[]);
}

/// How many times the pad form shows `label`.
fn count(app: &Signex, label: &str) -> usize {
    pad_texts(app).iter().filter(|t| *t == label).count()
}

/// Half-typed text in a copper offset or a hole tolerance is refused: the
/// pad keeps its stored value and the field shows the reason. A number is
/// taken and empty text clears the value.
#[test]
fn a_half_typed_offset_or_tolerance_keeps_the_stored_value() {
    let mut app = app_with_eight_pads();
    edit(&mut app, FootprintEditorMsg::SelectPad(Some(2)));
    panel(
        &mut app,
        PanelMsg::FpEditorSetSelectedPadSide {
            idx: 2,
            side: PadSide::All,
        },
    );
    let offset_x = |value: &str| PanelMsg::FpEditorSetSelectedPadCopperOffsetX {
        idx: 2,
        value: value.into(),
    };
    let tolerance_plus = |value: &str| PanelMsg::FpEditorSetSelectedPadHoleTolerancePlus {
        idx: 2,
        value: value.into(),
    };
    panel(&mut app, offset_x("0.1"));
    panel(&mut app, tolerance_plus("0.05"));
    assert_eq!(count(&app, "Enter a number"), 0);

    panel(&mut app, offset_x("-"));
    panel(&mut app, tolerance_plus("0,0"));
    let pad = selected_pad_3(&app);
    assert_eq!(pad.copper_offset_x_mm, Some(0.1), "the offset is kept");
    assert_eq!(
        pad.hole_tolerance_plus_mm,
        Some(0.05),
        "the tolerance is kept"
    );
    assert_eq!(count(&app, "Enter a number"), 2, "both fields say why");

    panel(&mut app, offset_x("-0.2"));
    panel(&mut app, tolerance_plus(""));
    let pad = selected_pad_3(&app);
    assert_eq!(pad.copper_offset_x_mm, Some(-0.2));
    assert_eq!(pad.hole_tolerance_plus_mm, None, "empty clears");
    assert_eq!(count(&app, "Enter a number"), 0);
}

/// The next pad's offset and tolerance defaults follow the same rule.
#[test]
fn a_half_typed_next_pad_offset_or_tolerance_keeps_the_default() {
    let mut app = app_with_eight_pads();
    let defaults = |app: &Signex| {
        let d = &app.document_state.footprint_editors[&PathBuf::from(PATH)]
            .state
            .next_pad_defaults;
        (d.copper_offset_y_mm, d.hole_tolerance_minus_mm)
    };
    panel(
        &mut app,
        PanelMsg::FpEditorSetNextPadCopperOffsetY("0.3".into()),
    );
    panel(
        &mut app,
        PanelMsg::FpEditorSetNextPadHoleToleranceMinus("0.02".into()),
    );
    panel(
        &mut app,
        PanelMsg::FpEditorSetNextPadCopperOffsetY("-".into()),
    );
    panel(
        &mut app,
        PanelMsg::FpEditorSetNextPadHoleToleranceMinus("x".into()),
    );
    assert_eq!(defaults(&app), (Some(0.3), Some(0.02)));
    panel(
        &mut app,
        PanelMsg::FpEditorSetNextPadCopperOffsetY(String::new()),
    );
    assert_eq!(defaults(&app), (None, Some(0.02)));
}

/// Undo and redo drop the refused text: it belonged to the state they
/// leave, so the field shows the restored value again.
#[test]
fn undo_and_redo_clear_the_refused_field_text() {
    use crate::app::EditMsg;
    let mut app = app_with_eight_pads();
    edit(&mut app, FootprintEditorMsg::SelectPad(Some(2)));
    let size_x = |value: &str| PanelMsg::FpEditorSetSelectedPadSizeX {
        idx: 2,
        value: value.into(),
    };
    let before = selected_pad_3(&app).size_mm;
    panel(&mut app, size_x("2"));
    panel(&mut app, size_x("1,5"));
    assert_eq!(count(&app, "Enter a number"), 1);

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(selected_pad_3(&app).size_mm, before);
    assert_eq!(count(&app, "Enter a number"), 0, "undo drops the text");

    panel(&mut app, size_x("1,5"));
    assert_eq!(count(&app, "Enter a number"), 1);
    let _ = app.update(Message::Edit(EditMsg::Redo));
    assert_eq!(selected_pad_3(&app).size_mm.0, 2.0);
    assert_eq!(count(&app, "Enter a number"), 0, "redo drops the text");
}
