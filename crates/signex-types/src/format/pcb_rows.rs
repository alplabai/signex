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
use super::tsv::{format_f64, parse_f64, parse_nm, parse_uuid};
use super::units::{NonFiniteField, mm_to_nm, nm_to_mm};
use super::unknown_columns::impl_unknown_columns_accessors;
use super::*;
use crate::pcb::{
    DrillDef, Footprint, Pad, PadNet, PadShape, PadType, Point as PcbPoint, Segment, SymbolLink,
    Via, ViaType,
};
use crate::pcb_layer::{CopperLayer, InnerCopper, MechanicalLayer, PcbLayer, TechnicalLayer};
use crate::schematic::InstancePath;
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
    pub unknown_columns: UnknownColumns,
}

impl SnxTable for PcbFootprintRow {
    fn columns() -> &'static [&'static str] {
        &[
            "uuid", "ref", "library", "pos_x", "pos_y", "rotation", "layer", "value",
        ]
    }

    fn to_row(&self) -> Result<Vec<String>, NonFiniteField> {
        Ok(vec![
            self.uuid.to_string(),
            self.ref_des.clone(),
            self.library.clone(),
            self.pos_x.to_string(),
            self.pos_y.to_string(),
            format_f64(self.rotation, "rotation")?,
            self.layer.clone(),
            self.value.clone(),
        ])
    }

    impl_unknown_columns_accessors!();

    fn from_row(values: &[&str], block: &str, row: usize) -> Result<Self, FormatError> {
        Ok(PcbFootprintRow {
            uuid: parse_uuid(values[0], block, row, "uuid")?,
            ref_des: values[1].to_string(),
            library: values[2].to_string(),
            pos_x: parse_nm(values[3], block, row, "pos_x")?,
            pos_y: parse_nm(values[4], block, row, "pos_y")?,
            rotation: parse_f64(values[5], block, row, "rotation")?,
            layer: values[6].to_string(),
            value: values[7].to_string(),
            unknown_columns: UnknownColumns::default(),
        })
    }
}

/// Bulk row for one [`Pad`] in the `[pads]` block. The row is keyed
/// to its parent footprint by `footprint_ref`: the footprint's uuid in
/// format 1.1 (designators are not unique), or its reference designator in
/// 1.0 files, which the reader still accepts.
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
    pub unknown_columns: UnknownColumns,
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

    fn to_row(&self) -> Result<Vec<String>, NonFiniteField> {
        Ok(vec![
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
            format_f64(self.roundrect_ratio, "roundrect_ratio")?,
        ])
    }

    impl_unknown_columns_accessors!();

    fn from_row(values: &[&str], block: &str, row: usize) -> Result<Self, FormatError> {
        Ok(PcbPadRow {
            uuid: parse_uuid(values[0], block, row, "uuid")?,
            footprint_ref: values[1].to_string(),
            pin: values[2].to_string(),
            pos_x: parse_nm(values[3], block, row, "pos_x")?,
            pos_y: parse_nm(values[4], block, row, "pos_y")?,
            size_x: parse_nm(values[5], block, row, "size_x")?,
            size_y: parse_nm(values[6], block, row, "size_y")?,
            pad_type: values[7].to_string(),
            shape: values[8].to_string(),
            layers: values[9].to_string(),
            drill: parse_nm(values[10], block, row, "drill")?,
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
            unknown_columns: UnknownColumns::default(),
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
    pub unknown_columns: UnknownColumns,
}

impl SnxTable for PcbTrackRow {
    fn columns() -> &'static [&'static str] {
        &[
            "uuid", "net", "layer", "width", "start_x", "start_y", "end_x", "end_y",
        ]
    }

    fn to_row(&self) -> Result<Vec<String>, NonFiniteField> {
        Ok(vec![
            self.uuid.to_string(),
            self.net.to_string(),
            self.layer.clone(),
            self.width.to_string(),
            self.start_x.to_string(),
            self.start_y.to_string(),
            self.end_x.to_string(),
            self.end_y.to_string(),
        ])
    }

    impl_unknown_columns_accessors!();

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
            width: parse_nm(values[3], block, row, "width")?,
            start_x: parse_nm(values[4], block, row, "start_x")?,
            start_y: parse_nm(values[5], block, row, "start_y")?,
            end_x: parse_nm(values[6], block, row, "end_x")?,
            end_y: parse_nm(values[7], block, row, "end_y")?,
            unknown_columns: UnknownColumns::default(),
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
    pub unknown_columns: UnknownColumns,
}

impl SnxTable for PcbViaRow {
    fn columns() -> &'static [&'static str] {
        &[
            "uuid", "net", "pos_x", "pos_y", "drill", "diameter", "layers", "via_type",
        ]
    }

    fn to_row(&self) -> Result<Vec<String>, NonFiniteField> {
        Ok(vec![
            self.uuid.to_string(),
            self.net.to_string(),
            self.pos_x.to_string(),
            self.pos_y.to_string(),
            self.drill.to_string(),
            self.diameter.to_string(),
            self.layers.clone(),
            self.via_type.clone(),
        ])
    }

    impl_unknown_columns_accessors!();

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
            pos_x: parse_nm(values[2], block, row, "pos_x")?,
            pos_y: parse_nm(values[3], block, row, "pos_y")?,
            drill: parse_nm(values[4], block, row, "drill")?,
            diameter: parse_nm(values[5], block, row, "diameter")?,
            layers: values[6].to_string(),
            via_type: values[7].to_string(),
            unknown_columns: UnknownColumns::default(),
        })
    }
}

