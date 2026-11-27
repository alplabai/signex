//! The optional `thermal` cell of the `pads` rows (#2593): a pad's own air
//! gap, spoke width and spoke count as `<gap>;<width>;<spokes>`, lengths in
//! whole nm, a part left empty following the pour (`300000;;2`). The column
//! is written only when some pad sets a value, so a board with none re-saves
//! unchanged. No token change: a build that does not know the column keeps
//! it as a foreign cell and draws no spokes at all (see
//! `docs/FORMAT_VERSIONS.md`, "thermal").

use super::FormatError;
use super::UnknownColumns;
use super::pcb_rows::row_field_error;
use super::units::{NonFiniteField, mm_to_nm, nm_to_mm};
use crate::pad_thermal::PadThermal;

pub(in crate::format) const THERMAL_COLUMN: &str = "thermal";

/// The cell of `thermal`; `None` when every value follows the pour. A value
/// the reader would refuse is not written, so the board can always reopen.
pub(in crate::format) fn thermal_cell(
    thermal: &PadThermal,
) -> Result<Option<String>, NonFiniteField> {
    if thermal.is_rule() {
        return Ok(None);
    }
    if thermal.fault().is_some() {
        return Err(NonFiniteField(THERMAL_COLUMN));
    }
    let length = |v: Option<f64>| -> Result<String, NonFiniteField> {
        v.map(|mm| mm_to_nm(mm, THERMAL_COLUMN).map(|nm| nm.to_string()))
            .transpose()
            .map(Option::unwrap_or_default)
    };
    let spokes = thermal.spokes.map(|n| n.to_string()).unwrap_or_default();
    Ok(Some(format!(
        "{};{};{}",
        length(thermal.air_gap)?,
        length(thermal.spoke_width)?,
        spokes
    )))
}

fn read(text: &str) -> Option<PadThermal> {
    let mut parts = text.split(';');
    let (gap, width, spokes) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() {
        return None;
    }
    let length = |part: &str| -> Option<Option<f64>> {
        match part.trim() {
            "" => Some(None),
            nm => nm.parse::<i64>().ok().map(|nm| Some(nm_to_mm(nm))),
        }
    };
    let spokes = match spokes.trim() {
        "" => None,
        n => Some(n.parse::<u8>().ok()?),
    };
    let thermal = PadThermal {
        air_gap: length(gap)?,
        spoke_width: length(width)?,
        spokes,
    };
    thermal.fault().is_none().then_some(thermal)
}

/// A cell as a pad's values; an empty or absent cell follows the pour. A
/// cell that does not read, or holds a value no pour can use, is a load
/// error naming it.
pub(in crate::format) fn read_thermal(
    cell: Option<&str>,
    block: &str,
    row: usize,
) -> Result<PadThermal, FormatError> {
    match cell {
        None | Some("") => Ok(PadThermal::RULE),
        Some(text) => read(text).ok_or_else(|| {
            row_field_error(
                block,
                row,
                THERMAL_COLUMN,
                text,
                "a pad's thermal relief must be <air gap>;<spoke width>;<spokes>, lengths in \
                 whole nm above 0, spokes 1 to 4, a part left empty to follow the pour",
            )
        }),
    }
}

/// Write the cell of `thermal` into a row's foreign columns.
pub(in crate::format) fn with_thermal_cell(
    mut columns: UnknownColumns,
    thermal: &PadThermal,
) -> Result<UnknownColumns, NonFiniteField> {
    if let Some(cell) = thermal_cell(thermal)? {
        columns.set(THERMAL_COLUMN, cell);
    }
    Ok(columns)
}

/// Take the cell out of a row's foreign columns.
pub(in crate::format) fn take_thermal_cell(
    columns: &mut UnknownColumns,
    block: &str,
    row: usize,
) -> Result<PadThermal, FormatError> {
    read_thermal(columns.take(THERMAL_COLUMN).as_deref(), block, row)
}
