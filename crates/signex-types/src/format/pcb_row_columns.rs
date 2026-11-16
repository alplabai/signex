//! The optional board-row columns that ride the foreign-column path: a cell
//! is written only for a value that differs from the default, so a board
//! that sets none has no such header and re-saves byte-identical, and an
//! older build keeps the cells it cannot read.
//!
//! Pads: `rotation` and `net_locked`. Tracks and vias (#2378): `locked`,
//! `tented_top`, `tented_bottom` and `mask_from_hole_edge` are `true` or
//! empty; a mask column is empty for the rule or an integer expansion in nm
//! for a manual one.

use super::FormatError;
use super::UnknownColumns;
use super::pcb_rows::row_field_error;
use super::tsv::format_f64;
use super::units::{NonFiniteField, mm_to_nm, nm_to_mm};
use crate::pcb_route_attrs::{MaskExpansion, TrackAttributes, ViaAttributes, ViaMaskSide};

/// Header of the optional `rotation` pad column: the pad's own turn in
/// degrees. Rides the foreign-column path like `net_locked`; written only
/// when some pad is turned, so an unturned board re-saves unchanged.
const PAD_ROTATION_COLUMN: &str = "rotation";

pub(in crate::format) fn with_rotation_cell(
    mut columns: UnknownColumns,
    rotation: f64,
) -> Result<UnknownColumns, NonFiniteField> {
    if rotation != 0.0 {
        columns.set(PAD_ROTATION_COLUMN, format_f64(rotation, "rotation")?);
    }
    Ok(columns)
}

pub(in crate::format) fn take_rotation_cell(
    columns: &mut UnknownColumns,
    block: &str,
    row: usize,
) -> Result<f64, FormatError> {
    match columns.take(PAD_ROTATION_COLUMN) {
        None => Ok(0.0),
        Some(cell) if cell.is_empty() => Ok(0.0),
        Some(cell) => match cell.trim().parse::<f64>() {
            Ok(degrees) if degrees.is_finite() => Ok(degrees),
            _ => Err(row_field_error(
                block,
                row,
                PAD_ROTATION_COLUMN,
                &cell,
                "a pad rotation must be a finite number of degrees",
            )),
        },
    }
}

/// Header of the optional `net_locked` pad column. Like the schematic's
/// `locked` column it is not a known column: it rides the foreign-column
/// path. A board with no locked pad has no such header and re-saves
/// byte-identical, and a build that predates the lock keeps it. When any pad
/// is locked the header is written after the known columns, in first-seen
/// order among the block's foreign columns (its position among the known
/// ones is not kept), and every unlocked pad writes an empty cell. See
/// `unknown_columns` for the contract.
const NET_LOCKED_COLUMN: &str = "net_locked";

pub(in crate::format) fn with_net_locked_cell(
    mut columns: UnknownColumns,
    locked: bool,
) -> UnknownColumns {
    set_flag(&mut columns, NET_LOCKED_COLUMN, locked);
    columns
}

pub(in crate::format) fn take_net_locked_cell(columns: &mut UnknownColumns) -> bool {
    take_flag(columns, NET_LOCKED_COLUMN)
}

const LOCKED_COLUMN: &str = "locked";
const PASTE_MASK_COLUMN: &str = "paste_mask";
const SOLDER_MASK_COLUMN: &str = "solder_mask";
const SOLDER_MASK_TOP_COLUMN: &str = "solder_mask_top";
const SOLDER_MASK_BOTTOM_COLUMN: &str = "solder_mask_bottom";
const TENTED_TOP_COLUMN: &str = "tented_top";
const TENTED_BOTTOM_COLUMN: &str = "tented_bottom";
const FROM_HOLE_EDGE_COLUMN: &str = "mask_from_hole_edge";

const TRUE_CELL: &str = "true";

/// `columns` with the cells of a track's non-default settings.
pub(in crate::format) fn with_track_cells(
    mut columns: UnknownColumns,
    attributes: &TrackAttributes,
) -> Result<UnknownColumns, NonFiniteField> {
    set_flag(&mut columns, LOCKED_COLUMN, attributes.locked);
    set_mask(&mut columns, PASTE_MASK_COLUMN, attributes.paste_mask)?;
    set_mask(&mut columns, SOLDER_MASK_COLUMN, attributes.solder_mask)?;
    Ok(columns)
}

/// A track's settings, read out of (and removed from) its row's cells.
pub(in crate::format) fn take_track_cells(
    columns: &mut UnknownColumns,
    block: &str,
    row: usize,
) -> Result<TrackAttributes, FormatError> {
    Ok(TrackAttributes {
        locked: take_flag(columns, LOCKED_COLUMN),
        paste_mask: take_mask(columns, PASTE_MASK_COLUMN, block, row)?,
        solder_mask: take_mask(columns, SOLDER_MASK_COLUMN, block, row)?,
    })
}

/// `columns` with the cells of a via's non-default settings.
pub(in crate::format) fn with_via_cells(
    mut columns: UnknownColumns,
    attributes: &ViaAttributes,
) -> Result<UnknownColumns, NonFiniteField> {
    set_flag(&mut columns, LOCKED_COLUMN, attributes.locked);
    set_mask(
        &mut columns,
        SOLDER_MASK_TOP_COLUMN,
        attributes.top.expansion,
    )?;
    set_mask(
        &mut columns,
        SOLDER_MASK_BOTTOM_COLUMN,
        attributes.bottom.expansion,
    )?;
    set_flag(&mut columns, TENTED_TOP_COLUMN, attributes.top.tented);
    set_flag(&mut columns, TENTED_BOTTOM_COLUMN, attributes.bottom.tented);
    set_flag(
        &mut columns,
        FROM_HOLE_EDGE_COLUMN,
        attributes.from_hole_edge,
    );
    Ok(columns)
}

