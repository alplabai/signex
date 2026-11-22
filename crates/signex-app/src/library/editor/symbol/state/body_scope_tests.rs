//! The Applies to row of the symbol Properties (mockup row 123).

use super::*;
use signex_library::{BODY_ALTERNATE, BODY_COMMON, BODY_NORMAL, SymbolPin};
use signex_types::schematic::BodyStyle;

fn symbol_with_pins(tags: &[u8]) -> Symbol {
    let mut sym = Symbol::empty("T");
    for (i, tag) in tags.iter().enumerate() {
        let mut pin = SymbolPin::new((i + 1).to_string(), "P");
        pin.body_style = *tag;
        sym.pins.push(pin);
    }
    sym
}

fn pins(indices: &[usize]) -> Option<SymbolSelection> {
    Some(SymbolSelection::Multiple {
        pin_indices: indices.to_vec(),
        graphic_indices: Vec::new(),
    })
}

#[test]
fn nothing_selected_reads_what_is_shown_and_assigns_nothing() {
    let sym = symbol_with_pins(&[BODY_COMMON, BODY_COMMON]);
    let row = body_scope_row(&sym, &None, PartView::normal(1));
    assert_eq!(row.scope, Some(BodyScope::Both));
    assert!(!row.can_assign);
}

#[test]
fn a_selection_reads_only_its_own_items() {
    let sym = symbol_with_pins(&[BODY_COMMON, BODY_NORMAL]);
    let view = PartView::normal(1);
    let row = body_scope_row(&sym, &pins(&[1]), view);
    assert_eq!(row.scope, Some(BodyScope::ThisOnly));
    assert!(row.can_assign);
}

#[test]
fn items_that_differ_light_neither_choice() {
    let sym = symbol_with_pins(&[BODY_COMMON, BODY_NORMAL]);
    let row = body_scope_row(&sym, &pins(&[0, 1]), PartView::normal(1));
    assert_eq!(row.scope, None);
    assert!(row.can_assign);
}

#[test]
fn this_only_writes_the_body_shown() {
    let alternate = PartView::new(1, BodyStyle::Alternate);
    assert_eq!(BodyScope::ThisOnly.tag(alternate), BODY_ALTERNATE);
    assert_eq!(BodyScope::Both.tag(alternate), BODY_COMMON);
}
