//! Rubber-band wires: keep connections when a move carries pins away.
//!
//! A plain move translates the selected items and nothing else, so a wire
//! ending on a moved symbol's pin is left behind and the connection silently
//! breaks (#963). With `keep_wires` the engine runs this pass inside the same
//! command, so the move and the wire edits are one history step and one
//! patch.
//!
//! # The rule: never change connectivity or net names
//!
//! Every moved pin whose old tip was connected to something is reconnected
//! at its new tip, and nothing else changes connectivity. Per old tip `K`
//! (matched in the netlist's own 1 um key space, so "same point" cannot
//! disagree with the netlist):
//!
//! * **Anchored** - something that does not move also holds `K`: another
//!   symbol's pin tip, a net label, a sheet pin, a junction that a stationary
//!   wire passes through, a wire that already moved away from the pair. The
//!   wires and labels stay exactly where they are and a new orthogonal path
//!   `K -> corner -> new tip` is added, so the old net keeps every member and
//!   every name and the pin joins it at its new position.
//! * **Free** - only the moved pin and unselected wire ends (plus movable
//!   junction dots) sit on `K`. Each attached wire end follows the pin:
//!   - a wire between two moved pins that move alike translates whole;
//!   - an orthogonal wire stays orthogonal: if its far joint is a plain
//!     elbow (exactly two wire ends, nothing else) the elbow slides along
//!     its other leg, so repeated moves never pile up segments; otherwise a
//!     corner is inserted. The preferred corner keeps the wire's direction
//!     at the fixed end (horizontal wire: `(new.x, fixed.y)`, vertical wire:
//!     `(fixed.x, new.y)`), the alternative is the other L;
//!   - a result of zero length is dropped when something else still holds
//!     the point, and otherwise left alone (the pin lands on that end).
//!     Dots on `K` move with the tip.
//!
//! # Collisions
//!
//! A new or stretched span may not touch any foreign wire (end or interior),
//! pin tip, label, junction or no-connect, except at the points it is meant
//! to join (the fixed end, the old tip, the new tip). The candidates are
//! tried in order - slide, preferred corner, other corner, then (free tips)
//! the anchored path with both corners. If every candidate collides the pin is
//! moved without rubber-banding, and a warning names it. A wire with a label
//! or another wire's end on its interior is never shortened or translated;
//! that makes the tip anchored instead.
//!
//! Which items drag wires: pins of moved symbols (power ports included,
//! unit-filtered), pins of moved child sheets and individually moved sheet
//! pins. Labels, junctions, no-connects and text never drag wires on their
//! own; wires that are themselves selected move as they always did.

mod env;
mod guard;

pub use guard::{Guard, Policy};

use std::collections::{BTreeMap, HashSet};

use env::Env;

#[cfg(test)]
pub(crate) use env::wires_overlap;

use signex_net::{pt_key, uf::Key};
use signex_types::schematic::{Point, SchematicSheet, SelectedItem, SelectedKind, SymbolTransform};
use uuid::Uuid;

/// A pin that is about to move, remembered by where its tip is now.
#[derive(Debug, Clone)]
pub struct PinTip {
    owner: Uuid,
    source: TipSource,
    old: Point,
    label: String,
    ident: PinIdent,
    power: bool,
}

/// How the netlist names a pin: a component terminal or a child-sheet pin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinIdent {
    Terminal(Uuid, String),
    SheetPin(Uuid),
}

impl PinTip {
    /// The netlist identity of this pin.
    #[must_use]
    pub fn ident(&self) -> &PinIdent {
        &self.ident
    }

    /// "R1 pin 2" - how the pin is named to the user.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// True for a power port's pin, which can be what names its net.
    #[must_use]
    pub fn is_power(&self) -> bool {
        self.power
    }

    /// Key of the tip's position before the move.
    #[must_use]
    pub fn old_key(&self) -> Key {
        pt_key(&self.old)
    }
}

#[derive(Debug, Clone, Copy)]
enum TipSource {
    /// A library pin at this local position of the owning symbol.
    SymbolPin(Point),
    /// A child sheet's pin (owner is the pin's uuid).
    SheetPin,
}

/// What the pass changed: wires and dots for the patch.
#[derive(Debug, Default)]
pub struct Follow {
    pub touched: Vec<SelectedItem>,
}

