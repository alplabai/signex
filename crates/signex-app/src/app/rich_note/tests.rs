//! #1114, mockup row 203: rich text notes driven through `Signex::update`.
//! A rich note compiles once in a task and draws from the cache after; an
//! edit recompiles only that note; a failed compile keeps the last good
//! drawing, marked. The in-place editor opens on a double-click, Edit… or a
//! search hit, previews as you type, wraps the selection, applies as one
//! undo step and drops the typing on Cancel.

use std::path::Path;
use std::sync::Arc;

use iced::widget::text_editor::{Action, Edit};
use signex_types::schematic::{
    HAlign, Point, RichNoteLayout, SchematicSheet, SelectedItem, SelectedKind, TextNote,
    TextNoteFormat, VAlign,
};
use uuid::Uuid;

use super::RichNoteMsg;
use crate::app::handlers::editing_commands::net_color_driver::add_sheet_with;
use crate::app::{Message, Signex};
use crate::task_driver::TaskDriver;

pub(super) const SOURCE: &str =
    "= Layout notes\n*Keep the hot loop small.*\n$V = 0.8 dot (1 + R_7 / R_8)$\n";

pub(super) fn note_id() -> Uuid {
    Uuid::from_u128(0x1114)
}

pub(super) fn rich_note(text: &str) -> TextNote {
    TextNote {
        uuid: note_id(),
        text: text.into(),
        position: Point::new(100.0, 60.0),
        rotation: 0.0,
        font_size: 1.27,
        font_family: None,
        justify_h: HAlign::Left,
        justify_v: VAlign::Top,
        locked: false,
        format: TextNoteFormat::Typst,
        rich: RichNoteLayout::default(),
    }
}

pub(super) fn sheet_with(note: TextNote) -> SchematicSheet {
    SchematicSheet {
        paper_size: "A4".into(),
        text_notes: vec![note],
        ..Default::default()
    }
}

pub(super) fn app_with(dir: &Path, note: TextNote) -> Signex {
    let (mut app, _task) = Signex::new();
    add_sheet_with(&mut app, &dir.join("main.snxsch"), sheet_with(note));
    app
}

/// Send `msg` and run every task it starts to the end.
pub(super) fn send(app: &mut Signex, msg: Message) {
    let task = app.update(msg);
    TaskDriver::new()
        .settle(app, task)
        .expect("the tasks settle");
}

pub(super) fn note(app: &Signex) -> &TextNote {
    &app.document_state
        .active_engine()
        .expect("engine")
        .document()
        .text_notes[0]
}

fn undo(app: &mut Signex) {
    send(app, Message::Edit(crate::app::EditMsg::Undo));
}

/// A rich note compiles in a task; the canvas draws it from the cache and
/// an unchanged note never compiles again.
#[test]
fn a_rich_note_compiles_once_and_draws_from_the_cache() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), rich_note(SOURCE));
    send(&mut app, Message::noop());
    let shown = app
        .ui_state
        .rich_notes
        .renders
        .get(note_id())
        .expect("shown");
    assert!(shown.drawing.is_some() && !shown.failed);
    assert_eq!(app.ui_state.rich_notes.compiles, 1);
    let revision = app.ui_state.rich_notes.renders.revision();

    send(&mut app, Message::noop());
    send(&mut app, Message::noop());
    assert_eq!(app.ui_state.rich_notes.compiles, 1, "no recompile");
    assert_eq!(app.ui_state.rich_notes.renders.revision(), revision);

    // The canvas boxes the note as it draws: wider than its anchor.
    let widths = app.active_wire_widths();
    let snapshot = app.active_render_snapshot().expect("snapshot");
    let hit = crate::schematic_runtime::hit_test::hit_test_styled(
        snapshot,
        &crate::schematic_runtime::Designators::new(),
        110.0,
        61.0,
        widths,
    );
    assert_eq!(
        hit,
        Some(SelectedItem::new(note_id(), SelectedKind::TextNote))
    );
}

