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

/// One drawing a row offers: its default symbol or an alternate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymbolChoice {
    pub symbol_ref: PrimitiveRef,
    /// "Default" for the default symbol, else the alternate's label.
    pub label: String,
    pub is_default: bool,
}

impl ComponentRow {
    /// The default symbol first, then the alternates in their order.
    pub fn symbol_choices(&self) -> Vec<SymbolChoice> {
        let default = SymbolChoice {
            symbol_ref: self.symbol_ref,
            label: "Default".to_string(),
            is_default: true,
        };
        let alternates = self.alternate_symbols.iter().map(|a| SymbolChoice {
            symbol_ref: a.symbol_ref,
            label: a.label.clone(),
            is_default: false,
        });
        std::iter::once(default).chain(alternates).collect()
    }

    /// Whether `symbol` is the row's default or one of its alternates.
    pub fn binds_symbol(&self, symbol: &PrimitiveRef) -> bool {
        self.symbol_ref == *symbol
            || self
                .alternate_symbols
                .iter()
                .any(|a| a.symbol_ref == *symbol)
    }

    /// Make `symbol` the row's default symbol. A symbol that was an
    /// alternate stops being one; the previous default does not become an
    /// alternate.
    pub fn bind_default_symbol(&mut self, symbol: PrimitiveRef) {
        self.alternate_symbols.retain(|a| a.symbol_ref != symbol);
        self.symbol_ref = symbol;
    }

    /// Bind `new` wherever the row binds `old`: as its default (as
    /// [`Self::bind_default_symbol`] does) and in its alternates, each
    /// keeping its label. An alternate that would then repeat the default or
    /// another alternate is dropped; other alternates are left as they are.
    /// Returns whether the row changed.
    pub fn replace_symbol(&mut self, old: &PrimitiveRef, new: PrimitiveRef) -> bool {
        if *old == new || !self.binds_symbol(old) {
            return false;
        }
        if self.symbol_ref == *old {
            self.bind_default_symbol(new);
        }
        let mut new_bound =
            self.symbol_ref == new || self.alternate_symbols.iter().any(|a| a.symbol_ref == new);
        self.alternate_symbols.retain_mut(|a| {
            if a.symbol_ref != *old {
                return true;
            }
            if new_bound {
                return false;
            }
            a.symbol_ref = new;
            new_bound = true;
            true
        });
        true
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
    /// The label is empty or blank.
    EmptyLabel,
    /// Another alternate has the same label (trimmed, case-insensitive).
    DuplicateLabel { label: String },
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
            Self::EmptyLabel => write!(f, "an alternate symbol has an empty label"),
            Self::DuplicateLabel { label } => {
                write!(f, "alternate symbol label {label:?} is used twice")
            }
        }
    }
}

