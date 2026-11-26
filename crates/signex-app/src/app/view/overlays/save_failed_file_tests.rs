//! #2991 — the Save Failed card of a write that failed names the file
//! (mockup row 11): "<file> could not be written." and then that the edits
//! are still open; the path and the system's error stay in the mono block.
//! Driven by Ctrl+S through `update` and by `save_document` headless.

use std::path::{Path, PathBuf};

use super::*;
use crate::app::command::CommandArgs;
use crate::app::handlers::document_files::save_async_tests::empty_board;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{FileMsg, Signex};
use crate::layout_probe::lay_out_texts;
use signex_i18n::RenderTranslatable as _;

const WINDOW: iced::Size = iced::Size::new(1400.0, 900.0);
const SENTENCE: &str = "board.snxpcb could not be written.";

/// A workspace with an unsaved board whose file cannot be written: a folder
/// stands where the board belongs, so the write fails.
fn blocked_board(dir: &Path) -> (Signex, PathBuf) {
    let _ = crate::diagnostics::init_logging();
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    app.ui_state.first_run_tour_open = false;
    app.ui_state.window_size = (WINDOW.width, WINDOW.height);
    let path = dir.join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), empty_board());
    std::fs::create_dir(&path).expect("block the target");
    let tab = app.document_state.tabs[0].path.clone();
    app.document_state.mark_document_dirty(&tab);
    app.document_state.mark_document_dirty(&path);
    (app, path)
}

/// The card names the file in its sentence and keeps the path and the
/// error in its system block.
fn assert_names_the_file(app: &Signex, path: &Path) {
    let notice = app.document_state.error_notice.as_ref().expect("a card");
    assert_eq!(notice.title.render_en_us(), "Save Failed");
    assert_eq!(notice.detail.render_en_us(), SENTENCE);
    let en = signex_i18n::I18n::en_us();
    assert_eq!(
        notice.sentence_text(&en),
        format!("{SENTENCE} Your edits are still open and unsaved.")
    );
    let system = notice.system.as_ref().map(|s| s.as_str()).unwrap_or("");
    assert!(
        system.contains(&path.display().to_string()),
        "the path stays in the system block: {system:?}"
    );
    assert!(app.document_state.is_document_dirty(path), "still unsaved");
}

fn unmarked(text: &str) -> String {
    text.replace(['\u{2068}', '\u{2069}'], "")
}

#[test]
fn ctrl_s_on_a_board_that_cannot_be_written_names_it_on_the_card() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = blocked_board(dir.path());
    crate::app::path_ops::settle(&mut app, Message::File(FileMsg::Save));
    assert_names_the_file(&app, &path);

    // As the window lays the card out: the named sentence, then the note,
    // then the system block holding the path.
    let mut layers = app.error_notice_overlay();
    assert_eq!(layers.len(), 1, "one layer: backdrop and card together");
    let card = super::scroll_guard(layers.pop().expect("one layer"));
    let texts = lay_out_texts(card, WINDOW);
    let at = |wanted: &dyn Fn(&str) -> bool| {
        texts
            .iter()
            .find(|t| wanted(&unmarked(&t.text)))
            .unwrap_or_else(|| panic!("not laid out: {texts:#?}"))
            .bounds
    };
    let sentence = at(&|t| t == SENTENCE);
    let note = at(&|t| t == "Your edits are still open and unsaved.");
    let shown = path.display().to_string();
    let system = at(&|t| t.contains(&shown));
    assert!(sentence.y < note.y, "the named sentence leads");
    assert!(
        note.y < system.y,
        "the system block sits below the sentence"
    );
    assert!(
        !texts
            .iter()
            .any(|t| unmarked(&t.text) == "Failed to save active document"),
        "the old sentence is gone"
    );
}

#[test]
fn a_headless_save_document_that_cannot_write_names_the_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = blocked_board(dir.path());
    let command = crate::keymap::AppCommandId::new("save_document").expect("an id");
    let document = format!("document={}", path.display());
    let args = CommandArgs::parse_operands(&command, &[document.as_str()]).expect("operands");
    let (_outcome, task) = app.run_headless_command_in_test(&command, args);
    crate::app::path_ops::settle_task(&mut app, task);
    assert_names_the_file(&app, &path);
    // The reason signex-cli reports: the sentence, its period giving way to
    // the colon, then the system part.
    let notice = app.document_state.error_notice.as_ref().expect("a card");
    let line = notice.detail_line_en_us();
    assert!(
        line.starts_with("board.snxpcb could not be written: ")
            && line.contains(&path.display().to_string()),
        "{line}"
    );
}