// ---------------------------------------------------------------------------
// Enum string codecs
// ---------------------------------------------------------------------------

crate::token_enum! {
    fn pad_type_str / parse_pad_type for PadType, fallback Thru {
        Thru => "thru",
        Smd => "smd",
        Connect => "connect",
        NpThru => "np_thru",
    }
}

fn pad_shape_str(shape: PadShape) -> &'static str {
    match shape {
        PadShape::Circle => "circle",
        PadShape::Rect => "rect",
        PadShape::Oval => "oval",
        PadShape::RoundRect => "roundrect",
    }
}

/// An unknown shape token is a load error naming the field and the value:
/// reading it as some other shape would draw and hit-test the wrong copper.
fn parse_pad_shape(text: &str, block: &str, row: usize) -> Result<PadShape, FormatError> {
    Ok(match text {
        "circle" => PadShape::Circle,
        "rect" => PadShape::Rect,
        "oval" => PadShape::Oval,
        "roundrect" => PadShape::RoundRect,
        other => {
            return Err(pad_field_error(
                block,
                row,
                "shape",
                other,
                "not a pad shape (circle, rect, oval, roundrect)",
            ));
        }
    })
}

fn pad_field_error(block: &str, row: usize, field: &str, value: &str, why: &str) -> FormatError {
    FormatError::TsvFieldParse {
        block: block.to_string(),
        row,
        field: field.to_string(),
        message: format!("value {value:?}: {why}"),
    }
}

/// Header of the optional `rotation` pad column: the pad's own turn in
/// degrees. Rides the foreign-column path like `net_locked`; written only
/// when some pad is turned, so an unturned board re-saves unchanged.
pub(in crate::format) const PAD_ROTATION_COLUMN: &str = "rotation";

fn with_rotation_cell(
    mut columns: UnknownColumns,
    rotation: f64,
) -> Result<UnknownColumns, NonFiniteField> {
    if rotation != 0.0 {
        columns.set(PAD_ROTATION_COLUMN, format_f64(rotation, "rotation")?);
    }
    Ok(columns)
}

fn take_rotation_cell(
    columns: &mut UnknownColumns,
    block: &str,
    row: usize,
) -> Result<f64, FormatError> {
    match columns.take(PAD_ROTATION_COLUMN) {
        None => Ok(0.0),
        Some(cell) if cell.is_empty() => Ok(0.0),
        Some(cell) => match cell.trim().parse::<f64>() {
            Ok(degrees) if degrees.is_finite() => Ok(degrees),
            _ => Err(pad_field_error(
                block,
                row,
                PAD_ROTATION_COLUMN,
                &cell,
                "a pad rotation must be a finite number of degrees",
            )),
        },
    }
}

