//! Keepout from a selection (#2409): a selected shape becomes a keepout on
//! its layer, one undo step. A selected copper area (Fill, Solid Region,
//! pour) turns into a keepout over its outline, keeping its uuid; selected
//! lines on no net that close one loop on one layer are replaced by a
//! keepout over that loop (lines carrying a net are routed copper and are
//! refused). A keepout drawn on a copper layer keeps that copper clear,
//! one on a keep-out layer its side's outer copper
//! (`zone_keepout::keepout_copper`).

use std::collections::BTreeSet;

use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{PcbBoard, Zone};
use uuid::Uuid;

use super::outline::closed_lines;
use super::{ToolEdit, ToolRefusal, ToolReport};
use crate::pcb::command::BoardCommand;
use crate::pcb::draw::new_keepout;
use crate::pcb::items::{BoardItem, BoardItemId};

/// The selected zones `zones` turned into keepouts, or, with no zone
/// selected, the loop the selected tracks `tracks` close as a new keepout
/// named `id`.
pub fn keepout_from_selection(
    board: &PcbBoard,
    zones: &BTreeSet<Uuid>,
    tracks: &BTreeSet<Uuid>,
    id: Uuid,
) -> Result<ToolEdit, ToolRefusal> {
    let picked: Vec<&Zone> = board
        .zones
        .iter()
        .filter(|z| zones.contains(&z.uuid))
        .collect();
    if !picked.is_empty() {
        return zones_to_keepouts(&picked);
    }
    let lines: Vec<_> = board
        .segments
        .iter()
        .filter(|s| tracks.contains(&s.uuid))
        .collect();
    let Some(first) = lines.first() else {
        return Err(ToolRefusal::NothingSelected);
    };
    // Routed copper is not turned into a keepout: its connection would go.
    let on_net = lines.iter().filter(|s| s.net != 0).count();
    if on_net > 0 {
        return Err(ToolRefusal::LinesOnNet(on_net));
    }
    if lines.iter().any(|s| s.layer != first.layer) {
        return Err(ToolRefusal::Path(
            "the lines are on more than one layer".to_string(),
        ));
    }
    let outline = closed_lines(board, tracks)?;
    let keepout = parse_pcb_layer(&first.layer)
        .and_then(|layer| new_keepout(id, outline, layer))
        .ok_or_else(|| ToolRefusal::NoKeepoutLayer(first.layer.clone()))?;
    let removed: Vec<Uuid> = lines.iter().map(|s| s.uuid).collect();
    Ok(ToolEdit {
        commands: vec![
            BoardCommand::DeleteItems {
                items: removed.iter().copied().map(BoardItemId::Segment).collect(),
            },
            BoardCommand::AddItems {
                items: vec![BoardItem::Zone(keepout)],
            },
        ],
        report: ToolReport {
            added: vec![id],
            removed,
            ..ToolReport::default()
        },
    })
}

/// Each zone as a keepout over its outline on its layer, in place.
fn zones_to_keepouts(picked: &[&Zone]) -> Result<ToolEdit, ToolRefusal> {
    let mut replaced = Vec::with_capacity(picked.len());
    for zone in picked {
        if zone.as_keepout().is_some() {
            return Err(ToolRefusal::AlreadyKeepout(area_name(zone)));
        }
        let keepout = parse_pcb_layer(&zone.layer)
            .and_then(|layer| new_keepout(zone.uuid, zone.outline.clone(), layer))
            .ok_or_else(|| ToolRefusal::NoKeepoutLayer(area_name(zone)))?;
        replaced.push(keepout);
    }
    Ok(ToolEdit {
        report: ToolReport {
            changed: replaced.iter().map(|z| z.uuid).collect(),
            ..ToolReport::default()
        },
        commands: vec![BoardCommand::ReplaceItems {
            items: replaced.into_iter().map(BoardItem::Zone).collect(),
        }],
    })
}

/// How a refusal names an area: its layer and net.
fn area_name(zone: &Zone) -> String {
    if zone.net_name.is_empty() {
        format!("the area on {}", zone.layer)
    } else {
        format!("the {} area on {}", zone.net_name, zone.layer)
    }
}