/// Defects in `row.alternate_symbols`, in list order. Empty = valid.
pub fn alternate_symbol_issues(row: &ComponentRow) -> Vec<AlternateSymbolIssue> {
    let mut seen: Vec<PrimitiveRef> = Vec::new();
    let mut seen_labels: Vec<String> = crate::alternate_symbol_edit::RESERVED_SYMBOL_LABELS
        .iter()
        .map(|l| l.to_lowercase())
        .collect();
    let mut out = Vec::new();
    for alt in &row.alternate_symbols {
        let label = alt.label.clone();
        let key = alt.label.trim().to_lowercase();
        if key.is_empty() {
            out.push(AlternateSymbolIssue::EmptyLabel);
        } else if seen_labels.contains(&key) {
            out.push(AlternateSymbolIssue::DuplicateLabel {
                label: label.clone(),
            });
        }
        seen_labels.push(key);
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

    #[test]
    fn binding_an_alternate_as_default_drops_it_from_the_alternates() {
        let lib = Uuid::now_v7();
        let (iec, ansi) = (
            PrimitiveRef::new(lib, Uuid::now_v7()),
            PrimitiveRef::new(lib, Uuid::now_v7()),
        );
        let mut r = crate::component::tests::fixture_row();
        let old_default = r.symbol_ref;
        r.alternate_symbols = vec![
            AlternateSymbol::new(iec, "", "IEC"),
            AlternateSymbol::new(ansi, "", "ANSI"),
        ];
        r.bind_default_symbol(iec);
        assert_eq!(r.symbol_ref, iec);
        assert_eq!(
            r.alternate_symbols,
            vec![AlternateSymbol::new(ansi, "", "ANSI")]
        );
        assert!(
            r.alternate_symbols
                .iter()
                .all(|a| a.symbol_ref != old_default)
        );
        assert!(alternate_symbol_issues(&r).is_empty());
    }

    #[test]
    fn replacing_a_symbol_moves_the_default_and_the_alternates_off_it() {
        let lib = Uuid::now_v7();
        let (iec, new) = (
            PrimitiveRef::new(lib, Uuid::now_v7()),
            PrimitiveRef::new(lib, Uuid::now_v7()),
        );
        let mut r = crate::component::tests::fixture_row();
        let old = r.symbol_ref;
        r.alternate_symbols = vec![AlternateSymbol::new(iec, "", "IEC")];
        let before = r.clone();
        assert!(!r.replace_symbol(&new, iec), "the row does not bind it");
        assert!(!r.replace_symbol(&old, old), "the same symbol");
        assert_eq!(r, before);

        assert!(r.replace_symbol(&old, new));
        assert_eq!(r.symbol_ref, new);
        assert_eq!(r.alternate_symbols, before.alternate_symbols);

        // An alternate keeps its label and its place.
        let mut r = before.clone();
        assert!(r.replace_symbol(&iec, new));
        assert_eq!((r.symbol_ref, r.alternate_symbols.len()), (old, 1));
        assert_eq!(r.alternate_symbols[0].symbol_ref, new);
        assert_eq!(r.alternate_symbols[0].label, "IEC");
        assert!(!r.binds_symbol(&iec));
    }

    #[test]
    fn a_replaced_alternate_that_would_repeat_a_binding_is_dropped() {
        let lib = Uuid::now_v7();
        let (iec, ansi) = (
            PrimitiveRef::new(lib, Uuid::now_v7()),
            PrimitiveRef::new(lib, Uuid::now_v7()),
        );
        let mut r = crate::component::tests::fixture_row();
        let default = r.symbol_ref;
        r.alternate_symbols = vec![
            AlternateSymbol::new(iec, "", "IEC"),
            AlternateSymbol::new(ansi, "", "ANSI"),
        ];
        assert!(r.replace_symbol(&iec, default));
        assert_eq!(
            r.alternate_symbols,
            vec![AlternateSymbol::new(ansi, "", "ANSI")]
        );
        assert!(alternate_symbol_issues(&r).is_empty());
    }

    fn row_with(alts: Vec<AlternateSymbol>) -> ComponentRow {
        let mut r = crate::component::tests::fixture_row();
        r.alternate_symbols = alts;
        r
    }

    #[test]
    fn issues_flag_unbound_default_and_duplicate_alternates() {
        let lib = Uuid::now_v7();
        let other = PrimitiveRef::new(lib, Uuid::now_v7());
        let mut r = row_with(Vec::new());
        assert!(alternate_symbol_issues(&r).is_empty());

        r.alternate_symbols = vec![
            AlternateSymbol::new(PrimitiveRef::new(lib, Uuid::nil()), "", "unbound"),
            AlternateSymbol::new(r.symbol_ref, "", "same"),
            AlternateSymbol::new(other, "", "first"),
            AlternateSymbol::new(other, "", "second"),
            AlternateSymbol::new(PrimitiveRef::new(lib, Uuid::now_v7()), "", "  "),
            AlternateSymbol::new(PrimitiveRef::new(lib, Uuid::now_v7()), "", " FIRST "),
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
                AlternateSymbolIssue::EmptyLabel,
                AlternateSymbolIssue::DuplicateLabel {
                    label: " FIRST ".into()
                },
            ]
        );
    }
}
