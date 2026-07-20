//! Schematic bulk-row DTOs (`[sheets.*]` TSV blocks) and their
//! model translation.
//!
//! Holds the flat row schemas — [`SchComponentRow`], [`SchWireRow`],
//! [`SchJunctionRow`], [`SchLabelRow`] — plus their [`SnxTable`] impls,
//! the schematic-side enum string codecs, and the `Symbol` / `Wire` /
//! `Junction` / `Label` ↔ row translation. Pure code motion out of
//! `mod.rs`; the row types stay `pub` (public surface via the `format`
//! re-exports), the translation helpers are `pub(in crate::format)` so
//! the container `SnxSchematic` can reach them.

use super::extras::SymbolExtras;
use super::tsv::{format_f64, parse_f64, parse_i64, parse_uuid};
use super::units::{mm_to_nm, nm_to_mm};
use super::*;
use crate::schematic::{HAlign, Junction, Label, LabelType, Point, Symbol, VAlign, Wire};
use uuid::Uuid;

/// Bulk row for one [`Symbol`] in the `[sheets.components]` block.
///
/// Captures the fields with one cell per concept (ref designator,
/// library id, position in nanometres, rotation in degrees, value,
/// MPN). Symbol-level fields that don't fit a flat row — `fields`
/// map, `custom_properties`, `pin_uuids`, `instances`, `ref_text` /
/// `val_text` text-prop overrides — survive in the
/// `[sheets.component_extras.<uuid>]` auxiliary TOML tables.
#[derive(Debug, Clone, PartialEq)]
pub struct SchComponentRow {
    pub uuid: Uuid,
    pub ref_des: String,
    pub library: String,
    pub pos_x: i64,
    pub pos_y: i64,
    pub rotation: f64,
    pub value: String,
    pub mpn: String,
}

impl SnxTable for SchComponentRow {
    fn columns() -> &'static [&'static str] {
        &[
            "uuid", "ref", "library", "pos_x", "pos_y", "rotation", "value", "mpn",
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
            self.value.clone(),
            self.mpn.clone(),
        ]
    }

    fn from_row(values: &[&str], block: &str, row: usize) -> Result<Self, FormatError> {
        Ok(SchComponentRow {
            uuid: parse_uuid(values[0], block, row, "uuid")?,
            ref_des: values[1].to_string(),
            library: values[2].to_string(),
            pos_x: parse_i64(values[3], block, row, "pos_x")?,
            pos_y: parse_i64(values[4], block, row, "pos_y")?,
            rotation: parse_f64(values[5], block, row, "rotation")?,
            value: values[6].to_string(),
            mpn: values[7].to_string(),
        })
    }
}

/// Bulk row for one [`Wire`] in the `[sheets.wires]` block.
#[derive(Debug, Clone, PartialEq)]
pub struct SchWireRow {
    pub uuid: Uuid,
    pub net: String,
    pub start_x: i64,
    pub start_y: i64,
    pub end_x: i64,
    pub end_y: i64,
    pub stroke_width: f64,
}

impl SnxTable for SchWireRow {
    fn columns() -> &'static [&'static str] {
        &[
            "uuid",
            "net",
            "start_x",
            "start_y",
            "end_x",
            "end_y",
            "stroke_width",
        ]
    }

    fn to_row(&self) -> Vec<String> {
        vec![
            self.uuid.to_string(),
            self.net.clone(),
            self.start_x.to_string(),
            self.start_y.to_string(),
            self.end_x.to_string(),
            self.end_y.to_string(),
            format_f64(self.stroke_width),
        ]
    }

    fn from_row(values: &[&str], block: &str, row: usize) -> Result<Self, FormatError> {
        Ok(SchWireRow {
            uuid: parse_uuid(values[0], block, row, "uuid")?,
            net: values[1].to_string(),
            start_x: parse_i64(values[2], block, row, "start_x")?,
            start_y: parse_i64(values[3], block, row, "start_y")?,
            end_x: parse_i64(values[4], block, row, "end_x")?,
            end_y: parse_i64(values[5], block, row, "end_y")?,
            stroke_width: parse_f64(values[6], block, row, "stroke_width")?,
        })
    }
}

/// Bulk row for one [`Junction`] in the `[sheets.junctions]` block.
#[derive(Debug, Clone, PartialEq)]
pub struct SchJunctionRow {
    pub uuid: Uuid,
    pub pos_x: i64,
    pub pos_y: i64,
    pub diameter: f64,
}

