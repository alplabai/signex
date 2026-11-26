//! Area fields (#2393): the layer and net of a fill, solid region, polygon
//! pour, keepout or pour cutout, and a pour's clearance, minimum width, priority and
//! island rules (#2216), whether it is shelved (#2523) or unfilled
//! (#2506), its corners (#2578) and its fill mode and hatch (#2579).

use signex_types::format::{parse_pcb_layer, pcb_layer_token};
use signex_types::pcb::{PcbBoard, Zone};
use signex_types::pcb_layer::PcbLayer;
use signex_types::zone_keepout::{KEEPOUT_FILL, keepout_copper};
use signex_types::zone_pour_shape::{PourCorners, PourFillMode};
use uuid::Uuid;

use super::{PrimitiveEditRefusal, finite};
use crate::pcb::{BoardCommand, BoardItem};

/// One field of the selected areas, set to an absolute value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ZoneEdit {
    /// A copper area or a pour cutout takes a copper layer; a keepout a
    /// copper or keep-out layer.
    Layer(PcbLayer),
    /// 0 is no net. A keepout or a pour cutout (#2871) has no net and is
    /// left as it is.
    Net(u32),
    /// A pour's clearance in mm; 0 follows the board's. Other areas are
    /// left as they are.
    Clearance(f64),
    /// A pour's minimum copper width in mm. Other areas are left as they
    /// are.
    MinThickness(f64),
    /// A pour's priority (#2216): a higher one fills first. A whole number
    /// from 0. Other areas are left as they are.
    Priority(f64),
    /// The smallest island, in mm², a pour keeps; 0 keeps every island.
    /// Other areas are left as they are.
    MinIslandArea(f64),
    /// Whether a pour drops islands touching no copper of its net. Other
    /// areas are left as they are.
    RemoveDeadIslands(bool),
    /// Whether a pour is shelved (#2523): it keeps its outline and settings
    /// but holds no copper. Other areas are left as they are.
    Shelved(bool),
    /// Whether a pour is unfilled (#2506): it keeps its outline and settings
    /// but holds no copper until the next Repour. Other areas are left as
    /// they are.
    Unfilled(bool),
    /// How a pour's fill shapes the convex corners of its outline (#2578).
    /// Other areas are left as they are.
    Corners(PourCorners),
    /// A pour's corner radius or chamfer, in mm, from 0. Other areas are
    /// left as they are.
    CornerSize(f64),
    /// Solid or hatched copper (#2579). Other areas are left as they are.
    FillMode(PourFillMode),
    /// A hatched pour's line width and the gap between lines, in mm, from
    /// 0 (0 uses the default). Other areas are left as they are.
    HatchWidth(f64),
    HatchGap(f64),
}

/// Whether `layer` can hold `zone`: copper for a copper area, a layer that
/// keeps copper clear for a keepout.
pub fn zone_layer_allowed(zone: &Zone, layer: PcbLayer) -> bool {
    if zone.fill_type == KEEPOUT_FILL {
        keepout_copper(layer).is_some()
    } else {
        layer.is_copper()
    }
}

/// The command setting `edit` on every area in `zones`.
pub fn zone_edit_command(
    board: &PcbBoard,
    zones: &[Uuid],
    edit: ZoneEdit,
) -> Result<Option<BoardCommand>, PrimitiveEditRefusal> {
    zone_edits_command(board, zones, &[edit])
}

/// The command setting every edit of `edits`, in order, on every area in
/// `zones`: one replace, so one undo step. A refused edit refuses all.
pub fn zone_edits_command(
    board: &PcbBoard,
    zones: &[Uuid],
    edits: &[ZoneEdit],
) -> Result<Option<BoardCommand>, PrimitiveEditRefusal> {
    if zones.is_empty() {
        return Err(PrimitiveEditRefusal::Empty);
    }
    let mut changed = Vec::new();
    for id in zones {
        let old = board
            .zones
            .iter()
            .find(|z| z.uuid == *id)
            .ok_or(PrimitiveEditRefusal::Gone)?;
        let mut new = old.clone();
        for edit in edits {
            apply_zone_edit(board, &mut new, *edit)?;
        }
        if new != *old {
            changed.push(BoardItem::Zone(new));
        }
    }
    Ok((!changed.is_empty()).then_some(BoardCommand::ReplaceItems { items: changed }))
}

