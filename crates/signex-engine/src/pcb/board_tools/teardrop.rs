//! Teardrops (#2408): add them where tracks of the chosen nets enter a pad
//! or via of their net, or remove them. A teardrop is a zone with the
//! teardrop fill (`signex_types::zone_teardrop`) on the track's net and
//! layer. Adding first removes the chosen nets' old teardrops, so running
//! it twice gives the same board. One undo step each.

use std::collections::BTreeSet;

use pcb_geom::teardrops;
use signex_types::pcb::{PcbBoard, Zone};
use signex_types::zone_teardrop::TEARDROP_FILL;
use uuid::Uuid;

use super::{ToolEdit, ToolRefusal, ToolReport};
use crate::pcb::command::BoardCommand;
use crate::pcb::items::{BoardItem, BoardItemId};

/// The teardrops of `nets` (`None`: every net) on `board`, built fresh, as
/// one edit that also removes the old ones of those nets. `ids` names each
/// new teardrop.
pub fn add_teardrops<F: Fn(u32, u32) -> i64>(
    board: &PcbBoard,
    nets: Option<&BTreeSet<u32>>,
    clearance: F,
    mut ids: impl FnMut() -> Uuid,
) -> Result<ToolEdit, ToolRefusal> {
    // The old teardrops are not obstacles of their own net; plan on the
    // board without them so a rerun lands on the same shapes.
    let old = old_teardrops(board, nets);
    let mut bare = board.clone();
    bare.zones.retain(|z| !old.contains(&z.uuid));
    let plan = teardrops(&bare, nets, clearance);
    let zones: Vec<Zone> = plan
        .drops
        .into_iter()
        .map(|drop| teardrop_zone(board, ids(), drop))
        .collect();
    let mut commands = Vec::new();
    if !old.is_empty() {
        commands.push(BoardCommand::DeleteItems {
            items: old.iter().copied().map(BoardItemId::Zone).collect(),
        });
    }
    let report = ToolReport {
        added: zones.iter().map(|z| z.uuid).collect(),
        removed: old,
        skipped: plan.skipped,
        ..ToolReport::default()
    };
    if zones.is_empty() && report.skipped == 0 {
        return Err(ToolRefusal::NoTeardropPlace);
    }
    if !zones.is_empty() {
        commands.push(BoardCommand::AddItems {
            items: zones.into_iter().map(BoardItem::Zone).collect(),
        });
    }
    Ok(ToolEdit { commands, report })
}

/// Remove the teardrops of `nets` (`None`: every net), one undo step.
pub fn remove_teardrops(
    board: &PcbBoard,
    nets: Option<&BTreeSet<u32>>,
) -> Result<ToolEdit, ToolRefusal> {
    let old = old_teardrops(board, nets);
    if old.is_empty() {
        return Err(ToolRefusal::NoTeardrops);
    }
    Ok(ToolEdit {
        commands: vec![BoardCommand::DeleteItems {
            items: old.iter().copied().map(BoardItemId::Zone).collect(),
        }],
        report: ToolReport {
            removed: old,
            ..ToolReport::default()
        },
    })
}

fn old_teardrops(board: &PcbBoard, nets: Option<&BTreeSet<u32>>) -> Vec<Uuid> {
    board
        .zones
        .iter()
        .filter(|z| z.is_teardrop() && nets.is_none_or(|nets| nets.contains(&z.net)))
        .map(|z| z.uuid)
        .collect()
}

fn teardrop_zone(board: &PcbBoard, id: Uuid, drop: pcb_geom::Teardrop) -> Zone {
    let net_name = board
        .nets
        .iter()
        .find(|n| n.number == drop.net)
        .map(|n| n.name.clone())
        .unwrap_or_default();
    Zone {
        uuid: id,
        net: drop.net,
        net_name,
        layer: drop.layer,
        outline: drop.outline.into_iter().map(|p| p.to_mm()).collect(),
        holes: Vec::new(),
        priority: 0,
        fill_type: TEARDROP_FILL.into(),
        thermal_relief: false,
        thermal_gap: 0.0,
        thermal_width: 0.0,
        clearance: 0.0,
        min_thickness: 0.0,
        min_island_area: 0.0,
        remove_dead_islands: false,
        shelved: false,
        unfilled: false,
        unknown: Default::default(),
    }
}
