//! The optional board-row columns that ride the foreign-column path: a cell
//! is written only for a value that differs from the default, so a board
//! that sets none has no such header and re-saves byte-identical, and an
//! older build keeps the cells it cannot read.
//!
//! The mask settings of tracks and vias (#2378): `tented_top`,
//! `tented_bottom` and `mask_from_hole_edge` are `true` or empty; a mask
//! column is empty for the rule or an integer expansion in nm for a manual
//! one. The pad cells and the track and via `locked` cell (#2407) are in
//! `pcb_cells`.

use super::FormatError;
use super::UnknownColumns;
use super::units::{NonFiniteField, mm_to_nm, nm_to_mm};
use crate::pcb_route_attrs::{MaskExpansion, TrackAttributes, ViaAttributes, ViaMaskSide};

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
        paste_mask: take_mask(columns, PASTE_MASK_COLUMN, block, row),
        solder_mask: take_mask(columns, SOLDER_MASK_COLUMN, block, row),
    })
}

/// `columns` with the cells of a via's non-default settings.
pub(in crate::format) fn with_via_cells(
    mut columns: UnknownColumns,
    attributes: &ViaAttributes,
) -> Result<UnknownColumns, NonFiniteField> {
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
        top: ViaMaskSide {
            expansion: take_mask(columns, SOLDER_MASK_TOP_COLUMN, block, row),
            tented: take_flag(columns, TENTED_TOP_COLUMN),
        },
        bottom: ViaMaskSide {
            expansion: take_mask(columns, SOLDER_MASK_BOTTOM_COLUMN, block, row),
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

/// An empty cell is the rule; a number is a manual expansion in nm. Any
/// other value is not ours to read: like a flag's, it stays a foreign cell
/// that round-trips untouched, and the mask reads as the rule.
fn take_mask(columns: &mut UnknownColumns, column: &str, block: &str, row: usize) -> MaskExpansion {
    let read = match columns.get(column) {
        None => return MaskExpansion::Rule,
        Some(cell) if cell.trim().is_empty() => Some(MaskExpansion::Rule),
        Some(cell) => super::tsv::parse_nm(cell.trim(), block, row, column)
            .ok()
            .map(|nm| MaskExpansion::Manual { mm: nm_to_mm(nm) }),
    };
    match read {
        Some(mask) => {
            columns.take(column);
            mask
        }
        None => MaskExpansion::Rule,
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
    fn a_cell_that_is_not_a_flag_or_a_mask_stays_foreign() {
        let mut cells = UnknownColumns::default();
        cells.set(TENTED_TOP_COLUMN, "maybe".to_string());
        let read = take_via_cells(&mut cells, "vias", 3).unwrap();
        assert!(!read.top.tented);
        assert_eq!(cells.get(TENTED_TOP_COLUMN), Some("maybe"));
        let mut cells = UnknownColumns::default();
        cells.set(SOLDER_MASK_COLUMN, "wide".to_string());
        let read = take_track_cells(&mut cells, "tracks", 3).expect("the row still loads");
        assert_eq!(read.solder_mask, MaskExpansion::Rule);
        assert_eq!(cells.get(SOLDER_MASK_COLUMN), Some("wide"));
        let written = with_track_cells(cells, &read).expect("finite");
        assert_eq!(written.get(SOLDER_MASK_COLUMN), Some("wide"));

        let mut cells = UnknownColumns::default();
        cells.set(SOLDER_MASK_TOP_COLUMN, "open".to_string());
        let read = take_via_cells(&mut cells, "vias", 0).expect("the row still loads");
        assert_eq!(read.top.expansion, MaskExpansion::Rule);
        assert_eq!(cells.get(SOLDER_MASK_TOP_COLUMN), Some("open"));
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
