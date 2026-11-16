//! Optional per-row cells of the board's TSV blocks that ride the
//! foreign-column path: a column is written only when some row needs it,
//! so a board without one re-saves unchanged, and a build that predates it
//! keeps it as a foreign column. Pads: `rotation`, `net_locked` and
//! `layer_sizes`; tracks and vias: `locked`; vias: `layer_sizes` (#2401,
//! #2407). See `unknown_columns` for the contract.

use super::pcb_rows::row_field_error;
use super::tsv::format_f64;
use super::units::{NonFiniteField, mm_to_nm, nm_to_mm};
use super::{FormatError, UnknownColumns};
use crate::pcb::{LayerSize, LayerSizes, Point};
use crate::pcb_stack::size_storable;

/// Header of the optional `rotation` pad column: the pad's own turn in
/// degrees. Rides the foreign-column path like `net_locked`; written only
/// when some pad is turned, so an unturned board re-saves unchanged.
pub(in crate::format) const PAD_ROTATION_COLUMN: &str = "rotation";

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
pub(in crate::format) const NET_LOCKED_COLUMN: &str = "net_locked";

/// The cell a locked pad writes; every other pad of the block writes empty.
const NET_LOCKED_CELL: &str = "true";

pub(in crate::format) fn with_net_locked_cell(
    mut columns: UnknownColumns,
    locked: bool,
) -> UnknownColumns {
    if locked {
        columns.set(NET_LOCKED_COLUMN, NET_LOCKED_CELL.to_string());
    }
    columns
}

