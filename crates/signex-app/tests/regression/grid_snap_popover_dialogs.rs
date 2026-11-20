//! The status-bar Grid & Snap popover (mockup row 6) goes when Find and
//! Replace, Paste Array or the Parameter Manager opens over it.

use signex_app::app::{EditMsg, Message, OverlayMsg, ParameterManagerMsg, Signex};

use crate::paste_stamp_array::open_array;
use crate::paste_support::sheet::{app_with_sheet, populated_sheet, select};

fn open_popover(app: &mut Signex) {
    let window = iced::window::Id::unique();
    let _ = app.update(Message::Overlay(OverlayMsg::GridSnapPopoverToggled(window)));
    assert!(
        app.ui_state.grid_snap_popover.is_some(),
        "the popover is open"
    );
}

#[test]
fn find_and_replace_closes_the_grid_and_snap_popover() {
    let mut app = app_with_sheet(populated_sheet());
    open_popover(&mut app);
    let _ = app.update(Message::Overlay(OverlayMsg::OpenFind));
    assert!(app.ui_state.find_replace.open);
    assert!(app.ui_state.grid_snap_popover.is_none());
}

#[test]
fn paste_array_closes_the_grid_and_snap_popover() {
    let mut app = app_with_sheet(populated_sheet());
    select(&mut app, &[("R1", 1)], true);
    let _ = app.update(Message::Edit(EditMsg::Copy));
    open_popover(&mut app);
    open_array(&mut app);
    assert!(app.ui_state.paste_array.is_some(), "the dialog is open");
    assert!(app.ui_state.grid_snap_popover.is_none());
}

#[test]
fn the_parameter_manager_closes_the_grid_and_snap_popover() {
    let mut app = app_with_sheet(populated_sheet());
    open_popover(&mut app);
    let _ = app.update(Message::ParameterManager(ParameterManagerMsg::Open));
    assert!(app.ui_state.parameter_manager_open);
    assert!(app.ui_state.grid_snap_popover.is_none());
}
