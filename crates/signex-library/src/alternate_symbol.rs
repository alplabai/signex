//! Alternative symbol drawings for a component row.
//!
//! A row's `symbol_ref` is its default drawing; each [`AlternateSymbol`]
//! is another drawing of the same part (IEC vs ANSI, a different pin
//! arrangement). The footprint pin map and `pin_map_overrides` are shared
//! by every drawing, so an alternate must expose the same pin numbers.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::component::ComponentRow;
use crate::primitive::{PrimitiveRef, Symbol};

/// One alternative drawing of a row's component.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AlternateSymbol {
    pub symbol_ref: PrimitiveRef,
    /// Pinned revision of `symbol_ref`, like `ComponentRow::symbol_version`.
    #[serde(default)]
    pub symbol_version: String,
    /// User-facing name, e.g. "IEC".
    pub label: String,
}

impl AlternateSymbol {
    pub fn new(
        symbol_ref: PrimitiveRef,
        symbol_version: impl Into<String>,
        label: impl Into<String>,
    ) -> Self {
        Self {
            symbol_ref,
            symbol_version: symbol_version.into(),
            label: label.into(),
        }
    }
}

/// A data-only defect in a row's alternate-symbol list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AlternateSymbolIssue {
    /// The alternate has the nil-UUID "unbound" sentinel.
    Unbound { label: String },
    /// The alternate is the row's default symbol.
    SameAsDefault { label: String },
    /// Two alternates bind the same symbol.
    Duplicate { label: String },
}

impl std::fmt::Display for AlternateSymbolIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unbound { label } => write!(f, "alternate symbol {label:?} is unbound"),
            Self::SameAsDefault { label } => {
                write!(f, "alternate symbol {label:?} is the default symbol")
            }
            Self::Duplicate { label } => {
                write!(f, "alternate symbol {label:?} duplicates another alternate")
            }
        }
    }
}

/// Defects in `row.alternate_symbols`, in list order. Empty = valid.
pub fn alternate_symbol_issues(row: &ComponentRow) -> Vec<AlternateSymbolIssue> {
    let mut seen: Vec<PrimitiveRef> = Vec::new();
    let mut out = Vec::new();
    for alt in &row.alternate_symbols {
        let label = alt.label.clone();
        if alt.symbol_ref.uuid.is_nil() {
            out.push(AlternateSymbolIssue::Unbound { label });
        } else if alt.symbol_ref == row.symbol_ref {
            out.push(AlternateSymbolIssue::SameAsDefault { label });
        } else if seen.contains(&alt.symbol_ref) {
            out.push(AlternateSymbolIssue::Duplicate { label });
        }
        seen.push(alt.symbol_ref);
    }
    out
}

/// Pin numbers on which an alternate symbol differs from the default.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PinCompatibility {
    /// On the default symbol but not on the alternate.
    pub missing_in_alternate: Vec<String>,
    /// On the alternate but not on the default.
    pub extra_in_alternate: Vec<String>,
}

impl PinCompatibility {
    pub fn is_compatible(&self) -> bool {
        self.missing_in_alternate.is_empty() && self.extra_in_alternate.is_empty()
    }
}

/// Compare the pin-number sets of `default` and `alternate`.
pub fn check_pin_compatibility(default: &Symbol, alternate: &Symbol) -> PinCompatibility {
    let numbers =
        |s: &Symbol| -> BTreeSet<String> { s.pins.iter().map(|p| p.number.clone()).collect() };
    let (d, a) = (numbers(default), numbers(alternate));
    PinCompatibility {
        missing_in_alternate: d.difference(&a).cloned().collect(),
        extra_in_alternate: a.difference(&d).cloned().collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitive::SymbolPin;
    use uuid::Uuid;

    fn symbol(pins: &[&str]) -> Symbol {
        let mut s = Symbol::empty("S");
        s.pins = pins.iter().map(|n| SymbolPin::new(*n, *n)).collect();
        s
    }

    #[test]
    fn same_pin_numbers_in_any_order_are_compatible() {
        let c = check_pin_compatibility(&symbol(&["1", "2", "3"]), &symbol(&["3", "1", "2"]));
        assert!(c.is_compatible());
    }

    #[test]
    fn differing_pin_numbers_are_reported_per_side() {
        let c = check_pin_compatibility(&symbol(&["1", "2", "3"]), &symbol(&["1", "4"]));
        assert!(!c.is_compatible());
        assert_eq!(c.missing_in_alternate, vec!["2", "3"]);
        assert_eq!(c.extra_in_alternate, vec!["4"]);
    }

    fn row_with(alts: Vec<AlternateSymbol>) -> ComponentRow {
        let mut r = crate::component::tests::fixture_row();
        r.alternate_symbols = alts;
        r
    }

    #[test]
    fn issues_flag_unbound_default_and_duplicate_alternates() {
        let lib = Uuid::new_v4();
        let other = PrimitiveRef::new(lib, Uuid::new_v4());
        let mut r = row_with(Vec::new());
        assert!(alternate_symbol_issues(&r).is_empty());

        r.alternate_symbols = vec![
            AlternateSymbol::new(PrimitiveRef::new(lib, Uuid::nil()), "", "unbound"),
            AlternateSymbol::new(r.symbol_ref, "", "same"),
            AlternateSymbol::new(other, "", "first"),
            AlternateSymbol::new(other, "", "second"),
        ];
        assert_eq!(
            alternate_symbol_issues(&r),
            vec![
                AlternateSymbolIssue::Unbound {
                    label: "unbound".into()
                },
                AlternateSymbolIssue::SameAsDefault {
                    label: "same".into()
                },
                AlternateSymbolIssue::Duplicate {
                    label: "second".into()
                },
            ]
        );
    }
}
