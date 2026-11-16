//! The board's layer stack (#2401): replace it as one edit, which also
//! writes the copper layers of the layer list (the layer tabs read it) and
//! renames the copper of every item when a copper layer moved place.
//!
//! Copper is named by place, top to bottom (`copper_at`): a new inner layer
//! between Inner 1 and Inner 2 is Inner 2, and what was on Inner 2 moves to
//! Inner 3 with it. The rename is a bijection on the copper that stays, so
//! the inverse ([`StackSnapshot`]) restores the board bit for bit.

use std::collections::{HashMap, HashSet};

use signex_types::format::{parse_pcb_layer, pcb_layer_token};
use signex_types::pcb::{LayerDef, PcbBoard};
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use signex_types::pcb_stack::{
    LayerSizes, LayerStack, MAX_COPPER_LAYERS, board_copper_layers, copper_at,
};

use super::command::{Applied, BoardCommand, BoardPatch, BoardPatchKind};
use super::error::BoardCommandError;

/// What a stack edit replaced, to put back as it was.
#[derive(Debug, Clone, PartialEq)]
pub struct StackSnapshot {
    /// `None`: the board had no stored stack.
    pub stackup: Option<LayerStack>,
    pub layers: Vec<LayerDef>,
    pub thickness: f64,
    /// Copper layer tokens to rename, `(now, then)`.
    pub renames: Vec<(String, String)>,
}

/// Replace the stack with `stack`. Refused, before any write, for a row
/// whose numbers are not finite or negative, fewer than two or more than
/// [`MAX_COPPER_LAYERS`] copper rows, a copper row naming a layer the board
/// does not have or one named twice, the outer copper moved off the top or
/// bottom, and a copper layer left out that items or a keepout are still on.
pub(super) fn set(
    board: &mut PcbBoard,
    mut stack: LayerStack,
) -> Result<Applied, BoardCommandError> {
    let Plan { renames, origins } = plan(board, &mut stack)?;
    let snapshot = StackSnapshot {
        stackup: board.stackup.clone(),
        layers: board.layers.clone(),
        thickness: board.thickness,
        renames: renames
            .iter()
            .map(|(a, b)| (b.clone(), a.clone()))
            .collect(),
    };
    board.layers = copper_defs(&board.layers, &stack, &origins)?;
    rename_layers(board, &renames);
    let total = stack.total_thickness_mm();
    if total.is_finite() && total > 0.0 {
        board.thickness = total;
    }
    board.stackup = Some(stack);
    Ok(applied(snapshot))
}

/// Put back what [`set`] replaced. The values were on the board before, so
/// nothing is checked.
pub(super) fn restore(
    board: &mut PcbBoard,
    snapshot: StackSnapshot,
) -> Result<Applied, BoardCommandError> {
    let renames: Vec<(String, String)> = snapshot.renames;
    let inverse = StackSnapshot {
        stackup: board.stackup.take(),
        layers: std::mem::replace(&mut board.layers, snapshot.layers),
        thickness: std::mem::replace(&mut board.thickness, snapshot.thickness),
        renames: renames
            .iter()
            .map(|(a, b)| (b.clone(), a.clone()))
            .collect(),
    };
    rename_layers(board, &renames);
    board.stackup = snapshot.stackup;
    Ok(applied(inverse))
}

fn applied(snapshot: StackSnapshot) -> Applied {
    Applied {
        inverse: BoardCommand::RestoreLayerStack(Box::new(snapshot)),
        patch: BoardPatch {
            kind: BoardPatchKind::LayerStackChanged,
            items: Vec::new(),
        },
    }
}

/// What a checked stack edit does to the copper.
struct Plan {
    /// `(old, new)` tokens of the copper that moved place.
    renames: Vec<(String, String)>,
    /// Per copper row, top to bottom: the board layer it was; `None` for a
    /// new layer.
    origins: Vec<Option<String>>,
}

/// Check `stack` and give its copper rows their tokens by place.
fn plan(board: &PcbBoard, stack: &mut LayerStack) -> Result<Plan, BoardCommandError> {
    for (row, layer) in stack.layers.iter().enumerate() {
        if !layer.numbers_valid() {
            return Err(BoardCommandError::StackRowInvalid { row });
        }
    }
    let count = stack.copper_count();
    if !(2..=MAX_COPPER_LAYERS).contains(&count) {
        return Err(BoardCommandError::CopperLayerCount(count));
    }
    let current = board_copper_layers(&board.layers);
    let mut kept: HashSet<CopperLayer> = HashSet::new();
    let mut renames = Vec::new();
    let mut origins = Vec::with_capacity(count);
    let mut place = 0;
    for row in &mut stack.layers {
        if !row.kind.is_copper() {
            row.layer.clear();
            continue;
        }
        let new = copper_at(place, count).ok_or(BoardCommandError::CopperLayerCount(count))?;
        place += 1;
        if !row.layer.is_empty() {
            let old = parse_pcb_layer(&row.layer)
                .and_then(PcbLayer::copper)
                .filter(|layer| current.contains(layer))
                .ok_or_else(|| BoardCommandError::NotCopperLayer(row.layer.clone()))?;
            if !kept.insert(old) {
                return Err(BoardCommandError::DuplicateStackLayer(row.layer.clone()));
            }
            let outer_moved = matches!(old, CopperLayer::Top | CopperLayer::Bottom) && old != new;
            if outer_moved {
                return Err(BoardCommandError::OuterCopperMoved(row.layer.clone()));
            }
            if old != new {
                renames.push((row.layer.clone(), token(new)));
            }
            origins.push(Some(row.layer.clone()));
        } else {
            origins.push(None);
        }
        row.layer = token(new);
    }
    for gone in current.iter().filter(|layer| !kept.contains(layer)) {
        check_empty(board, &token(*gone))?;
    }
    let moved: Vec<&String> = renames.iter().map(|(old, _)| old).collect();
    for old in moved {
        if keepout_names(board, old) {
            return Err(BoardCommandError::KeepoutOnLayer(old.clone()));
        }
    }
    Ok(Plan { renames, origins })
}