/// A via's settings, read out of (and removed from) its row's cells.
pub(in crate::format) fn take_via_cells(
    columns: &mut UnknownColumns,
    block: &str,
    row: usize,
) -> Result<ViaAttributes, FormatError> {
    Ok(ViaAttributes {
        locked: take_flag(columns, LOCKED_COLUMN),
        top: ViaMaskSide {
            expansion: take_mask(columns, SOLDER_MASK_TOP_COLUMN, block, row)?,
            tented: take_flag(columns, TENTED_TOP_COLUMN),
        },
        bottom: ViaMaskSide {
            expansion: take_mask(columns, SOLDER_MASK_BOTTOM_COLUMN, block, row)?,
            tented: take_flag(columns, TENTED_BOTTOM_COLUMN),
        },
        from_hole_edge: take_flag(columns, FROM_HOLE_EDGE_COLUMN),
    })
}

fn set_flag(columns: &mut UnknownColumns, column: &str, on: bool) {
    if on {
        columns.set(column, TRUE_CELL.to_string());
    }
}

/// `true` sets the flag and an empty cell clears it; any other value is not
/// ours to read, so it stays a foreign cell and round-trips untouched.
fn take_flag(columns: &mut UnknownColumns, column: &str) -> bool {
    match columns.get(column) {
        Some(TRUE_CELL) => {
            columns.take(column);
            true
        }
        Some("") => {
            columns.take(column);
            false
        }
        _ => false,
    }
}

fn set_mask(
    columns: &mut UnknownColumns,
    column: &'static str,
    mask: MaskExpansion,
) -> Result<(), NonFiniteField> {
    if let Some(mm) = mask.manual_mm() {
        columns.set(column, mm_to_nm(mm, column)?.to_string());
    }
    Ok(())
}

/// An empty cell is the rule; a number is a manual expansion in nm.
fn take_mask(
    columns: &mut UnknownColumns,
    column: &str,
    block: &str,
    row: usize,
) -> Result<MaskExpansion, FormatError> {
    match columns.take(column) {
        None => Ok(MaskExpansion::Rule),
        Some(cell) if cell.trim().is_empty() => Ok(MaskExpansion::Rule),
        Some(cell) => {
            let nm = super::tsv::parse_nm(cell.trim(), block, row, column)?;
            Ok(MaskExpansion::Manual { mm: nm_to_mm(nm) })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_write_no_cell() {
        let track = with_track_cells(UnknownColumns::default(), &TrackAttributes::default());
        assert!(track.expect("finite").is_empty());
        let via = with_via_cells(UnknownColumns::default(), &ViaAttributes::default());
        assert!(via.expect("finite").is_empty());
    }

    #[test]
    fn track_settings_round_trip_through_their_cells() {
        let attributes = TrackAttributes {
            locked: true,
            paste_mask: MaskExpansion::Manual { mm: -0.025 },
            solder_mask: MaskExpansion::Rule,
        };
        let mut cells = with_track_cells(UnknownColumns::default(), &attributes).unwrap();
        assert_eq!(cells.get(PASTE_MASK_COLUMN), Some("-25000"));
        assert_eq!(cells.get(SOLDER_MASK_COLUMN), None);
        let back = take_track_cells(&mut cells, "tracks", 0).unwrap();
        assert_eq!(back, attributes);
        assert!(cells.is_empty(), "every cell read was taken");
    }

    #[test]
    fn via_settings_round_trip_through_their_cells() {
        let attributes = ViaAttributes {
            locked: true,
            top: ViaMaskSide {
                expansion: MaskExpansion::Manual { mm: 0.1 },
                tented: false,
            },
            bottom: ViaMaskSide {
                expansion: MaskExpansion::Rule,
                tented: true,
            },
            from_hole_edge: true,
        };
        let mut cells = with_via_cells(UnknownColumns::default(), &attributes).unwrap();
        let back = take_via_cells(&mut cells, "vias", 0).unwrap();
        assert_eq!(back, attributes);
        assert!(cells.is_empty());
    }

    #[test]
    fn a_cell_that_is_not_a_flag_stays_foreign_and_a_bad_mask_is_refused() {
        let mut cells = UnknownColumns::default();
        cells.set(LOCKED_COLUMN, "maybe".to_string());
        let read = take_track_cells(&mut cells, "tracks", 3).unwrap();
        assert!(!read.locked);
        assert_eq!(cells.get(LOCKED_COLUMN), Some("maybe"));
        cells.set(SOLDER_MASK_COLUMN, "wide".to_string());
        assert!(take_track_cells(&mut cells, "tracks", 3).is_err());
    }

    #[test]
    fn a_non_finite_manual_mask_is_not_written() {
        let attributes = TrackAttributes {
            solder_mask: MaskExpansion::Manual { mm: f64::NAN },
            ..TrackAttributes::default()
        };
        assert!(with_track_cells(UnknownColumns::default(), &attributes).is_err());
    }
}
