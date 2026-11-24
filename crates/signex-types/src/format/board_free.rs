//! The optional `[free]` section of a `.snxpcb` (#2373): the board's free
//! arcs, pads and dimensions (`pcb_free::FreePrimitives`).
//!
//! Written only when the board has any, so a board without them re-saves
//! byte-identical and a file from before the section loads unchanged.
//! Lengths are integer nm like every other on-disk geometry; a pad's turn
//! and corner ratio are plain numbers, as in the `pads` rows.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::FormatError;
use super::board_dimension::{self, DimensionRaw};
use super::finite_toml;
use super::pad_shape_cells::{
    check_shape, delta_cell, outline_cell, read_delta, read_outline, refuse_unreadable_shape,
};
use super::pcb_cells::{LAYER_SIZES_COLUMN, layer_sizes_cell, read_layer_sizes};
use super::pcb_rows::row_field_error;
use super::units::{NonFiniteField, mm_to_nm, nm_to_mm};
use crate::pcb::{DrillDef, Pad, PadNet, PadShape, PadType, PcbBoard, Point};
use crate::pcb_arc::TrackArc;
use crate::pcb_free::FreePrimitives;

const ARCS: &str = "free.arcs";
const PADS: &str = "free.pads";
const DIMENSIONS: &str = "free.dimensions";

/// `[free]` as stored.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(in crate::format) struct FreeRaw {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    arcs: Vec<ArcRaw>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pads: Vec<PadRaw>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    dimensions: Vec<DimensionRaw>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ArcRaw {
    uuid: Uuid,
    start_x: i64,
    start_y: i64,
    mid_x: i64,
    mid_y: i64,
    end_x: i64,
    end_y: i64,
    width: i64,
    layer: String,
    #[serde(default)]
    net: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PadRaw {
    uuid: Uuid,
    #[serde(default)]
    number: String,
    pad_type: PadType,
    shape: PadShape,
    x: i64,
    y: i64,
    size_x: i64,
    size_y: i64,
    /// The hole's diameter; absent for a pad with none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    drill: Option<i64>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    drill_shape: String,
    #[serde(default)]
    layers: Vec<String>,
    #[serde(default)]
    net: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    net_name: String,
    #[serde(default)]
    roundrect_ratio: f64,
    #[serde(default)]
    rotation: f64,
    #[serde(default)]
    net_locked: bool,
    /// Per-layer copper sizes (#2401), in the `pads` rows' `layer_sizes`
    /// cell form; absent for one size on every layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    layer_sizes: Option<String>,
    /// A trapezoid's delta and a custom pad's outline (#2242), in the
    /// `pads` rows' cell forms; absent when the pad has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    trapezoid_delta: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    custom_outline: Option<String>,
}

fn point(x: i64, y: i64) -> Point {
    Point::new(nm_to_mm(x), nm_to_mm(y))
}

/// The free primitives of the file; none when it has no section.
pub(in crate::format) fn read(raw: Option<FreeRaw>) -> Result<FreePrimitives, FormatError> {
    let Some(raw) = raw else {
        return Ok(FreePrimitives::default());
    };
    let arcs = raw
        .arcs
        .into_iter()
        .map(|a| TrackArc {
            uuid: a.uuid,
            start: point(a.start_x, a.start_y),
            mid: point(a.mid_x, a.mid_y),
            end: point(a.end_x, a.end_y),
            width: nm_to_mm(a.width),
            layer: a.layer,
            net: a.net,
        })
        .collect();
    let pads = raw
        .pads
        .into_iter()
        .enumerate()
        .map(|(row, pad)| read_pad(row, pad))
        .collect::<Result<_, _>>()?;
    let dimensions = raw
        .dimensions
        .into_iter()
        .map(board_dimension::read)
        .collect();
    Ok(FreePrimitives {
        arcs,
        pads,
        dimensions,
    })
}

