//! #919 follow-up — the four styled power ports (Arrow / Wave / Bar / Circle)
//! are ordinary power ports that merge by value. They once shared the
//! `power:PWR_FLAG` lib_id, which the netlist and ERC treat as a flag that
//! names no net, so same-value styled ports silently stopped joining.

use signex_app::active_bar::{ActiveBarAction, ActiveBarMsg};
use signex_app::app::{Message, Signex};

#[test]
fn styled_power_ports_are_not_power_flags() {
    for action in [
        ActiveBarAction::PlacePowerArrow,
        ActiveBarAction::PlacePowerWave,
        ActiveBarAction::PlacePowerBar,
        ActiveBarAction::PlacePowerCircle,
    ] {
        let (mut app, _task) = Signex::new();
        let _ = app.update(Message::ActiveBar(ActiveBarMsg::Action(action.clone())));
        let (_, lib_id) = app
            .interaction_state
            .pending_power
            .clone()
            .unwrap_or_else(|| panic!("{action:?} should arm a pending power port"));
        assert!(
            !signex_net::is_power_flag(&lib_id),
            "{action:?} armed lib_id {lib_id:?}, which the netlist reads as a power flag"
        );
    }
}

/// #1284 - the Active Bar's chassis row arms a chassis-style port that names
/// its own net.
#[test]
fn the_chassis_row_arms_a_chassis_port() {
    let (mut app, _task) = Signex::new();
    let _ = app.update(Message::ActiveBar(ActiveBarMsg::Action(
        ActiveBarAction::PlacePowerChassis,
    )));
    let (net, lib_id) = app
        .interaction_state
        .pending_power
        .clone()
        .expect("the chassis row arms a pending power port");
    assert_eq!(net, "Chassis");
    assert_eq!(lib_id, "power:Chassis");
    assert_eq!(
        signex_types::schematic::PowerPortStyle::of(&lib_id),
        signex_types::schematic::PowerPortStyle::Chassis
    );
    assert!(!signex_net::is_power_flag(&lib_id));
}
