//! Which pins and graphics a body assignment names (De Morgan authoring).

use super::{PartView, SymbolSelection, graphic_on_part, pin_on_part};
use signex_library::Symbol;

/// The `(pin indices, graphic indices)` the current selection names, for
/// retagging their body. `All` resolves to everything shown in `view`;
/// a field names nothing.
pub fn body_assign_targets(
    sym: &Symbol,
    selected: &Option<SymbolSelection>,
    view: PartView,
) -> (Vec<usize>, Vec<usize>) {
    match selected {
        Some(SymbolSelection::Pin(i)) => (vec![*i], Vec::new()),
        Some(SymbolSelection::Graphic(i)) => (Vec::new(), vec![*i]),
        Some(SymbolSelection::Multiple {
            pin_indices,
            graphic_indices,
        }) => (pin_indices.clone(), graphic_indices.clone()),
        Some(SymbolSelection::All) => (
            (0..sym.pins.len())
                .filter(|&i| pin_on_part(&sym.pins[i], view))
                .collect(),
            (0..sym.graphics.len())
                .filter(|&i| graphic_on_part(&sym.graphics[i], view))
                .collect(),
        ),
        Some(SymbolSelection::Field(_)) | None => (Vec::new(), Vec::new()),
    }
}

/// Retag the named items with body `tag` (0 common, 1 normal, 2 alternate).
/// Stale indices are skipped. Returns whether anything changed.
pub fn assign_body(sym: &mut Symbol, targets: &(Vec<usize>, Vec<usize>), tag: u8) -> bool {
    let mut changed = false;
    for &i in &targets.0 {
        if let Some(pin) = sym.pins.get_mut(i)
            && pin.body_style != tag
        {
            pin.body_style = tag;
            changed = true;
        }
    }
    for &i in &targets.1 {
        if let Some(g) = sym.graphics.get_mut(i)
            && g.body_style != tag
        {
            g.body_style = tag;
            changed = true;
        }
    }
    changed
}
