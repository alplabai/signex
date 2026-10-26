//! Which of a library symbol's bodies a placed symbol shows.
//!
//! A library item (pin or graphic) is tagged with a unit and a body-style
//! number: body 0 is common to every body, 1 is the normal body and 2 is the
//! alternate (De Morgan) body. [`item_shown`] is the one predicate every
//! consumer (canvas, hit-test, netlist, ERC, export, engine) asks, so they can
//! never disagree about which pins a placed symbol has.

use super::{LibSymbol, Symbol};

/// Body-style tag shared by items drawn on every body.
pub const BODY_COMMON: u32 = 0;
/// Body-style tag of the normal body.
pub const BODY_NORMAL: u32 = 1;
/// Body-style tag of the alternate (De Morgan) body.
pub const BODY_ALTERNATE: u32 = 2;

/// The body a placed symbol asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BodyStyle {
    #[default]
    Normal,
    Alternate,
}

impl BodyStyle {
    /// The library body-style tag of this body (1 normal, 2 alternate).
    pub fn tag(self) -> u32 {
        match self {
            Self::Normal => BODY_NORMAL,
            Self::Alternate => BODY_ALTERNATE,
        }
    }

    pub fn is_normal(&self) -> bool {
        matches!(self, Self::Normal)
    }
}

/// Is a library item tagged `(item_unit, item_body)` shown on a placed symbol
/// of `unit` showing `body`? Unit 0 and body 0 are shared by all.
pub fn item_shown(item_unit: u32, item_body: u32, unit: u32, body: BodyStyle) -> bool {
    (item_unit == 0 || item_unit == unit) && (item_body == BODY_COMMON || item_body == body.tag())
}

impl LibSymbol {
    /// Does `unit` (or the all-units set) carry any pin or graphic of the
    /// alternate body?
    pub fn has_alternate_body(&self, unit: u32) -> bool {
        let on_unit = |u: u32| u == 0 || u == unit;
        self.pins
            .iter()
            .any(|p| p.body_style == BODY_ALTERNATE && on_unit(p.unit))
            || self
                .graphics
                .iter()
                .any(|g| g.body_style == BODY_ALTERNATE && on_unit(g.unit))
    }

    /// The body actually drawn and wired for a request: the alternate only when
    /// it was asked for AND this unit has one, else the normal body. The one
    /// fallback every consumer shares.
    pub fn effective_body(&self, unit: u32, requested: BodyStyle) -> BodyStyle {
        if requested == BodyStyle::Alternate && self.has_alternate_body(unit) {
            BodyStyle::Alternate
        } else {
            BodyStyle::Normal
        }
    }
}

/// Is a library item tagged `(item_unit, item_body)` shown on `sym`, given the
/// library symbol it instantiates?
pub fn shown_on(lib: &LibSymbol, item_unit: u32, item_body: u32, sym: &Symbol) -> bool {
    let body = lib.effective_body(sym.unit, sym_body(sym));
    item_shown(item_unit, item_body, sym.unit, body)
}

fn sym_body(_sym: &Symbol) -> BodyStyle {
    BodyStyle::Normal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_by_body_table() {
        use BodyStyle::{Alternate, Normal};
        // (item_unit, item_body, unit, body, shown)
        let table = [
            (0, 0, 1, Normal, true),
            (0, 0, 2, Alternate, true),
            (0, 1, 1, Normal, true),
            (0, 1, 1, Alternate, false),
            (0, 2, 1, Normal, false),
            (0, 2, 1, Alternate, true),
            (1, 1, 1, Normal, true),
            (1, 1, 2, Normal, false),
            (1, 2, 1, Alternate, true),
            (1, 2, 2, Alternate, false),
            (2, 0, 2, Normal, true),
            (2, 0, 1, Alternate, false),
        ];
        for (iu, ib, u, b, want) in table {
            assert_eq!(item_shown(iu, ib, u, b), want, "{iu} {ib} {u} {b:?}");
        }
    }
}
