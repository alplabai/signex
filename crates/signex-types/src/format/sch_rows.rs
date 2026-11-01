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

use super::extras::{JunctionExtras, SymbolExtras};
use super::tsv::{format_f64, parse_f64, parse_nm, parse_uuid};
use super::units::{NonFiniteField, mm_to_nm, nm_to_mm};
use super::unknown_columns::impl_unknown_columns_accessors;
use super::*;
use crate::schematic::{
    BODY_ALTERNATE, BODY_NORMAL, BodyStyle, HAlign, Junction, Label, LabelType, Point, Symbol,
    VAlign, Wire,
};
use uuid::Uuid;

/// Bulk row for one [`Symbol`] in the `[sheets.components]` block.
///
/// Captures the fields with one cell per concept (ref designator,
/// library id, position in nanometres, rotation in degrees, value,
/// MPN). Symbol-level fields that don't fit a flat row — `fields`
/// map, `custom_properties`, `pin_uuids`, `instances`, `ref_text` /
/// `val_text` text-prop overrides — survive in the
/// `[extras.symbols.<uuid>]` auxiliary TOML tables.
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
    pub unknown_columns: UnknownColumns,
}

impl SnxTable for SchComponentRow {
    fn columns() -> &'static [&'static str] {
        &[
            "uuid", "ref", "library", "pos_x", "pos_y", "rotation", "value", "mpn",
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
            self.value.clone(),
            self.mpn.clone(),
        ])
    }

    impl_unknown_columns_accessors!();

    fn from_row(values: &[&str], block: &str, row: usize) -> Result<Self, FormatError> {
        Ok(SchComponentRow {
            uuid: parse_uuid(values[0], block, row, "uuid")?,
            ref_des: values[1].to_string(),
            library: values[2].to_string(),
            pos_x: parse_nm(values[3], block, row, "pos_x")?,
            pos_y: parse_nm(values[4], block, row, "pos_y")?,
            rotation: parse_f64(values[5], block, row, "rotation")?,
            value: values[6].to_string(),
            mpn: values[7].to_string(),
            unknown_columns: UnknownColumns::default(),
        })
    }
}

/// Bulk row for one [`Wire`] in the `[sheets.wires]` block.
#[derive(Debug, Clone, PartialEq)]
pub struct SchWireRow {
    pub uuid: Uuid,
    pub start_x: i64,
    pub start_y: i64,
    pub end_x: i64,
    pub end_y: i64,
    pub stroke_width: f64,
    pub unknown_columns: UnknownColumns,
}

