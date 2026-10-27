//! Editing a row's alternate symbols: add, remove, relabel and reorder.
//!
//! Pure domain operations on a [`ComponentRow`]; the editor calls them and
//! shows the error of a refused one. Adding checks the alternate against the
//! default symbol with [`check_pin_compatibility`], because the footprint pin
//! map is shared by every drawing.

use crate::alternate_symbol::{AlternateSymbol, PinCompatibility, check_pin_compatibility};
use crate::component::ComponentRow;
use crate::primitive::{PrimitiveRef, Symbol};

/// Label of the row's default drawing; no alternate may use it.
pub const DEFAULT_SYMBOL_LABEL: &str = "Default";

/// Label the Properties list gives a drawing its row no longer offers; no
/// alternate may use it either.
pub const NO_LONGER_OFFERED_LABEL: &str = "(no longer offered)";

/// Labels no alternate may take (compared trimmed, case-insensitive).
pub const RESERVED_SYMBOL_LABELS: [&str; 2] = [DEFAULT_SYMBOL_LABEL, NO_LONGER_OFFERED_LABEL];

/// Why an alternate symbol was not added.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AddAlternateError {
    /// The picked symbol is the unbound sentinel.
    Unbound,
    /// The picked symbol is the row's default.
    SameAsDefault,
    /// The row already lists the picked symbol as an alternate.
    Duplicate,
    /// The pin numbers differ from the default symbol's.
    PinMismatch(PinCompatibility),
}

impl std::fmt::Display for AddAlternateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unbound => write!(f, "no symbol was picked"),
            Self::SameAsDefault => write!(f, "that symbol is already the default symbol"),
            Self::Duplicate => write!(f, "that symbol is already an alternate"),
            Self::PinMismatch(c) => write!(
                f,
                "its pin numbers differ from the default symbol's ({c}); the footprint \
                 pin map is shared by every drawing"
            ),
        }
    }
}

impl std::fmt::Display for PinCompatibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let join = |pins: &[String]| pins.join(", ");
        let mut parts = Vec::new();
        if !self.missing_in_alternate.is_empty() {
            parts.push(format!("missing {}", join(&self.missing_in_alternate)));
        }
        if !self.extra_in_alternate.is_empty() {
            parts.push(format!("extra {}", join(&self.extra_in_alternate)));
        }
        write!(f, "{}", parts.join("; "))
    }
}

/// A label for a new alternate: `base`, made unique among the row's
/// labels (trimmed, case-insensitive) and distinct from "Default".
fn unique_label(row: &ComponentRow, base: &str) -> String {
    let base = match base.trim() {
        "" => "Alternate",
        b => b,
    };
    let taken = |candidate: &str| {
        let key = candidate.to_lowercase();
        RESERVED_SYMBOL_LABELS
            .iter()
            .any(|r| r.to_lowercase() == key)
            || row
                .alternate_symbols
                .iter()
                .any(|a| a.label.trim().to_lowercase() == key)
    };
    let mut candidate = base.to_string();
    let mut n = 2;
    while taken(&candidate) {
        candidate = format!("{base} {n}");
        n += 1;
    }
    candidate
}

/// Add `alternate` (resolved as `alternate_symbol`) to the row, labelled
/// after the symbol. Refused, leaving the row untouched, when it is unbound,
/// the default, already listed, or its pin numbers differ from `default`.
pub fn add_alternate_symbol(
    row: &mut ComponentRow,
    default: &Symbol,
    alternate: PrimitiveRef,
    alternate_symbol: &Symbol,
) -> Result<(), AddAlternateError> {
    if alternate.uuid.is_nil() {
        return Err(AddAlternateError::Unbound);
    }
    if alternate == row.symbol_ref {
        return Err(AddAlternateError::SameAsDefault);
    }
    if row
        .alternate_symbols
        .iter()
        .any(|a| a.symbol_ref == alternate)
    {
        return Err(AddAlternateError::Duplicate);
    }
    let compat = check_pin_compatibility(default, alternate_symbol);
    if !compat.is_compatible() {
        return Err(AddAlternateError::PinMismatch(compat));
    }
    let label = unique_label(row, &alternate_symbol.name);
    row.alternate_symbols.push(AlternateSymbol::new(
        alternate,
        alternate_symbol.version.clone(),
        label,
    ));
    Ok(())
}

/// Remove the alternate at `idx`; `false` when there is none.
pub fn remove_alternate_symbol(row: &mut ComponentRow, idx: usize) -> bool {
    if idx >= row.alternate_symbols.len() {
        return false;
    }
    row.alternate_symbols.remove(idx);
    true
}

/// Set the label of the alternate at `idx`; `false` when there is none.
/// The label is stored as typed: blank or repeated labels show up in
/// [`crate::alternate_symbol_issues`] and are refused on save.
pub fn relabel_alternate_symbol(row: &mut ComponentRow, idx: usize, label: String) -> bool {
    match row.alternate_symbols.get_mut(idx) {
        Some(alt) => {
            alt.label = label;
            true
        }
        None => false,
    }
}