impl SnxTable for SchJunctionRow {
    fn columns() -> &'static [&'static str] {
        &["uuid", "pos_x", "pos_y", "diameter"]
    }

    fn to_row(&self) -> Vec<String> {
        vec![
            self.uuid.to_string(),
            self.pos_x.to_string(),
            self.pos_y.to_string(),
            format_f64(self.diameter),
        ]
    }

    fn from_row(values: &[&str], block: &str, row: usize) -> Result<Self, FormatError> {
        Ok(SchJunctionRow {
            uuid: parse_uuid(values[0], block, row, "uuid")?,
            pos_x: parse_i64(values[1], block, row, "pos_x")?,
            pos_y: parse_i64(values[2], block, row, "pos_y")?,
            diameter: parse_f64(values[3], block, row, "diameter")?,
        })
    }
}

/// Bulk row for one [`Label`] in the `[sheets.labels]` block.
#[derive(Debug, Clone, PartialEq)]
pub struct SchLabelRow {
    pub uuid: Uuid,
    pub text: String,
    pub pos_x: i64,
    pub pos_y: i64,
    pub rotation: f64,
    pub kind: String,
    pub shape: String,
    pub font_size: f64,
    pub justify: String,
    pub justify_v: String,
}

impl SnxTable for SchLabelRow {
    fn columns() -> &'static [&'static str] {
        &[
            "uuid",
            "text",
            "pos_x",
            "pos_y",
            "rotation",
            "kind",
            "shape",
            "font_size",
            "justify",
            "justify_v",
        ]
    }

    fn to_row(&self) -> Vec<String> {
        vec![
            self.uuid.to_string(),
            self.text.clone(),
            self.pos_x.to_string(),
            self.pos_y.to_string(),
            format_f64(self.rotation),
            self.kind.clone(),
            self.shape.clone(),
            format_f64(self.font_size),
            self.justify.clone(),
            self.justify_v.clone(),
        ]
    }

    fn from_row(values: &[&str], block: &str, row: usize) -> Result<Self, FormatError> {
        Ok(SchLabelRow {
            uuid: parse_uuid(values[0], block, row, "uuid")?,
            text: values[1].to_string(),
            pos_x: parse_i64(values[2], block, row, "pos_x")?,
            pos_y: parse_i64(values[3], block, row, "pos_y")?,
            rotation: parse_f64(values[4], block, row, "rotation")?,
            kind: values[5].to_string(),
            shape: values[6].to_string(),
            font_size: parse_f64(values[7], block, row, "font_size")?,
            justify: values[8].to_string(),
            justify_v: values[9].to_string(),
        })
    }
}

// ---------------------------------------------------------------------------
// Enum string codecs
// ---------------------------------------------------------------------------

fn label_kind_str(t: LabelType) -> &'static str {
    match t {
        LabelType::Net => "local",
        LabelType::Global => "global",
        LabelType::Hierarchical => "hierarchical",
        LabelType::Power => "power",
    }
}

fn parse_label_kind(s: &str) -> LabelType {
    match s {
        "global" => LabelType::Global,
        "hierarchical" => LabelType::Hierarchical,
        "power" => LabelType::Power,
        _ => LabelType::Net,
    }
}

fn halign_str(a: HAlign) -> &'static str {
    match a {
        HAlign::Left => "left",
        HAlign::Center => "center",
        HAlign::Right => "right",
    }
}

fn parse_halign(s: &str) -> HAlign {
    match s {
        "left" => HAlign::Left,
        "right" => HAlign::Right,
        _ => HAlign::Center,
    }
}

fn valign_str(a: VAlign) -> &'static str {
    match a {
        VAlign::Top => "top",
        VAlign::Center => "center",
        VAlign::Bottom => "bottom",
    }
}

fn parse_valign(s: &str) -> VAlign {
    match s {
        "top" => VAlign::Top,
        "center" => VAlign::Center,
        _ => VAlign::Bottom,
    }
}

// ---------------------------------------------------------------------------
// Symbol ↔ row translation
// ---------------------------------------------------------------------------

pub(in crate::format) fn symbol_to_row(s: &Symbol) -> SchComponentRow {
    SchComponentRow {
        uuid: s.uuid,
        ref_des: s.reference.clone(),
        library: s.lib_id.clone(),
        pos_x: mm_to_nm(s.position.x),
        pos_y: mm_to_nm(s.position.y),
        rotation: s.rotation,
        value: s.value.clone(),
        mpn: s.fields.get("MPN").cloned().unwrap_or_default(),
    }
}

