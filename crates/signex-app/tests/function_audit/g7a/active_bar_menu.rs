//! m61 Schematic active bar and m62 canvas context menu.

use super::*;

fn bar(app: &mut Signex, msg: ActiveBarMsg) {
    let _ = app.update(Message::ActiveBar(msg));
}

fn tool_is(app: &Signex, tool: Tool) -> bool {
    app.interaction_state.current_tool == tool
}

fn show_menu(app: &mut Signex) {
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::Show(720.0, 360.0)));
}

fn menu_command(app: &mut Signex, id: &str) {
    drive(
        app,
        Message::ContextMenu(ContextMenuMsg::Action(ContextAction::Command {
            id: command(id),
            project: None,
        })),
    );
}

fn select_part(app: &mut Signex, reference: &str) -> Uuid {
    let id = symbol_id(app, reference).expect("the part is on the sheet");
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(id, SelectedKind::Symbol)];
    id
}

#[test]
fn m61_the_wire_row_arms_the_wire_tool_and_closes_the_menu() {
    let (mut app, _fx) = sample();
    bar(&mut app, ActiveBarMsg::ToggleMenu(ActiveBarMenu::Wiring));
    assert!(app.interaction_state.active_bar_menu.is_some());

    bar(&mut app, ActiveBarMsg::Action(ActiveBarAction::DrawWire));

    assert!(tool_is(&app, Tool::Wire));
    assert!(app.interaction_state.active_bar_menu.is_none());
    assert!(is_clean(&app), "arming a tool is no edit");
}

#[test]
fn m61_closing_a_menu_leaves_the_tool_and_the_sheet() {
    let (mut app, _fx) = sample();
    let before = doc_counts(&app);
    bar(&mut app, ActiveBarMsg::ToggleMenu(ActiveBarMenu::Power));
    bar(&mut app, ActiveBarMsg::CloseMenus);
    assert!(app.interaction_state.active_bar_menu.is_none());
    assert!(tool_is(&app, Tool::Select));
    assert_eq!(doc_counts(&app), before);
}

#[test]
fn m61_a_right_press_outside_the_flyout_closes_it_and_keeps_the_armed_tool() {
    let (mut app, _fx) = sample();
    bar(&mut app, ActiveBarMsg::Action(ActiveBarAction::DrawWire));
    bar(&mut app, ActiveBarMsg::ToggleMenu(ActiveBarMenu::Wiring));

    bar(&mut app, ActiveBarMsg::FlyoutRightPressed);

    assert!(app.interaction_state.active_bar_menu.is_none());
    assert!(tool_is(&app, Tool::Wire), "the armed tool stays armed");
    assert!(app.interaction_state.swallow_right_release);
    bar(&mut app, ActiveBarMsg::RightReleaseSwallowed);
    assert!(!app.interaction_state.swallow_right_release);
}

#[test]
fn m61_the_corner_pill_sets_how_wires_bend() {
    let (mut app, _fx) = sample();
    bar(&mut app, ActiveBarMsg::Action(ActiveBarAction::DrawWire));
    bar(
        &mut app,
        ActiveBarMsg::CornerModePicked(CornerMode::Angle45Start),
    );
    assert!(matches!(
        app.interaction_state.corner_rule.mode,
        CornerMode::Angle45Start
    ));
    bar(&mut app, ActiveBarMsg::CornerModePicked(CornerMode::Ortho90));
    assert!(matches!(
        app.interaction_state.corner_rule.mode,
        CornerMode::Ortho90
    ));
}

#[test]
fn m61_a_planned_tool_is_refused_and_arms_nothing() {
    let (mut app, _fx) = sample();
    let before = doc_counts(&app);
    bar(&mut app, ActiveBarMsg::ToggleMenu(ActiveBarMenu::SheetSymbol));
    bar(
        &mut app,
        ActiveBarMsg::Action(ActiveBarAction::PlaceDeviceSheetSymbol),
    );
    assert!(tool_is(&app, Tool::Select));
    assert!(app.interaction_state.active_bar_menu.is_none());
    assert_eq!(doc_counts(&app), before);
    assert!(is_clean(&app));
}

#[test]
fn m61_a_net_colour_arms_the_pen_and_a_right_click_puts_it_down() {
    let (mut app, _fx) = sample();
    bar(&mut app, ActiveBarMsg::ToggleMenu(ActiveBarMenu::NetColor));
    bar(&mut app, ActiveBarMsg::Action(ActiveBarAction::NetColorRed));
    assert!(app.ui_state.pending_net_color.is_some(), "the pen is armed");
    assert!(app.interaction_state.active_bar_menu.is_none());

    let _ = app.update(Message::ContextMenu(ContextMenuMsg::Show(300.0, 300.0)));

    assert!(app.ui_state.pending_net_color.is_none());
    assert!(
        app.interaction_state.context_menu.is_none(),
        "that right-click opens no menu"
    );
    assert!(is_clean(&app));
}

