//! Fixes that resize one item (#2940): a track to the width its rule asks,
//! and a via or pad grown to the copper ring its rule asks around the hole.

use pcb_geom::{ItemRef, mm_to_nm, nm_to_mm};
use signex_drc::Violation;
use signex_types::pcb::{Pad, PadShape, PcbBoard, Point};

use super::LeftReason;
use super::plan::{Plan, locked_name};
use crate::pcb::{BoardCommand, BoardItem, FootprintPad};

/// Track width: set the track or copper arc to the limit it breaks, the
/// rule's minimum or maximum.
pub(super) fn track_width(board: &PcbBoard, v: &Violation) -> Result<Vec<Plan>, LeftReason> {
    let &[item] = v.items.as_slice() else {
        return Err(LeftReason::CannotChange(
            "the finding does not name one track",
        ));
    };
    if let Some(name) = locked_name(board, item) {
        return Err(LeftReason::Locked(vec![name]));
    }
    let width = nm_to_mm(v.required_nm);
    let resized = match item {
        ItemRef::Track(id) => board.segments.iter().find(|s| s.uuid == id).map(|s| {
            let mut s = s.clone();
            s.width = width;
            BoardItem::Segment(s)
        }),
        ItemRef::Arc(id) => board.free.arcs.iter().find(|a| a.uuid == id).map(|a| {
            let mut a = a.clone();
            a.width = width;
            BoardItem::Arc(a)
        }),
        _ => None,
    };
    let resized = resized.ok_or(LeftReason::CannotChange(
        "the finding's track is not on the board",
    ))?;
    Ok(vec![Plan::replacing(vec![resized])])
}

/// Annular ring: grow the via or pad so its copper reaches the rule's ring
/// past the hole on every side, keeping its hole.
pub(super) fn annular_ring(board: &PcbBoard, v: &Violation) -> Result<Vec<Plan>, LeftReason> {
    let &[item] = v.items.as_slice() else {
        return Err(LeftReason::CannotChange(
            "the finding does not name one hole",
        ));
    };
    if let Some(name) = locked_name(board, item) {
        return Err(LeftReason::Locked(vec![name]));
    }
    let ring = v.required_nm;
    let gone = LeftReason::CannotChange("the finding's via or pad is not on the board");
    match item {
        ItemRef::Via(id) => {
            let via = board.vias.iter().find(|via| via.uuid == id).ok_or(gone)?;
            if via.layer_sizes.is_some() {
                return Err(LeftReason::CannotChange(
                    "the via has sizes by layer; set them in its Properties",
                ));
            }
            let mut grown = via.clone();
            grown.diameter = via.diameter.max(around_hole(via.drill, ring)?);
            Ok(vec![Plan::replacing(vec![BoardItem::Via(grown)])])
        }
        ItemRef::Pad { footprint, pad } => {
            // `locked_name` has kept out a locked part.
            let pad = board
                .footprints
                .iter()
                .find(|f| f.uuid == footprint)
                .and_then(|fp| fp.pads.iter().find(|p| p.uuid == pad))
                .ok_or(gone)?;
            let footprint_pad = FootprintPad {
                footprint,
                pad: grown_pad(pad, ring)?,
            };
            let commands = vec![BoardCommand::ReplacePads {
                pads: vec![footprint_pad],
            }];
            Ok(vec![Plan::of_commands(
                format!("{footprint}/{}", pad.uuid),
                commands,
            )])
        }
        ItemRef::FreePad(id) => {
            let pad = board.free.pads.iter().find(|p| p.uuid == id).ok_or(gone)?;
            let grown = BoardItem::FreePad(grown_pad(pad, ring)?);
            Ok(vec![Plan::replacing(vec![grown])])
        }
        _ => Err(LeftReason::CannotChange(
            "the finding does not name a via or pad",
        )),
    }
}

/// `pad` with each side at least its hole plus `ring` on both sides. A pad
/// drawn from its own corners, or sized by layer, is left to a person.
fn grown_pad(pad: &Pad, ring: i64) -> Result<Pad, LeftReason> {
    if matches!(pad.shape, PadShape::Trapezoid | PadShape::Custom) {
        return Err(LeftReason::CannotChange(
            "the pad is drawn from its own corners; resize it in the footprint editor",
        ));
    }
    if pad.layer_sizes.is_some() {
        return Err(LeftReason::CannotChange(
            "the pad has sizes by layer; set them in its Properties",
        ));
    }
    let drill = pad.drill.as_ref().map_or(0.0, |d| d.diameter);
    let side = around_hole(drill, ring)?;
    let mut grown = pad.clone();
    grown.size = Point::new(pad.size.x.max(side), pad.size.y.max(side));
    Ok(grown)
}

/// The copper size, mm, that leaves `ring` nm around a `drill` mm hole.
fn around_hole(drill: f64, ring: i64) -> Result<f64, LeftReason> {
    let drill = mm_to_nm(drill, "hole drill")
        .map_err(|_| LeftReason::CannotChange("the hole's size is not a number"))?;
    Ok(nm_to_mm(drill.abs() + 2 * ring.max(0)))
}
