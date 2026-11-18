//! The optional `trapezoid_delta` and `custom_outline` pad cells
//! (`snxpcb/1.4`, #2242), shared by the `pads` rows and the `[free]` pads.
//! Lengths are whole nm: a delta is `<x>,<y>`, an outline `<x>,<y>;<x>,<y>;..`
//! in the pad's own frame. Each column is written only when some pad needs
//! it, so a board with neither re-saves unchanged.

use super::FormatError;
use super::UnknownColumns;
use super::pcb_rows::row_field_error;
use super::units::{NonFiniteField, mm_to_nm, nm_to_mm};
use crate::pcb::{Pad, Point};
use crate::pcb_pad_shape::pad_shape_fault;

pub(in crate::format) const TRAPEZOID_DELTA_COLUMN: &str = "trapezoid_delta";
pub(in crate::format) const CUSTOM_OUTLINE_COLUMN: &str = "custom_outline";

fn point_cell(p: Point, field: &'static str) -> Result<String, NonFiniteField> {
    Ok(format!(
        "{},{}",
        mm_to_nm(p.x, field)?,
        mm_to_nm(p.y, field)?
    ))
}

fn read_point(text: &str) -> Option<Point> {
    let (x, y) = text.split_once(',')?;
    Some(Point::new(
        nm_to_mm(x.trim().parse().ok()?),
        nm_to_mm(y.trim().parse().ok()?),
    ))
}

/// The delta cell; `None` for a zero delta, which writes nothing.
pub(in crate::format) fn delta_cell(pad: &Pad) -> Result<Option<String>, NonFiniteField> {
    if pad.trapezoid_delta == Point::ZERO {
        return Ok(None);
    }
    point_cell(pad.trapezoid_delta, TRAPEZOID_DELTA_COLUMN).map(Some)
}

/// The outline cell; `None` for no outline, which writes nothing.
pub(in crate::format) fn outline_cell(pad: &Pad) -> Result<Option<String>, NonFiniteField> {
    if pad.custom_outline.is_empty() {
        return Ok(None);
    }
    let corners = pad
        .custom_outline
        .iter()
        .map(|&p| point_cell(p, CUSTOM_OUTLINE_COLUMN))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(corners.join(";")))
}

/// A delta cell as a delta; an empty cell is none. A cell that does not read
/// is a load error naming it.
pub(in crate::format) fn read_delta(
    cell: Option<&str>,
    block: &str,
    row: usize,
) -> Result<Point, FormatError> {
    match cell {
        None | Some("") => Ok(Point::ZERO),
        Some(text) => read_point(text).ok_or_else(|| {
            row_field_error(
                block,
                row,
                TRAPEZOID_DELTA_COLUMN,
                text,
                "a trapezoid delta must be <x>,<y> in whole nm",
            )
        }),
    }
}

/// An outline cell as corners; an empty cell is none.
pub(in crate::format) fn read_outline(
    cell: Option<&str>,
    block: &str,
    row: usize,
) -> Result<Vec<Point>, FormatError> {
    match cell {
        None | Some("") => Ok(Vec::new()),
        Some(text) => text
            .split(';')
            .map(read_point)
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| {
                row_field_error(
                    block,
                    row,
                    CUSTOM_OUTLINE_COLUMN,
                    text,
                    "a custom outline must be <x>,<y>;<x>,<y>;.. in whole nm",
                )
            }),
    }
}

/// Write both cells of `pad` into its row's foreign columns.
pub(in crate::format) fn with_shape_cells(
    mut columns: UnknownColumns,
    pad: &Pad,
) -> Result<UnknownColumns, NonFiniteField> {
    if let Some(cell) = delta_cell(pad)? {
        columns.set(TRAPEZOID_DELTA_COLUMN, cell);
    }
    if let Some(cell) = outline_cell(pad)? {
        columns.set(CUSTOM_OUTLINE_COLUMN, cell);
    }
    Ok(columns)
}

/// Take both cells out of a row's foreign columns: `(delta, outline)`.
pub(in crate::format) fn take_shape_cells(
    columns: &mut UnknownColumns,
    block: &str,
    row: usize,
) -> Result<(Point, Vec<Point>), FormatError> {
    let delta = read_delta(columns.take(TRAPEZOID_DELTA_COLUMN).as_deref(), block, row)?;
    let outline = read_outline(columns.take(CUSTOM_OUTLINE_COLUMN).as_deref(), block, row)?;
    Ok((delta, outline))
}

/// Refuse a pad whose shape cannot make copper, naming the field. The
/// reader calls this once the pad is whole.
pub(in crate::format) fn check_shape(
    pad: &Pad,
    block: &str,
    row: usize,
) -> Result<(), FormatError> {
    match pad_shape_fault(pad) {
        None => Ok(()),
        Some(fault) => {
            let value = match fault.field() {
                TRAPEZOID_DELTA_COLUMN => delta_cell(pad).ok().flatten().unwrap_or_default(),
                _ => outline_cell(pad).ok().flatten().unwrap_or_default(),
            };
            Err(row_field_error(
                block,
                row,
                fault.field(),
                &value,
                &fault.to_string(),
            ))
        }
    }
}

/// The writer's side of [`check_shape`]: a pad the reader would refuse is
/// not saved, so the board can always reopen.
pub(in crate::format) fn refuse_unreadable_shape(pad: &Pad) -> Result<(), NonFiniteField> {
    match pad_shape_fault(pad) {
        None => Ok(()),
        Some(fault) => Err(NonFiniteField(fault.field())),
    }
}
