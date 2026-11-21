//! A core route is gated from core facts alone (ADR-0006 slice 7): the
//! `Needs` category says what a gate reads, the parity `owner` column says
//! who owns the route, and a core-owned route behind a `CoreAndUi` gate is a
//! listed exception that may only shrink.

use std::collections::BTreeSet;

use super::forms::{Form, Owner};
use super::needs::{self, NeedReads};
use super::rows;
use crate::keymap::AppCommandId;

/// Core-owned routes still gated on the window's selection. Each entry
/// becomes core-gated when slice 6's `Targets` operand lands; remove it then.
const AWAITS_TARGETS: &[&str] = &[
    "arrange_in_box",
    "board_shape_from_selection",
    "bring_drawings_forward",
    "bring_drawings_to_front",
    "delete_selection",
    "gloss_selected_routes",
    "keepout_from_selection",
    "place_board_reuse_block",
    "save_board_reuse_block",
    "send_drawings_backward",
    "send_drawings_to_back",
    "swap_gates",
    "swap_pins",
];

/// The ids of core-owned dispatch routes whose gate reads UI facts.
fn core_routes_gated_with_ui() -> BTreeSet<&'static str> {
    let routed = [
        Form::Bridge,
        Form::BridgeContextMenu,
        Form::ActiveBar,
        Form::Operand,
    ];
    rows()
        .into_iter()
        .filter(|row| row.owner == Owner::Core && routed.contains(&row.form))
        .filter(|row| {
            let command = AppCommandId::new(row.id).expect("catalog id is valid");
            needs::needs(&command).is_some_and(|gate| gate.reads() == NeedReads::CoreAndUi)
        })
        .map(|row| row.id)
        .collect()
}

#[test]
fn a_core_route_is_gated_from_core_facts_alone() {
    let found = core_routes_gated_with_ui();
    let listed: BTreeSet<&str> = AWAITS_TARGETS.iter().copied().collect();
    let new: Vec<&&str> = found.difference(&listed).collect();
    assert!(
        new.is_empty(),
        "core routes gated on UI facts, not in AWAITS_TARGETS: {new:?}"
    );
    let stale: Vec<&&str> = listed.difference(&found).collect();
    assert!(
        stale.is_empty(),
        "AWAITS_TARGETS entries that are core-gated now; remove them: {stale:?}"
    );
}