crate::token_enum! {
    fn via_type_str / parse_via_type for ViaType, fallback Through {
        Through => "through",
        Blind => "blind",
        Micro => "micro",
    }
}

// The fallback is a placeholder: `parse_pcb_layer` re-checks the token, so
// an unknown name is never read as `Margin`.
crate::token_enum! {
    fn technical_layer_str / parse_technical_layer_or_margin for TechnicalLayer, fallback Margin {
        EdgeCuts => "EdgeCuts",
        Margin => "Margin",
        SilkscreenTop => "TopSilkscreen",
        SilkscreenBottom => "BottomSilkscreen",
        SolderMaskTop => "TopSolderMask",
        SolderMaskBottom => "BottomSolderMask",
        PasteTop => "TopPaste",
        PasteBottom => "BottomPaste",
        AdhesiveTop => "TopAdhesive",
        AdhesiveBottom => "BottomAdhesive",
        FabTop => "TopFab",
        FabBottom => "BottomFab",
        AssemblyTop => "TopAssembly",
        AssemblyBottom => "BottomAssembly",
        CourtyardTop => "TopCourtyard",
        CourtyardBottom => "BottomCourtyard",
        Body3dTop => "Top3DBody",
        Body3dBottom => "Bottom3DBody",
        KeepoutTop => "TopKeepout",
        KeepoutBottom => "BottomKeepout",
        FiducialTop => "TopFiducial",
        FiducialBottom => "BottomFiducial",
        DesignatorTop => "TopDesignator",
        DesignatorBottom => "BottomDesignator",
        Dimensions => "Dimensions",
        Notes => "Notes",
        DrillDrawing => "DrillDrawing",
        Eco1 => "Eco1",
        Eco2 => "Eco2",
    }
}

/// The on-disk name of `layer`: `TopCopper`, `Inner<n>Copper`,
/// `BottomCopper`, a technical name such as `TopSilkscreen`, or `Mechanical<n>`.
pub fn pcb_layer_token(layer: PcbLayer) -> String {
    match layer {
        PcbLayer::Copper(CopperLayer::Top) => "TopCopper".to_string(),
        PcbLayer::Copper(CopperLayer::Inner(n)) => format!("Inner{}Copper", n.index()),
        PcbLayer::Copper(CopperLayer::Bottom) => "BottomCopper".to_string(),
        PcbLayer::Technical(t) => technical_layer_str(t).to_string(),
        PcbLayer::Mechanical(n) => format!("Mechanical{}", n.index()),
    }
}

/// The layer `token` names, or `None` for a name this build does not know.
/// Exact match only, so `pcb_layer_token` of the result is `token` again;
/// the caller keeps an unknown name as its string.
pub fn parse_pcb_layer(token: &str) -> Option<PcbLayer> {
    match token {
        "TopCopper" => return Some(PcbLayer::Copper(CopperLayer::Top)),
        "BottomCopper" => return Some(PcbLayer::Copper(CopperLayer::Bottom)),
        _ => {}
    }
    if let Some(n) = token
        .strip_prefix("Inner")
        .and_then(|rest| rest.strip_suffix("Copper"))
    {
        return canonical_index(n)
            .and_then(InnerCopper::new)
            .map(|n| PcbLayer::Copper(CopperLayer::Inner(n)));
    }
    if let Some(n) = token.strip_prefix("Mechanical") {
        return canonical_index(n)
            .and_then(MechanicalLayer::new)
            .map(PcbLayer::Mechanical);
    }
    let technical = parse_technical_layer_or_margin(token);
    (technical_layer_str(technical) == token).then_some(PcbLayer::Technical(technical))
}