#[test]
fn m61_the_filter_menu_edits_the_filter_the_panels_show() {
    let (mut app, _fx) = sample();
    bar(&mut app, ActiveBarMsg::ToggleMenu(ActiveBarMenu::Filter));
    bar(&mut app, ActiveBarMsg::NoFiltersPicked);
    assert_eq!(on_count(&app), 0);
    assert!(app.document_state.panel_ctx.selection_filters.is_empty());

    bar(&mut app, ActiveBarMsg::ToggleFilter(SelectionFilter::Wires));
    assert!(is_on(&app, SelectionFilter::Wires));
    assert_eq!(on_count(&app), 1);

    bar(&mut app, ActiveBarMsg::AllFiltersPicked);
    assert_eq!(on_count(&app), SelectionFilter::ALL.len());
}

#[test]
fn m61_save_current_then_a_preset_button_restores_the_saved_kinds() {
    let (mut app, _fx) = sample();
    bar(&mut app, ActiveBarMsg::NoFiltersPicked);
    bar(&mut app, ActiveBarMsg::ToggleFilter(SelectionFilter::Ports));
    bar(&mut app, ActiveBarMsg::FilterPresetSavePicked);
    bar(&mut app, ActiveBarMsg::AllFiltersPicked);

    bar(&mut app, ActiveBarMsg::ApplyCustomFilter(0));

    assert_eq!(on_count(&app), 1);
    assert!(is_on(&app, SelectionFilter::Ports));
}

#[test]
fn m61_a_preset_button_with_no_preset_changes_nothing() {
    let (mut app, _fx) = sample();
    bar(&mut app, ActiveBarMsg::ToggleFilter(SelectionFilter::Texts));
    let on = on_count(&app);
    bar(&mut app, ActiveBarMsg::ApplyCustomFilter(2));
    assert_eq!(on_count(&app), on);
}

#[test]
fn m62_a_right_click_opens_the_menu_and_close_changes_nothing() {
    let (mut app, _fx) = sample();
    let before = doc_counts(&app);
    show_menu(&mut app);
    let menu = app.interaction_state.context_menu.as_ref().expect("menu");
    assert_eq!((menu.x, menu.y), (720.0, 360.0));

    let _ = app.update(Message::ContextMenu(ContextMenuMsg::Close));

    assert!(app.interaction_state.context_menu.is_none());
    assert_eq!(doc_counts(&app), before);
    assert!(is_clean(&app));
}

#[test]
fn m62_the_place_flyout_opens_and_a_second_press_folds_it() {
    let (mut app, _fx) = sample();
    show_menu(&mut app);
    let place = || Message::ContextMenu(ContextMenuMsg::SubmenuOpen(ContextSubmenu::Place));
    let _ = app.update(place());
    assert_eq!(
        app.interaction_state.context_submenu,
        Some(ContextSubmenu::Place)
    );
    let _ = app.update(place());
    assert_eq!(app.interaction_state.context_submenu, None);
}

#[test]
fn m62_closing_the_menu_also_closes_its_flyout() {
    let (mut app, _fx) = sample();
    show_menu(&mut app);
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::SubmenuOpen(
        ContextSubmenu::Place,
    )));
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::Close));
    assert_eq!(app.interaction_state.context_submenu, None);
}

#[test]
fn m62_a_place_row_arms_its_tool_and_closes_the_menu() {
    let (mut app, _fx) = sample();
    show_menu(&mut app);
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::SubmenuOpen(
        ContextSubmenu::Place,
    )));
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::Action(
        ContextAction::ActiveBar(ActiveBarAction::DrawWire),
    )));
    assert!(tool_is(&app, Tool::Wire));
    assert!(app.interaction_state.context_menu.is_none());
    assert_eq!(app.interaction_state.context_submenu, None);
}

#[test]
fn m62_delete_from_the_menu_removes_the_part_marks_dirty_and_undo_restores_it() {
    let (mut app, _fx) = sample();
    let before = doc_counts(&app);
    let r1 = select_part(&mut app, "R1");
    show_menu(&mut app);

    menu_command(&mut app, "delete_selection");

    assert!(app.interaction_state.context_menu.is_none());
    assert_eq!(symbol_id(&app, "R1"), None, "R1 is gone");
    assert_eq!(doc_counts(&app).0, before.0 - 1);
    assert!(!is_clean(&app), "the deletion is an unsaved edit");

    drive(&mut app, Message::Edit(EditMsg::Undo));

    assert_eq!(symbol_id(&app, "R1"), Some(r1), "undo brings R1 back");
    assert_eq!(doc_counts(&app), before);
}

#[test]
fn m62_delete_with_nothing_selected_removes_nothing() {
    let (mut app, _fx) = sample();
    let before = doc_counts(&app);
    app.interaction_state.active_canvas_mut().selected.clear();
    show_menu(&mut app);
    menu_command(&mut app, "delete_selection");
    assert_eq!(doc_counts(&app), before);
    assert!(is_clean(&app));
}

#[test]
fn m62_a_right_click_with_a_tool_armed_ends_the_tool_and_opens_no_menu() {
    let (mut app, _fx) = sample();
    bar(&mut app, ActiveBarMsg::Action(ActiveBarAction::DrawWire));
    show_menu(&mut app);
    assert!(tool_is(&app, Tool::Select));
    assert!(app.interaction_state.context_menu.is_none());
}
