//! m9 command palette, m10 hover tooltip.

use signex_app::app::command_palette::{CommandAction, CommandEntry, CommandSource};
use iced::keyboard;
use signex_app::app::{CommandPaletteMsg, InputMsg, Message, Signex};
use signex_app::canvas::CanvasEvent;
use signex_app::keymap::AppCommandId;

use super::support::{app, desk, is_dirty, main_window, sample_with_sheet};
use crate::settle_support::settle;

/// The palette row of the `open_preferences` catalog command.
const PREFERENCES_ROW: &str = "Open preferences";

fn palette(app: &mut Signex, msg: CommandPaletteMsg) {
    let _ = app.update(Message::CommandPalette(msg));
}

fn query(app: &mut Signex, text: &str) {
    palette(app, CommandPaletteMsg::QueryChanged(text.to_string()));
}

/// Esc as the keyboard sends it: the open palette claims the raw key before
/// the Esc ladder runs.
fn press_esc(app: &mut Signex) {
    let key = keyboard::Key::Named(keyboard::key::Named::Escape);
    let event = keyboard::Event::KeyPressed {
        key: key.clone(),
        modified_key: key,
        physical_key: keyboard::key::Physical::Unidentified(
            keyboard::key::NativeCode::Unidentified,
        ),
        location: keyboard::Location::Standard,
        modifiers: keyboard::Modifiers::default(),
        text: None,
        repeat: false,
    };
    let window = main_window(app);
    settle(app, Message::Input(InputMsg::KeyInput { window, event }));
}

// ---- m9 Command palette ---------------------------------------------------

#[test]
fn m9_opening_the_palette_lists_rows_with_the_first_highlighted() {
    let mut app = app();
    palette(&mut app, CommandPaletteMsg::Open);
    let state = &app.ui_state.command_palette;
    assert!(state.open);
    assert!(!state.results.is_empty());
    assert_eq!(state.selected_index, 0);
}

#[test]
fn m9_typing_narrows_the_rows_to_the_query_and_resets_the_highlight() {
    let mut app = app();
    palette(&mut app, CommandPaletteMsg::Open);
    let everything = app.ui_state.command_palette.total_matches;
    palette(&mut app, CommandPaletteMsg::MoveSelection(1));

    query(&mut app, PREFERENCES_ROW);

    let state = &app.ui_state.command_palette;
    assert!(state.total_matches < everything);
    assert!(state.results.iter().any(|row| row.label == PREFERENCES_ROW));
    assert_eq!(state.selected_index, 0);
}

#[test]
fn m9_the_arrow_keys_wrap_at_both_ends() {
    let mut app = app();
    palette(&mut app, CommandPaletteMsg::Open);
    let last = app.ui_state.command_palette.results.len() - 1;
    palette(&mut app, CommandPaletteMsg::MoveSelection(-1));
    assert_eq!(app.ui_state.command_palette.selected_index, last);
    palette(&mut app, CommandPaletteMsg::MoveSelection(1));
    assert_eq!(app.ui_state.command_palette.selected_index, 0);
}

#[test]
fn m9_running_a_row_does_its_command_and_closes_the_palette() {
    let mut app = app();
    palette(&mut app, CommandPaletteMsg::Open);
    query(&mut app, PREFERENCES_ROW);
    let row = app
        .ui_state
        .command_palette
        .results
        .iter()
        .position(|row| row.label == PREFERENCES_ROW)
        .expect("the Preferences row");

    settle(
        &mut app,
        Message::CommandPalette(CommandPaletteMsg::Select(row)),
    );

    assert!(app.ui_state.preferences_open, "the command did not run");
    let state = &app.ui_state.command_palette;
    assert!(!state.open);
    assert!(state.query.is_empty());
}

#[test]
fn m9_esc_closes_without_running_and_keeps_the_query() {
    let mut app = app();
    palette(&mut app, CommandPaletteMsg::Open);
    query(&mut app, PREFERENCES_ROW);

    press_esc(&mut app);

    assert!(!app.ui_state.command_palette.open);
    assert_eq!(app.ui_state.command_palette.query, PREFERENCES_ROW);
    assert!(!app.ui_state.preferences_open);
}

#[test]
fn m9_enter_on_a_query_with_no_match_closes_and_runs_nothing() {
    let mut desk = desk();
    let path = desk.sheet("a.snxsch", "A4");
    palette(&mut desk.app, CommandPaletteMsg::Open);
    query(&mut desk.app, "zzzzqqqqxxxx");
    assert_eq!(desk.app.ui_state.command_palette.total_matches, 0);

    settle(
        &mut desk.app,
        Message::CommandPalette(CommandPaletteMsg::ExecuteSelected),
    );

    assert!(!desk.app.ui_state.command_palette.open);
    assert!(!is_dirty(&desk.app, &path));
}

#[test]
fn m9_a_command_with_nothing_to_act_on_is_refused() {
    let mut app = app();
    let tool = app.interaction_state.current_tool;
    let state = &mut app.ui_state.command_palette;
    state.open = true;
    state.results = vec![CommandEntry {
        source: CommandSource::Command,
        label: "Place Wire".to_string(),
        value: String::new(),
        detail: String::new(),
        action: CommandAction::Command(AppCommandId::new("place_wire").expect("a command")),
    }];
    state.selected_index = 0;

    settle(
        &mut app,
        Message::CommandPalette(CommandPaletteMsg::ExecuteSelected),
    );

    assert_eq!(app.interaction_state.current_tool, tool);
    assert!(!app.ui_state.command_palette.open);
}

// ---- m10 Hover tooltip ----------------------------------------------------

fn cursor_at(app: &mut Signex, x: f32, y: f32) {
    let _ = app.update(Message::CanvasEvent(CanvasEvent::CursorAt {
        x,
        y,
        zoom_pct: 100.0,
    }));
}

#[test]
fn m10_resting_on_a_symbol_starts_its_card_and_moving_off_ends_it() {
    let mut sample = sample_with_sheet();
    let path = sample
        .app
        .document_state
        .active_path
        .clone()
        .expect("the sheet");
    let (uuid, at) = {
        let sheet = sample.app.document_state.engines[&path].document();
        let symbol = sheet
            .symbols
            .iter()
            .find(|symbol| symbol.reference == "U1")
            .expect("U1 on the sample sheet");
        (symbol.uuid, symbol.position)
    };

    cursor_at(&mut sample.app, at.x as f32, at.y as f32);
    assert_eq!(sample.app.interaction_state.hover_symbol_uuid, Some(uuid));
    assert!(sample.app.interaction_state.hover_started_at.is_some());

    cursor_at(&mut sample.app, -500.0, -500.0);
    assert_eq!(sample.app.interaction_state.hover_symbol_uuid, None);
    assert!(sample.app.interaction_state.hover_started_at.is_none());
    assert!(!is_dirty(&sample.app, &path), "a hover edits nothing");
}

#[test]
fn m10_moving_within_a_symbol_keeps_the_dwell_running() {
    let mut sample = sample_with_sheet();
    let path = sample
        .app
        .document_state
        .active_path
        .clone()
        .expect("the sheet");
    let at = sample.app.document_state.engines[&path]
        .document()
        .symbols
        .iter()
        .find(|symbol| symbol.reference == "U1")
        .expect("U1")
        .position;

    cursor_at(&mut sample.app, at.x as f32, at.y as f32);
    let started = sample.app.interaction_state.hover_started_at;
    cursor_at(&mut sample.app, at.x as f32 + 0.01, at.y as f32);

    assert_eq!(sample.app.interaction_state.hover_started_at, started);
}