/// Pin tips carried by `items`, read before they move.
pub fn moved_pin_tips(document: &SchematicSheet, items: &[SelectedItem]) -> Vec<PinTip> {
    let mut tips = Vec::new();
    for item in items {
        match item.kind {
            SelectedKind::Symbol => {
                let Some(symbol) = document.symbols.iter().find(|s| s.uuid == item.uuid) else {
                    continue;
                };
                let Some(lib) = document.lib_symbols.get(&symbol.lib_id) else {
                    continue;
                };
                let xform = SymbolTransform::from_symbol(symbol);
                for lib_pin in &lib.pins {
                    if lib_pin.unit != 0 && lib_pin.unit != symbol.unit {
                        continue;
                    }
                    let name = if lib_pin.pin.number.is_empty() {
                        &lib_pin.pin.name
                    } else {
                        &lib_pin.pin.number
                    };
                    tips.push(PinTip {
                        owner: symbol.uuid,
                        source: TipSource::SymbolPin(lib_pin.pin.position),
                        old: xform.apply(lib_pin.pin.position),
                        label: format!("{} pin {name}", symbol.reference),
                        ident: PinIdent::Terminal(symbol.uuid, name.clone()),
                        power: symbol.is_power,
                    });
                }
            }
            SelectedKind::ChildSheet => {
                if let Some(child) = document.child_sheets.iter().find(|c| c.uuid == item.uuid) {
                    for pin in &child.pins {
                        tips.push(sheet_pin_tip(&child.name, pin));
                    }
                }
            }
            SelectedKind::SheetPin => {
                for child in &document.child_sheets {
                    if let Some(pin) = child.pins.iter().find(|p| p.uuid == item.uuid) {
                        tips.push(sheet_pin_tip(&child.name, pin));
                    }
                }
            }
            _ => {}
        }
    }
    tips
}

fn sheet_pin_tip(sheet_name: &str, pin: &signex_types::schematic::SheetPin) -> PinTip {
    PinTip {
        owner: pin.uuid,
        source: TipSource::SheetPin,
        old: pin.position,
        label: format!("{sheet_name} pin {}", pin.name),
        ident: PinIdent::SheetPin(pin.uuid),
        power: false,
    }
}

/// One pin that really moved.
pub(super) struct Move {
    pub(super) old: Point,
    pub(super) new: Point,
    /// A child-sheet pin: it also connects through a wire's interior.
    pub(super) sheet_pin: bool,
}

fn resolve_moves(document: &SchematicSheet, tips: &[PinTip]) -> (Vec<Move>, HashSet<Uuid>) {
    let mut moves = Vec::new();
    let mut owners = HashSet::new();
    for tip in tips {
        let new = match tip.source {
            TipSource::SymbolPin(local) => document
                .symbols
                .iter()
                .find(|s| s.uuid == tip.owner)
                .map(|s| SymbolTransform::from_symbol(s).apply(local)),
            TipSource::SheetPin => document
                .child_sheets
                .iter()
                .flat_map(|c| c.pins.iter())
                .find(|p| p.uuid == tip.owner)
                .map(|p| p.position),
        };
        let Some(new) = new else { continue };
        if pt_key(&new) == pt_key(&tip.old) {
            continue;
        }
        owners.insert(tip.owner);
        moves.push(Move {
            old: tip.old,
            new,
            sheet_pin: matches!(tip.source, TipSource::SheetPin),
        });
    }
    (moves, owners)
}

/// Drag the wires attached to `tips` along with a move that has already
/// been applied to `document`. See the module docs for the rules.
pub fn follow_moved_pins(
    document: &mut SchematicSheet,
    tips: &[PinTip],
    items: &[SelectedItem],
    policy: &Policy,
) -> Follow {
    let (moves, moving) = resolve_moves(document, tips);
    if moves.is_empty() {
        return Follow::default();
    }
    let selected: HashSet<Uuid> = items.iter().map(|item| item.uuid).collect();
    let old_tips = moves.iter().map(|mv| pt_key(&mv.old)).collect();
    let mut env = Env::new(document, selected, moving, old_tips, policy.clone());
    let mut out = Follow::default();

    let mut by_tip: BTreeMap<Key, Vec<&Move>> = BTreeMap::new();
    for mv in &moves {
        by_tip.entry(pt_key(&mv.old)).or_default().push(mv);
    }

    env.translate_whole_wires(&by_tip, &mut out);
    for (key, group) in &by_tip {
        env.reroute_tip(*key, group, &mut out);
    }
    env.merge_overlaps(&mut out);
    env.write_back(document);
    out
}
