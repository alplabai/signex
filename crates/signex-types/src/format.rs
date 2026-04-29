//! Signex native file formats — `.snxsch` (schematic) and `.snxpcb` (PCB).
//!
//! Wire format: TOML envelope + TSV bulk-block pattern (matches
//! `.snxlib` / `.snxsym` / `.snxfpt` from the v0.9 library refactor).
//! The first lines of every file are a TOML manifest (`format`, IDs).
//! For each bulk entity type, a single TOML table emits a `content`
//! key whose value is a literal multi-line TSV string — the first row
//! is the column header, subsequent rows are data, columns are
//! whitespace-separated. Hierarchical or rare-field data (zone
//! polygons, the stackup, custom properties) lives in regular TOML
//! sub-tables alongside the TSV blocks.
//!
//! `.snxprj` (project) is unchanged and uses its own pre-existing
//! format. Stays as-is.
//!
//! These types are the canonical Signex schema. KiCad I/O — when it
//! returns via the `signex-kicad-import` companion repo (GPL-3.0) —
//! translates to/from these types at the file-format boundary; no
//! KiCad-shaped types live in this Apache codebase.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::pcb::{
    DrillDef, Footprint, Pad, PadNet, PadShape, PadType, PcbBoard, Point as PcbPoint, Segment, Via,
    ViaType, Zone,
};
use crate::schematic::{
    HAlign, Junction, Label, LabelType, Point, SchematicSheet, Symbol, VAlign, Wire,
};

// ---------------------------------------------------------------------------
// Format version tokens
// ---------------------------------------------------------------------------

/// Current `.snxsch` format version. Bumping this is a wire-format
/// break: older Signex versions refuse to open the file.
pub const SNXSCH_FORMAT_V1: &str = "snxsch/1";

/// Current `.snxpcb` format version.
pub const SNXPCB_FORMAT_V1: &str = "snxpcb/1";

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum FormatError {
    #[error("toml serialisation failed: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
    #[error("toml deserialisation failed: {0}")]
    TomlDeserialize(#[from] toml::de::Error),
    #[error("unsupported format version: {found:?}; this build supports {expected:?}")]
    UnsupportedVersion { found: String, expected: String },
    #[error(
        "TSV block {block:?}: header has {expected} columns ({columns:?}), data row {row} has {got}"
    )]
    TsvCellCountMismatch {
        block: String,
        row: usize,
        got: usize,
        expected: usize,
        columns: Vec<String>,
    },
    #[error("TSV block {block:?}: header columns {got:?} do not match expected {expected:?}")]
    TsvHeaderMismatch {
        block: String,
        got: Vec<String>,
        expected: Vec<String>,
    },
    #[error("TSV block {block:?}: row {row} field {field:?} parse error: {message}")]
    TsvFieldParse {
        block: String,
        row: usize,
        field: String,
        message: String,
    },
    #[error("TSV block {block:?}: TSV body is empty — at minimum a header row is required")]
    TsvEmpty { block: String },
}

// ---------------------------------------------------------------------------
// SnxTable trait — manual row schemas (no derive macro for simplicity).
// ---------------------------------------------------------------------------

/// A row schema for TSV bulk blocks.
///
/// Implementors describe the TSV column order, how to render an
/// in-memory row to its column-cell strings, and how to parse a vector
/// of cell `&str` slices back into a row.
pub trait SnxTable: Sized {
    /// Static column ordering — what gets emitted as the TSV header
    /// row and what `from_row` expects to receive back.
    fn columns() -> &'static [&'static str];

    /// Emit one cell per declared column. Length must equal
    /// [`SnxTable::columns`]. Empty cells use `""`.
    fn to_row(&self) -> Vec<String>;

    /// Parse a row of cell strings (length matches [`SnxTable::columns`]).
    /// Implementations report parse failures via [`FormatError::TsvFieldParse`].
    fn from_row(values: &[&str], block: &str, row: usize) -> Result<Self, FormatError>;
}

// ---------------------------------------------------------------------------
// TSV reader / writer
// ---------------------------------------------------------------------------

/// Encode a single TSV cell. Empty strings emit `""` so column
/// boundaries stay legible when split on whitespace.
fn encode_cell(cell: &str) -> String {
    if cell.is_empty() {
        "\"\"".to_string()
    } else if cell.contains(char::is_whitespace) || cell.contains('"') {
        // Quote anything containing whitespace or a quote so the
        // whitespace-flexible parser can still recover the original
        // value. Inner quotes are doubled, matching CSV convention.
        let escaped = cell.replace('"', "\"\"");
        format!("\"{escaped}\"")
    } else {
        cell.to_string()
    }
}

/// Decode a single TSV cell. `""` returns empty, surrounding double
/// quotes strip with inner `""` collapsing back to `"`. Bare `-` is
/// also treated as empty per the format spec.
fn decode_cell(cell: &str) -> String {
    if cell == "\"\"" || cell == "-" {
        return String::new();
    }
    if cell.starts_with('"') && cell.ends_with('"') && cell.len() >= 2 {
        let inner = &cell[1..cell.len() - 1];
        return inner.replace("\"\"", "\"");
    }
    cell.to_string()
}

/// Split a TSV row on whitespace, honouring `"` quoting so cells
/// containing spaces are kept atomic.
fn split_row(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut buf = String::new();
    let mut in_quotes = false;
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if in_quotes {
            buf.push(c);
            if c == '"' {
                if i + 1 < chars.len() && chars[i + 1] == '"' {
                    // doubled quote — keep advancing, leave to decode_cell
                    buf.push('"');
                    i += 2;
                    continue;
                }
                in_quotes = false;
            }
            i += 1;
        } else if c == '"' {
            buf.push(c);
            in_quotes = true;
            i += 1;
        } else if c.is_whitespace() {
            if !buf.is_empty() {
                cells.push(std::mem::take(&mut buf));
            }
            i += 1;
        } else {
            buf.push(c);
            i += 1;
        }
    }
    if !buf.is_empty() {
        cells.push(buf);
    }
    cells
}

/// Write a TSV block: header row + one row per item, columns aligned
/// to the longest cell per column for legibility (matches the
/// `.snxlib` writer's whitespace-flexible style).
pub fn write_tsv_block<R: SnxTable>(rows: &[R]) -> String {
    let columns = R::columns();
    if columns.is_empty() {
        return String::new();
    }

    // Pre-compute every row's encoded cell so we can pad to the
    // widest column.
    let mut all_rows: Vec<Vec<String>> = Vec::with_capacity(rows.len() + 1);
    all_rows.push(columns.iter().map(|c| (*c).to_string()).collect());
    for row in rows {
        let cells = row.to_row();
        debug_assert_eq!(
            cells.len(),
            columns.len(),
            "to_row produced {} cells but columns has {}",
            cells.len(),
            columns.len()
        );
        all_rows.push(cells.into_iter().map(|c| encode_cell(&c)).collect());
    }

    // Compute per-column widths. Two-space separator between columns.
    let mut widths = vec![0usize; columns.len()];
    for row in &all_rows {
        for (idx, cell) in row.iter().enumerate() {
            if idx < widths.len() && cell.chars().count() > widths[idx] {
                widths[idx] = cell.chars().count();
            }
        }
    }

    let mut out = String::new();
    for row in &all_rows {
        for (idx, cell) in row.iter().enumerate() {
            if idx > 0 {
                out.push_str("  ");
            }
            // Don't pad the last column — avoids trailing spaces.
            if idx + 1 == row.len() {
                out.push_str(cell);
            } else {
                let pad = widths[idx].saturating_sub(cell.chars().count());
                out.push_str(cell);
                for _ in 0..pad {
                    out.push(' ');
                }
            }
        }
        out.push('\n');
    }
    out
}

