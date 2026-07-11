//! PCB bulk-row DTOs (`[footprints]` / `[pads]` / `[tracks]` /
//! `[vias]` TSV blocks) and their model translation.
//!
//! Holds the flat row schemas — [`PcbFootprintRow`], [`PcbPadRow`],
//! [`PcbTrackRow`], [`PcbViaRow`] — plus their [`SnxTable`] impls, the
//! PCB-side enum string codecs, and the `Footprint` / `Pad` /
//! `Segment` / `Via` ↔ row translation. Pure code motion out of
//! `mod.rs`; the row types stay `pub` (public surface via the `format`
//! re-exports), the translation helpers are `pub(in crate::format)` so
//! the container `SnxPcb` can reach them.

use super::extras::{FootprintExtras, PadExtras};
use super::tsv::{format_f64, parse_f64, parse_i64, parse_uuid};
use super::units::{mm_to_nm, nm_to_mm};
use super::*;
use crate::pcb::{
    DrillDef, Footprint, Pad, PadNet, PadShape, PadType, Point as PcbPoint, Segment, Via, ViaType,
};
use uuid::Uuid;

/// Bulk row for one [`Footprint`] in the `[footprints]` block.
#[derive(Debug, Clone, PartialEq)]
pub struct PcbFootprintRow {
    pub uuid: Uuid,
    pub ref_des: String,
    pub library: String,
    pub pos_x: i64,
    pub pos_y: i64,
    pub rotation: f64,
    pub layer: String,
    pub value: String,
}

impl SnxTable for PcbFootprintRow {
    fn columns() -> &'static [&'static str] {
        &[
            "uuid", "ref", "library", "pos_x", "pos_y", "rotation", "layer", "value",
        ]
    }

    fn to_row(&self) -> Vec<String> {
        vec![
            self.uuid.to_string(),
            self.ref_des.clone(),
            self.library.clone(),
            self.pos_x.to_string(),
            self.pos_y.to_string(),
            format_f64(self.rotation),
            self.layer.clone(),
            self.value.clone(),
        ]
    }

    fn from_row(values: &[&str], block: &str, row: usize) -> Result<Self, FormatError> {
        Ok(PcbFootprintRow {
            uuid: parse_uuid(values[0], block, row, "uuid")?,
            ref_des: values[1].to_string(),
            library: values[2].to_string(),
            pos_x: parse_i64(values[3], block, row, "pos_x")?,
            pos_y: parse_i64(values[4], block, row, "pos_y")?,
            rotation: parse_f64(values[5], block, row, "rotation")?,
            layer: values[6].to_string(),
            value: values[7].to_string(),
        })
    }
}

/// Bulk row for one [`Pad`] in the `[pads]` block. The row is keyed
/// to its parent footprint by `footprint_ref` (the user-facing
/// reference designator), keeping the file readable in code-review.
#[derive(Debug, Clone, PartialEq)]
pub struct PcbPadRow {
    pub uuid: Uuid,
    pub footprint_ref: String,
    pub pin: String,
    pub pos_x: i64,
    pub pos_y: i64,
    pub size_x: i64,
    pub size_y: i64,
    pub pad_type: String,
    pub shape: String,
    pub layers: String,
    pub drill: i64,
    pub net_number: u32,
    pub net_name: String,
    pub roundrect_ratio: f64,
}

