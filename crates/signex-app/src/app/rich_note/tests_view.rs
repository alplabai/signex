//! #1114, mockup row 203: the editor's view, laid out (title, toolbar,
//! source, preview, hint, Cancel and Apply, the first error), its place over
//! the sheet, the toolbar's markup and the link rules.

use std::path::Path;

use iced::{Rectangle, Size};

use super::tests::{SOURCE, app_with, note_id, rich_note, send};
use super::view::{EDITOR_HEIGHT, EDITOR_WIDTH, editor_card, editor_origin};
use super::{LinkTarget, RichNoteMsg, Wrap, line_and_index, resolve_link};
use crate::app::Message;

const WINDOW: Size = Size::new(1600.0, 1000.0);

fn texts_of(app: &crate::app::Signex) -> Vec<String> {
    let editor = app.ui_state.rich_notes.editor.as_ref().expect("open");
    let title = signex_i18n::t!(app.ui_state.i18n, signex_i18n::ids::OVERLAY_RICH_NOTE_TITLE);
    let card = editor_card(&app.ui_state.i18n, editor, title, Some(40.0));
    crate::layout_probe::lay_out_texts(card, WINDOW)
        .into_iter()
        .map(|t| t.text)
        .collect()
}

/// The card shows the toolbar, the line numbers, the preview's width and
/// the buttons; it is the size mockup row 203 draws.
#[test]
fn the_editor_lays_out_its_parts() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), rich_note(SOURCE));
    send(
        &mut app,
        RichNoteMsg::EditorRequested {
            note: note_id(),
            at: 0,
        }
        .into(),
    );
    let texts = texts_of(&app);
    for wanted in ["Bold", "Italic", "Heading", "List", "Math", "Link", "Table"] {
        assert!(texts.iter().any(|t| t == wanted), "{wanted}: {texts:?}");
    }
    for wanted in ["Cancel", "Apply", "1", "2", "3"] {
        assert!(texts.iter().any(|t| t == wanted), "{wanted}: {texts:?}");
    }
    assert!(
        texts
            .iter()
            .any(|t| t.starts_with("As drawn on the sheet") && t.contains("40.00")),
        "{texts:?}"
    );
    let editor = app.ui_state.rich_notes.editor.as_ref().expect("open");
    let title = signex_i18n::t!(
        app.ui_state.i18n,
        signex_i18n::ids::OVERLAY_RICH_NOTE_COMPILING
    );
    let size = crate::layout_probe::root_size(
        editor_card(&app.ui_state.i18n, editor, title, None),
        WINDOW,
    );
    assert_eq!(size, Size::new(EDITOR_WIDTH, EDITOR_HEIGHT));
}

/// A compile error shows under the source, with its line.
#[test]
fn the_first_error_shows_under_the_source() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), rich_note("fine\n#nope("));
    send(
        &mut app,
        RichNoteMsg::EditorRequested {
            note: note_id(),
            at: 0,
        }
        .into(),
    );
    let texts = texts_of(&app);
    assert!(
        texts
            .iter()
            .any(|t| t.starts_with("Line") && t.contains('2')),
        "{texts:?}"
    );
}

/// Apply in the card is the Apply message.
#[test]
fn apply_in_the_card_sends_apply() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), rich_note(SOURCE));
    send(
        &mut app,
        RichNoteMsg::EditorRequested {
            note: note_id(),
            at: 0,
        }
        .into(),
    );
    let editor = app.ui_state.rich_notes.editor.as_ref().expect("open");
    let title = signex_i18n::t!(
        app.ui_state.i18n,
        signex_i18n::ids::OVERLAY_RICH_NOTE_COMPILING
    );
    let card = editor_card(&app.ui_state.i18n, editor, title, None);
    let pressed =
        crate::layout_probe::messages_on_click_at(card, WINDOW, Some("Apply"), iced::Point::ORIGIN);
    assert!(
        pressed.iter().any(|m| matches!(
            m,
            Message::TextEdit(crate::app::TextEditMsg::RichNote(RichNoteMsg::ApplyPressed))
        )),
        "{pressed:?}"
    );
}

/// The overlay is drawn over the sheet the note is on, above the note.
#[test]
fn the_overlay_shows_over_its_sheet() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_with(dir.path(), rich_note(SOURCE));
    let main = app.ui_state.main_window_id.expect("the main window");
    assert!(app.rich_note_editor_overlay(main).is_none(), "closed");
    send(
        &mut app,
        RichNoteMsg::EditorRequested {
            note: note_id(),
            at: 0,
        }
        .into(),
    );
    let overlay = app.rich_note_editor_overlay(main).expect("open");
    let texts = crate::layout_probe::lay_out_texts(overlay, WINDOW);
    assert!(
        texts
            .iter()
            .any(|t| t.text.starts_with("Typst") && t.text.contains("main.snxsch")),
        "{texts:?}"
    );
}

#[test]
fn the_editor_sits_above_the_note_else_below_inside_the_canvas() {
    let canvas = Rectangle::new(iced::Point::new(0.0, 40.0), Size::new(1200.0, 800.0));
    let (x, y) = editor_origin((300.0, 600.0), canvas);
    assert_eq!(x, 300.0);
    assert!(y + EDITOR_HEIGHT <= 600.0, "above");
    let (_, y) = editor_origin((300.0, 100.0), canvas);
    assert!(y > 100.0, "no room above: below");
    let (x, _) = editor_origin((1100.0, 600.0), canvas);
    assert!(x + EDITOR_WIDTH <= canvas.x + canvas.width, "kept inside");
}

#[test]
fn the_toolbar_wraps_the_selection_or_inserts_the_empty_markup() {
    assert_eq!(Wrap::Bold.around(Some("loop")), "*loop*");
    assert_eq!(Wrap::Italic.around(None), "__");
    assert_eq!(Wrap::Heading.around(Some("Power")), "= Power");
    assert_eq!(Wrap::List.around(Some("a\nb")), "- a\n- b");
    assert_eq!(Wrap::Math.around(Some("x^2")), "$x^2$");
    assert_eq!(Wrap::Link.around(Some("a.pdf")), "#link(\"a.pdf\")");
    assert_eq!(Wrap::Table.around(None), "#table(columns: 2, [], [])");
    for wrap in Wrap::ALL {
        assert_eq!(Wrap::from_word(wrap.as_str()), Some(wrap));
    }
}

#[test]
fn a_link_opens_a_web_page_or_a_file_inside_the_project() {
    let folder = Path::new("/projects/buck");
    assert_eq!(
        resolve_link("https://example.com/a", folder),
        Ok(LinkTarget::Web("https://example.com/a".into()))
    );
    assert_eq!(
        resolve_link("assets/docs/guide.pdf", folder),
        Ok(LinkTarget::ProjectFile(
            folder.join("assets/docs/guide.pdf")
        ))
    );
    for refused in [
        "../secret.txt",
        "/etc/passwd",
        "file:///etc/passwd",
        "C:x",
        "",
    ] {
        assert!(resolve_link(refused, folder).is_err(), "{refused}");
    }
}

#[test]
fn a_byte_offset_is_a_line_and_an_index() {
    assert_eq!(line_and_index("ab\ncd\nef", 0), (0, 0));
    assert_eq!(line_and_index("ab\ncd\nef", 4), (1, 1));
    assert_eq!(line_and_index("ab\ncd\nef", 99), (2, 2), "clamped");
    assert_eq!(line_and_index("é", 1), (0, 0), "a character boundary");
}
