//! A click on a pin of a placed part selects the part and brings that pin
//! forward in Properties (#1963); a click on the body keeps the part view.

use signex_types::schematic::{Point, SelectedKind, SymbolTransform};

use super::fixture::*;

fn info(p: &Project, key: &str) -> Option<String> {
    p.app
        .document_state
        .panel_ctx
        .selection_info
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.clone())
}

/// The first pin of the root sheet's `R1`: its key and its tip in world mm.
fn first_pin(p: &Project) -> (String, Point) {
    let doc = p
        .app
        .document_state
        .active_engine()
        .expect("engine")
        .document();
    let sym = doc
        .symbols
        .iter()
        .find(|s| s.uuid == id(ROOT_R1))
        .expect("R1");
    let lib = &doc.lib_symbols[&sym.lib_id];
    let pin = &lib.pins[0].pin;
    let key = if pin.number.is_empty() {
        pin.name.clone()
    } else {
        pin.number.clone()
    };
    (key, SymbolTransform::from_symbol(sym).apply(pin.position))
}

#[test]
fn a_point_at_a_pin_end_picks_that_pin_and_the_body_picks_none() {
    let p = project();
    let (key, tip) = first_pin(&p);
    let doc = p
        .app
        .document_state
        .active_engine()
        .expect("engine")
        .document();
    let at = |dx: f64, dy: f64| Point::new(tip.x + dx, tip.y + dy);
    assert_eq!(
        signex_engine::symbol_pin_at(doc, id(ROOT_R1), at(0.2, 0.1)),
        Some(key)
    );
    let far = signex_engine::PIN_PICK_TOLERANCE_MM * 10.0;
    assert_eq!(
        signex_engine::symbol_pin_at(doc, id(ROOT_R1), at(far, far)),
        None
    );
}

#[test]
fn a_pin_click_leaves_the_part_selected_and_properties_focused_on_that_pin() {
    let mut p = project();
    let (key, _) = first_pin(&p);
    p.select(ROOT_R1, SelectedKind::Symbol);
    p.app.interaction_state.clicked_pin = Some((id(ROOT_R1), key.clone()));
    p.app.update_selection_info();
    assert_eq!(
        p.app.document_state.panel_ctx.selected_uuid,
        Some(id(ROOT_R1))
    );
    assert_eq!(info(&p, "Focused Pin"), Some(key.clone()));
    let row = info(&p, &format!("Pin: {key}")).expect("its row");
    assert_eq!(row.split('\t').count(), 3, "name, type, hidden: {row}");
    assert!(info(&p, &format!("Pin Net: {key}")).is_some(), "its net");
    // The focus is read once: the next refresh is the part-level view.
    p.app.update_selection_info();
    assert!(info(&p, "Focused Pin").is_none());
    assert!(
        info(&p, &format!("Pin: {key}")).is_some(),
        "pins still list"
    );
}

#[test]
fn a_click_whose_pin_belongs_to_another_part_focuses_nothing() {
    let mut p = project();
    let (key, _) = first_pin(&p);
    p.select(ROOT_R1, SelectedKind::Symbol);
    p.app.interaction_state.clicked_pin = Some((id(MID_SYMBOL), key));
    p.app.update_selection_info();
    assert!(info(&p, "Focused Pin").is_none());
}