/// A malformed `layer_sizes` is a load error, as in the `pads` rows: read as
/// no sizes, it would draw and check copper that is not there.
fn read_pad(row: usize, p: PadRaw) -> Result<Pad, FormatError> {
    let layer_sizes = match p.layer_sizes.as_deref() {
        None | Some("") => None,
        Some(cell) => Some(read_layer_sizes(cell).ok_or_else(|| {
            row_field_error(
                PADS,
                row,
                LAYER_SIZES_COLUMN,
                cell,
                "per-layer sizes must be top=..;middle=..;bottom=.. or <copper layer>=..;.. \
                 with positive whole-nm sizes",
            )
        })?),
    };
    let pad = Pad {
        uuid: p.uuid,
        number: p.number,
        pad_type: p.pad_type,
        shape: p.shape,
        position: point(p.x, p.y),
        size: point(p.size_x, p.size_y),
        drill: p.drill.map(|d| DrillDef {
            diameter: nm_to_mm(d),
            shape: p.drill_shape,
        }),
        layers: p.layers,
        net: (p.net != 0).then_some(PadNet {
            number: p.net,
            name: p.net_name,
        }),
        roundrect_ratio: p.roundrect_ratio,
        rotation: p.rotation,
        trapezoid_delta: read_delta(p.trapezoid_delta.as_deref(), PADS, row)?,
        custom_outline: read_outline(p.custom_outline.as_deref(), PADS, row)?,
        net_locked: p.net_locked,
        layer_sizes,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    };
    check_shape(&pad, PADS, row)?;
    Ok(pad)
}

/// Append `[free]` to `out`; nothing for a board without free primitives.
pub(in crate::format) fn write(out: &mut String, board: &PcbBoard) -> Result<(), FormatError> {
    let free = &board.free;
    if free.is_empty() {
        return Ok(());
    }
    let refuse = |block: &str, row: usize, e: NonFiniteField| FormatError::NonFiniteField {
        block: block.to_string(),
        row,
        field: e.0.to_string(),
    };
    let arcs = free
        .arcs
        .iter()
        .enumerate()
        .map(|(row, a)| write_arc(a).map_err(|e| refuse(ARCS, row, e)))
        .collect::<Result<_, _>>()?;
    let pads = free
        .pads
        .iter()
        .enumerate()
        .map(|(row, p)| write_pad(p).map_err(|e| refuse(PADS, row, e)))
        .collect::<Result<_, _>>()?;
    let dimensions = free
        .dimensions
        .iter()
        .enumerate()
        .map(|(row, d)| board_dimension::write(d).map_err(|e| refuse(DIMENSIONS, row, e)))
        .collect::<Result<_, _>>()?;
    #[derive(Serialize)]
    struct Wrapper {
        free: FreeRaw,
    }
    let wrapper = Wrapper {
        free: FreeRaw {
            arcs,
            pads,
            dimensions,
        },
    };
    // The pads' turn and corner ratio are the only floats left.
    finite_toml::ensure_finite(&wrapper)?;
    out.push('\n');
    out.push_str(&toml::to_string_pretty(&wrapper)?);
    Ok(())
}

fn write_arc(a: &TrackArc) -> Result<ArcRaw, NonFiniteField> {
    Ok(ArcRaw {
        uuid: a.uuid,
        start_x: mm_to_nm(a.start.x, "start_x")?,
        start_y: mm_to_nm(a.start.y, "start_y")?,
        mid_x: mm_to_nm(a.mid.x, "mid_x")?,
        mid_y: mm_to_nm(a.mid.y, "mid_y")?,
        end_x: mm_to_nm(a.end.x, "end_x")?,
        end_y: mm_to_nm(a.end.y, "end_y")?,
        width: mm_to_nm(a.width, "width")?,
        layer: a.layer.clone(),
        net: a.net,
    })
}

fn write_pad(p: &Pad) -> Result<PadRaw, NonFiniteField> {
    refuse_unreadable_shape(p)?;
    let drill = p
        .drill
        .as_ref()
        .map(|d| mm_to_nm(d.diameter, "drill"))
        .transpose()?;
    Ok(PadRaw {
        uuid: p.uuid,
        number: p.number.clone(),
        pad_type: p.pad_type,
        shape: p.shape,
        x: mm_to_nm(p.position.x, "x")?,
        y: mm_to_nm(p.position.y, "y")?,
        size_x: mm_to_nm(p.size.x, "size_x")?,
        size_y: mm_to_nm(p.size.y, "size_y")?,
        drill,
        drill_shape: p
            .drill
            .as_ref()
            .map(|d| d.shape.clone())
            .unwrap_or_default(),
        layers: p.layers.clone(),
        net: p.net.as_ref().map_or(0, |n| n.number),
        net_name: p.net.as_ref().map(|n| n.name.clone()).unwrap_or_default(),
        roundrect_ratio: p.roundrect_ratio,
        rotation: p.rotation,
        net_locked: p.net_locked,
        layer_sizes: p.layer_sizes.as_ref().map(layer_sizes_cell).transpose()?,
        trapezoid_delta: delta_cell(p)?,
        custom_outline: outline_cell(p)?,
    })
}