/// A decimal index written without sign or leading zero.
fn canonical_index(digits: &str) -> Option<u8> {
    let canonical = !digits.is_empty()
        && !digits.starts_with('0')
        && digits.bytes().all(|b| b.is_ascii_digit());
    if !canonical {
        return None;
    }
    digits.parse().ok()
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

pub(in crate::format) fn footprint_to_row(
    fp: &Footprint,
) -> Result<PcbFootprintRow, NonFiniteField> {
    let unknown_columns = with_link_cell(fp.unknown_columns.clone(), fp.schematic_link.as_ref());
    Ok(PcbFootprintRow {
        uuid: fp.uuid,
        ref_des: fp.reference.clone(),
        library: fp.footprint_id.clone(),
        pos_x: mm_to_nm(fp.position.x, "pos_x")?,
        pos_y: mm_to_nm(fp.position.y, "pos_y")?,
        rotation: fp.rotation,
        layer: fp.layer.clone(),
        value: fp.value.clone(),
        unknown_columns,
    })
}

pub(in crate::format) fn row_to_footprint(
    mut row: PcbFootprintRow,
    extras: FootprintExtras,
) -> Footprint {
    let schematic_link = take_link_cell(&mut row.unknown_columns);
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
        schematic_link,
        unknown: extras.unknown,
        unknown_columns: row.unknown_columns,
    }
}

/// Header of the optional `sch_link` footprint column: the footprint's
/// schematic part as `<unit uuid>[+<unit uuid>...]@<sheet path>`, units in
/// ascending order, e.g. `0190...@/` for a one-unit part on a root page or
/// `0190...a+0190...b@/<sheet uuid>` for a two-unit part on a child sheet
/// occurrence. A foreign column like `net_locked`: absent when no footprint
/// is linked, an empty cell for an unlinked footprint of a block that has
/// linked ones.
pub(in crate::format) const SCH_LINK_COLUMN: &str = "sch_link";

fn link_cell(link: &SymbolLink) -> String {
    let units: Vec<String> = link.units.iter().map(Uuid::to_string).collect();
    format!("{}@{}", units.join("+"), link.sheet)
}

fn with_link_cell(mut columns: UnknownColumns, link: Option<&SymbolLink>) -> UnknownColumns {
    if let Some(link) = link {
        columns.set(SCH_LINK_COLUMN, link_cell(link));
    }
    columns
}

/// An empty cell is "no link"; a cell this build cannot parse is not ours
/// to read, so it stays a foreign cell and round-trips untouched.
fn take_link_cell(columns: &mut UnknownColumns) -> Option<SymbolLink> {
    let cell = columns.get(SCH_LINK_COLUMN)?;
    if cell.is_empty() {
        columns.take(SCH_LINK_COLUMN);
        return None;
    }
    let (units, sheet) = cell.split_once('@')?;
    let units = units
        .split('+')
        .map(|u| Uuid::parse_str(u).ok())
        .collect::<Option<Vec<_>>>()?;
    let link = SymbolLink::new(InstancePath::parse(sheet)?, units);
    // Only a cell that writes back identically is ours.
    if link_cell(&link) != cell {
        return None;
    }
    columns.take(SCH_LINK_COLUMN);
    Some(link)
}

/// Header of the optional `net_locked` pad column. Like the schematic's
/// `locked` column it is not a known column: it rides the foreign-column
/// path. A board with no locked pad has no such header and re-saves
/// byte-identical, and a build that predates the lock keeps it. When any pad
/// is locked the header is written after the known columns, in first-seen
/// order among the block's foreign columns (its position among the known
/// ones is not kept), and every unlocked pad writes an empty cell. See
/// `unknown_columns` for the contract.
pub(in crate::format) const NET_LOCKED_COLUMN: &str = "net_locked";

/// The cell a locked pad writes; every other pad of the block writes empty.
const NET_LOCKED_CELL: &str = "true";

fn with_net_locked_cell(mut columns: UnknownColumns, locked: bool) -> UnknownColumns {
    if locked {
        columns.set(NET_LOCKED_COLUMN, NET_LOCKED_CELL.to_string());
    }
    columns
}

/// `true` locks and an empty cell is an unlocked pad; any other value is not
/// ours to read, so it stays a foreign cell and round-trips untouched.
fn take_net_locked_cell(columns: &mut UnknownColumns) -> bool {
    match columns.get(NET_LOCKED_COLUMN) {
        Some(NET_LOCKED_CELL) => {
            columns.take(NET_LOCKED_COLUMN);
            true
        }
        Some("") => {
            columns.take(NET_LOCKED_COLUMN);
            false
        }
        _ => false,
    }
}

