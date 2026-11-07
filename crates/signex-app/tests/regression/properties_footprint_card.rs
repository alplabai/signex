//! #114 — the FOOTPRINT card of a placed part's Properties: the footprint
//! is read off the UI thread, keyed by the footprint and the library data
//! version, follows a footprint switch, and a name typed over the binding
//! is not shown as the bound footprint.
//!
//! The real `Stock.snxlib`: its chip resistor has 0805 as the default and
//! six alternates.

use signex_app::app::{Message, Signex};
use signex_app::dock::DockMessage;
use signex_app::library::LibraryMessage;
use signex_app::panels::{FootprintCardStatus, FootprintCardView, PanelMsg};

use crate::library_footprint_pick::{
    alternates, app_with_stock, binding_key, key_of, place_selected, select,
};
use crate::library_place_symbol::doc;
use crate::library_symbol_drift::select_first;
use crate::settle_support::settle;

/// Refresh the selection the way a drift pass does, running every task it
/// starts (the card's footprint read among them).
fn refresh_selection(app: &mut Signex) {
    app.library.drift.check_pending = true;
    let job = app.take_drift_job().expect("a library is mounted");
    settle(
        app,
        Message::Library(LibraryMessage::DriftChecked(Box::new(job.run()))),
    );
}

fn card(app: &Signex) -> &FootprintCardView {
    app.document_state
        .panel_ctx
        .footprint_card
        .as_ref()
        .expect("the selected part has a footprint card")
}

fn placed_resistor() -> (Signex, uuid::Uuid) {
    let mut app = app_with_stock();
    let key = key_of(&app, "STOCK-R");
    select(&mut app, &key);
    place_selected(&mut app, &key);
    let id = select_first(&mut app);
    refresh_selection(&mut app);
    (app, id)
}

#[test]
fn the_card_shows_the_placed_parts_library_footprint_once_read() {
    let (app, _) = placed_resistor();
    let shown = card(&app);
    assert_eq!(shown.name, doc(&app).symbols[0].footprint);
    assert!(shown.key.is_some(), "the part is linked to its library footprint");
    assert_eq!(shown.version, app.library.data_version);
    let FootprintCardStatus::Ready(facts) = &shown.status else {
        panic!("the read landed: {:?}", shown.status);
    };
    assert_eq!(facts.name, shown.name);
    assert_eq!(facts.pad_count, 2, "a chip resistor has two pads");
    assert_eq!(facts.summary(), "2 pads", "two pads have no pitch");
}

#[test]
fn selecting_the_part_again_serves_the_card_from_the_cache() {
    let (mut app, _) = placed_resistor();
    let first = card(&app).key.expect("linked");
    app.interaction_state.active_canvas_mut().selected.clear();
    refresh_selection(&mut app);
    assert!(app.document_state.panel_ctx.footprint_card.is_none());

    select_first(&mut app);
    refresh_selection(&mut app);
    let again = card(&app);
    assert_eq!(again.key, Some(first), "same footprint, same library version");
    assert!(matches!(again.status, FootprintCardStatus::Ready(_)));
}

#[test]
fn switching_the_footprint_moves_the_card_to_the_new_one() {
    let (mut app, id) = placed_resistor();
    let before = card(&app).key.expect("linked").footprint;
    let (alt, alt_name) = alternates(&app).remove(0);
    settle(
        &mut app,
        Message::Dock(DockMessage::Panel(PanelMsg::SwitchFootprintChoice(
            id,
            binding_key(&alt),
        ))),
    );
    refresh_selection(&mut app);
    let shown = card(&app);
    assert_eq!(shown.name, alt_name);
    assert_eq!(shown.key.map(|k| k.footprint), Some(alt));
    assert_ne!(Some(before), shown.key.map(|k| k.footprint));
    let FootprintCardStatus::Ready(facts) = &shown.status else {
        panic!("the alternate was read: {:?}", shown.status);
    };
    assert_eq!(facts.name, alt_name);
}

#[test]
fn a_name_typed_over_the_binding_is_not_the_library_footprint() {
    let (mut app, id) = placed_resistor();
    settle(
        &mut app,
        Message::Dock(DockMessage::Panel(PanelMsg::EditSymbolFootprint(
            id,
            "MY-OWN-FP".into(),
        ))),
    );
    refresh_selection(&mut app);
    let shown = card(&app);
    assert_eq!(shown.name, "MY-OWN-FP");
    assert_eq!(shown.key, None);
    assert!(matches!(shown.status, FootprintCardStatus::Unlinked));
}
