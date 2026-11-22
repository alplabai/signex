//! m122 and m123: the symbol editor's Properties, with the Pins dock panel
//! folded into its Pins tab. The Pin table document tab is recorded as not
//! built (2026-11-19 decision); its interim table inside the tab is toggled.

use signex_app::app::{EditMsg, Message};
use signex_app::dock::DockMessage;
use signex_app::library::editor::symbol::{SymbolPropsMsg, SymbolPropsTab};
use signex_app::panels::PanelMsg;
use signex_library::{PinDirection, PinOrientation};

use super::support::{SymbolRig, symbol_rig};

fn panel(r: &mut SymbolRig, msg: PanelMsg) {
    let _ = r.app.update(Message::Dock(DockMessage::Panel(msg)));
}

fn undo(r: &mut SymbolRig) {
    let _ = r.app.update(Message::Edit(EditMsg::Undo));
}

#[test]
fn pins_is_a_tab_of_properties_and_no_longer_a_panel() {
    assert!(
        !signex_app::panels::ALL_PANELS
            .iter()
            .any(|kind| format!("{kind:?}") == "SymbolPins"),
        "Pins is a tab of Properties, not a panel"
    );
}

#[test]
fn editing_a_pin_name_changes_the_symbol_and_undo_restores_it() {
    let mut r = symbol_rig();
    let before = r.pins()[0].name.clone();

    panel(
        &mut r,
        PanelMsg::SymEditorSetPinName {
            pin_idx: 0,
            value: "+INA".to_string(),
        },
    );
    assert_eq!(r.pins()[0].name, "+INA");
    assert!(r.dirty(), "the edit marks the symbol unsaved");

    undo(&mut r);
    assert_eq!(r.pins()[0].name, before);
    assert!(!r.dirty(), "undo lands on the save point");
}

#[test]
fn editing_a_pin_number_changes_only_that_pin() {
    let mut r = symbol_rig();
    let other = r.pins()[1].number.clone();

    panel(
        &mut r,
        PanelMsg::SymEditorSetPinNumber {
            pin_idx: 0,
            value: "42".to_string(),
        },
    );

    assert_eq!(r.pins()[0].number, "42");
    assert_eq!(r.pins()[1].number, other);
}

#[test]
fn pin_location_length_type_and_orientation_reach_the_pin() {
    let mut r = symbol_rig();
    for msg in [
        PanelMsg::SymEditorSetPinX {
            pin_idx: 0,
            value: 5.08,
        },
        PanelMsg::SymEditorSetPinY {
            pin_idx: 0,
            value: -2.54,
        },
        PanelMsg::SymEditorSetPinLength {
            pin_idx: 0,
            value: 3.81,
        },
        PanelMsg::SymEditorSetPinElectrical {
            pin_idx: 0,
            value: PinDirection::Output,
        },
        PanelMsg::SymEditorSetPinOrientation {
            pin_idx: 0,
            value: PinOrientation::Up,
        },
    ] {
        panel(&mut r, msg);
    }

    let pin = &r.pins()[0];
    assert_eq!(pin.position, [5.08, -2.54]);
    assert_eq!(pin.length, 3.81);
    assert_eq!(pin.electrical, PinDirection::Output);
    assert_eq!(pin.orientation, PinOrientation::Up);
}

#[test]
fn a_pin_length_that_is_no_number_is_refused() {
    let mut r = symbol_rig();
    let before = r.pins()[0].length;

    for bad in [f64::NAN, f64::INFINITY] {
        panel(
            &mut r,
            PanelMsg::SymEditorSetPinLength {
                pin_idx: 0,
                value: bad,
            },
        );
    }

    assert_eq!(r.pins()[0].length, before);
    assert!(!r.dirty(), "a refused value changes nothing");
}

#[test]
fn the_visibility_checks_flip_their_flags_and_undo_restores_them() {
    let mut r = symbol_rig();
    let before = r.pins()[0].designator_visible;

    panel(&mut r, PanelMsg::SymEditorTogglePinDesignatorVisible(0));
    assert_eq!(r.pins()[0].designator_visible, !before);

    undo(&mut r);
    assert_eq!(r.pins()[0].designator_visible, before);
}

#[test]
fn the_pins_tab_filter_and_table_toggle_are_not_document_edits() {
    let mut r = symbol_rig();

    panel(
        &mut r,
        PanelMsg::SymbolProps(SymbolPropsMsg::TabPicked(SymbolPropsTab::Pins)),
    );
    panel(
        &mut r,
        PanelMsg::SymbolProps(SymbolPropsMsg::PinFilterChanged("1".to_string())),
    );
    panel(
        &mut r,
        PanelMsg::SymbolProps(SymbolPropsMsg::PinTableToggled),
    );

    let props = &r
        .app
        .document_state
        .panel_ctx
        .symbol_editor
        .as_ref()
        .expect("a symbol tab is active")
        .props;
    assert_eq!(props.tab, SymbolPropsTab::Pins);
    assert_eq!(props.pin_filter, "1");
    assert!(props.pin_table);
    assert!(!r.dirty(), "panel state is never saved or undone");
}
