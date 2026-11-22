//! m4 title and menu bar, m5 tabs and tab menu, m6 status bar.

use std::path::PathBuf;

use signex_app::app::{
    ContextMenuMsg, Message, OverlayMsg, Signex, StatusBarRequest, TabContextAction, UiMsg,
};
use signex_app::dock::DockMessage;
use signex_app::menu_bar::{MenuMessage, model::root_titles};
use signex_app::panels::{PanelKind, PanelMsg};
use signex_app::tab_bar::TabMessage;
use signex_app::task_driver::TaskDriver;
use signex_types::coord::Unit;

use super::support::{Desk, desk, esc, is_dirty, main_window, sample_with_sheet, set_paper};
use crate::settle_support::settle;

fn three_tabs() -> (Desk, [PathBuf; 3]) {
    let mut desk = desk();
    let a = desk.sheet("a.snxsch", "A4");
    let b = desk.sheet("b.snxsch", "A4");
    let c = desk.sheet("c.snxsch", "A4");
    (desk, [a, b, c])
}

fn tab_paths(app: &Signex) -> Vec<PathBuf> {
    app.document_state
        .tabs
        .iter()
        .map(|tab| tab.path.clone())
        .collect()
}

fn tab_action(app: &mut Signex, action: TabContextAction) {
    settle(app, Message::ContextMenu(ContextMenuMsg::TabAction(action)));
}

fn tab_msg(app: &mut Signex, msg: TabMessage) {
    let window_id = main_window(app);
    settle(app, Message::Tab { window_id, msg });
}

fn status(app: &mut Signex, request: StatusBarRequest) {
    let _ = app.update(Message::Ui(UiMsg::StatusBar(request)));
}

fn panel(app: &mut Signex, msg: PanelMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
}

// ---- m4 Title bar and menu bar --------------------------------------------

#[test]
fn m4_the_menu_bar_has_the_eight_decided_roots() {
    let app = super::support::app();
    assert_eq!(
        root_titles(&app.ui_state.i18n),
        [
            "File", "Edit", "View", "Project", "Place", "Design", "Tools", "Help"
        ]
    );
}

#[test]
fn m4_the_window_title_counts_the_unsaved_documents_and_clears_after_save() {
    let (mut desk, [a, b, _]) = three_tabs();
    let main = main_window(&desk.app);
    assert!(!desk.app.title(main).contains("unsaved"));

    set_paper(&mut desk.app, &a, "A3");
    set_paper(&mut desk.app, &b, "A3");
    assert!(desk.app.title(main).contains("2 unsaved"));

    settle(&mut desk.app, Message::Menu(MenuMessage::SaveAll));
    assert!(!desk.app.title(main).contains("unsaved"));
}

#[test]
fn m4_save_all_writes_every_unsaved_sheet_and_leaves_a_clean_one_alone() {
    let (mut desk, [a, b, c]) = three_tabs();
    let before = [
        std::fs::read(&a).expect("read"),
        std::fs::read(&b).expect("read"),
        std::fs::read(&c).expect("read"),
    ];
    set_paper(&mut desk.app, &a, "A3");
    set_paper(&mut desk.app, &b, "A2");

    settle(&mut desk.app, Message::Menu(MenuMessage::SaveAll));

    assert_ne!(std::fs::read(&a).expect("read"), before[0]);
    assert_ne!(std::fs::read(&b).expect("read"), before[1]);
    assert_eq!(std::fs::read(&c).expect("read"), before[2]);
    assert!(!is_dirty(&desk.app, &a) && !is_dirty(&desk.app, &b));
}

#[test]
fn m4_file_exit_with_unsaved_edits_asks_first() {
    let mut desk = desk();
    let path = desk.sheet("a.snxsch", "A4");
    set_paper(&mut desk.app, &path, "A3");

    let _ = desk.app.update(Message::Menu(MenuMessage::Exit));

    let prompt = desk.app.ui_state.app_quit_confirm.as_ref().expect("prompt");
    assert_eq!(prompt.dirty_paths, [path]);
}

// ---- m5 Tabs and tab menu -------------------------------------------------

#[test]
fn m5_selecting_a_tab_makes_its_document_active() {
    let (mut desk, [a, ..]) = three_tabs();
    tab_msg(&mut desk.app, TabMessage::Select(0));
    assert_eq!(desk.app.document_state.active_tab, 0);
    assert_eq!(desk.app.document_state.active_path, Some(a));
}

#[test]
fn m5_the_close_button_closes_exactly_that_tab() {
    let (mut desk, [a, b, c]) = three_tabs();
    tab_msg(&mut desk.app, TabMessage::CloseClicked(1));
    assert!(!tab_paths(&desk.app).contains(&b));
    assert_eq!(tab_paths(&desk.app), [a, c]);
}

