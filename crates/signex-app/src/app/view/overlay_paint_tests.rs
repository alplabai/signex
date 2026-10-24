//! #114 — an overlay paints whenever its own state is open, whatever
//! tab is active.
//!
//! `view_main_for` used to gate `collect_overlays` behind a hand-written
//! predicate that had drifted from the builders' own guards: the error
//! card, the netlist-incomplete prompt and the print preview were missing
//! from it, so they only painted while a schematic / footprint / symbol
//! tab happened to be active. These tests lay the main window out on the
//! start screen (no tab, so no editor chrome) and read the overlay's text
//! back out of the laid-out tree.

use super::*;
use crate::app::state::{ErrorNotice, NetlistIncompletePrompt};
use crate::layout_probe::{LaidOutText, lay_out_texts};

const WINDOW: iced::Size = iced::Size::new(1400.0, 900.0);

/// Texts that exist only inside an overlay card.
const ERROR_TITLE: &str = "Save Failed";
const ERROR_DETAIL: &str = "disk full while closing the project";
const PROMPT_HEADING: &str = "Netlist is incomplete";

/// A start-screen app (no tabs, so `has_active_schematic()` is false)
/// with the first-run tour closed so it cannot paint into the negative
/// case.
fn start_screen_app() -> (Signex, iced::window::Id) {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let main_window = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main_window);
    assert!(
        app.document_state.tabs.is_empty() && !app.has_active_schematic(),
        "fixture must have no editor tab, or it would not exercise the bug"
    );
    (app, main_window)
}

fn main_window_texts(app: &Signex, window: iced::window::Id) -> Vec<LaidOutText> {
    lay_out_texts(app.view(window), WINDOW)
}

fn has_text(texts: &[LaidOutText], label: &str) -> bool {
    texts.iter().any(|t| t.text == label)
}

#[test]
fn an_error_card_paints_with_no_editor_tab_active() {
    let (mut app, window) = start_screen_app();
    app.document_state.error_notice = Some(ErrorNotice::save(ERROR_DETAIL));

    let texts = main_window_texts(&app, window);

    assert!(
        has_text(&texts, ERROR_TITLE) && has_text(&texts, ERROR_DETAIL),
        "the error card was never painted on the start screen: {texts:#?}"
    );
}

#[test]
fn the_netlist_incomplete_prompt_paints_with_no_editor_tab_active() {
    let (mut app, window) = start_screen_app();
    app.document_state.netlist_incomplete_prompt = Some(NetlistIncompletePrompt {
        save_path: "out.net".into(),
        messages: vec!["sheet two is not in the netlist".to_owned()],
        ctx: signex_output::ExportContext {
            sheets: Vec::new(),
            metadata: signex_output::ProjectMetadata::default(),
            netlist: None,
            netlist_sheets: Vec::new(),
        },
    });

    let texts = main_window_texts(&app, window);

    assert!(
        has_text(&texts, PROMPT_HEADING),
        "the netlist-incomplete prompt was never painted on the start screen: {texts:#?}"
    );
}

#[test]
fn nothing_open_and_no_editor_tab_paints_no_overlay_text() {
    let (app, window) = start_screen_app();

    let texts = main_window_texts(&app, window);

    for sentinel in [
        ERROR_TITLE,
        "Export Failed",
        PROMPT_HEADING,
        "Placement Paused",
    ] {
        assert!(
            !has_text(&texts, sentinel),
            "{sentinel:?} painted with every overlay closed: {texts:#?}"
        );
    }
}