impl SnxTable for PcbPadRow {
    fn columns() -> &'static [&'static str] {
        &[
            "uuid",
            "footprint_ref",
            "pin",
            "pos_x",
            "pos_y",
            "size_x",
            "size_y",
            "pad_type",
            "shape",
            "layers",
            "drill",
            "net_number",
            "net_name",
            "roundrect_ratio",
        ]
    }

    fn to_row(&self) -> Vec<String> {
        vec![
            self.uuid.to_string(),
            self.footprint_ref.clone(),
            self.pin.clone(),
            self.pos_x.to_string(),
            self.pos_y.to_string(),
            self.size_x.to_string(),
            self.size_y.to_string(),
            self.pad_type.clone(),
            self.shape.clone(),
            self.layers.clone(),
            self.drill.to_string(),
            self.net_number.to_string(),
            self.net_name.clone(),
            format_f64(self.roundrect_ratio),
        ]
    }

    fn from_row(values: &[&str], block: &str, row: usize) -> Result<Self, FormatError> {
        Ok(PcbPadRow {
            uuid: parse_uuid(values[0], block, row, "uuid")?,
            footprint_ref: values[1].to_string(),
            pin: values[2].to_string(),
            pos_x: parse_i64(values[3], block, row, "pos_x")?,
            pos_y: parse_i64(values[4], block, row, "pos_y")?,
            size_x: parse_i64(values[5], block, row, "size_x")?,
            size_y: parse_i64(values[6], block, row, "size_y")?,
            pad_type: values[7].to_string(),
            shape: values[8].to_string(),
            layers: values[9].to_string(),
            drill: parse_i64(values[10], block, row, "drill")?,
            net_number: values[11].parse().map_err(|e: std::num::ParseIntError| {
                FormatError::TsvFieldParse {
                    block: block.to_string(),
                    row,
                    field: "net_number".to_string(),
                    message: e.to_string(),
                }
            })?,
            net_name: values[12].to_string(),
            roundrect_ratio: parse_f64(values[13], block, row, "roundrect_ratio")?,
        })
    }
}

/// Bulk row for one [`Segment`] in the `[tracks]` block.
#[derive(Debug, Clone, PartialEq)]
pub struct PcbTrackRow {
    pub uuid: Uuid,
    pub net: u32,
    pub layer: String,
    pub width: i64,
    pub start_x: i64,
    pub start_y: i64,
    pub end_x: i64,
    pub end_y: i64,
}

impl SnxTable for PcbTrackRow {
    fn columns() -> &'static [&'static str] {
        &[
            "uuid", "net", "layer", "width", "start_x", "start_y", "end_x", "end_y",
        ]
    }

    fn to_row(&self) -> Vec<String> {
        vec![
            self.uuid.to_string(),
            self.net.to_string(),
            self.layer.clone(),
            self.width.to_string(),
            self.start_x.to_string(),
            self.start_y.to_string(),
            self.end_x.to_string(),
            self.end_y.to_string(),
        ]
    }

    fn from_row(values: &[&str], block: &str, row: usize) -> Result<Self, FormatError> {
        Ok(PcbTrackRow {
            uuid: parse_uuid(values[0], block, row, "uuid")?,
            net: values[1].parse().map_err(|e: std::num::ParseIntError| {
                FormatError::TsvFieldParse {
                    block: block.to_string(),
                    row,
                    field: "net".to_string(),
                    message: e.to_string(),
                }
            })?,
            layer: values[2].to_string(),
            width: parse_i64(values[3], block, row, "width")?,
            start_x: parse_i64(values[4], block, row, "start_x")?,
            start_y: parse_i64(values[5], block, row, "start_y")?,
            end_x: parse_i64(values[6], block, row, "end_x")?,
            end_y: parse_i64(values[7], block, row, "end_y")?,
        })
    }
}

/// Bulk row for one [`Via`] in the `[vias]` block.
#[derive(Debug, Clone, PartialEq)]
pub struct PcbViaRow {
    pub uuid: Uuid,
    pub net: u32,
    pub pos_x: i64,
    pub pos_y: i64,
    pub drill: i64,
    pub diameter: i64,
    pub layers: String,
    pub via_type: String,
}