pub(in crate::format) fn pad_to_row(
    pad: &Pad,
    footprint_ref: &str,
) -> Result<PcbPadRow, NonFiniteField> {
    let drill_nm = pad
        .drill
        .as_ref()
        .map(|d| mm_to_nm(d.diameter, "drill"))
        .transpose()?
        .unwrap_or(0);
    let (net_number, net_name) = pad
        .net
        .as_ref()
        .map(|n| (n.number, n.name.clone()))
        .unwrap_or((0, String::new()));
    Ok(PcbPadRow {
        uuid: pad.uuid,
        footprint_ref: footprint_ref.to_string(),
        pin: pad.number.clone(),
        pos_x: mm_to_nm(pad.position.x, "pos_x")?,
        pos_y: mm_to_nm(pad.position.y, "pos_y")?,
        size_x: mm_to_nm(pad.size.x, "size_x")?,
        size_y: mm_to_nm(pad.size.y, "size_y")?,
        pad_type: pad_type_str(pad.pad_type).to_string(),
        shape: pad_shape_str(pad.shape).to_string(),
        layers: join_layers(&pad.layers),
        drill: drill_nm,
        net_number,
        net_name,
        roundrect_ratio: pad.roundrect_ratio,
        unknown_columns: with_rotation_cell(
            with_net_locked_cell(pad.unknown_columns.clone(), pad.net_locked),
            pad.rotation,
        )?,
    })
}

pub(in crate::format) fn row_to_pad(
    mut row: PcbPadRow,
    extras: PadExtras,
    block: &str,
    index: usize,
) -> Result<Pad, FormatError> {
    let shape = parse_pad_shape(&row.shape, block, index)?;
    let ratio = row.roundrect_ratio;
    if shape == PadShape::RoundRect && !(0.0..=0.5).contains(&ratio) {
        return Err(pad_field_error(
            block,
            index,
            "roundrect_ratio",
            &ratio.to_string(),
            "a roundrect corner ratio must be within 0..=0.5",
        ));
    }
    let rotation = take_rotation_cell(&mut row.unknown_columns, block, index)?;
    let net_locked = take_net_locked_cell(&mut row.unknown_columns);
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
    Ok(Pad {
        uuid: row.uuid,
        number: row.pin,
        pad_type: parse_pad_type(&row.pad_type),
        shape,
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
        rotation,
        net_locked,
        unknown: extras.unknown,
        unknown_columns: row.unknown_columns,
    })
}

pub(in crate::format) fn track_to_row(s: &Segment) -> Result<PcbTrackRow, NonFiniteField> {
    Ok(PcbTrackRow {
        uuid: s.uuid,
        net: s.net,
        layer: s.layer.clone(),
        width: mm_to_nm(s.width, "width")?,
        start_x: mm_to_nm(s.start.x, "start_x")?,
        start_y: mm_to_nm(s.start.y, "start_y")?,
        end_x: mm_to_nm(s.end.x, "end_x")?,
        end_y: mm_to_nm(s.end.y, "end_y")?,
        unknown_columns: s.unknown_columns.clone(),
    })
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
        unknown_columns: row.unknown_columns,
    }
}

pub(in crate::format) fn via_to_row(v: &Via) -> Result<PcbViaRow, NonFiniteField> {
    Ok(PcbViaRow {
        uuid: v.uuid,
        net: v.net,
        pos_x: mm_to_nm(v.position.x, "pos_x")?,
        pos_y: mm_to_nm(v.position.y, "pos_y")?,
        drill: mm_to_nm(v.drill, "drill")?,
        diameter: mm_to_nm(v.diameter, "diameter")?,
        layers: join_layers(&v.layers),
        via_type: via_type_str(v.via_type).to_string(),
        unknown_columns: v.unknown_columns.clone(),
    })
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
        unknown_columns: row.unknown_columns,
    }
}