#[test]
fn m5_the_tab_menu_opens_on_that_tab_and_closing_it_changes_nothing() {
    let (mut desk, paths) = three_tabs();
    let _ = desk
        .app
        .update(Message::ContextMenu(ContextMenuMsg::ShowTab(1)));
    assert_eq!(
        desk.app
            .interaction_state
            .tab_context_menu
            .as_ref()
            .map(|menu| menu.tab_idx),
        Some(1)
    );

    let _ = desk
        .app
        .update(Message::ContextMenu(ContextMenuMsg::CloseTab));

    assert!(desk.app.interaction_state.tab_context_menu.is_none());
    assert_eq!(tab_paths(&desk.app), paths);
}

#[test]
fn m5_close_all_other_documents_keeps_only_that_tab() {
    let (mut desk, [_, b, _]) = three_tabs();
    tab_action(&mut desk.app, TabContextAction::CloseAllOthers(1));
    assert_eq!(tab_paths(&desk.app), [b.clone()]);
    assert_eq!(desk.app.document_state.active_path, Some(b));
}

#[test]
fn m5_close_tabs_to_the_right_keeps_the_tabs_up_to_it() {
    let (mut desk, [a, b, _]) = three_tabs();
    tab_action(&mut desk.app, TabContextAction::CloseToTheRight(1));
    assert_eq!(tab_paths(&desk.app), [a, b]);
}

#[test]
fn m5_close_tabs_to_the_right_on_the_last_tab_closes_nothing() {
    let (mut desk, paths) = three_tabs();
    tab_action(&mut desk.app, TabContextAction::CloseToTheRight(2));
    assert_eq!(tab_paths(&desk.app), paths);
}

#[test]
fn m5_copy_path_puts_the_tab_file_on_the_clipboard() {
    let (mut desk, [_, b, _]) = three_tabs();
    let mut driver = TaskDriver::new();
    driver
        .send(
            &mut desk.app,
            Message::ContextMenu(ContextMenuMsg::TabAction(TabContextAction::CopyPath(1))),
        )
        .expect("the copy settles");
    assert_eq!(
        driver.clipboard_text(),
        Some(b.display().to_string().as_str())
    );
}

#[test]
fn m5_reveal_in_projects_brings_the_projects_panel_forward() {
    let mut sample = sample_with_sheet();
    let index = sample.app.document_state.active_tab;
    sample
        .app
        .document_state
        .dock
        .show_panel(PanelKind::Components);

    tab_action(&mut sample.app, TabContextAction::RevealInProjects(index));

    assert!(
        sample
            .app
            .document_state
            .dock
            .is_frontmost(PanelKind::Projects)
    );
}

#[test]
fn m5_open_in_new_window_gives_the_tab_its_own_window() {
    let (mut desk, _) = three_tabs();
    let before = desk.app.ui_state.windows.len();
    tab_action(&mut desk.app, TabContextAction::Undock(1));
    assert_eq!(desk.app.ui_state.windows.len(), before + 1);
}

#[test]
fn m5_the_overflow_menu_opens_and_a_pick_makes_that_tab_active() {
    let (mut desk, [a, ..]) = three_tabs();
    tab_msg(&mut desk.app, TabMessage::OverflowMenuToggled);
    assert!(desk.app.ui_state.tab_overflow_menu_open);

    tab_msg(&mut desk.app, TabMessage::OverflowTabPicked(0));

    assert!(!desk.app.ui_state.tab_overflow_menu_open);
    assert_eq!(desk.app.document_state.active_path, Some(a));
}

#[test]
fn m5_dismissing_the_overflow_menu_keeps_the_active_tab() {
    let (mut desk, [.., c]) = three_tabs();
    tab_msg(&mut desk.app, TabMessage::OverflowMenuToggled);
    tab_msg(&mut desk.app, TabMessage::OverflowMenuDismissed);
    assert!(!desk.app.ui_state.tab_overflow_menu_open);
    assert_eq!(desk.app.document_state.active_path, Some(c));
}

#[test]
fn m5_an_overflow_pick_past_the_last_tab_is_refused() {
    let (mut desk, [.., c]) = three_tabs();
    tab_msg(&mut desk.app, TabMessage::OverflowTabPicked(9));
    assert_eq!(desk.app.document_state.active_path, Some(c));
}

// ---- m6 Status bar --------------------------------------------------------

#[test]
fn m6_the_grid_read_out_toggles_the_grid_without_dirtying_the_sheet() {
    let mut desk = desk();
    let path = desk.sheet("a.snxsch", "A4");
    let before = desk.app.ui_state.grid_visible;
    status(&mut desk.app, StatusBarRequest::ToggleGrid);
    assert_eq!(desk.app.ui_state.grid_visible, !before);
    status(&mut desk.app, StatusBarRequest::ToggleGrid);
    assert_eq!(desk.app.ui_state.grid_visible, before);
    assert!(!is_dirty(&desk.app, &path));
}

#[test]
fn m6_the_snap_pill_toggles_snap() {
    let mut desk = desk();
    desk.sheet("a.snxsch", "A4");
    let before = desk.app.ui_state.snap_enabled;
    status(&mut desk.app, StatusBarRequest::ToggleSnap);
    assert_eq!(desk.app.ui_state.snap_enabled, !before);
    status(&mut desk.app, StatusBarRequest::ToggleSnap);
    assert_eq!(desk.app.ui_state.snap_enabled, before);
}