impl SnxTable for PcbViaRow {
    fn columns() -> &'static [&'static str] {
        &[
            "uuid", "net", "pos_x", "pos_y", "drill", "diameter", "layers", "via_type",
        ]
    }

    fn to_row(&self) -> Vec<String> {
        vec![
            self.uuid.to_string(),
            self.net.to_string(),
            self.pos_x.to_string(),
            self.pos_y.to_string(),
            self.drill.to_string(),
            self.diameter.to_string(),
            self.layers.clone(),
            self.via_type.clone(),
        ]
    }

    fn from_row(values: &[&str], block: &str, row: usize) -> Result<Self, FormatError> {
        Ok(PcbViaRow {
            uuid: parse_uuid(values[0], block, row, "uuid")?,
            net: values[1].parse().map_err(|e: std::num::ParseIntError| {
                FormatError::TsvFieldParse {
                    block: block.to_string(),
                    row,
                    field: "net".to_string(),
                    message: e.to_string(),
                }
            })?,
            pos_x: parse_i64(values[2], block, row, "pos_x")?,
            pos_y: parse_i64(values[3], block, row, "pos_y")?,
            drill: parse_i64(values[4], block, row, "drill")?,
            diameter: parse_i64(values[5], block, row, "diameter")?,
            layers: values[6].to_string(),
            via_type: values[7].to_string(),
        })
    }
}

// ---------------------------------------------------------------------------
// Enum string codecs
// ---------------------------------------------------------------------------

fn pad_type_str(t: PadType) -> &'static str {
    match t {
        PadType::Thru => "thru",
        PadType::Smd => "smd",
        PadType::Connect => "connect",
        PadType::NpThru => "np_thru",
    }
}

fn parse_pad_type(s: &str) -> PadType {
    match s {
        "smd" => PadType::Smd,
        "connect" => PadType::Connect,
        "np_thru" => PadType::NpThru,
        _ => PadType::Thru,
    }
}

fn pad_shape_str(s: PadShape) -> &'static str {
    match s {
        PadShape::Circle => "circle",
        PadShape::Rect => "rect",
        PadShape::Oval => "oval",
        PadShape::Trapezoid => "trapezoid",
        PadShape::RoundRect => "roundrect",
        PadShape::Custom => "custom",
    }
}

fn parse_pad_shape(s: &str) -> PadShape {
    match s {
        "rect" => PadShape::Rect,
        "oval" => PadShape::Oval,
        "trapezoid" => PadShape::Trapezoid,
        "roundrect" => PadShape::RoundRect,
        "custom" => PadShape::Custom,
        _ => PadShape::Circle,
    }
}

fn via_type_str(t: ViaType) -> &'static str {
    match t {
        ViaType::Through => "through",
        ViaType::Blind => "blind",
        ViaType::Micro => "micro",
    }
}

fn parse_via_type(s: &str) -> ViaType {
    match s {
        "blind" => ViaType::Blind,
        "micro" => ViaType::Micro,
        _ => ViaType::Through,
    }
}

fn join_layers(layers: &[String]) -> String {
    if layers.is_empty() {
        return String::new();
    }
    layers.join(",")
}

fn split_layers(s: &str) -> Vec<String> {
    if s.is_empty() {
        return Vec::new();
    }
    s.split(',').map(str::to_string).collect()
}

// ---------------------------------------------------------------------------
// Footprint / Pad / Segment / Via translation
// ---------------------------------------------------------------------------

pub(in crate::format) fn footprint_to_row(fp: &Footprint) -> PcbFootprintRow {
    PcbFootprintRow {
        uuid: fp.uuid,
        ref_des: fp.reference.clone(),
        library: fp.footprint_id.clone(),
        pos_x: mm_to_nm(fp.position.x),
        pos_y: mm_to_nm(fp.position.y),
        rotation: fp.rotation,
        layer: fp.layer.clone(),
        value: fp.value.clone(),
    }
}

pub(in crate::format) fn row_to_footprint(
    row: PcbFootprintRow,
    extras: FootprintExtras,
) -> Footprint {
    Footprint {
        uuid: row.uuid,
        reference: row.ref_des,
        value: row.value,
        footprint_id: if !extras.footprint_id.is_empty() {
            extras.footprint_id
        } else {
            row.library
        },
        position: PcbPoint {
            x: nm_to_mm(row.pos_x),
            y: nm_to_mm(row.pos_y),
        },
        rotation: row.rotation,
        layer: row.layer,
        locked: extras.locked,
        pads: Vec::new(),
        graphics: extras.graphics,
        properties: extras.properties,
    }
}

