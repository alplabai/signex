//! The optional `[board_sketch]` section of a `.snxpcb` (#2234): the board
//! sketch and what it baked to.
//!
//! Written only for a board that has a sketch, so a board without one
//! re-saves byte-identical and a file from before the section loads
//! unchanged. The sketch is the serde form the footprint sketch already
//! uses in `.snxfpt` (`[board_sketch.sketch]`); keepouts, cutouts and
//! V-scores are TOML tables like `[[zones]]`; the mounting holes are a TSV
//! block in integer nm (`[board_sketch.holes]`). The baked outline is the
//! board's own `[extras.board] outline`. See `docs/PCB_SKETCH.md`.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::tsv::{parse_nm, parse_optional_block, parse_uuid, write_tsv_section};
use super::units::{NonFiniteField, mm_to_nm, nm_to_mm};
use super::unknown_columns::impl_unknown_columns_accessors;
use super::{FormatError, SnxTable, TsvBody, UnknownColumns, UnknownFields, build_rows};
use crate::pcb::{PcbBoard, Point};
use crate::pcb_sketch::{BoardSketch, PcbCutout, PcbHole, PcbKeepout, PcbVScore};
use crate::sketch::SketchData;

/// Name of the mounting-hole TSV block, as written and in errors.
pub(in crate::format) const HOLES_BLOCK: &str = "board_sketch.holes";

/// `[board_sketch]` as read.
#[derive(Debug, Clone, Default, Deserialize)]
pub(in crate::format) struct BoardSketchRaw {
    #[serde(default)]
    sketch: SketchData,
    #[serde(default)]
    keepouts: Vec<PcbKeepout>,
    #[serde(default)]
    cutouts: Vec<PcbCutout>,
    #[serde(default)]
    v_scores: Vec<PcbVScore>,
    #[serde(default)]
    holes: Option<TsvBody>,
    /// Foreign keys inside `[board_sketch]`.
    #[serde(flatten)]
    unknown: UnknownFields,
}

/// The board sketch and the foreign keys of its section; `None` when the
/// file has no `[board_sketch]`.
pub(in crate::format) fn read(
    raw: Option<BoardSketchRaw>,
) -> Result<(Option<BoardSketch>, UnknownFields), FormatError> {
    let Some(raw) = raw else {
        return Ok((None, UnknownFields::default()));
    };
    let holes = parse_optional_block::<PcbHoleRow>(HOLES_BLOCK, raw.holes)?
        .into_iter()
        .map(row_to_hole)
        .collect();
    let stored = BoardSketch {
        sketch: raw.sketch,
        keepouts: raw.keepouts,
        cutouts: raw.cutouts,
        holes,
        v_scores: raw.v_scores,
    };
    Ok((Some(stored), raw.unknown))
}

/// Append `[board_sketch]` to `out`. Nothing for a board without a sketch
/// and without foreign section keys.
pub(in crate::format) fn write(out: &mut String, board: &PcbBoard) -> Result<(), FormatError> {
    let unknown = &board.unknown.board_sketch;
    let empty = BoardSketch::default();
    let stored = match board.sketch.as_ref() {
        Some(stored) => stored,
        None if !unknown.is_empty() => &empty,
        None => return Ok(()),
    };

    #[derive(Serialize)]
    struct Wrapper<'a> {
        board_sketch: Section<'a>,
    }
    #[derive(Serialize)]
    struct Section<'a> {
        sketch: &'a SketchData,
        #[serde(skip_serializing_if = "is_empty")]
        keepouts: &'a [PcbKeepout],
        #[serde(skip_serializing_if = "is_empty")]
        cutouts: &'a [PcbCutout],
        #[serde(skip_serializing_if = "is_empty")]
        v_scores: &'a [PcbVScore],
        #[serde(flatten)]
        unknown: &'a UnknownFields,
    }
    let wrapper = Wrapper {
        board_sketch: Section {
            sketch: &stored.sketch,
            keepouts: &stored.keepouts,
            cutouts: &stored.cutouts,
            v_scores: &stored.v_scores,
            unknown,
        },
    };
    let hole_rows = build_rows(HOLES_BLOCK, &stored.holes, hole_to_row)?;
    super::finite_toml::ensure_finite(&wrapper)?;
    out.push('\n');
    out.push_str(&toml::to_string_pretty(&wrapper)?);
    if !hole_rows.is_empty() {
        write_tsv_section(out, HOLES_BLOCK, &hole_rows)?;
    }
    Ok(())
}

