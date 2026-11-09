//! A snapshot of what a person is doing in the window, for the tests that an
//! agent's call by document id leaves all of it as it was (#2172): which
//! document is active, the tab strip, focus, the selection and the tool, a
//! drag counter, a text edit in progress, the wire being drawn and the
//! command palette.

use crate::app::Signex;

/// Everything the principle says an agent must not move, as one comparable
/// string. Read through `Debug`, so a field that grows a value shows up.
pub(in crate::app) fn interaction_snapshot(app: &Signex) -> String {
    let docs = &app.document_state;
    let canvas = app.interaction_state.active_canvas();
    let tabs: Vec<String> = docs
        .tabs
        .iter()
        .map(|tab| format!("{} {:?}", tab.path.display(), tab.kind))
        .collect();
    format!(
        "active_path={:?}\nactive_tab={}\nactive_project={:?}\ntabs={tabs:?}\nfocused_window={:?}\n\
         tool={:?}\nselected={:?}\nmove_epoch={}\nediting_text={:?}\nwire_preview={:?}\n\
         drawing_mode={}\npalette={:?}\npending_net_color={:?}\nmove_pick={:?}\n\
         wire_drawing={}\narc={:?}\npolyline={:?}\nshape_anchor={:?}\n\
         pending_port={:?}\ndraw_mode={:?}\npre_placement={:?}\npanel_list_open={}\n\
         dock_tab_drag={:?}\ndock_floating={}\ndock_drop_preview={:?}\ndock_menu_open={}\nslot_move_pick={:?}",
        docs.active_path,
        docs.active_tab,
        docs.active_project,
        app.ui_state.focused_window,
        app.interaction_state.current_tool,
        canvas.selected,
        canvas.move_epoch,
        app.interaction_state.editing_text,
        canvas.wire_preview,
        canvas.drawing_mode,
        app.ui_state.command_palette,
        app.ui_state.pending_net_color,
        app.ui_state.move_pick,
        app.interaction_state.wire_drawing,
        app.interaction_state.arc_points,
        app.interaction_state.polyline_points,
        app.interaction_state.shape_anchor,
        app.interaction_state.pending_port,
        app.interaction_state.draw_mode,
        docs.panel_ctx.pre_placement,
        app.ui_state.panel_list_open,
        docs.dock.tab_drag,
        docs.dock.floating.len(),
        docs.dock.drop_preview,
        docs.dock.tab_menu.is_some(),
        canvas.move_pick,
    )
}
