//! The via types of a board (#96): span, kind, size and protection, as the
//! Via panel picks them. Derived from the board's own vias until the Layer
//! Stack Manager stores via types (#105). Applying a type or resetting a
//! via's size override is built from the `primitive_edit` via edits and
//! returned as one command, so the app runs it as one undo step.

use std::collections::BTreeMap;

use pcb_geom::ItemRef;
use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{PcbBoard, Point, Via, ViaType};
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use signex_types::pcb_stack::board_copper_layers;
use signex_types::pcb_via_protection::ViaProtection;
use uuid::Uuid;

use super::primitive_edit::{PrimitiveEditRefusal, ViaEdit, via_edit_command};
use super::{BoardCommand, BoardItem};

/// The copper a drilled via hole's wall is plated with, in mm: the finished
/// hole is the drill less twice this.
pub const VIA_PLATING_MM: f64 = 0.025;

/// Two via sizes closer than this are the same size, in mm (well under the
/// 1 nm the file stores).
const SAME_SIZE_MM: f64 = 1e-7;

/// What makes two vias the same type: kind, span ends and tenting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ViaTypeKey {
    pub kind: ViaType,
    pub top: CopperLayer,
    pub bottom: CopperLayer,
    pub tented_top: bool,
    pub tented_bottom: bool,
}

impl ViaTypeKey {
    /// The key of `via`: a through via runs Top to Bottom, any other the
    /// two ends it lists. `None` when it lists no copper layer.
    pub fn of_via(via: &Via) -> Option<Self> {
        let (top, bottom) = if via.via_type == ViaType::Through {
            (CopperLayer::Top, CopperLayer::Bottom)
        } else {
            let ends: Vec<CopperLayer> = via
                .layers
                .iter()
                .filter_map(|name| parse_pcb_layer(name).and_then(PcbLayer::copper))
                .collect();
            (*ends.iter().min()?, *ends.iter().max()?)
        };
        Some(Self {
            kind: via.via_type,
            top,
            bottom,
            tented_top: via.attributes.top.tented,
            tented_bottom: via.attributes.bottom.tented,
        })
    }
}

/// One via type of a board.
#[derive(Debug, Clone, PartialEq)]
pub struct ViaTypeDef {
    pub kind: ViaType,
    pub top: CopperLayer,
    pub bottom: CopperLayer,
    /// The 1-based places of `top` and `bottom` in the board's copper, top
    /// to bottom: what a span label ("Thru 1:4") counts.
    pub span: (usize, usize),
    pub diameter: f64,
    pub drill: f64,
    pub protection: ViaProtection,
}

impl ViaTypeDef {
    pub fn key(&self) -> ViaTypeKey {
        ViaTypeKey {
            kind: self.kind,
            top: self.top,
            bottom: self.bottom,
            tented_top: self.protection.tented_top,
            tented_bottom: self.protection.tented_bottom,
        }
    }

    /// The finished hole after plating: the drill less twice
    /// [`VIA_PLATING_MM`], never below zero.
    pub fn finished_hole(drill: f64) -> f64 {
        (drill - 2.0 * VIA_PLATING_MM).max(0.0)
    }
}

/// The board's via types: one per kind, span and tenting its vias have,
/// sized by the diameter and drill most of them share (on a tie the smaller
/// diameter, then the smaller drill). A board with no through via gets the
/// through type the Via tool places (`draw::new_via`). Through types first,
/// then by top layer.
pub fn via_types(board: &PcbBoard) -> Vec<ViaTypeDef> {
    let copper = board_copper_layers(&board.layers);
    let mut groups: Vec<(ViaTypeKey, Vec<(f64, f64)>)> = Vec::new();
    for via in &board.vias {
        let Some(key) = ViaTypeKey::of_via(via) else {
            continue;
        };
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, sizes)) => sizes.push((via.diameter, via.drill)),
            None => groups.push((key, vec![(via.diameter, via.drill)])),
        }
    }
    if !groups.iter().any(|(k, _)| k.kind == ViaType::Through) {
        let tool = super::draw::new_via(board, Uuid::nil(), Point::new(0.0, 0.0));
        if let Some(key) = ViaTypeKey::of_via(&tool) {
            groups.push((key, vec![(tool.diameter, tool.drill)]));
        }
    }
    let mut types: Vec<ViaTypeDef> = groups
        .into_iter()
        .filter_map(|(key, sizes)| {
            let place = |layer| copper.iter().position(|l| *l == layer).map(|i| i + 1);
            let (diameter, drill) = most_common(&sizes)?;
            Some(ViaTypeDef {
                kind: key.kind,
                top: key.top,
                bottom: key.bottom,
                span: (place(key.top)?, place(key.bottom)?),
                diameter,
                drill,
                protection: ViaProtection {
                    tented_top: key.tented_top,
                    tented_bottom: key.tented_bottom,
                    ..ViaProtection::default()
                },
            })
        })
        .collect();
    types.sort_by_key(|t| {
        (
            t.kind != ViaType::Through,
            t.span,
            kind_rank(t.kind),
            t.protection.tented_top,
            t.protection.tented_bottom,
        )
    });
    types
}