fn token(layer: CopperLayer) -> String {
    pcb_layer_token(PcbLayer::Copper(layer))
}

/// Refuse to drop `layer` while anything is on it.
fn check_empty(board: &PcbBoard, layer: &str) -> Result<(), BoardCommandError> {
    let on = |name: &String| name == layer;
    let mut items = board.segments.iter().filter(|s| on(&s.layer)).count();
    items += board
        .vias
        .iter()
        .filter(|v| v.layers.iter().any(on))
        .count();
    items += board.zones.iter().filter(|z| on(&z.layer)).count();
    items += board.texts.iter().filter(|t| on(&t.layer)).count();
    items += board.graphics.iter().filter(|g| on(&g.layer)).count();
    for fp in &board.footprints {
        items += usize::from(on(&fp.layer));
        items += fp.pads.iter().filter(|p| p.layers.iter().any(on)).count();
        items += fp.graphics.iter().filter(|g| on(&g.layer)).count();
    }
    if keepout_names(board, layer) {
        return Err(BoardCommandError::KeepoutOnLayer(layer.to_string()));
    }
    if items > 0 {
        return Err(BoardCommandError::LayerInUse {
            layer: layer.to_string(),
            items,
        });
    }
    Ok(())
}

/// Whether a board-sketch keepout covers the copper layer `layer`.
fn keepout_names(board: &PcbBoard, layer: &str) -> bool {
    let Some(copper) = parse_pcb_layer(layer) else {
        return false;
    };
    board
        .sketch
        .as_ref()
        .is_some_and(|s| s.keepouts.iter().any(|k| k.layers.contains(&copper)))
}

/// The layer list with its copper entries following `stack`: the other
/// entries keep their place, the copper goes where the first copper entry
/// was (first when there was none), top to bottom. A layer that stays keeps
/// its entry (id, display name) under its new token; a new one takes the
/// first free id.
fn copper_defs(
    layers: &[LayerDef],
    stack: &LayerStack,
    origins: &[Option<String>],
) -> Result<Vec<LayerDef>, BoardCommandError> {
    let is_copper = |def: &LayerDef| parse_pcb_layer(&def.name).is_some_and(|l| l.is_copper());
    let at = layers.iter().position(is_copper).unwrap_or(0);
    let mut out: Vec<LayerDef> = layers.iter().filter(|&d| !is_copper(d)).cloned().collect();
    let previous: Vec<Option<&LayerDef>> = origins
        .iter()
        .map(|was| {
            let was = was.as_deref()?;
            layers.iter().find(|d| d.name == was)
        })
        .collect();
    let mut used: HashSet<u8> = out.iter().map(|d| d.id).collect();
    used.extend(previous.iter().flatten().map(|d| d.id));
    let mut copper = Vec::with_capacity(stack.copper_count());
    for (row, previous) in stack.copper().zip(previous) {
        let mut def = match previous {
            Some(def) => def.clone(),
            None => {
                let id = (0..=u8::MAX)
                    .find(|id| !used.contains(id))
                    .ok_or(BoardCommandError::CopperLayerCount(stack.copper_count()))?;
                used.insert(id);
                LayerDef {
                    id,
                    name: String::new(),
                    layer_type: String::new(),
                    display_name: String::new(),
                }
            }
        };
        def.name = row.layer.clone();
        let plane = ["plane", "power"]
            .iter()
            .any(|k| def.layer_type.eq_ignore_ascii_case(k));
        if plane != (row.kind.layer_type() == "plane") || def.layer_type.is_empty() {
            def.layer_type = row.kind.layer_type().to_string();
        }
        copper.push(def);
    }
    let at = at.min(out.len());
    out.splice(at..at, copper);
    Ok(out)
}

/// Rename copper layer tokens `(from, to)` on every item at once.
fn rename_layers(board: &mut PcbBoard, renames: &[(String, String)]) {
    if renames.is_empty() {
        return;
    }
    let map: HashMap<&str, &str> = renames
        .iter()
        .map(|(from, to)| (from.as_str(), to.as_str()))
        .collect();
    let swap = |name: &mut String| {
        if let Some(to) = map.get(name.as_str()) {
            *name = (*to).to_string();
        }
    };
    for s in &mut board.segments {
        swap(&mut s.layer);
    }
    for v in &mut board.vias {
        v.layers.iter_mut().for_each(swap);
        if let Some(LayerSizes::Full { layers }) = v.layer_sizes.as_mut() {
            layers.iter_mut().for_each(|l| swap(&mut l.layer));
        }
    }
    for z in &mut board.zones {
        swap(&mut z.layer);
    }
    for t in &mut board.texts {
        swap(&mut t.layer);
    }
    for g in &mut board.graphics {
        swap(&mut g.layer);
    }
    for fp in &mut board.footprints {
        swap(&mut fp.layer);
        for g in &mut fp.graphics {
            swap(&mut g.layer);
        }
        for p in &mut fp.pads {
            p.layers.iter_mut().for_each(swap);
            if let Some(LayerSizes::Full { layers }) = p.layer_sizes.as_mut() {
                layers.iter_mut().for_each(|l| swap(&mut l.layer));
            }
        }
    }
}

#[cfg(test)]
#[path = "stack_tests.rs"]
mod tests;
