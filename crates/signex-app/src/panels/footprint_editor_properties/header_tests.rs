//! The Pad Properties header (#2262): the selected pad, its kind and its
//! place among the footprint's pads; the footprint header otherwise.

use std::path::PathBuf;

use super::selected_pad_heading;
use crate::app::{FootprintDocument, Message, Signex, TabInfo, TabKind};
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use crate::panels::FootprintEditorPanelContext;
use signex_library::{Footprint, FootprintFile, Pad};

/// The panel context of a footprint editor with eight pads and pad 3
/// selected, built the way the app builds it.
fn ctx_with_pad_3_selected() -> FootprintEditorPanelContext {
    let path = PathBuf::from("pad-heading.snxfpt");
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
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path,
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SelectPad(Some(2))),
    }));
    let mut ctx = app
        .document_state
        .panel_ctx
        .footprint_editor
        .clone()
        .expect("the footprint editor panel context is built");
    // The count is the footprint's baked pads; this editor bakes none.
    ctx.pad_count = 8;
    ctx
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

#[test]
fn a_footprint_with_one_pad_says_pad_not_pads() {
    let mut ctx = ctx_with_pad_3_selected();
    ctx.pad_count = 1;
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
    ctx.pad_count = 2;
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