/// `true` locks and an empty cell is an unlocked pad; any other value is not
/// ours to read, so it stays a foreign cell and round-trips untouched.
pub(in crate::format) fn take_net_locked_cell(columns: &mut UnknownColumns) -> bool {
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

/// Header of the optional `locked` track and via column (#2407): `true`
/// for a locked item, an empty cell for the others of a block that has one.
pub(in crate::format) const LOCKED_COLUMN: &str = "locked";

pub(in crate::format) fn with_locked_cell(
    mut columns: UnknownColumns,
    locked: bool,
) -> UnknownColumns {
    if locked {
        columns.set(LOCKED_COLUMN, NET_LOCKED_CELL.to_string());
    }
    columns
}

/// `true` locks and an empty cell does not; any other value is not ours to
/// read, so it stays a foreign cell and round-trips untouched.
pub(in crate::format) fn take_locked_cell(columns: &mut UnknownColumns) -> bool {
    match columns.get(LOCKED_COLUMN) {
        Some(NET_LOCKED_CELL) => {
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

/// Header of the optional `layer_sizes` via and pad column (#2401): the
/// copper sizes that differ by layer, in nm, as `top=<v>;middle=<v>;bottom=<v>`
/// or `<layer token>=<v>;...` per layer. A via size is one number, a pad
/// size `<x>x<y>`. Absent when no item has per-layer sizes, an empty cell
/// for an item without them in a block that has some.
pub(in crate::format) const LAYER_SIZES_COLUMN: &str = "layer_sizes";

const TOP_MIDDLE_BOTTOM: [&str; 3] = ["top", "middle", "bottom"];

/// One size as a cell value: whole nm, positive.
pub(in crate::format) trait SizeCell: Copy + Sized {
    fn write(self) -> Result<String, NonFiniteField>;
    fn read(text: &str) -> Option<Self>;
}

/// The engine refuses an edit to a size [`size_storable`] refuses, so this
/// refusal only meets a board built outside it.
fn positive_nm(mm: f64) -> Result<i64, NonFiniteField> {
    if !size_storable(mm) {
        return Err(NonFiniteField(LAYER_SIZES_COLUMN));
    }
    mm_to_nm(mm, LAYER_SIZES_COLUMN)
}

fn read_nm(text: &str) -> Option<f64> {
    let nm: i64 = text.parse().ok()?;
    (nm > 0).then(|| nm_to_mm(nm))
}

impl SizeCell for f64 {
    fn write(self) -> Result<String, NonFiniteField> {
        Ok(positive_nm(self)?.to_string())
    }

    fn read(text: &str) -> Option<Self> {
        read_nm(text)
    }
}

impl SizeCell for Point {
    fn write(self) -> Result<String, NonFiniteField> {
        Ok(format!("{}x{}", positive_nm(self.x)?, positive_nm(self.y)?))
    }

    fn read(text: &str) -> Option<Self> {
        let (x, y) = text.split_once('x')?;
        Some(Point::new(read_nm(x)?, read_nm(y)?))
    }
}

pub(in crate::format) fn layer_sizes_cell<T: SizeCell>(
    sizes: &LayerSizes<T>,
) -> Result<String, NonFiniteField> {
    // The list rule the engine checks (`LayerSizes::list_fault`).
    if sizes.list_fault().is_some() {
        return Err(NonFiniteField(LAYER_SIZES_COLUMN));
    }
    let entries: Vec<(String, T)> = match sizes {
        LayerSizes::TopMiddleBottom {
            top,
            middle,
            bottom,
        } => TOP_MIDDLE_BOTTOM
            .iter()
            .map(|k| k.to_string())
            .zip([*top, *middle, *bottom])
            .collect(),
        LayerSizes::Full { layers } => layers.iter().map(|l| (l.layer.clone(), l.size)).collect(),
    };
    let mut parts = Vec::with_capacity(entries.len());
    for (key, size) in entries {
        parts.push(format!("{key}={}", size.write()?));
    }
    Ok(parts.join(";"))
}

/// The sizes a cell holds; `None` for a cell that is not one.
pub(in crate::format) fn read_layer_sizes<T: SizeCell>(cell: &str) -> Option<LayerSizes<T>> {
    let mut entries = Vec::new();
    for part in cell.split(';') {
        let (key, value) = part.split_once('=')?;
        entries.push((key, T::read(value)?));
    }
    let keys: Vec<&str> = entries.iter().map(|(k, _)| *k).collect();
    if keys == TOP_MIDDLE_BOTTOM {
        return Some(LayerSizes::TopMiddleBottom {
            top: entries[0].1,
            middle: entries[1].1,
            bottom: entries[2].1,
        });
    }
    let layers = entries
        .into_iter()
        .map(|(key, size)| LayerSize {
            layer: key.to_string(),
            size,
        })
        .collect();
    let sizes = LayerSizes::Full { layers };
    sizes.list_fault().is_none().then_some(sizes)
}

pub(in crate::format) fn with_layer_sizes_cell<T: SizeCell>(
    mut columns: UnknownColumns,
    sizes: Option<&LayerSizes<T>>,
) -> Result<UnknownColumns, NonFiniteField> {
    if let Some(sizes) = sizes {
        columns.set(LAYER_SIZES_COLUMN, layer_sizes_cell(sizes)?);
    }
    Ok(columns)
}

/// An empty cell is "one size on every layer". Any other cell must read; a
/// malformed one is a load error, as reading it as no sizes would draw and
/// check copper that is not there.
pub(in crate::format) fn take_layer_sizes_cell<T: SizeCell>(
    columns: &mut UnknownColumns,
    block: &str,
    row: usize,
) -> Result<Option<LayerSizes<T>>, FormatError> {
    match columns.take(LAYER_SIZES_COLUMN) {
        None => Ok(None),
        Some(cell) if cell.is_empty() => Ok(None),
        Some(cell) => read_layer_sizes(&cell).map(Some).ok_or_else(|| {
            row_field_error(
                block,
                row,
                LAYER_SIZES_COLUMN,
                &cell,
                "per-layer sizes must be top=..;middle=..;bottom=.. or <copper layer>=..;.. \
                 with positive whole-nm sizes",
            )
        }),
    }
}