impl SnxTable for SchWireRow {
    fn columns() -> &'static [&'static str] {
        &[
            "uuid",
            "start_x",
            "start_y",
            "end_x",
            "end_y",
            "stroke_width",
        ]
    }

    fn to_row(&self) -> Result<Vec<String>, NonFiniteField> {
        Ok(vec![
            self.uuid.to_string(),
            self.start_x.to_string(),
            self.start_y.to_string(),
            self.end_x.to_string(),
            self.end_y.to_string(),
            format_f64(self.stroke_width, "stroke_width")?,
        ])
    }

    impl_unknown_columns_accessors!();

    fn from_row(values: &[&str], block: &str, row: usize) -> Result<Self, FormatError> {
        Ok(SchWireRow {
            uuid: parse_uuid(values[0], block, row, "uuid")?,
            start_x: parse_nm(values[1], block, row, "start_x")?,
            start_y: parse_nm(values[2], block, row, "start_y")?,
            end_x: parse_nm(values[3], block, row, "end_x")?,
            end_y: parse_nm(values[4], block, row, "end_y")?,
            stroke_width: parse_f64(values[5], block, row, "stroke_width")?,
            unknown_columns: UnknownColumns::default(),
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
    pub unknown_columns: UnknownColumns,
}

impl SnxTable for SchJunctionRow {
    fn columns() -> &'static [&'static str] {
        &["uuid", "pos_x", "pos_y", "diameter"]
    }

    fn to_row(&self) -> Result<Vec<String>, NonFiniteField> {
        Ok(vec![
            self.uuid.to_string(),
            self.pos_x.to_string(),
            self.pos_y.to_string(),
            format_f64(self.diameter, "diameter")?,
        ])
    }

    impl_unknown_columns_accessors!();

    fn from_row(values: &[&str], block: &str, row: usize) -> Result<Self, FormatError> {
        Ok(SchJunctionRow {
            uuid: parse_uuid(values[0], block, row, "uuid")?,
            pos_x: parse_nm(values[1], block, row, "pos_x")?,
            pos_y: parse_nm(values[2], block, row, "pos_y")?,
            diameter: parse_f64(values[3], block, row, "diameter")?,
            unknown_columns: UnknownColumns::default(),
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
    pub unknown_columns: UnknownColumns,
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

    fn to_row(&self) -> Result<Vec<String>, NonFiniteField> {
        Ok(vec![
            self.uuid.to_string(),
            self.text.clone(),
            self.pos_x.to_string(),
            self.pos_y.to_string(),
            format_f64(self.rotation, "rotation")?,
            self.kind.clone(),
            self.shape.clone(),
            format_f64(self.font_size, "font_size")?,
            self.justify.clone(),
            self.justify_v.clone(),
        ])
    }

    impl_unknown_columns_accessors!();

    fn from_row(values: &[&str], block: &str, row: usize) -> Result<Self, FormatError> {
        Ok(SchLabelRow {
            uuid: parse_uuid(values[0], block, row, "uuid")?,
            text: values[1].to_string(),
            pos_x: parse_nm(values[2], block, row, "pos_x")?,
            pos_y: parse_nm(values[3], block, row, "pos_y")?,
            rotation: parse_f64(values[4], block, row, "rotation")?,
            kind: values[5].to_string(),
            shape: values[6].to_string(),
            font_size: parse_f64(values[7], block, row, "font_size")?,
            justify: values[8].to_string(),
            justify_v: values[9].to_string(),
            unknown_columns: UnknownColumns::default(),
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

pub(in crate::format) fn symbol_to_row(s: &Symbol) -> Result<SchComponentRow, NonFiniteField> {
    Ok(SchComponentRow {
        uuid: s.uuid,
        ref_des: s.reference.clone(),
        library: s.lib_id.clone(),
        pos_x: mm_to_nm(s.position.x, "pos_x")?,
        pos_y: mm_to_nm(s.position.y, "pos_y")?,
        rotation: s.rotation,
        value: s.value.clone(),
        mpn: s.fields.get("MPN").cloned().unwrap_or_default(),
        unknown_columns: s.unknown_columns.clone(),
    })
}

/// The body a stored `body_style` tag asks for. Absent and `1` are the normal
/// body; `2` is the alternate. Anything else is a body this build does not
/// know, so the symbol loads normal and the unreadable value is logged.
/// An unreadable value is kept in `unknown` (so a re-save writes it back)
/// until the user picks a body, which clears it.
fn body_style_from_tag(
    tag: Option<toml::Value>,
    reference: &str,
    symbol: uuid::Uuid,
    unknown: &mut super::unknown::UnknownFields,
) -> BodyStyle {
    // Not an integer (or out of range) is as unreadable as an unknown number.
    let known = match &tag {
        None => Some(BodyStyle::Normal),
        Some(toml::Value::Integer(n)) => match u32::try_from(*n) {
            Ok(BODY_NORMAL) => Some(BodyStyle::Normal),
            Ok(BODY_ALTERNATE) => Some(BodyStyle::Alternate),
            _ => None,
        },
        Some(_) => None,
    };
    if let (None, Some(other)) = (known, tag) {
        tracing::warn!(
            target: "signex::format",
            "symbol {reference} ({symbol}): body_style = {other} is not 1 (normal) or 2 (alternate); showing the normal body and keeping the value in the file"
        );
        unknown.insert_value("body_style", other);
    }
    known.unwrap_or(BodyStyle::Normal)
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
    let mut unknown = extras.unknown;
    let body_style = body_style_from_tag(extras.body_style, &row.ref_des, row.uuid, &mut unknown);
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
        body_style,
        is_power: extras.is_power,
        ref_text: extras.ref_text.map(|mut prop| {
            prop.font_family = extras.ref_font_family.or(prop.font_family);
            prop
        }),
        val_text: extras.val_text.map(|mut prop| {
            prop.font_family = extras.val_font_family.or(prop.font_family);
            prop
        }),
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
        library_id: extras.library_id,
        row_id: extras.row_id,
        library_version: extras.library_version,
        instances: extras.instances,
        unknown,
        unknown_columns: row.unknown_columns,
    }
}

/// Header of the optional `locked` column on wire and label rows (#1573).
/// Like [`LABEL_FONT_FAMILY_COLUMN`] it is not a known column: it rides the
/// foreign-column path, so a block with no locked row has no such header and
/// re-saves byte-identical, and a build that predates the lock keeps it.
/// Format: `docs/architecture/snxsch-object-lock.md`.
pub(in crate::format) const LOCKED_COLUMN: &str = "locked";

/// The cell a locked row writes; every other row of the block writes empty.
const LOCKED_CELL: &str = "true";

/// Add the lock cell to a row's foreign columns when the row is locked.
fn with_locked_cell(mut columns: UnknownColumns, locked: bool) -> UnknownColumns {
    if locked {
        columns.set(LOCKED_COLUMN, LOCKED_CELL.to_string());
    }
    columns
}

/// Take the lock cell out of a row's foreign columns. `true` locks and an
/// empty cell is a row of the block that is not locked; any other value is
/// not ours to read, so it stays a foreign cell and round-trips untouched.
fn take_locked_cell(columns: &mut UnknownColumns) -> bool {
    match columns.get(LOCKED_COLUMN) {
        Some(LOCKED_CELL) => {
            columns.take(LOCKED_COLUMN);
            true
        }
        Some("") => {
            columns.take(LOCKED_COLUMN);
            false
        }
        _ => false,
    }
}

pub(in crate::format) fn wire_to_row(w: &Wire) -> Result<SchWireRow, NonFiniteField> {
    Ok(SchWireRow {
        uuid: w.uuid,
        start_x: mm_to_nm(w.start.x, "start_x")?,
        start_y: mm_to_nm(w.start.y, "start_y")?,
        end_x: mm_to_nm(w.end.x, "end_x")?,
        end_y: mm_to_nm(w.end.y, "end_y")?,
        stroke_width: w.stroke_width,
        unknown_columns: with_locked_cell(w.unknown_columns.clone(), w.locked),
    })
}

pub(in crate::format) fn row_to_wire(mut row: SchWireRow) -> Wire {
    let locked = take_locked_cell(&mut row.unknown_columns);
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
        locked,
        unknown_columns: row.unknown_columns,
    }
}

pub(in crate::format) fn junction_to_row(j: &Junction) -> Result<SchJunctionRow, NonFiniteField> {
    Ok(SchJunctionRow {
        uuid: j.uuid,
        pos_x: mm_to_nm(j.position.x, "pos_x")?,
        pos_y: mm_to_nm(j.position.y, "pos_y")?,
        diameter: j.diameter,
        unknown_columns: j.unknown_columns.clone(),
    })
}

pub(in crate::format) fn row_to_junction(row: SchJunctionRow, extra: JunctionExtras) -> Junction {
    Junction {
        uuid: row.uuid,
        position: Point {
            x: nm_to_mm(row.pos_x),
            y: nm_to_mm(row.pos_y),
        },
        diameter: row.diameter,
        minted: extra.minted,
        unknown: extra.unknown,
        unknown_columns: row.unknown_columns,
    }
}

/// Header of the optional label font column. It is not part of
/// [`SchLabelRow::columns`]: a file whose labels set no family must stay
/// byte-identical (and a legacy file must still load), so the cell rides the
/// foreign-column path and the header appears only when some label sets one.
/// A build that predates the field keeps the column as foreign cells.
const LABEL_FONT_FAMILY_COLUMN: &str = "font_family";

pub(in crate::format) fn label_to_row(l: &Label) -> Result<SchLabelRow, NonFiniteField> {
    let mut unknown_columns = l.unknown_columns.clone();
    if let Some(family) = &l.font_family {
        unknown_columns.set(LABEL_FONT_FAMILY_COLUMN, family.clone());
    }
    let unknown_columns = with_locked_cell(unknown_columns, l.locked);
    Ok(SchLabelRow {
        uuid: l.uuid,
        text: l.text.clone(),
        pos_x: mm_to_nm(l.position.x, "pos_x")?,
        pos_y: mm_to_nm(l.position.y, "pos_y")?,
        rotation: l.rotation,
        kind: label_kind_str(l.label_type).to_string(),
        shape: l.shape.clone(),
        font_size: l.font_size,
        justify: halign_str(l.justify).to_string(),
        justify_v: valign_str(l.justify_v).to_string(),
        unknown_columns,
    })
}

pub(in crate::format) fn row_to_label(mut row: SchLabelRow) -> Label {
    // An empty cell is a label that sets no family (rows of a block where
    // only some labels do).
    let font_family = row
        .unknown_columns
        .take(LABEL_FONT_FAMILY_COLUMN)
        .filter(|family| !family.is_empty());
    let locked = take_locked_cell(&mut row.unknown_columns);
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
        font_family,
        justify: parse_halign(&row.justify),
        justify_v: parse_valign(&row.justify_v),
        locked,
        unknown_columns: row.unknown_columns,
    }
}