/// A plain note draws as text and is never compiled.
#[test]
fn a_plain_note_is_never_compiled() {
    let dir = tempfile::tempdir().expect("tempdir");
    let plain = TextNote {
        format: TextNoteFormat::Plain,
        ..rich_note(SOURCE)
    };
    let mut app = app_with(dir.path(), plain);
    send(&mut app, Message::noop());
    assert_eq!(app.ui_state.rich_notes.compiles, 0);
    assert!(app.ui_state.rich_notes.renders.get(note_id()).is_none());
}

/// A compile error keeps the last good drawing on the sheet, marked; the
/// next good text clears the mark.
#[test]
fn a_failed_compile_keeps_the_last_good_drawing_marked() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), rich_note(SOURCE));
    send(&mut app, Message::noop());
    let good = (app.ui_state.rich_notes.renders.get(note_id()))
        .and_then(|s| s.drawing.clone())
        .expect("a drawing");

    let item = SelectedItem::new(note_id(), SelectedKind::TextNote);
    app.set_rich_note_source(item, "#broken(").expect("set");
    send(&mut app, Message::noop());
    let shown = app
        .ui_state
        .rich_notes
        .renders
        .get(note_id())
        .expect("shown");
    assert!(shown.failed, "the mark shows");
    let kept = shown.drawing.clone().expect("the last good drawing stays");
    assert!(Arc::ptr_eq(&kept, &good));
    assert_eq!(app.ui_state.rich_notes.compiles, 2, "only the edited note");

    app.set_rich_note_source(item, SOURCE).expect("set");
    send(&mut app, Message::noop());
    let shown = app
        .ui_state
        .rich_notes
        .renders
        .get(note_id())
        .expect("shown");
    assert!(!shown.failed);
    assert_eq!(
        app.ui_state.rich_notes.compiles, 2,
        "the old key was cached"
    );
}

fn open_editor(app: &mut Signex, at: usize) {
    send(
        app,
        RichNoteMsg::EditorRequested {
            note: note_id(),
            at,
        }
        .into(),
    );
}

fn type_in(app: &mut Signex, text: &str) {
    let paste = Action::Edit(Edit::Paste(Arc::new(text.to_string())));
    send(app, RichNoteMsg::EditorActed(paste).into());
}

/// Typing compiles the preview after a pause; Apply makes the typed text
/// the note's in one undo step and closes the editor.
#[test]
fn the_editor_previews_as_you_type_and_applies_in_one_undo_step() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), rich_note(SOURCE));
    open_editor(&mut app, 0);
    let editor = app.ui_state.rich_notes.editor.as_ref().expect("open");
    assert_eq!(editor.text(), SOURCE);
    assert!(editor.shown_drawing().is_some(), "the preview compiled");

    type_in(&mut app, "Note: ");
    let editor = app.ui_state.rich_notes.editor.as_ref().expect("open");
    assert!(editor.text().starts_with("Note: "));
    assert!(editor.preview.is_some(), "the typed text compiled");
    assert_eq!(note(&app).text, SOURCE, "nothing reaches the sheet yet");

    send(&mut app, RichNoteMsg::ApplyPressed.into());
    assert!(app.ui_state.rich_notes.editor.is_none());
    assert!(note(&app).text.starts_with("Note: "));
    undo(&mut app);
    assert_eq!(note(&app).text, SOURCE, "one step");
}

/// Cancel drops the typing; the note is untouched.
#[test]
fn cancel_drops_the_typing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), rich_note(SOURCE));
    open_editor(&mut app, 0);
    type_in(&mut app, "draft ");
    send(&mut app, RichNoteMsg::CancelPressed.into());
    assert!(app.ui_state.rich_notes.editor.is_none());
    assert_eq!(note(&app).text, SOURCE);
}

/// A toolbar button wraps the selection; with nothing selected it inserts
/// the empty markup at the cursor.
#[test]
fn a_toolbar_button_wraps_the_selection() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), rich_note("hot loop"));
    open_editor(&mut app, 0);
    send(&mut app, RichNoteMsg::EditorActed(Action::SelectAll).into());
    send(&mut app, RichNoteMsg::WrapPressed(super::Wrap::Bold).into());
    let editor = app.ui_state.rich_notes.editor.as_ref().expect("open");
    assert_eq!(editor.text(), "*hot loop*");
    send(&mut app, RichNoteMsg::WrapPressed(super::Wrap::Math).into());
    let editor = app.ui_state.rich_notes.editor.as_ref().expect("open");
    assert_eq!(editor.text(), "*hot loop*$$");
}