pub(in crate::format) fn row_to_symbol(row: SchComponentRow, extras: SymbolExtras) -> Symbol {
    let mut fields: std::collections::HashMap<String, String> = extras.fields.into_iter().collect();
    // LO-6: precedence is `extras.fields["MPN"] > row.mpn`. The TSV row
    // column wins ONLY when no extras-side `MPN` is set, mirroring how
    // `symbol_to_row` populates the column from `fields["MPN"]`. A
    // hand-edited file that disagrees between the two sources keeps
    // the extras value (round-tripping through symbol_to_row would
    // overwrite the row anyway, so this avoids an asymmetric "the
    // file we wrote isn't the file we read" case).
    if !row.mpn.is_empty() && !fields.contains_key("MPN") {
        fields.insert("MPN".to_string(), row.mpn.clone());
    }
    Symbol {
        uuid: row.uuid,
        lib_id: row.library,
        reference: row.ref_des,
        value: row.value,
        footprint: extras.footprint,
        datasheet: extras.datasheet,
        position: Point {
            x: nm_to_mm(row.pos_x),
            y: nm_to_mm(row.pos_y),
        },
        rotation: row.rotation,
        mirror_x: extras.mirror_x,
        mirror_y: extras.mirror_y,
        unit: extras.unit,
        is_power: extras.is_power,
        ref_text: extras.ref_text,
        val_text: extras.val_text,
        fields_autoplaced: extras.fields_autoplaced,
        fields_user_placed: extras.fields_user_placed,
        dnp: extras.dnp,
        in_bom: extras.in_bom,
        on_board: extras.on_board,
        exclude_from_sim: extras.exclude_from_sim,
        locked: extras.locked,
        fields,
        custom_properties: extras.custom_properties,
        pin_uuids: extras.pin_uuids.into_iter().collect(),
        library_id: None,
        row_id: None,
        library_version: String::new(),
        instances: extras.instances,
    }
}

pub(in crate::format) fn wire_to_row(w: &Wire) -> SchWireRow {
    SchWireRow {
        uuid: w.uuid,
        net: String::new(),
        start_x: mm_to_nm(w.start.x),
        start_y: mm_to_nm(w.start.y),
        end_x: mm_to_nm(w.end.x),
        end_y: mm_to_nm(w.end.y),
        stroke_width: w.stroke_width,
    }
}

pub(in crate::format) fn row_to_wire(row: SchWireRow) -> Wire {
    Wire {
        uuid: row.uuid,
        start: Point {
            x: nm_to_mm(row.start_x),
            y: nm_to_mm(row.start_y),
        },
        end: Point {
            x: nm_to_mm(row.end_x),
            y: nm_to_mm(row.end_y),
        },
        stroke_width: row.stroke_width,
    }
}

pub(in crate::format) fn junction_to_row(j: &Junction) -> SchJunctionRow {
    SchJunctionRow {
        uuid: j.uuid,
        pos_x: mm_to_nm(j.position.x),
        pos_y: mm_to_nm(j.position.y),
        diameter: j.diameter,
    }
}

pub(in crate::format) fn row_to_junction(row: SchJunctionRow) -> Junction {
    Junction {
        uuid: row.uuid,
        position: Point {
            x: nm_to_mm(row.pos_x),
            y: nm_to_mm(row.pos_y),
        },
        diameter: row.diameter,
    }
}

pub(in crate::format) fn label_to_row(l: &Label) -> SchLabelRow {
    SchLabelRow {
        uuid: l.uuid,
        text: l.text.clone(),
        pos_x: mm_to_nm(l.position.x),
        pos_y: mm_to_nm(l.position.y),
        rotation: l.rotation,
        kind: label_kind_str(l.label_type).to_string(),
        shape: l.shape.clone(),
        font_size: l.font_size,
        justify: halign_str(l.justify).to_string(),
        justify_v: valign_str(l.justify_v).to_string(),
    }
}

pub(in crate::format) fn row_to_label(row: SchLabelRow) -> Label {
    Label {
        uuid: row.uuid,
        text: row.text,
        position: Point {
            x: nm_to_mm(row.pos_x),
            y: nm_to_mm(row.pos_y),
        },
        rotation: row.rotation,
        label_type: parse_label_kind(&row.kind),
        shape: row.shape,
        font_size: row.font_size,
        justify: parse_halign(&row.justify),
        justify_v: parse_valign(&row.justify_v),
    }
}