fn kind_rank(kind: ViaType) -> u8 {
    match kind {
        ViaType::Through => 0,
        ViaType::Blind => 1,
        ViaType::Micro => 2,
    }
}

/// The (diameter, drill) pair most of `sizes` share, counted per whole
/// nanometre as the file stores them; on a tie the smaller diameter, then
/// the smaller drill.
fn most_common(sizes: &[(f64, f64)]) -> Option<(f64, f64)> {
    let nm = |mm: f64| (mm * 1e6).round() as i64;
    let mut counts: BTreeMap<(i64, i64), (usize, (f64, f64))> = BTreeMap::new();
    for &pair in sizes {
        counts
            .entry((nm(pair.0), nm(pair.1)))
            .or_insert((0, pair))
            .0 += 1;
    }
    // The map runs smallest first: the first of the highest count wins.
    let mut best: Option<(usize, (f64, f64))> = None;
    for &(count, pair) in counts.values() {
        if best.is_none_or(|(b, _)| count > b) {
            best = Some((count, pair));
        }
    }
    best.map(|(_, pair)| pair)
}

/// The smallest via drill the board's setup allows; `None` when it sets
/// none.
pub fn min_via_drill(board: &PcbBoard) -> Option<f64> {
    board
        .setup
        .as_ref()
        .map(|s| s.via_min_drill)
        .filter(|d| *d > 0.0)
}

/// The type of `types` that `via` is, by kind, span and tenting.
pub fn type_of(types: &[ViaTypeDef], via: &Via) -> Option<usize> {
    let key = ViaTypeKey::of_via(via)?;
    types.iter().position(|t| t.key() == key)
}

/// Whether `via` has a size of its own: its diameter or drill differs from
/// the type's.
pub fn is_overridden(t: &ViaTypeDef, via: &Via) -> bool {
    (via.diameter - t.diameter).abs() > SAME_SIZE_MM || (via.drill - t.drill).abs() > SAME_SIZE_MM
}

/// The command making every via of `items` a via of type `t`: its span,
/// kind, diameter, drill and tenting. Refused as the via edits refuse it (a
/// locked via, a via that is gone); empty when nothing changes.
pub fn apply_type_commands(
    board: &PcbBoard,
    items: &[ItemRef],
    t: &ViaTypeDef,
) -> Result<Vec<BoardCommand>, PrimitiveEditRefusal> {
    let edits = [
        ViaEdit::Span(t.top, t.bottom),
        ViaEdit::Diameter(t.diameter),
        ViaEdit::HoleSize(t.drill),
        ViaEdit::TentedTop(t.protection.tented_top),
        ViaEdit::TentedBottom(t.protection.tented_bottom),
    ];
    folded_command(board, items, &edits, Some(t.kind))
}

/// The command putting the diameter and drill of every via of `items` back
/// to type `t`'s.
pub fn reset_override_commands(
    board: &PcbBoard,
    items: &[ItemRef],
    t: &ViaTypeDef,
) -> Result<Vec<BoardCommand>, PrimitiveEditRefusal> {
    let edits = [ViaEdit::Diameter(t.diameter), ViaEdit::HoleSize(t.drill)];
    folded_command(board, items, &edits, None)
}

/// `edits` run in turn on the vias of `items`, each through
/// `via_edit_command` on the vias as the edits before it left them, then
/// gathered into one replace. `kind` sets the via kind last: a span edit
/// keeps a micro via micro, a type may not.
fn folded_command(
    board: &PcbBoard,
    items: &[ItemRef],
    edits: &[ViaEdit],
    kind: Option<ViaType>,
) -> Result<Vec<BoardCommand>, PrimitiveEditRefusal> {
    let ids: Vec<Uuid> = items
        .iter()
        .filter_map(|item| match item {
            ItemRef::Via(id) => Some(*id),
            _ => None,
        })
        .collect();
    if ids.is_empty() {
        return Err(PrimitiveEditRefusal::Empty);
    }
    let mut scratch = board.clone();
    for edit in edits {
        let Some(BoardCommand::ReplaceItems { items }) =
            via_edit_command(&scratch, &ids, edit.clone())?
        else {
            continue;
        };
        for item in items {
            if let BoardItem::Via(new) = item
                && let Some(slot) = scratch.vias.iter_mut().find(|v| v.uuid == new.uuid)
            {
                *slot = new;
            }
        }
    }
    let mut changed = Vec::new();
    for id in &ids {
        let (Some(old), Some(new)) = (
            board.vias.iter().find(|v| v.uuid == *id),
            scratch.vias.iter().find(|v| v.uuid == *id),
        ) else {
            return Err(PrimitiveEditRefusal::Gone);
        };
        let mut new = new.clone();
        if let Some(kind) = kind {
            new.via_type = kind;
        }
        if new != *old {
            changed.push(BoardItem::Via(new));
        }
    }
    Ok(if changed.is_empty() {
        Vec::new()
    } else {
        vec![BoardCommand::ReplaceItems { items: changed }]
    })
}

#[cfg(test)]
#[path = "via_types_tests.rs"]
mod tests;
