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

/// The Parts section's Applies to choice: both bodies, or only the body
/// shown (mockup row 123).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyScope {
    Both,
    ThisOnly,
}

impl BodyScope {
    /// The body tag this choice writes while `view` is shown.
    pub fn tag(self, view: PartView) -> u8 {
        match self {
            Self::Both => signex_library::BODY_COMMON,
            Self::ThisOnly => view.body_tag(),
        }
    }
}

/// What the Applies to row shows: the choice its items share (`None` when
/// they differ or there are none), and whether a selection can take a new one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BodyScopeRow {
    pub scope: Option<BodyScope>,
    pub can_assign: bool,
}

/// The row for `selected`. With nothing selected it reads what `view` shows
/// and assigns nothing.
pub fn body_scope_row(
    sym: &Symbol,
    selected: &Option<SymbolSelection>,
    view: PartView,
) -> BodyScopeRow {
    let targets = body_assign_targets(sym, selected, view);
    let can_assign = !targets.0.is_empty() || !targets.1.is_empty();
    let read = if can_assign {
        targets
    } else {
        body_assign_targets(sym, &Some(SymbolSelection::All), view)
    };
    let pins = read.0.iter().filter_map(|&i| sym.pins.get(i));
    let graphics = read.1.iter().filter_map(|&i| sym.graphics.get(i));
    let tags: Vec<u8> = pins
        .map(|p| p.body_style)
        .chain(graphics.map(|g| g.body_style))
        .collect();
    let all = |tag: u8| !tags.is_empty() && tags.iter().all(|t| *t == tag);
    let scope = if all(signex_library::BODY_COMMON) {
        Some(BodyScope::Both)
    } else if all(view.body_tag()) {
        Some(BodyScope::ThisOnly)
    } else {
        None
    };
    BodyScopeRow { scope, can_assign }
}

#[cfg(test)]
#[path = "body_scope_tests.rs"]
mod tests;