/// Row m6: the unit "is still the click target that cycles mm / mil / in".
#[test]
fn m6_the_unit_button_cycles_mm_mil_inch_and_micrometre() {
    let mut desk = desk();
    desk.sheet("a.snxsch", "A4");
    desk.app.ui_state.unit = Unit::Mm;
    status(&mut desk.app, StatusBarRequest::CycleUnit);
    assert_eq!(desk.app.ui_state.unit, Unit::Mil);
    status(&mut desk.app, StatusBarRequest::CycleUnit);
    assert_eq!(desk.app.ui_state.unit, Unit::Inch);
    status(&mut desk.app, StatusBarRequest::CycleUnit);
    assert_eq!(desk.app.ui_state.unit, Unit::Micrometer);
    status(&mut desk.app, StatusBarRequest::CycleUnit);
    assert_eq!(desk.app.ui_state.unit, Unit::Mm);
}

#[test]
fn m6_the_panels_button_opens_and_closes_the_panel_list() {
    let mut desk = desk();
    desk.sheet("a.snxsch", "A4");
    status(&mut desk.app, StatusBarRequest::TogglePanelList);
    assert!(desk.app.ui_state.panel_list_open);
    status(&mut desk.app, StatusBarRequest::TogglePanelList);
    assert!(!desk.app.ui_state.panel_list_open);
}

#[test]
fn m6_the_selection_pill_opens_properties() {
    let mut desk = desk();
    desk.sheet("a.snxsch", "A4");
    desk.app
        .document_state
        .dock
        .close_panel(PanelKind::Properties);
    status(&mut desk.app, StatusBarRequest::OpenPropertiesForSelection);
    assert!(
        desk.app
            .document_state
            .dock
            .is_frontmost(PanelKind::Properties)
    );
}

#[test]
fn m6_a_right_click_on_grid_opens_the_popover_and_a_second_closes_it() {
    let mut desk = desk();
    desk.sheet("a.snxsch", "A4");
    let main = main_window(&desk.app);
    let toggle = Message::Overlay(OverlayMsg::GridSnapPopoverToggled(main));
    let _ = desk.app.update(toggle.clone());
    assert!(desk.app.ui_state.grid_snap_popover.is_some());
    let _ = desk.app.update(toggle);
    assert!(desk.app.ui_state.grid_snap_popover.is_none());
}

#[test]
fn m6_the_popover_edits_the_same_grid_settings_as_document_options() {
    let mut desk = desk();
    let path = desk.sheet("a.snxsch", "A4");
    let main = main_window(&desk.app);
    let _ = desk
        .app
        .update(Message::Overlay(OverlayMsg::GridSnapPopoverToggled(main)));

    let (grid, snap, hotspots) = (
        desk.app.ui_state.grid_visible,
        desk.app.ui_state.snap_enabled,
        desk.app.ui_state.snap_hotspots,
    );
    panel(&mut desk.app, PanelMsg::ToggleGrid);
    panel(&mut desk.app, PanelMsg::ToggleSnap);
    panel(&mut desk.app, PanelMsg::ToggleSnapHotspots);
    panel(&mut desk.app, PanelMsg::SetVisibleGridSize(5.08));
    panel(&mut desk.app, PanelMsg::SetGridSize(0.635));

    let ui = &desk.app.ui_state;
    assert_eq!(
        (ui.grid_visible, ui.snap_enabled, ui.snap_hotspots),
        (!grid, !snap, !hotspots)
    );
    assert_eq!((ui.visible_grid_mm, ui.grid_size_mm), (5.08, 0.635));
    let ctx = &desk.app.document_state.panel_ctx;
    assert_eq!((ctx.visible_grid_mm, ctx.grid_size_mm), (5.08, 0.635));
    assert!(!is_dirty(&desk.app, &path));
}

#[test]
fn m6_esc_closes_the_popover_and_keeps_its_settings() {
    let mut desk = desk();
    desk.sheet("a.snxsch", "A4");
    let main = main_window(&desk.app);
    let _ = desk
        .app
        .update(Message::Overlay(OverlayMsg::GridSnapPopoverToggled(main)));
    let grid = desk.app.ui_state.grid_size_mm;

    esc(&mut desk.app);

    assert!(desk.app.ui_state.grid_snap_popover.is_none());
    assert_eq!(desk.app.ui_state.grid_size_mm, grid);
}

#[test]
fn m6_opening_a_dialog_closes_the_popover() {
    let mut desk = desk();
    desk.sheet("a.snxsch", "A4");
    let main = main_window(&desk.app);
    let _ = desk
        .app
        .update(Message::Overlay(OverlayMsg::GridSnapPopoverToggled(main)));

    let _ = desk.app.update(Message::Menu(MenuMessage::OpenAbout));

    assert!(desk.app.ui_state.about_open);
    assert!(desk.app.ui_state.grid_snap_popover.is_none());
}
