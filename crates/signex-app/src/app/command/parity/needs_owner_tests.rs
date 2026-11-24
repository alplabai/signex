//! A core route is gated from core facts alone (ADR-0006 slice 7): the
//! `Needs` category says what a gate reads, the parity `owner` column says
//! who owns the route, and a core-owned route behind a `CoreAndUi` gate is a
//! listed exception that may only shrink.

use std::collections::BTreeSet;

use super::forms::{Form, Owner};
use super::needs::{self, NeedReads};
use super::operand_handlers;
use super::rows;
use crate::app::command::translator;
use crate::keymap::AppCommandId;

/// Core-owned routes still gated on the window's selection. An entry leaves
/// once no core-owned route of it is gated on the UI: its bridge row became
/// a GUI translator (`translator.rs`, slice 6) and its operand handler names
/// its document, so `run_operands` runs it past the gate. Remove it then;
/// `a_route_past_the_gate_by_its_document_alone_has_a_translated_gui_route`
/// checks the second reason never removes an id whose GUI route still acts
/// on the selection itself. Empty since slice 6.P2b; it stays so that a
/// route gated on the UI again fails here.
const AWAITS_TARGETS: &[&str] = &[];

/// Translated GUI routes the parity table cannot name `translator`: the
/// bridge arm only arms the tool, and a later board click runs the operand
/// handler (`pcb_arrange_box_clicked`).
const ARMED_THEN_TRANSLATED: &[&str] = &["arrange_in_box"];

/// The ids of core-owned dispatch routes whose gate reads UI facts. With
/// `past_named_documents`, an operand handler that names its document runs
/// past the gate (`run_operands`), so only one that acts on the active tab
/// is gated.
fn core_routes_gated_with_ui(past_named_documents: bool) -> BTreeSet<&'static str> {
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
            let past_the_gate = past_named_documents
                && row.form == Form::Operand
                && operand_handlers::names_its_document(&command);
            !past_the_gate
                && needs::needs(&command).is_some_and(|gate| gate.reads() == NeedReads::CoreAndUi)
        })
        .map(|row| row.id)
        .collect()
}

#[test]
fn a_core_route_is_gated_from_core_facts_alone() {
    let found = core_routes_gated_with_ui(true);
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

/// An id whose only UI-gated core route is an operand handler naming its
/// document is off the list because that handler runs past the gate; its
/// GUI route must then be a translator over the handler, not a second path
/// that reads the selection and edits on its own.
#[test]
fn a_route_past_the_gate_by_its_document_alone_has_a_translated_gui_route() {
    let gated = core_routes_gated_with_ui(true);
    let untranslated: Vec<&str> = core_routes_gated_with_ui(false)
        .difference(&gated)
        .copied()
        .filter(|id| !translator::is_translated(id) && !ARMED_THEN_TRANSLATED.contains(id))
        .collect();
    assert!(
        untranslated.is_empty(),
        "past the gate by their named document, GUI route not translated: {untranslated:?}"
    );
}