/// A double-click on a rich note opens its editor, not the one-line box.
#[test]
fn a_double_click_on_a_rich_note_opens_its_editor() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), rich_note(SOURCE));
    send(&mut app, Message::noop());
    let task = app
        .open_rich_note_editor_on(SelectedItem::new(note_id(), SelectedKind::TextNote))
        .expect("a rich note");
    TaskDriver::new().settle(&mut app, task).expect("settles");
    assert!(app.ui_state.rich_notes.editor.is_some());
    assert!(app.interaction_state.editing_text.is_none());
}

/// Edit… in Properties opens the editor through the dock router.
#[test]
fn edit_in_properties_opens_the_editor() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), rich_note(SOURCE));
    let item = SelectedItem::new(note_id(), SelectedKind::TextNote);
    let msg = crate::panels::PlacedEditMsg::RichNoteEditRequested { item };
    send(
        &mut app,
        Message::Dock(crate::dock::DockMessage::Panel(
            crate::panels::PanelMsg::PlacedEdit(msg),
        )),
    );
    assert_eq!(
        app.ui_state.rich_notes.editor.as_ref().map(|e| e.note),
        Some(note_id())
    );
}

/// Choosing a search hit inside a rich note opens its editor with the
/// cursor on the match.
#[test]
fn a_search_hit_opens_the_editor_at_the_match() {
    use crate::find_replace::FindReplaceMsg;
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), rich_note(SOURCE));
    send(
        &mut app,
        Message::FindReplaceMsg(FindReplaceMsg::QueryChanged("hot loop".into())),
    );
    assert_eq!(app.ui_state.find_replace.matches.len(), 1);
    send(
        &mut app,
        Message::FindReplaceMsg(FindReplaceMsg::SelectResult(0)),
    );
    let editor = app
        .ui_state
        .rich_notes
        .editor
        .as_ref()
        .expect("the editor opened");
    let at = SOURCE.find("hot loop").expect("in the source");
    assert_eq!(editor.cursor(), super::line_and_index(SOURCE, at));
    assert_eq!(editor.cursor().0, 1, "the second line");
}

/// The Frame row is one undo step and keeps the text.
#[test]
fn the_frame_row_sets_the_section_in_one_undo_step() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), rich_note(SOURCE));
    let item = SelectedItem::new(note_id(), SelectedKind::TextNote);
    let layout = RichNoteLayout {
        frame: signex_types::schematic::NoteFrame::Filled,
        ..RichNoteLayout::default()
    };
    let msg = crate::panels::PlacedEditMsg::NoteLayoutPicked {
        item,
        layout: crate::panels::NoteLayout::Rich(layout),
    };
    send(
        &mut app,
        Message::Dock(crate::dock::DockMessage::Panel(
            crate::panels::PanelMsg::PlacedEdit(msg),
        )),
    );
    assert_eq!(note(&app).rich, layout);
    assert_eq!(note(&app).text, SOURCE);
    undo(&mut app);
    assert!(note(&app).rich.is_default());
}

/// A typed Width commits on Enter in the panel's unit.
#[test]
fn a_typed_width_commits_on_enter() {
    use crate::panels::{NoteLength, PanelMsg, PlacedEditMsg, PlacedField};
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), rich_note(SOURCE));
    app.ui_state.unit = signex_types::coord::Unit::Mm;
    let item = SelectedItem::new(note_id(), SelectedKind::TextNote);
    let field = PlacedField::NoteLength(NoteLength::Width);
    let dock = |msg| Message::Dock(crate::dock::DockMessage::Panel(PanelMsg::PlacedEdit(msg)));
    send(
        &mut app,
        dock(PlacedEditMsg::Typed {
            item,
            field: field.clone(),
            text: "40".into(),
        }),
    );
    send(&mut app, dock(PlacedEditMsg::Submitted { item, field }));
    assert!((note(&app).rich.width_mm - 40.0).abs() < 1e-9);
}