pub(in crate::format) fn pad_to_row(pad: &Pad, footprint_ref: &str) -> PcbPadRow {
    let drill_nm = pad
        .drill
        .as_ref()
        .map(|d| mm_to_nm(d.diameter))
        .unwrap_or(0);
    let (net_number, net_name) = pad
        .net
        .as_ref()
        .map(|n| (n.number, n.name.clone()))
        .unwrap_or((0, String::new()));
    PcbPadRow {
        uuid: pad.uuid,
        footprint_ref: footprint_ref.to_string(),
        pin: pad.number.clone(),
        pos_x: mm_to_nm(pad.position.x),
        pos_y: mm_to_nm(pad.position.y),
        size_x: mm_to_nm(pad.size.x),
        size_y: mm_to_nm(pad.size.y),
        pad_type: pad_type_str(pad.pad_type).to_string(),
        shape: pad_shape_str(pad.shape).to_string(),
        layers: join_layers(&pad.layers),
        drill: drill_nm,
        net_number,
        net_name,
        roundrect_ratio: pad.roundrect_ratio,
    }
}

pub(in crate::format) fn row_to_pad(row: PcbPadRow, extras: PadExtras) -> Pad {
    let drill = if row.drill > 0 {
        Some(DrillDef {
            diameter: nm_to_mm(row.drill),
            shape: extras.drill_shape,
        })
    } else {
        None
    };
    let net = if row.net_number != 0 || !row.net_name.is_empty() {
        Some(PadNet {
            number: row.net_number,
            name: row.net_name,
        })
    } else {
        None
    };
    Pad {
        uuid: row.uuid,
        number: row.pin,
        pad_type: parse_pad_type(&row.pad_type),
        shape: parse_pad_shape(&row.shape),
        position: PcbPoint {
            x: nm_to_mm(row.pos_x),
            y: nm_to_mm(row.pos_y),
        },
        size: PcbPoint {
            x: nm_to_mm(row.size_x),
            y: nm_to_mm(row.size_y),
        },
        drill,
        layers: split_layers(&row.layers),
        net,
        roundrect_ratio: row.roundrect_ratio,
    }
}

pub(in crate::format) fn track_to_row(s: &Segment) -> PcbTrackRow {
    PcbTrackRow {
        uuid: s.uuid,
        net: s.net,
        layer: s.layer.clone(),
        width: mm_to_nm(s.width),
        start_x: mm_to_nm(s.start.x),
        start_y: mm_to_nm(s.start.y),
        end_x: mm_to_nm(s.end.x),
        end_y: mm_to_nm(s.end.y),
    }
}

pub(in crate::format) fn row_to_track(row: PcbTrackRow) -> Segment {
    Segment {
        uuid: row.uuid,
        start: PcbPoint {
            x: nm_to_mm(row.start_x),
            y: nm_to_mm(row.start_y),
        },
        end: PcbPoint {
            x: nm_to_mm(row.end_x),
            y: nm_to_mm(row.end_y),
        },
        width: nm_to_mm(row.width),
        layer: row.layer,
        net: row.net,
    }
}

pub(in crate::format) fn via_to_row(v: &Via) -> PcbViaRow {
    PcbViaRow {
        uuid: v.uuid,
        net: v.net,
        pos_x: mm_to_nm(v.position.x),
        pos_y: mm_to_nm(v.position.y),
        drill: mm_to_nm(v.drill),
        diameter: mm_to_nm(v.diameter),
        layers: join_layers(&v.layers),
        via_type: via_type_str(v.via_type).to_string(),
    }
}

pub(in crate::format) fn row_to_via(row: PcbViaRow) -> Via {
    Via {
        uuid: row.uuid,
        position: PcbPoint {
            x: nm_to_mm(row.pos_x),
            y: nm_to_mm(row.pos_y),
        },
        diameter: nm_to_mm(row.diameter),
        drill: nm_to_mm(row.drill),
        layers: split_layers(&row.layers),
        net: row.net,
        via_type: parse_via_type(&row.via_type),
    }
}
