//! A power port's net-name text is fixed to its port: it is drawn where the
//! port's own geometry puts it and is not moved on its own (maintainer
//! decision 2026-11-07, see `docs/DECISIONS.md`). A stored value-field
//! position from an older file stays on disk and is ignored when drawing.

use std::borrow::Cow;
use std::collections::HashSet;

use super::power_port_symbol::{VALUE_GAP_MM, lib_head_reach_mm};
use super::{
    HAlign, LibSymbol, Point, PowerPortStyle, SCHEMATIC_TEXT_MM, SchematicSheet, SelectedItem,
    SelectedKind, Symbol, SymbolTransform, TextProp, VAlign,
};

/// The value text of the power port `sym`, whose library entry is `lib`:
/// just past the head along the stem, turned and mirrored with the port,
/// upright and justified away from the head. The stored size, face and
/// visibility are kept; the stored position, rotation and justification are
/// not read.
///
/// Like every field, the result is stored relative to the symbol and read
/// through `field_effective_style`, so its rotation and justification undo
/// the port's own turn and mirrors.
#[must_use]
pub fn power_port_value_text(lib: &LibSymbol, sym: &Symbol) -> TextProp {
    let style = PowerPortStyle::of(&lib.id);
    let reach = lib_head_reach_mm(lib, style) + VALUE_GAP_MM;
    // Library space is Y-up; the grounds hang to negative y.
    let along = if style.hangs_down() { -reach } else { reach };
    let anchor = SymbolTransform::from_symbol(sym).apply(Point::new(0.0, along));
    let (dx, dy) = (anchor.x - sym.position.x, anchor.y - sym.position.y);
    let (justify_h, justify_v) = if dy.abs() >= dx.abs() {
        // Schematic space is Y-down: a head above the port has dy < 0.
        let v = if dy <= 0.0 {
            VAlign::Bottom
        } else {
            VAlign::Top
        };
        (HAlign::Center, v)
    } else if dx < 0.0 {
        (HAlign::Right, VAlign::Center)
    } else {
        (HAlign::Left, VAlign::Center)
    };
    let stored = sym.val_text.as_ref();
    TextProp {
        position: anchor,
        rotation: (-sym.rotation).rem_euclid(360.0),
        font_size: stored.map_or(SCHEMATIC_TEXT_MM, |t| t.font_size),
        justify_h: if sym.mirror_y {
            flip_h(justify_h)
        } else {
            justify_h
        },
        justify_v: if sym.mirror_x {
            flip_v(justify_v)
        } else {
            justify_v
        },
        hidden: stored.is_some_and(|t| t.hidden),
        font_family: stored.and_then(|t| t.font_family.clone()),
    }
}

fn flip_h(h: HAlign) -> HAlign {
    match h {
        HAlign::Left => HAlign::Right,
        HAlign::Center => HAlign::Center,
        HAlign::Right => HAlign::Left,
    }
}

fn flip_v(v: VAlign) -> VAlign {
    match v {
        VAlign::Top => VAlign::Bottom,
        VAlign::Center => VAlign::Center,
        VAlign::Bottom => VAlign::Top,
    }
}

impl SchematicSheet {
    /// The value text `sym` is drawn with: the stored field, or for a power
    /// port the fixed [`power_port_value_text`]. Every reader that draws or
    /// boxes the value (canvas, hit box, SVG / PDF, the inline editor) reads
    /// it here, so none can draw a power port's text at a stored offset.
    #[must_use]
    pub fn drawn_value_text<'a>(&self, sym: &'a Symbol) -> Option<Cow<'a, TextProp>> {
        if sym.is_power
            && let Some(lib) = self.resolve_lib_symbol(sym)
        {
            return Some(Cow::Owned(power_port_value_text(&lib, sym)));
        }
        sym.val_text.as_ref().map(Cow::Borrowed)
    }

    /// The item selecting `item` takes: a power port's designator or value
    /// field stands for the port, since its text is part of the port and is
    /// not moved on its own. Any other item is itself.
    #[must_use]
    pub fn selection_unit(&self, item: SelectedItem) -> SelectedItem {
        if is_field(item.kind)
            && self
                .symbols
                .iter()
                .any(|s| s.uuid == item.uuid && s.is_power)
        {
            return SelectedItem::new(item.uuid, SelectedKind::Symbol);
        }
        item
    }

    /// [`Self::selection_unit`] of each of `items`, in order, each unit once.
    #[must_use]
    pub fn selection_units(&self, items: &[SelectedItem]) -> Vec<SelectedItem> {
        let ports: HashSet<uuid::Uuid> = if items.iter().any(|i| is_field(i.kind)) {
            self.symbols
                .iter()
                .filter(|s| s.is_power)
                .map(|s| s.uuid)
                .collect()
        } else {
            HashSet::new()
        };
        let mut seen = HashSet::with_capacity(items.len());
        items
            .iter()
            .map(|item| {
                if is_field(item.kind) && ports.contains(&item.uuid) {
                    SelectedItem::new(item.uuid, SelectedKind::Symbol)
                } else {
                    *item
                }
            })
            .filter(|item| seen.insert(*item))
            .collect()
    }
}

fn is_field(kind: SelectedKind) -> bool {
    matches!(
        kind,
        SelectedKind::SymbolRefField | SelectedKind::SymbolValField
    )
}

#[cfg(test)]
#[path = "power_port_text_tests.rs"]
mod tests;