/// Move the alternate at `from` to position `to`; `false` when either index
/// is out of range or they are equal. Alternate order is meaningful.
pub fn move_alternate_symbol(row: &mut ComponentRow, from: usize, to: usize) -> bool {
    let len = row.alternate_symbols.len();
    if from >= len || to >= len || from == to {
        return false;
    }
    let moved = row.alternate_symbols.remove(from);
    row.alternate_symbols.insert(to, moved);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitive::SymbolPin;
    use uuid::Uuid;

    fn symbol(name: &str, pins: &[&str]) -> Symbol {
        let mut s = Symbol::empty(name);
        s.pins = pins.iter().map(|n| SymbolPin::new(*n, *n)).collect();
        s
    }

    fn row() -> ComponentRow {
        crate::component::tests::fixture_row()
    }

    fn reference() -> PrimitiveRef {
        PrimitiveRef::new(Uuid::new_v4(), Uuid::new_v4())
    }

    #[test]
    fn a_compatible_alternate_is_added_with_the_symbol_name_as_label() {
        let mut r = row();
        let alt = symbol("IEC", &["1", "2"]);
        let at = reference();
        add_alternate_symbol(&mut r, &symbol("D", &["2", "1"]), at, &alt).unwrap();
        assert_eq!(r.alternate_symbols.len(), 1);
        assert_eq!(r.alternate_symbols[0].symbol_ref, at);
        assert_eq!(r.alternate_symbols[0].label, "IEC");
        assert_eq!(r.alternate_symbols[0].symbol_version, alt.version);
    }

    #[test]
    fn a_pin_mismatch_is_refused_and_names_the_pins() {
        let mut r = row();
        let err = add_alternate_symbol(
            &mut r,
            &symbol("D", &["1", "2", "3"]),
            reference(),
            &symbol("A", &["1", "4"]),
        )
        .unwrap_err();
        assert!(r.alternate_symbols.is_empty(), "the row is untouched");
        let AddAlternateError::PinMismatch(c) = &err else {
            panic!("{err:?}");
        };
        assert_eq!(c.missing_in_alternate, vec!["2", "3"]);
        assert_eq!(c.extra_in_alternate, vec!["4"]);
        assert!(err.to_string().contains("missing 2, 3; extra 4"), "{err}");
    }

    #[test]
    fn default_unbound_and_repeated_symbols_are_refused() {
        let mut r = row();
        let s = symbol("S", &["1"]);
        let at = reference();
        let default_ref = r.symbol_ref;
        assert_eq!(
            add_alternate_symbol(&mut r, &s, default_ref, &s),
            Err(AddAlternateError::SameAsDefault)
        );
        assert_eq!(
            add_alternate_symbol(
                &mut r,
                &s,
                PrimitiveRef::new(at.library_id, Uuid::nil()),
                &s
            ),
            Err(AddAlternateError::Unbound)
        );
        add_alternate_symbol(&mut r, &s, at, &s).unwrap();
        assert_eq!(
            add_alternate_symbol(&mut r, &s, at, &s),
            Err(AddAlternateError::Duplicate)
        );
    }

    #[test]
    fn labels_stay_unique_and_never_take_the_default_label() {
        let mut r = row();
        let named = |n: &str| symbol(n, &["1"]);
        for name in ["Default", "IEC", "iec", " ", "(No longer offered)"] {
            add_alternate_symbol(&mut r, &named("D"), reference(), &named(name)).unwrap();
        }
        let labels: Vec<&str> = r
            .alternate_symbols
            .iter()
            .map(|a| a.label.as_str())
            .collect();
        assert_eq!(
            labels,
            vec![
                "Default 2",
                "IEC",
                "iec 2",
                "Alternate",
                "(No longer offered) 2"
            ]
        );
        assert!(crate::alternate_symbol_issues(&r).is_empty());
    }

    #[test]
    fn remove_relabel_and_move_edit_in_place() {
        let mut r = row();
        let s = symbol("S", &["1"]);
        for _ in 0..3 {
            add_alternate_symbol(&mut r, &s, reference(), &symbol("S", &["1"])).unwrap();
        }
        assert!(relabel_alternate_symbol(&mut r, 1, "ANSI".into()));
        let ansi = r.alternate_symbols[1].symbol_ref;
        assert!(move_alternate_symbol(&mut r, 1, 0));
        assert_eq!(r.alternate_symbols[0].symbol_ref, ansi);
        assert!(remove_alternate_symbol(&mut r, 0));
        assert_eq!(r.alternate_symbols.len(), 2);
        assert!(!remove_alternate_symbol(&mut r, 5));
        assert!(!move_alternate_symbol(&mut r, 0, 9));
        assert!(!relabel_alternate_symbol(&mut r, 9, "x".into()));
    }
}
