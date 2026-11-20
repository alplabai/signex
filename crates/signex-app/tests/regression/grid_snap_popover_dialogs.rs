//! The status-bar Grid & Snap popover (mockup row 6) goes when Find and
//! Replace, Paste Array, the Parameter Manager or any other dialog opens over
//! it.

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

/// No dialog clears the popover by hand: any dialog that opens closes it,
/// the Passive Network Calculator included.
#[test]
fn any_dialog_that_opens_closes_the_grid_and_snap_popover() {
    let mut app = app_with_sheet(populated_sheet());
    open_popover(&mut app);
    let _ = app.update(Message::Overlay(OverlayMsg::OpenPassiveCalculator));
    assert!(app.ui_state.passive_calculator_open);
    assert!(app.ui_state.grid_snap_popover.is_none());
}

/// A popover opened while a dialog is already up stays open: only a dialog
/// opening over it closes it.
#[test]
fn a_popover_opened_beside_an_open_dialog_stays_open() {
    let mut app = app_with_sheet(populated_sheet());
    let _ = app.update(Message::Overlay(OverlayMsg::OpenPassiveCalculator));
    open_popover(&mut app);
    let _ = app.update(Message::Overlay(OverlayMsg::GridSnapPreviewHovered(
        signex_widgets::property_illustration::IllustrationMsg::FieldHovered(
            signex_widgets::property_illustration::FieldId::new("grid-snap.visible-pitch"),
        ),
    )));
    assert!(app.ui_state.grid_snap_popover.is_some());
}
