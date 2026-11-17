//! The Pad Properties header (#2262): the selected pad, its kind and its
//! place among the footprint's pads; the footprint header otherwise.

use std::path::PathBuf;

use super::selected_pad_heading;
use crate::app::{FootprintDocument, Message, Signex, TabInfo, TabKind};
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use crate::panels::FootprintEditorPanelContext;
use signex_library::{Footprint, FootprintFile, Pad};

const PATH: &str = "pad-heading.snxfpt";

/// One footprint edit, sent through `Signex::update` as the canvas sends it.
fn edit(app: &mut Signex, msg: FootprintEditorMsg) {
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: PathBuf::from(PATH),
        msg: PrimitiveEdit::Footprint(msg),
    }));
}

fn panel_ctx(app: &Signex) -> FootprintEditorPanelContext {
    app.document_state
        .panel_ctx
        .footprint_editor
        .clone()
        .expect("the footprint editor panel context is built")
}

/// An app with a footprint editor tab open on eight saved pads, nothing
/// selected.
fn app_with_eight_pads() -> Signex {
    let path = PathBuf::from(PATH);
    // Pads go in through the library primitive, as a loaded file brings
    // them, so the test does not reach into the editor's internals.
    let mut footprint = Footprint::empty("SOIC-8");
    footprint.pads = (1..=8)
        .map(|n| Pad {
            number: n.to_string(),
            size: [1.0, 1.0],
            position: [f64::from(n), 0.0],
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

/// The panel context of the eight-pad editor with pad 3 selected.
fn ctx_with_pad_3_selected() -> FootprintEditorPanelContext {
    let mut app = app_with_eight_pads();
    edit(&mut app, FootprintEditorMsg::SelectPad(Some(2)));
    panel_ctx(&app)
}

fn shown(ctx: &FootprintEditorPanelContext) -> Option<String> {
    let i18n = signex_i18n::I18n::en_us();
    selected_pad_heading(ctx, &i18n).map(|h| h.to_string().replace(['\u{2068}', '\u{2069}'], ""))
}

#[test]
fn a_selected_pad_names_its_number_kind_and_place_among_the_pads() {
    let ctx = ctx_with_pad_3_selected();
    assert_eq!(shown(&ctx).as_deref(), Some("Pad 3 · SMD · 3 of 8 pads"));
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
        Some(format!("Pad {number} · SMD · 9 of 9 pads"))
    );
}

#[test]
fn a_footprint_with_one_pad_says_pad_not_pads() {
    let mut ctx = ctx_with_pad_3_selected();
    ctx.editor_pad_count = 1;
    ctx.selected_pad.as_mut().expect("pad 3 is selected").idx = 0;
    assert_eq!(shown(&ctx).as_deref(), Some("Pad 3 · SMD · 1 of 1 pad"));
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

/// The Pad Properties body of `app` laid out at a dock's width, with the
/// isolation marks Fluent puts around arguments removed.
fn pad_texts(app: &Signex) -> Vec<crate::layout_probe::LaidOutText> {
    let ctx = &app.document_state.panel_ctx;
    let fp = ctx.footprint_editor.as_ref().expect("footprint context");
    let palette = crate::panels::PanelPalette::from_tokens(&ctx.tokens);
    let i18n = signex_i18n::I18n::en_us();
    let view = super::view_footprint_editor_properties(
        fp,
        palette,
        &ctx.collapsed_sections,
        signex_types::coord::Unit::Mm,
        &i18n,
    );
    crate::layout_probe::lay_out_texts(view, DOCK)
        .into_iter()
        .map(|mut t| {
            t.text = t.text.replace(['\u{2068}', '\u{2069}'], "");
            t
        })
        .collect()
}

const DOCK: iced::Size = iced::Size::new(260.0, 3000.0);

/// #2262: the Simple pad stack shows paste and solder mask as one labelled
/// row each, not the cramped tables, and the Paste switch on its row writes
/// the selected pad through `Signex::update`.
#[test]
fn the_simple_pad_stack_lists_paste_and_mask_as_rows_and_its_switch_writes_the_pad() {
    let mut app = app_with_eight_pads();
    edit(&mut app, FootprintEditorMsg::SelectPad(Some(2)));
    let texts = pad_texts(&app);
    let has = |label: &str| texts.iter().any(|t| t.text == label);
    for label in [
        "Top Paste",
        "Top Paste on",
        "Top Solder",
        "Top Solder tented",
    ] {
        assert!(has(label), "{label:?} missing: {texts:#?}");
    }
    for header in ["PASTE", "SOLDER", "Expansion"] {
        assert!(!has(header), "{header:?} table header in Simple");
    }
    // The read-only position is two rows, X and Y, not one "(x, y) mm" field.
    for row in ["X (mm)", "3.000", "Y (mm)", "0.000"] {
        assert!(has(row), "{row:?} missing");
    }
    assert!(!texts.iter().any(|t| t.text.starts_with("(3.000")));

    let before = app.document_state.footprint_editors[&PathBuf::from(PATH)]
        .state
        .pads[2]
        .stack
        .paste_enabled_top;
    let row = texts
        .iter()
        .find(|t| t.text == "Top Paste on")
        .expect("the Paste switch row")
        .bounds;
    let y = row.y + row.height / 2.0;
    let msg = (0..)
        .map(|i| row.x + row.width + i as f32 * 4.0)
        .take_while(|x| *x < DOCK.width)
        .find_map(|x| {
            let ctx = &app.document_state.panel_ctx;
            let fp = ctx.footprint_editor.as_ref().expect("footprint context");
            let palette = crate::panels::PanelPalette::from_tokens(&ctx.tokens);
            let i18n = signex_i18n::I18n::en_us();
            let view = super::view_footprint_editor_properties(
                fp,
                palette,
                &ctx.collapsed_sections,
                signex_types::coord::Unit::Mm,
                &i18n,
            );
            crate::layout_probe::messages_on_click_at(view, DOCK, None, iced::Point::new(x, y))
                .into_iter()
                .next()
        })
        .expect("a click right of the label reaches the switch");
    let _ = app.update(Message::Dock(crate::dock::DockMessage::Panel(msg)));
    let after = app.document_state.footprint_editors[&PathBuf::from(PATH)]
        .state
        .pads[2]
        .stack
        .paste_enabled_top;
    assert_ne!(before, after, "the switch toggled the pad's top paste");
}