/// `skip_serializing_if` for a borrowed list: an empty one is not written.
fn is_empty<T>(items: &&[T]) -> bool {
    items.is_empty()
}

/// Bulk row for one [`PcbHole`] in the `[board_sketch.holes]` block.
#[derive(Debug, Clone, PartialEq)]
pub(in crate::format) struct PcbHoleRow {
    uuid: Uuid,
    pos_x: i64,
    pos_y: i64,
    drill: i64,
    plated: bool,
    net: Option<u32>,
    unknown_columns: UnknownColumns,
}

const TRUE_CELL: &str = "true";
const FALSE_CELL: &str = "false";

fn plated_cell(plated: bool) -> &'static str {
    if plated { TRUE_CELL } else { FALSE_CELL }
}

impl SnxTable for PcbHoleRow {
    fn columns() -> &'static [&'static str] {
        &["uuid", "pos_x", "pos_y", "drill", "plated", "net"]
    }

    fn to_row(&self) -> Result<Vec<String>, NonFiniteField> {
        Ok(vec![
            self.uuid.to_string(),
            self.pos_x.to_string(),
            self.pos_y.to_string(),
            self.drill.to_string(),
            plated_cell(self.plated).to_string(),
            self.net.map(|n| n.to_string()).unwrap_or_default(),
        ])
    }

    impl_unknown_columns_accessors!();

    fn from_row(values: &[&str], block: &str, row: usize) -> Result<Self, FormatError> {
        let bad = |field: &str, message: String| FormatError::TsvFieldParse {
            block: block.to_string(),
            row,
            field: field.to_string(),
            message,
        };
        let plated = match values[4] {
            TRUE_CELL => true,
            FALSE_CELL => false,
            other => return Err(bad("plated", format!("{other:?} is not true or false"))),
        };
        let net = match values[5] {
            "" => None,
            cell => Some(
                cell.parse()
                    .map_err(|e: std::num::ParseIntError| bad("net", e.to_string()))?,
            ),
        };
        Ok(PcbHoleRow {
            uuid: parse_uuid(values[0], block, row, "uuid")?,
            pos_x: parse_nm(values[1], block, row, "pos_x")?,
            pos_y: parse_nm(values[2], block, row, "pos_y")?,
            drill: parse_nm(values[3], block, row, "drill")?,
            plated,
            net,
            unknown_columns: UnknownColumns::default(),
        })
    }
}

fn hole_to_row(hole: &PcbHole) -> Result<PcbHoleRow, NonFiniteField> {
    Ok(PcbHoleRow {
        uuid: hole.uuid,
        pos_x: mm_to_nm(hole.position.x, "pos_x")?,
        pos_y: mm_to_nm(hole.position.y, "pos_y")?,
        drill: mm_to_nm(hole.drill, "drill")?,
        plated: hole.plated,
        net: hole.net,
        unknown_columns: hole.unknown_columns.clone(),
    })
}

fn row_to_hole(row: PcbHoleRow) -> PcbHole {
    PcbHole {
        uuid: row.uuid,
        position: Point {
            x: nm_to_mm(row.pos_x),
            y: nm_to_mm(row.pos_y),
        },
        drill: nm_to_mm(row.drill),
        plated: row.plated,
        net: row.net,
        unknown_columns: row.unknown_columns,
    }
}
