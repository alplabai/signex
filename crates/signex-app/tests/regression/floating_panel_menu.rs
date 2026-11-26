//! A right-click menu opened from a floating panel (#2376). The menu paints
//! above the floating panels now, so it is also the first thing Esc closes;
//! the floating panel stays where the user left it. The paint order itself
//! and a click on a menu row over the panel are pinned in
//! `app/view/overlay_id.rs`.

use signex_app::app::{ContextMenuMsg, InputMsg, Message, Signex};
use signex_app::dock::{DockMessage, DockTabAction, PanelPosition};

fn app_with_a_floating_panel() -> (Signex, iced::window::Id) {
    let (mut app, _task) = Signex::new();
    let main = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main);
    app.ui_state.window_size = (1400.0, 900.0);
    app.ui_state.first_run_tour_open = false;
    let _ = app.update(Message::Dock(DockMessage::TabContextMenuRequested(
        PanelPosition::Left,
        0,
    )));
    let _ = app.update(Message::Dock(DockMessage::TabMenuActionPicked(
        DockTabAction::Float,
    )));
    assert_eq!(app.document_view.dock.floating.len(), 1, "nothing floats");
    (app, main)
}

#[test]
fn esc_closes_a_menu_opened_over_a_floating_panel_and_keeps_the_panel() {
    let (mut app, main) = app_with_a_floating_panel();
    let panel = &app.document_view.dock.floating[0];
    let (x, y) = (panel.x + 20.0, panel.y + 40.0);
    let (kind, at) = (panel.kind, (panel.x, panel.y));

    app.interaction_state.last_mouse_pos = (x, y);
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::ShowProjectTree(None)));
    assert!(app.interaction_state.project_tree_context_menu.is_some());

    let _ = app.update(Message::Input(InputMsg::EscapePressed {
        window: Some(main),
    }));
    assert!(
        app.interaction_state.project_tree_context_menu.is_none(),
        "Esc did not close the menu over the floating panel"
    );
    let floating = &app.document_view.dock.floating;
    assert_eq!(floating.len(), 1, "Esc closed the floating panel");
    assert_eq!(
        (floating[0].kind, (floating[0].x, floating[0].y)),
        (kind, at)
    );
}