/// Parse a TSV block: validate the header against `R::columns()`,
/// then parse each data row through `R::from_row`.
pub fn parse_tsv_block<R: SnxTable>(block: &str, content: &str) -> Result<Vec<R>, FormatError> {
    // Strip the leading/trailing newlines TOML's literal multi-line
    // string padding adds, but preserve interior newlines.
    let trimmed = content.trim_matches('\n');
    if trimmed.trim().is_empty() {
        return Err(FormatError::TsvEmpty {
            block: block.to_string(),
        });
    }

    let mut lines = trimmed.split('\n').filter(|l| !l.trim().is_empty());
    let header_line = lines.next().ok_or_else(|| FormatError::TsvEmpty {
        block: block.to_string(),
    })?;
    let header_cells = split_row(header_line);
    let expected: Vec<String> = R::columns().iter().map(|c| (*c).to_string()).collect();
    if header_cells != expected {
        return Err(FormatError::TsvHeaderMismatch {
            block: block.to_string(),
            got: header_cells,
            expected,
        });
    }

    let mut rows = Vec::new();
    for (idx, line) in lines.enumerate() {
        let cells = split_row(line);
        if cells.len() != expected.len() {
            return Err(FormatError::TsvCellCountMismatch {
                block: block.to_string(),
                row: idx,
                got: cells.len(),
                expected: expected.len(),
                columns: expected,
            });
        }
        let decoded: Vec<String> = cells.iter().map(|c| decode_cell(c)).collect();
        let refs: Vec<&str> = decoded.iter().map(String::as_str).collect();
        rows.push(R::from_row(&refs, block, idx)?);
    }
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Field-level parse helpers
// ---------------------------------------------------------------------------

fn parse_i64(value: &str, block: &str, row: usize, field: &str) -> Result<i64, FormatError> {
    value.parse().map_err(|e: std::num::ParseIntError| {
        FormatError::TsvFieldParse {
            block: block.to_string(),
            row,
            field: field.to_string(),
            message: e.to_string(),
        }
    })
}

fn parse_f64(value: &str, block: &str, row: usize, field: &str) -> Result<f64, FormatError> {
    if value.is_empty() {
        return Ok(0.0);
    }
    value.parse().map_err(|e: std::num::ParseFloatError| {
        FormatError::TsvFieldParse {
            block: block.to_string(),
            row,
            field: field.to_string(),
            message: e.to_string(),
        }
    })
}

fn parse_uuid(value: &str, block: &str, row: usize, field: &str) -> Result<Uuid, FormatError> {
    if value.is_empty() {
        return Ok(Uuid::nil());
    }
    Uuid::parse_str(value).map_err(|e| FormatError::TsvFieldParse {
        block: block.to_string(),
        row,
        field: field.to_string(),
        message: e.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Coordinate convention
// ---------------------------------------------------------------------------
//
// On disk, schematic and PCB positions are emitted as integer
// nanometres so the wire format is precision-stable across hand
// edits. In memory, `Point` is `f64` mm. The conversion happens at
// the row-row boundary: `mm_to_nm` / `nm_to_mm`.

const MM_PER_NM: f64 = 1.0e-6;
const NM_PER_MM: f64 = 1.0e6;

fn mm_to_nm(mm: f64) -> i64 {
    (mm * NM_PER_MM).round() as i64
}

fn nm_to_mm(nm: i64) -> f64 {
    (nm as f64) * MM_PER_NM
}

// ---------------------------------------------------------------------------
// Schematic adapter rows
// ---------------------------------------------------------------------------

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
// PCB adapter rows
// ---------------------------------------------------------------------------

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
// Helpers
// ---------------------------------------------------------------------------

/// Format an `f64` for TSV: trailing zeros stripped to keep diffs
/// minimal. Whole numbers emit as `0` rather than `0.0`.
fn format_f64(f: f64) -> String {
    if f == 0.0 {
        return "0".to_string();
    }
    if f.fract() == 0.0 && f.abs() < 1e15 {
        return format!("{}", f as i64);
    }
    let s = format!("{f}");
    s
}

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
// .snxsch — schematic file
// ---------------------------------------------------------------------------

/// On-disk representation of a `.snxsch` file.
///
/// Internally constructed from a [`SchematicSheet`] via
/// [`SnxSchematic::new`] (which decomposes the sheet into bulk TSV
/// rows + an extras-TOML auxiliary table that captures every field
/// the row schema doesn't cover) and rebuilt via [`SnxSchematic::parse`]
/// (which round-trips back to a fully-populated [`SchematicSheet`]).
///
/// Callers that just want the in-memory sheet read `self.sheet`.
#[derive(Debug, Clone)]
pub struct SnxSchematic {
    pub format: String,
    /// The reconstituted in-memory sheet. This is what callers
    /// consume; the TOML+TSV decomposition only matters at the disk
    /// boundary.
    pub sheet: SchematicSheet,
}

impl SnxSchematic {
    /// Wrap a `SchematicSheet` for serialisation as the current format version.
    pub fn new(sheet: SchematicSheet) -> Self {
        Self {
            format: SNXSCH_FORMAT_V1.to_string(),
            sheet,
        }
    }

    /// Serialise to a TOML+TSV string for writing to disk.
    pub fn write_string(&self) -> Result<String, FormatError> {
        let mut out = String::new();

        // Manifest header — emit as a small TOML document.
        let manifest = SchManifest {
            format: self.format.clone(),
            schematic_id: self.sheet.uuid,
            version: self.sheet.version,
            generator: self.sheet.generator.clone(),
            generator_version: self.sheet.generator_version.clone(),
            paper_size: self.sheet.paper_size.clone(),
            root_sheet_page: self.sheet.root_sheet_page.clone(),
        };
        out.push_str(&toml::to_string_pretty(&manifest)?);
        out.push('\n');

        // Bulk TSV blocks. Each is `[sheets.<entity>]` with a single
        // `content` key holding a literal multi-line string.
        let component_rows: Vec<SchComponentRow> =
            self.sheet.symbols.iter().map(symbol_to_row).collect();
        let wire_rows: Vec<SchWireRow> = self.sheet.wires.iter().map(wire_to_row).collect();
        let junction_rows: Vec<SchJunctionRow> = self
            .sheet
            .junctions
            .iter()
            .map(junction_to_row)
            .collect();
        let label_rows: Vec<SchLabelRow> = self.sheet.labels.iter().map(label_to_row).collect();

        write_tsv_section(&mut out, "sheets.components", &component_rows);
        write_tsv_section(&mut out, "sheets.wires", &wire_rows);
        write_tsv_section(&mut out, "sheets.junctions", &junction_rows);
        write_tsv_section(&mut out, "sheets.labels", &label_rows);

        // Extras: auxiliary TOML tables for fields the bulk row
        // doesn't carry. Wrap the whole extras tree in a single
        // serializable struct so toml::to_string_pretty emits the
        // correct nested-table headers (`[extras.symbols.<uuid>]`,
        // `[extras.symbols.<uuid>.fields]`, `[extras.sheet]`, …).
        // Hand-rolled per-section serialization breaks here because
        // the inner HashMaps render as their own `[fields]` sub-
        // tables which would attach to the wrong parent path.
        let symbols_extras: BTreeMap<String, SymbolExtras> = self
            .sheet
            .symbols
            .iter()
            .map(|s| (s.uuid.to_string(), SymbolExtras::from_symbol(s)))
            .filter(|(_, e)| !e.is_default())
            .collect();

        let sheet_extras = SheetExtras::from_sheet(&self.sheet);
        let sheet_extras_opt = if sheet_extras.is_default() {
            None
        } else {
            Some(sheet_extras)
        };

        if !symbols_extras.is_empty() || sheet_extras_opt.is_some() {
            #[derive(Serialize)]
            struct ExtrasWrapper {
                extras: ExtrasInner,
            }
            #[derive(Serialize)]
            struct ExtrasInner {
                #[serde(skip_serializing_if = "BTreeMap::is_empty")]
                symbols: BTreeMap<String, SymbolExtras>,
                #[serde(skip_serializing_if = "Option::is_none")]
                sheet: Option<SheetExtras>,
            }
            let body = toml::to_string_pretty(&ExtrasWrapper {
                extras: ExtrasInner {
                    symbols: symbols_extras,
                    sheet: sheet_extras_opt,
                },
            })?;
            out.push('\n');
            out.push_str(&body);
        }

        Ok(out)
    }

    /// Parse a TOML+TSV string from disk.
    pub fn parse(input: &str) -> Result<Self, FormatError> {
        // Stage 1: deserialise the document into the raw envelope —
        // manifest header + bulk blocks + extras.
        let raw: SchRaw = toml::from_str(input)?;

        if raw.format != SNXSCH_FORMAT_V1 {
            return Err(FormatError::UnsupportedVersion {
                found: raw.format,
                expected: SNXSCH_FORMAT_V1.to_string(),
            });
        }

        // Stage 2: parse each TSV block into adapter rows.
        let component_rows = match raw.sheets.components {
            Some(b) => parse_tsv_block::<SchComponentRow>("sheets.components", &b.content)?,
            None => Vec::new(),
        };
        let wire_rows = match raw.sheets.wires {
            Some(b) => parse_tsv_block::<SchWireRow>("sheets.wires", &b.content)?,
            None => Vec::new(),
        };
        let junction_rows = match raw.sheets.junctions {
            Some(b) => parse_tsv_block::<SchJunctionRow>("sheets.junctions", &b.content)?,
            None => Vec::new(),
        };
        let label_rows = match raw.sheets.labels {
            Some(b) => parse_tsv_block::<SchLabelRow>("sheets.labels", &b.content)?,
            None => Vec::new(),
        };

        // Stage 3: rebuild the SchematicSheet from rows + extras.
        let extras = raw.extras.unwrap_or_default();
        let sheet_extras = extras.sheet.unwrap_or_default();
        let symbols = component_rows
            .into_iter()
            .map(|row| {
                let key = row.uuid.to_string();
                let extra = extras.symbols.get(&key).cloned().unwrap_or_default();
                row_to_symbol(row, extra)
            })
            .collect();
        let wires = wire_rows.into_iter().map(row_to_wire).collect();
        let junctions = junction_rows.into_iter().map(row_to_junction).collect();
        let labels = label_rows.into_iter().map(row_to_label).collect();

        let sheet = SchematicSheet {
            uuid: raw.schematic_id,
            version: raw.version,
            generator: raw.generator,
            generator_version: raw.generator_version,
            paper_size: raw.paper_size,
            root_sheet_page: raw.root_sheet_page,
            symbols,
            wires,
            junctions,
            labels,
            child_sheets: sheet_extras.child_sheets,
            no_connects: sheet_extras.no_connects,
            text_notes: sheet_extras.text_notes,
            buses: sheet_extras.buses,
            bus_entries: sheet_extras.bus_entries,
            drawings: sheet_extras.drawings,
            no_erc_directives: sheet_extras.no_erc_directives,
            title_block: sheet_extras.title_block,
            lib_symbols: sheet_extras.lib_symbols,
        };

        Ok(SnxSchematic {
            format: raw.format,
            sheet,
        })
    }
}

fn write_tsv_section<R: SnxTable>(out: &mut String, name: &str, rows: &[R]) {
    let body = write_tsv_block(rows);
    out.push_str(&format!("\n[{name}]\n"));
    out.push_str("content = \"\"\"\n");
    out.push_str(&body);
    out.push_str("\"\"\"\n");
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SchManifest {
    format: String,
    schematic_id: Uuid,
    #[serde(default)]
    version: u32,
    #[serde(default)]
    generator: String,
    #[serde(default)]
    generator_version: String,
    #[serde(default)]
    paper_size: String,
    #[serde(default)]
    root_sheet_page: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct TsvBody {
    content: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct SchSheetsRaw {
    #[serde(default)]
    components: Option<TsvBody>,
    #[serde(default)]
    wires: Option<TsvBody>,
    #[serde(default)]
    junctions: Option<TsvBody>,
    #[serde(default)]
    labels: Option<TsvBody>,
}

#[derive(Debug, Clone, Deserialize)]
struct SchRaw {
    format: String,
    schematic_id: Uuid,
    #[serde(default)]
    version: u32,
    #[serde(default)]
    generator: String,
    #[serde(default)]
    generator_version: String,
    #[serde(default)]
    paper_size: String,
    #[serde(default)]
    root_sheet_page: String,
    #[serde(default)]
    sheets: SchSheetsRaw,
    #[serde(default)]
    extras: Option<SchExtrasRaw>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct SchExtrasRaw {
    #[serde(default)]
    symbols: BTreeMap<String, SymbolExtras>,
    #[serde(default)]
    sheet: Option<SheetExtras>,
}

/// Per-symbol auxiliary fields that don't fit into [`SchComponentRow`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct SymbolExtras {
    #[serde(default)]
    footprint: String,
    #[serde(default)]
    datasheet: String,
    #[serde(default)]
    mirror_x: bool,
    #[serde(default)]
    mirror_y: bool,
    #[serde(default = "default_unit")]
    unit: u32,
    #[serde(default)]
    is_power: bool,
    #[serde(default)]
    fields_autoplaced: bool,
    #[serde(default)]
    dnp: bool,
    #[serde(default = "default_true")]
    in_bom: bool,
    #[serde(default = "default_true")]
    on_board: bool,
    #[serde(default)]
    exclude_from_sim: bool,
    #[serde(default)]
    locked: bool,
    #[serde(default)]
    fields: std::collections::HashMap<String, String>,
    #[serde(default)]
    custom_properties: Vec<crate::property::SchematicProperty>,
    #[serde(default)]
    pin_uuids: std::collections::HashMap<String, Uuid>,
    #[serde(default)]
    instances: Vec<crate::schematic::SymbolInstance>,
    #[serde(default)]
    ref_text: Option<crate::schematic::TextProp>,
    #[serde(default)]
    val_text: Option<crate::schematic::TextProp>,
}

impl SymbolExtras {
    fn is_default(&self) -> bool {
        self.footprint.is_empty()
            && self.datasheet.is_empty()
            && !self.mirror_x
            && !self.mirror_y
            && self.unit == 1
            && !self.is_power
            && !self.fields_autoplaced
            && !self.dnp
            && self.in_bom
            && self.on_board
            && !self.exclude_from_sim
            && !self.locked
            && self.fields.is_empty()
            && self.custom_properties.is_empty()
            && self.pin_uuids.is_empty()
            && self.instances.is_empty()
            && self.ref_text.is_none()
            && self.val_text.is_none()
    }

    fn from_symbol(s: &Symbol) -> Self {
        SymbolExtras {
            footprint: s.footprint.clone(),
            datasheet: s.datasheet.clone(),
            mirror_x: s.mirror_x,
            mirror_y: s.mirror_y,
            unit: s.unit,
            is_power: s.is_power,
            fields_autoplaced: s.fields_autoplaced,
            dnp: s.dnp,
            in_bom: s.in_bom,
            on_board: s.on_board,
            exclude_from_sim: s.exclude_from_sim,
            locked: s.locked,
            fields: s.fields.clone(),
            custom_properties: s.custom_properties.clone(),
            pin_uuids: s.pin_uuids.clone(),
            instances: s.instances.clone(),
            ref_text: s.ref_text.clone(),
            val_text: s.val_text.clone(),
        }
    }
}

/// Fields on [`SchematicSheet`] that aren't yet TSV-tabularised
/// (rare in real designs, hierarchical or schema-rich): hierarchical
/// child sheets, no-connect markers, text notes, buses, bus entries,
/// drawing primitives, no-ERC directives, title block, and library
/// symbol cache.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct SheetExtras {
    #[serde(default)]
    child_sheets: Vec<crate::schematic::ChildSheet>,
    #[serde(default)]
    no_connects: Vec<crate::schematic::NoConnect>,
    #[serde(default)]
    text_notes: Vec<crate::schematic::TextNote>,
    #[serde(default)]
    buses: Vec<crate::schematic::Bus>,
    #[serde(default)]
    bus_entries: Vec<crate::schematic::BusEntry>,
    #[serde(default)]
    drawings: Vec<crate::schematic::SchDrawing>,
    #[serde(default)]
    no_erc_directives: Vec<crate::schematic::NoConnect>,
    #[serde(default)]
    title_block: std::collections::HashMap<String, String>,
    #[serde(default)]
    lib_symbols: std::collections::HashMap<String, crate::schematic::LibSymbol>,
}

impl SheetExtras {
    fn is_default(&self) -> bool {
        self.child_sheets.is_empty()
            && self.no_connects.is_empty()
            && self.text_notes.is_empty()
            && self.buses.is_empty()
            && self.bus_entries.is_empty()
            && self.drawings.is_empty()
            && self.no_erc_directives.is_empty()
            && self.title_block.is_empty()
            && self.lib_symbols.is_empty()
    }

    fn from_sheet(s: &SchematicSheet) -> Self {
        SheetExtras {
            child_sheets: s.child_sheets.clone(),
            no_connects: s.no_connects.clone(),
            text_notes: s.text_notes.clone(),
            buses: s.buses.clone(),
            bus_entries: s.bus_entries.clone(),
            drawings: s.drawings.clone(),
            no_erc_directives: s.no_erc_directives.clone(),
            title_block: s.title_block.clone(),
            lib_symbols: s.lib_symbols.clone(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_unit() -> u32 {
    1
}

// ---------------------------------------------------------------------------
// Symbol ↔ row translation
// ---------------------------------------------------------------------------

fn symbol_to_row(s: &Symbol) -> SchComponentRow {
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

fn row_to_symbol(row: SchComponentRow, extras: SymbolExtras) -> Symbol {
    let mut fields = extras.fields.clone();
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
        dnp: extras.dnp,
        in_bom: extras.in_bom,
        on_board: extras.on_board,
        exclude_from_sim: extras.exclude_from_sim,
        locked: extras.locked,
        fields,
        custom_properties: extras.custom_properties,
        pin_uuids: extras.pin_uuids,
        instances: extras.instances,
    }
}

fn wire_to_row(w: &Wire) -> SchWireRow {
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

fn row_to_wire(row: SchWireRow) -> Wire {
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

fn junction_to_row(j: &Junction) -> SchJunctionRow {
    SchJunctionRow {
        uuid: j.uuid,
        pos_x: mm_to_nm(j.position.x),
        pos_y: mm_to_nm(j.position.y),
        diameter: j.diameter,
    }
}

fn row_to_junction(row: SchJunctionRow) -> Junction {
    Junction {
        uuid: row.uuid,
        position: Point {
            x: nm_to_mm(row.pos_x),
            y: nm_to_mm(row.pos_y),
        },
        diameter: row.diameter,
    }
}

fn label_to_row(l: &Label) -> SchLabelRow {
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

fn row_to_label(row: SchLabelRow) -> Label {
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

// ---------------------------------------------------------------------------
// .snxpcb — PCB file
// ---------------------------------------------------------------------------

/// On-disk representation of a `.snxpcb` file.
///
/// Same shape as [`SnxSchematic`]: TOML manifest at the top, bulk
/// TSV blocks for footprints / pads / tracks / vias, regular TOML
/// for hierarchical or rare-field data (zone polygons, the stackup,
/// custom properties).
#[derive(Debug, Clone)]
pub struct SnxPcb {
    pub format: String,
    pub board: PcbBoard,
}

impl SnxPcb {
    /// Wrap a `PcbBoard` for serialisation as the current format version.
    pub fn new(board: PcbBoard) -> Self {
        Self {
            format: SNXPCB_FORMAT_V1.to_string(),
            board,
        }
    }

    /// Serialise to a TOML+TSV string for writing to disk.
    pub fn write_string(&self) -> Result<String, FormatError> {
        let mut out = String::new();

        // Manifest header.
        let manifest = PcbManifest {
            format: self.format.clone(),
            pcb_id: self.board.uuid,
            version: self.board.version,
            generator: self.board.generator.clone(),
            thickness: self.board.thickness,
        };
        out.push_str(&toml::to_string_pretty(&manifest)?);
        out.push('\n');

        // Stackup + setup + nets — emitted as one nested wrapper so
        // toml's serializer produces the correct `[stackup]`/`[nets]`
        // headers and any inner sub-tables (PcbSetup) attach to the
        // right parent path.
        if !self.board.layers.is_empty() || !self.board.nets.is_empty() {
            #[derive(Serialize)]
            struct StackupWrapper<'a> {
                #[serde(skip_serializing_if = "Option::is_none")]
                stackup: Option<StackBlock<'a>>,
                #[serde(skip_serializing_if = "Option::is_none")]
                nets: Option<NetsBlock<'a>>,
            }
            #[derive(Serialize)]
            struct StackBlock<'a> {
                layers: Vec<String>,
                #[serde(skip_serializing_if = "Option::is_none")]
                setup: Option<&'a crate::pcb::PcbSetup>,
            }
            #[derive(Serialize)]
            struct NetsBlock<'a> {
                entries: &'a [crate::pcb::NetDef],
            }
            let stackup = if self.board.layers.is_empty() {
                None
            } else {
                Some(StackBlock {
                    layers: self.board.layers.iter().map(|l| l.name.clone()).collect(),
                    setup: self.board.setup.as_ref(),
                })
            };
            let nets = if self.board.nets.is_empty() {
                None
            } else {
                Some(NetsBlock {
                    entries: &self.board.nets,
                })
            };
            out.push('\n');
            out.push_str(&toml::to_string_pretty(&StackupWrapper {
                stackup,
                nets,
            })?);
        }

        // TSV blocks: footprints, pads, tracks, vias.
        let footprint_rows: Vec<PcbFootprintRow> = self
            .board
            .footprints
            .iter()
            .map(footprint_to_row)
            .collect();
        let mut pad_rows: Vec<PcbPadRow> = Vec::new();
        for fp in &self.board.footprints {
            for pad in &fp.pads {
                pad_rows.push(pad_to_row(pad, &fp.reference));
            }
        }
        let track_rows: Vec<PcbTrackRow> = self.board.segments.iter().map(track_to_row).collect();
        let via_rows: Vec<PcbViaRow> = self.board.vias.iter().map(via_to_row).collect();

        write_tsv_section(&mut out, "footprints", &footprint_rows);
        write_tsv_section(&mut out, "pads", &pad_rows);
        write_tsv_section(&mut out, "tracks", &track_rows);
        write_tsv_section(&mut out, "vias", &via_rows);

        // Zones — full struct array (each zone serialises naturally
        // under `[[zones]]` with its own uuid scalar).
        if !self.board.zones.is_empty() {
            #[derive(Serialize)]
            struct ZonesWrapper<'a> {
                zones: &'a [Zone],
            }
            out.push('\n');
            out.push_str(&toml::to_string_pretty(&ZonesWrapper {
                zones: &self.board.zones,
            })?);
        }

        // Footprint extras / pad extras / board extras — wrap in a
        // single `[extras]` tree so toml's serializer produces the
        // correct nested-table headers.
        let extras = PcbExtras::from_board(&self.board);
        let footprints = extras.footprints;
        let pads = extras.pads;
        let board_extras = if extras.outline.is_empty()
            && extras.graphics.is_empty()
            && extras.texts.is_empty()
        {
            None
        } else {
            Some(BoardExtras {
                outline: extras.outline,
                graphics: extras.graphics,
                texts: extras.texts,
            })
        };
        if !footprints.is_empty() || !pads.is_empty() || board_extras.is_some() {
            #[derive(Serialize)]
            struct ExtrasWrapper {
                extras: ExtrasInner,
            }
            #[derive(Serialize)]
            struct ExtrasInner {
                #[serde(skip_serializing_if = "BTreeMap::is_empty")]
                footprints: BTreeMap<String, FootprintExtras>,
                #[serde(skip_serializing_if = "BTreeMap::is_empty")]
                pads: BTreeMap<String, PadExtras>,
                #[serde(skip_serializing_if = "Option::is_none")]
                board: Option<BoardExtras>,
            }
            out.push('\n');
            out.push_str(&toml::to_string_pretty(&ExtrasWrapper {
                extras: ExtrasInner {
                    footprints,
                    pads,
                    board: board_extras,
                },
            })?);
        }

        Ok(out)
    }

    /// Parse a TOML+TSV string from disk.
    pub fn parse(input: &str) -> Result<Self, FormatError> {
        let raw: PcbRaw = toml::from_str(input)?;

        if raw.format != SNXPCB_FORMAT_V1 {
            return Err(FormatError::UnsupportedVersion {
                found: raw.format,
                expected: SNXPCB_FORMAT_V1.to_string(),
            });
        }

        let footprint_rows = match raw.footprints {
            Some(b) => parse_tsv_block::<PcbFootprintRow>("footprints", &b.content)?,
            None => Vec::new(),
        };
        let pad_rows = match raw.pads {
            Some(b) => parse_tsv_block::<PcbPadRow>("pads", &b.content)?,
            None => Vec::new(),
        };
        let track_rows = match raw.tracks {
            Some(b) => parse_tsv_block::<PcbTrackRow>("tracks", &b.content)?,
            None => Vec::new(),
        };
        let via_rows = match raw.vias {
            Some(b) => parse_tsv_block::<PcbViaRow>("vias", &b.content)?,
            None => Vec::new(),
        };

        // Reconstruct footprints with their pads. Group pads by ref.
        let extras = raw.extras.unwrap_or_default();
        let board_extras = extras.board.unwrap_or_default();

        let mut footprints: Vec<Footprint> = footprint_rows
            .into_iter()
            .map(|row| {
                let key = row.uuid.to_string();
                let extra = extras.footprints.get(&key).cloned().unwrap_or_default();
                row_to_footprint(row, extra)
            })
            .collect();

        for prow in pad_rows {
            let extra = extras.pads.get(&prow.uuid.to_string()).cloned().unwrap_or_default();
            let pad = row_to_pad(prow.clone(), extra);
            // attach to footprint by ref
            if let Some(fp) = footprints.iter_mut().find(|f| f.reference == prow.footprint_ref) {
                fp.pads.push(pad);
            } else {
                // Orphan pad — preserve as a synthetic footprint
                // entry to avoid silent data loss.
                footprints.push(Footprint {
                    uuid: Uuid::nil(),
                    reference: prow.footprint_ref.clone(),
                    value: String::new(),
                    footprint_id: String::new(),
                    position: PcbPoint { x: 0.0, y: 0.0 },
                    rotation: 0.0,
                    layer: String::new(),
                    locked: false,
                    pads: vec![pad],
                    graphics: Vec::new(),
                    properties: Vec::new(),
                });
            }
        }

        let segments = track_rows.into_iter().map(row_to_track).collect();
        let vias = via_rows.into_iter().map(row_to_via).collect();

        // Reconstitute zones from `[[zones]]` array.
        let zones = raw.zones.unwrap_or_default();

        // Stackup → layers list (rebuild LayerDef with synthetic ids).
        let (layers, setup) = match raw.stackup {
            Some(s) => {
                let layers: Vec<crate::pcb::LayerDef> = s
                    .layers
                    .into_iter()
                    .enumerate()
                    .map(|(i, name)| crate::pcb::LayerDef {
                        id: i as u8,
                        name,
                        layer_type: String::new(),
                    })
                    .collect();
                (layers, s.setup)
            }
            None => (Vec::new(), None),
        };

        let board = PcbBoard {
            uuid: raw.pcb_id,
            version: raw.version,
            generator: raw.generator,
            thickness: raw.thickness,
            outline: board_extras.outline,
            layers,
            setup,
            nets: raw.nets.map(|n| n.entries).unwrap_or_default(),
            footprints,
            segments,
            vias,
            zones,
            graphics: board_extras.graphics,
            texts: board_extras.texts,
        };

        Ok(SnxPcb {
            format: raw.format,
            board,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PcbManifest {
    format: String,
    pcb_id: Uuid,
    #[serde(default)]
    version: u32,
    #[serde(default)]
    generator: String,
    #[serde(default)]
    thickness: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct PcbRaw {
    format: String,
    pcb_id: Uuid,
    #[serde(default)]
    version: u32,
    #[serde(default)]
    generator: String,
    #[serde(default)]
    thickness: f64,
    #[serde(default)]
    stackup: Option<StackRaw>,
    #[serde(default)]
    nets: Option<NetsRaw>,
    #[serde(default)]
    footprints: Option<TsvBody>,
    #[serde(default)]
    pads: Option<TsvBody>,
    #[serde(default)]
    tracks: Option<TsvBody>,
    #[serde(default)]
    vias: Option<TsvBody>,
    #[serde(default)]
    zones: Option<Vec<Zone>>,
    #[serde(default)]
    extras: Option<PcbExtrasRaw>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct StackRaw {
    #[serde(default)]
    layers: Vec<String>,
    #[serde(default)]
    setup: Option<crate::pcb::PcbSetup>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct NetsRaw {
    #[serde(default)]
    entries: Vec<crate::pcb::NetDef>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct PcbExtrasRaw {
    #[serde(default)]
    footprints: BTreeMap<String, FootprintExtras>,
    #[serde(default)]
    pads: BTreeMap<String, PadExtras>,
    #[serde(default)]
    board: Option<BoardExtras>,
}

struct PcbExtras {
    footprints: BTreeMap<String, FootprintExtras>,
    pads: BTreeMap<String, PadExtras>,
    outline: Vec<PcbPoint>,
    graphics: Vec<crate::pcb::BoardGraphic>,
    texts: Vec<crate::pcb::BoardText>,
}

impl PcbExtras {
    fn from_board(board: &PcbBoard) -> Self {
        let mut footprints = BTreeMap::new();
        let mut pads = BTreeMap::new();
        for fp in &board.footprints {
            let fpe = FootprintExtras::from_footprint(fp);
            if !fpe.is_default() {
                footprints.insert(fp.uuid.to_string(), fpe);
            }
            for pad in &fp.pads {
                let pe = PadExtras::from_pad(pad);
                if !pe.is_default() {
                    pads.insert(pad.uuid.to_string(), pe);
                }
            }
        }
        PcbExtras {
            footprints,
            pads,
            outline: board.outline.clone(),
            graphics: board.graphics.clone(),
            texts: board.texts.clone(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct FootprintExtras {
    #[serde(default)]
    footprint_id: String,
    #[serde(default)]
    locked: bool,
    #[serde(default)]
    graphics: Vec<crate::pcb::FpGraphic>,
    #[serde(default)]
    properties: Vec<crate::property::PcbProperty>,
}

impl FootprintExtras {
    fn is_default(&self) -> bool {
        self.footprint_id.is_empty()
            && !self.locked
            && self.graphics.is_empty()
            && self.properties.is_empty()
    }

    fn from_footprint(fp: &Footprint) -> Self {
        FootprintExtras {
            footprint_id: fp.footprint_id.clone(),
            locked: fp.locked,
            graphics: fp.graphics.clone(),
            properties: fp.properties.clone(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PadExtras {
    #[serde(default)]
    drill_shape: String,
}

impl PadExtras {
    fn is_default(&self) -> bool {
        self.drill_shape.is_empty()
    }

    fn from_pad(pad: &Pad) -> Self {
        PadExtras {
            drill_shape: pad
                .drill
                .as_ref()
                .map(|d| d.shape.clone())
                .unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct BoardExtras {
    #[serde(default)]
    outline: Vec<PcbPoint>,
    #[serde(default)]
    graphics: Vec<crate::pcb::BoardGraphic>,
    #[serde(default)]
    texts: Vec<crate::pcb::BoardText>,
}

// ---------------------------------------------------------------------------
// Footprint / Pad / Segment / Via translation helpers
// ---------------------------------------------------------------------------

fn footprint_to_row(fp: &Footprint) -> PcbFootprintRow {
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

fn row_to_footprint(row: PcbFootprintRow, extras: FootprintExtras) -> Footprint {
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

fn pad_to_row(pad: &Pad, footprint_ref: &str) -> PcbPadRow {
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

fn row_to_pad(row: PcbPadRow, extras: PadExtras) -> Pad {
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

fn track_to_row(s: &Segment) -> PcbTrackRow {
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

fn row_to_track(row: PcbTrackRow) -> Segment {
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

fn via_to_row(v: &Via) -> PcbViaRow {
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

fn row_to_via(row: PcbViaRow) -> Via {
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schematic::{LabelType as LType, Point as SchPoint};
    use uuid::Uuid;

    fn empty_sheet() -> SchematicSheet {
        SchematicSheet {
            uuid: Uuid::nil(),
            version: 1,
            generator: "signex-test".into(),
            generator_version: "0.9".into(),
            paper_size: "A4".into(),
            root_sheet_page: "1".into(),
            symbols: vec![],
            wires: vec![],
            junctions: vec![],
            labels: vec![],
            child_sheets: vec![],
            no_connects: vec![],
            text_notes: vec![],
            buses: vec![],
            bus_entries: vec![],
            drawings: vec![],
            no_erc_directives: vec![],
            title_block: Default::default(),
            lib_symbols: Default::default(),
        }
    }

    fn empty_board() -> PcbBoard {
        PcbBoard {
            uuid: Uuid::nil(),
            version: 1,
            generator: "signex-test".into(),
            thickness: 1.6,
            outline: vec![],
            layers: vec![],
            setup: None,
            nets: vec![],
            footprints: vec![],
            segments: vec![],
            vias: vec![],
            zones: vec![],
            graphics: vec![],
            texts: vec![],
        }
    }

    #[test]
    fn snxsch_round_trip_empty() {
        let snx = SnxSchematic::new(empty_sheet());
        let s = snx.write_string().expect("serialise");
        assert!(s.contains("format = \"snxsch/1\""));
        let back = SnxSchematic::parse(&s).expect("round-trip");
        assert_eq!(back.format, SNXSCH_FORMAT_V1);
        assert!(back.sheet.symbols.is_empty());
    }

    #[test]
    fn snxpcb_round_trip_empty() {
        let snx = SnxPcb::new(empty_board());
        let s = snx.write_string().expect("serialise");
        assert!(s.contains("format = \"snxpcb/1\""));
        let back = SnxPcb::parse(&s).expect("round-trip");
        assert_eq!(back.format, SNXPCB_FORMAT_V1);
    }

    #[test]
    fn rejects_wrong_format_version() {
        // Hand-craft a TOML document with an unsupported version token.
        let bad = "format = \"snxsch/99\"\nschematic_id = \"00000000-0000-0000-0000-000000000000\"\n";
        let err = SnxSchematic::parse(bad).expect_err("must reject");
        match err {
            FormatError::UnsupportedVersion { found, expected } => {
                assert_eq!(found, "snxsch/99");
                assert_eq!(expected, SNXSCH_FORMAT_V1);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn rejects_wrong_pcb_format_version() {
        let bad = "format = \"snxpcb/99\"\npcb_id = \"00000000-0000-0000-0000-000000000000\"\n";
        let err = SnxPcb::parse(bad).expect_err("must reject");
        assert!(matches!(err, FormatError::UnsupportedVersion { .. }));
    }

    #[test]
    fn snxsch_includes_tsv_blocks_substring() {
        let snx = SnxSchematic::new(empty_sheet());
        let s = snx.write_string().unwrap();
        assert!(s.contains("[sheets.components]\ncontent = \"\"\""));
        assert!(s.contains("[sheets.wires]\ncontent = \"\"\""));
        assert!(s.contains("[sheets.junctions]\ncontent = \"\"\""));
        assert!(s.contains("[sheets.labels]\ncontent = \"\"\""));
    }

    #[test]
    fn snxpcb_includes_tsv_blocks_substring() {
        let snx = SnxPcb::new(empty_board());
        let s = snx.write_string().unwrap();
        assert!(s.contains("[footprints]\ncontent = \"\"\""));
        assert!(s.contains("[pads]\ncontent = \"\"\""));
        assert!(s.contains("[tracks]\ncontent = \"\"\""));
        assert!(s.contains("[vias]\ncontent = \"\"\""));
    }

    fn sample_symbol() -> Symbol {
        Symbol {
            uuid: Uuid::parse_str("0192a8c0-0001-7000-8000-000000000001").unwrap(),
            lib_id: "lm2596.snxsym".to_string(),
            reference: "U1".to_string(),
            value: "LM2596".to_string(),
            footprint: "TO-263.snxfpt".to_string(),
            datasheet: String::new(),
            position: SchPoint { x: 50.8, y: 25.4 },
            rotation: 0.0,
            mirror_x: false,
            mirror_y: false,
            unit: 1,
            is_power: false,
            ref_text: None,
            val_text: None,
            fields_autoplaced: false,
            dnp: false,
            in_bom: true,
            on_board: true,
            exclude_from_sim: false,
            locked: false,
            fields: Default::default(),
            custom_properties: Vec::new(),
            pin_uuids: Default::default(),
            instances: Vec::new(),
        }
    }

    fn sample_wire(sx: f64, sy: f64, ex: f64, ey: f64) -> Wire {
        Wire {
            uuid: Uuid::new_v4(),
            start: SchPoint { x: sx, y: sy },
            end: SchPoint { x: ex, y: ey },
            stroke_width: 0.0,
        }
    }

    #[test]
    fn show_serialised_pcb_for_inspection() {
        // diagnostic — emits a human-readable serialisation of a small
        // PCB so reviewers can eyeball the on-disk shape. Intentionally
        // doesn't assert anything; the round-trip tests cover parity.
        let mut board = empty_board();
        board.layers = vec![
            crate::pcb::LayerDef {
                id: 0,
                name: "TopCopper".into(),
                layer_type: "copper".into(),
            },
            crate::pcb::LayerDef {
                id: 1,
                name: "BottomCopper".into(),
                layer_type: "copper".into(),
            },
        ];
        board.footprints.push(Footprint {
            uuid: Uuid::parse_str("0192a8c0-0010-7000-8000-000000000001").unwrap(),
            reference: "U1".into(),
            value: "STM32F407".into(),
            footprint_id: "stm32f407.snxfpt".into(),
            position: PcbPoint { x: 50.0, y: 25.0 },
            rotation: 0.0,
            layer: "TopCopper".into(),
            locked: false,
            pads: vec![],
            graphics: vec![],
            properties: vec![],
        });
        let snx = SnxPcb::new(board);
        let _ = snx.write_string().expect("serialise");
    }

    #[test]
    fn snxsch_round_trip_with_data() {
        let mut sheet = empty_sheet();
        sheet.symbols.push(sample_symbol());
        sheet.wires.push(sample_wire(10.0, 20.0, 30.0, 20.0));
        sheet.wires.push(sample_wire(30.0, 20.0, 30.0, 40.0));
        sheet.junctions.push(Junction {
            uuid: Uuid::parse_str("0192a8c0-0002-7000-8000-000000000001").unwrap(),
            position: SchPoint { x: 30.0, y: 20.0 },
            diameter: 0.5,
        });
        sheet.labels.push(Label {
            uuid: Uuid::parse_str("0192a8c0-0003-7000-8000-000000000001").unwrap(),
            text: "VIN".to_string(),
            position: SchPoint { x: 10.0, y: 20.0 },
            rotation: 0.0,
            label_type: LType::Net,
            shape: String::new(),
            font_size: 1.27,
            justify: HAlign::Left,
            justify_v: VAlign::Bottom,
        });

        let snx = SnxSchematic::new(sheet.clone());
        let serialised = snx.write_string().expect("serialise");

        let back = SnxSchematic::parse(&serialised).expect("round-trip");
        assert_eq!(back.sheet.symbols.len(), 1);
        assert_eq!(back.sheet.symbols[0].reference, "U1");
        assert_eq!(back.sheet.symbols[0].lib_id, "lm2596.snxsym");
        assert_eq!(back.sheet.symbols[0].value, "LM2596");
        assert!((back.sheet.symbols[0].position.x - 50.8).abs() < 1e-6);
        assert!((back.sheet.symbols[0].position.y - 25.4).abs() < 1e-6);
        assert_eq!(back.sheet.symbols[0].footprint, "TO-263.snxfpt");

        assert_eq!(back.sheet.wires.len(), 2);
        assert!((back.sheet.wires[0].start.x - 10.0).abs() < 1e-6);
        assert!((back.sheet.wires[1].end.y - 40.0).abs() < 1e-6);

        assert_eq!(back.sheet.junctions.len(), 1);
        assert_eq!(back.sheet.junctions[0].diameter, 0.5);

        assert_eq!(back.sheet.labels.len(), 1);
        assert_eq!(back.sheet.labels[0].text, "VIN");
        assert_eq!(back.sheet.labels[0].label_type, LType::Net);
    }

    #[test]
    fn snxpcb_round_trip_with_data() {
        let mut board = empty_board();

        let pad1 = Pad {
            uuid: Uuid::parse_str("0192a8c0-0011-7000-8000-000000000001").unwrap(),
            number: "1".into(),
            pad_type: PadType::Smd,
            shape: PadShape::RoundRect,
            position: PcbPoint { x: 50.5, y: 25.0 },
            size: PcbPoint { x: 1.0, y: 0.6 },
            drill: None,
            layers: vec!["TopCopper".into()],
            net: Some(PadNet {
                number: 1,
                name: "VCC".into(),
            }),
            roundrect_ratio: 0.25,
        };
        let pad2 = Pad {
            uuid: Uuid::parse_str("0192a8c0-0011-7000-8000-000000000002").unwrap(),
            number: "2".into(),
            pad_type: PadType::Smd,
            shape: PadShape::RoundRect,
            position: PcbPoint { x: 51.5, y: 25.0 },
            size: PcbPoint { x: 1.0, y: 0.6 },
            drill: None,
            layers: vec!["TopCopper".into()],
            net: Some(PadNet {
                number: 2,
                name: "GND".into(),
            }),
            roundrect_ratio: 0.25,
        };

        let footprint = Footprint {
            uuid: Uuid::parse_str("0192a8c0-0010-7000-8000-000000000001").unwrap(),
            reference: "U1".into(),
            value: "STM32F407".into(),
            footprint_id: "stm32f407.snxfpt".into(),
            position: PcbPoint { x: 50.0, y: 25.0 },
            rotation: 0.0,
            layer: "TopCopper".into(),
            locked: false,
            pads: vec![pad1, pad2],
            graphics: Vec::new(),
            properties: Vec::new(),
        };
        board.footprints.push(footprint);

        for (uuid, sx, sy, ex, ey) in [
            (
                "0192a8c0-0020-7000-8000-000000000001",
                100.0,
                200.0,
                150.0,
                200.0,
            ),
            (
                "0192a8c0-0020-7000-8000-000000000002",
                150.0,
                200.0,
                200.0,
                200.0,
            ),
            (
                "0192a8c0-0020-7000-8000-000000000003",
                200.0,
                200.0,
                300.0,
                200.0,
            ),
        ] {
            board.segments.push(Segment {
                uuid: Uuid::parse_str(uuid).unwrap(),
                start: PcbPoint { x: sx, y: sy },
                end: PcbPoint { x: ex, y: ey },
                width: 0.254,
                layer: "BottomCopper".into(),
                net: 1,
            });
        }

        board.vias.push(Via {
            uuid: Uuid::parse_str("0192a8c0-0030-7000-8000-000000000001").unwrap(),
            position: PcbPoint { x: 100.0, y: 200.0 },
            diameter: 0.6,
            drill: 0.3,
            layers: vec!["TopCopper".into(), "BottomCopper".into()],
            net: 1,
            via_type: ViaType::Through,
        });

        board.zones.push(Zone {
            uuid: Uuid::parse_str("0192a8c0-0040-7000-8000-000000000001").unwrap(),
            net: 2,
            net_name: "GND".into(),
            layer: "BottomCopper".into(),
            outline: vec![
                PcbPoint { x: 10.0, y: 20.0 },
                PcbPoint { x: 30.0, y: 20.0 },
                PcbPoint { x: 30.0, y: 40.0 },
                PcbPoint { x: 10.0, y: 40.0 },
            ],
            priority: 0,
            fill_type: String::new(),
            thermal_relief: false,
            thermal_gap: 0.0,
            thermal_width: 0.0,
            clearance: 0.0,
            min_thickness: 0.0,
        });

        let snx = SnxPcb::new(board.clone());
        let serialised = snx.write_string().expect("serialise");
        let back = SnxPcb::parse(&serialised).expect("round-trip");

        assert_eq!(back.board.footprints.len(), 1);
        assert_eq!(back.board.footprints[0].reference, "U1");
        assert_eq!(back.board.footprints[0].pads.len(), 2);
        assert_eq!(back.board.footprints[0].pads[0].number, "1");
        assert_eq!(
            back.board.footprints[0].pads[0]
                .net
                .as_ref()
                .unwrap()
                .name,
            "VCC"
        );

        assert_eq!(back.board.segments.len(), 3);
        assert!((back.board.segments[0].width - 0.254).abs() < 1e-6);
        assert_eq!(back.board.segments[0].layer, "BottomCopper");

        assert_eq!(back.board.vias.len(), 1);
        assert!((back.board.vias[0].diameter - 0.6).abs() < 1e-6);
        assert!((back.board.vias[0].drill - 0.3).abs() < 1e-6);

        assert_eq!(back.board.zones.len(), 1);
        assert_eq!(back.board.zones[0].net_name, "GND");
        assert_eq!(back.board.zones[0].outline.len(), 4);
    }

    #[test]
    fn tsv_writer_pads_columns_for_legibility() {
        let rows = vec![
            SchJunctionRow {
                uuid: Uuid::nil(),
                pos_x: 100,
                pos_y: 200,
                diameter: 0.5,
            },
            SchJunctionRow {
                uuid: Uuid::nil(),
                pos_x: 30000000,
                pos_y: 40000000,
                diameter: 0.5,
            },
        ];
        let body = write_tsv_block(&rows);
        let lines: Vec<&str> = body.lines().collect();
        assert_eq!(lines[0].split_whitespace().next().unwrap(), "uuid");
        // header columns "pos_x" "pos_y" "diameter" preserved
        assert!(lines[0].contains("pos_x"));
        assert!(lines[0].contains("diameter"));
        // round-trip
        let parsed: Vec<SchJunctionRow> =
            parse_tsv_block("sheets.junctions", &body).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].pos_x, 100);
        assert_eq!(parsed[1].pos_x, 30000000);
    }

    #[test]
    fn tsv_parser_rejects_header_mismatch() {
        let body = "uuid pos_x pos_y wrong_column\n";
        let err = parse_tsv_block::<SchJunctionRow>("sheets.junctions", body).unwrap_err();
        assert!(matches!(err, FormatError::TsvHeaderMismatch { .. }));
    }

    #[test]
    fn tsv_parser_rejects_cell_count_mismatch() {
        let body = "uuid  pos_x  pos_y  diameter\n00000000-0000-0000-0000-000000000000  100  200\n";
        let err = parse_tsv_block::<SchJunctionRow>("sheets.junctions", body).unwrap_err();
        assert!(matches!(err, FormatError::TsvCellCountMismatch { .. }));
    }

    #[test]
    fn integer_nanometre_coords_survive_round_trip() {
        // 50.8 mm = 50800000 nm; check round-trip via i64 wire format.
        let mut sheet = empty_sheet();
        let mut sym = sample_symbol();
        sym.position = SchPoint {
            x: 50.800001,
            y: 25.400002,
        };
        sheet.symbols.push(sym);
        let s = SnxSchematic::new(sheet).write_string().unwrap();
        let back = SnxSchematic::parse(&s).unwrap();
        // expect rounding to nearest nanometre
        assert!((back.sheet.symbols[0].position.x - 50.800001).abs() <= 1e-6);
    }

    #[test]
    fn extras_preserve_symbol_fields() {
        let mut sheet = empty_sheet();
        let mut sym = sample_symbol();
        sym.fields
            .insert("MPN".to_string(), "LM2596S-5.0".to_string());
        sym.fields
            .insert("Tolerance".to_string(), "1%".to_string());
        sym.dnp = true;
        sheet.symbols.push(sym);

        let s = SnxSchematic::new(sheet).write_string().unwrap();
        let back = SnxSchematic::parse(&s).unwrap();
        let recovered = &back.sheet.symbols[0];
        // MPN flows through TSV column.
        assert_eq!(recovered.fields.get("MPN").unwrap(), "LM2596S-5.0");
        // Tolerance survived through extras.
        assert_eq!(recovered.fields.get("Tolerance").unwrap(), "1%");
        assert!(recovered.dnp);
    }
}
