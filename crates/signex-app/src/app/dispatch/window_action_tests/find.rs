//! #2037 / #1671: the Find dialog of an undocked window searches that window's
//! sheet, paints where it was opened and closes with its window; the search
//! panel shortcuts toggle it from any window.

use super::surfaces::command;
use super::*;
use crate::app::path_ops::settle_task;
use crate::find_replace::FindReplaceMsg;

const FIND_FOOTER: &str = "Searches labels, notes, designators, values";

#[test]
fn find_opened_in_a_window_searches_and_selects_in_that_windows_sheet() {
    let mut f = fixture_with_a_labelled_a3_window();
    f.app
        .interaction_state
        .canvases
        .get_mut(&f.window)
        .unwrap()
        .selected
        .clear();

    let (_outcome, task) = command(&mut f.app, "find", f.window);
    settle_task(&mut f.app, task);
    assert_eq!(f.app.ui_state.find_replace.window, Some(f.window));
    let task = f
        .app
        .update(Message::FindReplaceMsg(FindReplaceMsg::QueryChanged(
            "WINDOW_NET".into(),
        )));
    settle_task(&mut f.app, task);

    let find = &f.app.ui_state.find_replace;
    assert_eq!(find.matches.len(), 1, "the window's label was not found");
    assert_eq!(
        find.matches[0].sheet.as_deref(),
        Some(f.window_path.as_path())
    );
    let selected = &f.app.interaction_state.canvases[&f.window].selected;
    assert_eq!(selected.len(), 1, "the hit was not selected in the window");
    assert_eq!(selected[0].uuid, label_id());
    assert!(
        f.app.interaction_state.canvas.selected.is_empty(),
        "the main window's selection changed"
    );
}

#[test]
fn the_find_dialog_paints_in_the_window_it_was_opened_in_only() {
    let mut f = fixture();
    let (_outcome, task) = command(&mut f.app, "find", f.window);
    settle_task(&mut f.app, task);
    assert!(has(&f.app, f.window, FIND_FOOTER));
    assert!(!has(&f.app, f.main, FIND_FOOTER));

    // Opened again from the main window, it moves there.
    let (_outcome, task) = command(&mut f.app, "find", f.main);
    settle_task(&mut f.app, task);
    assert!(has(&f.app, f.main, FIND_FOOTER));
    assert!(!has(&f.app, f.window, FIND_FOOTER));
}

#[test]
fn the_search_panel_toggle_closes_a_detached_find_from_any_window() {
    // Find opened from the undocked window shows in its own OS window, so
    // a press in that Find window, the main window or the undocked window
    // closes it, whichever sheet it searches.
    let mut f = fixture();
    f.app.ui_state.main_window_opened = true;
    let find_window = |app: &mut Signex| {
        let _ = app.sync_detached_modals();
        app.detached_window_of(crate::app::state::ModalId::FindReplace)
            .expect("Find opens in its own window")
    };
    for presser in ["find window", "main", "undocked"] {
        let (_outcome, task) = command(&mut f.app, "toggle_search_panel", f.window);
        settle_task(&mut f.app, task);
        assert_eq!(f.app.ui_state.find_replace.window, Some(f.window));
        let pressed_in = match presser {
            "find window" => find_window(&mut f.app),
            "main" => f.main,
            _ => f.window,
        };
        let (_outcome, task) = command(&mut f.app, "toggle_search_panel", pressed_in);
        settle_task(&mut f.app, task);
        assert!(
            !f.app.ui_state.find_replace.open,
            "a press in the {presser} closes Find"
        );
        let _ = f.app.sync_detached_modals();
    }
}

#[test]
fn the_search_panel_toggle_closes_a_replace_find_and_reopens_plain_find() {
    let mut f = fixture();
    let (_outcome, task) = command(&mut f.app, "find_and_replace", f.window);
    settle_task(&mut f.app, task);
    assert!(f.app.ui_state.find_replace.replace_mode);

    let (_outcome, task) = command(&mut f.app, "toggle_floating_panels", f.window);
    settle_task(&mut f.app, task);
    assert!(!f.app.ui_state.find_replace.open);

    let (_outcome, task) = command(&mut f.app, "toggle_floating_panels", f.window);
    settle_task(&mut f.app, task);
    assert!(f.app.ui_state.find_replace.open);
    assert!(!f.app.ui_state.find_replace.replace_mode);
}

#[test]
fn closing_the_window_closes_the_find_dialog_it_opened() {
    let mut f = fixture();
    let (_outcome, task) = command(&mut f.app, "find", f.window);
    settle_task(&mut f.app, task);
    assert!(f.app.ui_state.find_replace.open);

    f.app.close_overlays_owned_by(f.window);

    assert!(!f.app.ui_state.find_replace.open);
    assert_eq!(f.app.ui_state.find_replace.window, None);
}