/// Set `edit` on `zone`, as [`zone_edit_command`] does on each area. The
/// placement tools use it for the area about to be placed.
pub fn apply_zone_edit(
    board: &PcbBoard,
    zone: &mut Zone,
    edit: ZoneEdit,
) -> Result<(), PrimitiveEditRefusal> {
    let has_no_net = zone.fill_type == KEEPOUT_FILL || zone.is_pour_cutout();
    if let ZoneEdit::Clearance(v)
    | ZoneEdit::MinThickness(v)
    | ZoneEdit::MinIslandArea(v)
    | ZoneEdit::Priority(v)
    | ZoneEdit::CornerSize(v)
    | ZoneEdit::HatchWidth(v)
    | ZoneEdit::HatchGap(v) = edit
        && finite(v)? < 0.0
    {
        return Err(PrimitiveEditRefusal::Negative);
    }
    if let ZoneEdit::Priority(v) = edit
        && (v.fract() != 0.0 || v > f64::from(u32::MAX))
    {
        return Err(PrimitiveEditRefusal::NotWhole);
    }
    match edit {
        ZoneEdit::Layer(layer) => {
            if !zone_layer_allowed(zone, layer) {
                return Err(PrimitiveEditRefusal::LayerNotAllowed);
            }
            if parse_pcb_layer(&zone.layer) != Some(layer) {
                zone.layer = pcb_layer_token(layer);
            }
        }
        ZoneEdit::Net(_) if has_no_net => {}
        ZoneEdit::Net(net) => {
            zone.net = net;
            zone.net_name = board
                .nets
                .iter()
                .find(|n| n.number == net && net != 0)
                .map(|n| n.name.clone())
                .unwrap_or_default();
        }
        ZoneEdit::Clearance(_)
        | ZoneEdit::MinThickness(_)
        | ZoneEdit::Priority(_)
        | ZoneEdit::MinIslandArea(_)
        | ZoneEdit::RemoveDeadIslands(_)
        | ZoneEdit::Shelved(_)
        | ZoneEdit::Unfilled(_)
        | ZoneEdit::Corners(_)
        | ZoneEdit::CornerSize(_)
        | ZoneEdit::FillMode(_)
        | ZoneEdit::HatchWidth(_)
        | ZoneEdit::HatchGap(_)
            if !zone.is_pour() => {}
        ZoneEdit::Clearance(v) => zone.clearance = v,
        ZoneEdit::MinThickness(v) => zone.min_thickness = v,
        // Checked above: a whole number from 0 within `u32`.
        ZoneEdit::Priority(v) => zone.priority = v as u32,
        ZoneEdit::MinIslandArea(v) => zone.min_island_area = v,
        ZoneEdit::RemoveDeadIslands(on) => zone.remove_dead_islands = on,
        ZoneEdit::Shelved(on) => zone.shelved = on,
        // A shelved pour holds no copper already; Unshelve brings it back
        // as it was.
        ZoneEdit::Unfilled(_) if zone.shelved => {}
        ZoneEdit::Unfilled(on) => zone.unfilled = on,
        ZoneEdit::Corners(corners) => zone.corners = corners,
        ZoneEdit::CornerSize(v) => zone.corner_size = v,
        ZoneEdit::FillMode(mode) => zone.fill_mode = mode,
        ZoneEdit::HatchWidth(v) => zone.hatch_width = v,
        ZoneEdit::HatchGap(v) => zone.hatch_gap = v,
    }
    Ok(())
}
