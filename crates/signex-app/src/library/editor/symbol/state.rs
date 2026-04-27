//! Symbol-tab editor state.
//!
//! The editor mutates a typed [`signex_library::Symbol`] primitive
//! in-place. Helpers below operate on a `&mut Symbol` so the
//! dispatcher can call them directly off the active editor state.
//!
//! Selection / hit-test / pin-add / move / delete logic preserves
//! the canvas + AI-stub apply behaviour the pre-refactor `SymbolDoc`
//! had.

use signex_library::{PinElectricalType, PinOrientation, Symbol, SymbolPin};

/// Coarse pin classification — kept independent of the canonical
/// [`PinElectricalType`] so the AI-stub heuristic can hand back a
/// limited subset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PinKind {
    Input,
    Output,
    Bidirectional,
    Power,
    Passive,
    Unknown,
}

impl PinKind {
    pub fn from_ai_stub(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "input" => PinKind::Input,
            "output" => PinKind::Output,
            "bidirectional" | "bidir" => PinKind::Bidirectional,
            "power" | "power_in" | "power_out" => PinKind::Power,
            "passive" => PinKind::Passive,
            _ => PinKind::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldKey {
    Reference,
    Value,
}


/// Selected element on the Symbol canvas — drives delete + drag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolSelection {
    Pin(usize),
    Field(FieldKey),
}

/// Default new-pin layout: place new pins to the right of the body.
const DEFAULT_PIN_LENGTH_MM: f64 = 2.54;

/// Add a pin at the given canvas coordinates and return its index in
/// `Symbol::pins`. Auto-assigns the next free numeric pin number.
pub fn add_pin(sym: &mut Symbol, x: f64, y: f64) -> usize {
    let next_num = next_pin_number(sym);
    sym.pins.push(SymbolPin {
        number: next_num.clone(),
        name: format!("PIN{next_num}"),
        electrical: PinElectricalType::Unspecified,
        position: [x, y],
        orientation: PinOrientation::Right,
        length: DEFAULT_PIN_LENGTH_MM,
    });
    sym.pins.len() - 1
}

/// Pick the next integer pin number — one above the highest numeric
/// pin number, or `"1"` if no numeric pins exist.
fn next_pin_number(sym: &Symbol) -> String {
    let highest = sym
        .pins
        .iter()
        .filter_map(|p| p.number.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    (highest + 1).to_string()
}

/// Move the currently-selected element to a new canvas position.
/// Coordinates are in mm; callers should snap to the grid first.
pub fn move_selected(sym: &mut Symbol, sel: Option<SymbolSelection>, x: f64, y: f64) {
    if let Some(SymbolSelection::Pin(idx)) = sel
        && let Some(pin) = sym.pins.get_mut(idx)
    {
        pin.position = [x, y];
    }
    // SymbolSelection::Field — no-op; the on-canvas designator /
    // value drag re-binds against `ComponentRow` once that pipeline
    // ships.
}

/// Delete whatever is currently selected. Returns `Some(new_sel)` if
/// the caller should update its selection (typically `None` after a
/// pin removal), or `None` if no selection change is needed.
pub fn delete_selected(
    sym: &mut Symbol,
    sel: Option<SymbolSelection>,
) -> Option<Option<SymbolSelection>> {
    match sel {
        Some(SymbolSelection::Pin(idx)) => {
            if idx < sym.pins.len() {
                sym.pins.remove(idx);
                Some(None)
            } else {
                None
            }
        }
        Some(SymbolSelection::Field(_)) => None,
        None => None,
    }
}

/// Hit-test cursor world coordinates against pins.
pub fn hit_test(sym: &Symbol, x: f64, y: f64) -> Option<SymbolSelection> {
    const PIN_HIT_R_SQ: f64 = 1.5 * 1.5;
    for (i, pin) in sym.pins.iter().enumerate() {
        let dx = pin.position[0] - x;
        let dy = pin.position[1] - y;
        if dx * dx + dy * dy <= PIN_HIT_R_SQ {
            return Some(SymbolSelection::Pin(i));
        }
    }
    None
}


#[cfg(test)]
mod tests {
    use super::*;
    use signex_library::Symbol;

    #[test]
    fn add_pin_assigns_next_number() {
        let mut s = Symbol::empty("test");
        // Symbol::empty seeds one default pin "1".
        let idx = add_pin(&mut s, 1.0, 1.0);
        assert_eq!(idx, 1);
        assert_eq!(s.pins[1].number, "2");
    }

    #[test]
    fn delete_pin_clears_selection_via_return() {
        let mut s = Symbol::empty("test");
        add_pin(&mut s, 1.0, 1.0);
        let new_sel = delete_selected(&mut s, Some(SymbolSelection::Pin(0)));
        assert_eq!(new_sel, Some(None));
        assert_eq!(s.pins.len(), 1);
    }

    #[test]
    fn move_selected_updates_position() {
        let mut s = Symbol::empty("test");
        move_selected(&mut s, Some(SymbolSelection::Pin(0)), 5.5, -2.0);
        assert_eq!(s.pins[0].position, [5.5, -2.0]);
    }

    #[test]
    fn hit_test_returns_pin() {
        let mut s = Symbol::empty("test");
        s.pins[0].position = [3.0, 4.0];
        let sel = hit_test(&s, 3.0, 4.0);
        assert_eq!(sel, Some(SymbolSelection::Pin(0)));
    }

}
