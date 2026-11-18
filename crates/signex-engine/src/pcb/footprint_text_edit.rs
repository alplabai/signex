//! A footprint's designator or comment moved, turned, sized or justified on
//! its own (#2368): one [`BoardCommand::SetFootprintProperty`], the
//! position and turn kept footprint-local so the text still follows its
//! footprint. Pure: the caller runs the command as one undo step. A locked
//! footprint refuses, as for its other placement edits, and so does one that
//! locks its strings.

use std::collections::BTreeSet;

use pcb_geom::{BoardTextField, GeomError};
use signex_types::pcb::{Footprint, PcbBoard, Point};
use signex_types::schematic::HAlign;
use uuid::Uuid;

use super::command::BoardCommand;
use super::placement::{PlacementRefusal, editable_footprints};

/// What to change of the text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FootprintTextEdit {
    /// Move its anchor by an offset on the board, mm.
    MoveBy(Point),
    /// Put its anchor at a board point, mm.
    At(Point),
    /// Turn its baseline to an angle on the board, degrees, in the sense a
    /// pad turns.
    Rotation(f64),
    /// Its cap height, mm.
    Height(f64),
    Justify(HAlign),
}

/// Why the text was left as it is.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum FootprintTextRefusal {
    #[error(transparent)]
    Placement(#[from] PlacementRefusal),
    #[error("{designator} has no {field} text")]
    Missing {
        designator: String,
        field: &'static str,
    },
    #[error("the text cannot go there: {0}")]
    Geometry(GeomError),
    #[error("a text height is a length above zero")]
    BadHeight,
    #[error("the strings of {0} are locked")]
    StringsLocked(String),
}

/// `footprint` when its texts may be moved: it is on the board, not locked,
/// and does not lock its strings (#2365).
pub fn text_movable(board: &PcbBoard, footprint: Uuid) -> Result<&Footprint, FootprintTextRefusal> {
    let found = editable_footprints(board, &BTreeSet::from([footprint]))?;
    let Some(fp) = found.first().copied() else {
        return Err(PlacementRefusal::NothingSelected.into());
    };
    if fp.component.lock_strings {
        return Err(FootprintTextRefusal::StringsLocked(fp.reference.clone()));
    }
    Ok(fp)
}

/// The command for `edit` on `field` of `footprint`; `None` when it changes
/// nothing.
pub fn footprint_text_command(
    board: &PcbBoard,
    footprint: Uuid,
    field: BoardTextField,
    edit: FootprintTextEdit,
) -> Result<Option<BoardCommand>, FootprintTextRefusal> {
    let fp = text_movable(board, footprint)?;
    let missing = || FootprintTextRefusal::Missing {
        designator: fp.reference.clone(),
        field: field.id(),
    };
    let index = pcb_geom::property_index(fp, field).ok_or_else(missing)?;
    let placed = pcb_geom::footprint_text(fp, field)
        .ok_or_else(missing)?
        .map_err(FootprintTextRefusal::Geometry)?;
    let mut property = fp.properties[index].clone();
    let geometry = FootprintTextRefusal::Geometry;
    match edit {
        FootprintTextEdit::MoveBy(delta) if delta.x == 0.0 && delta.y == 0.0 => return Ok(None),
        FootprintTextEdit::MoveBy(delta) => {
            let at = placed.placement.anchor.to_mm();
            let to = Point::new(at.x + delta.x, at.y + delta.y);
            property.position = Some(pcb_geom::local_text_position(fp, to).map_err(geometry)?);
        }
        FootprintTextEdit::At(to) => {
            property.position = Some(pcb_geom::local_text_position(fp, to).map_err(geometry)?);
        }
        FootprintTextEdit::Rotation(degrees) => {
            property.rotation = pcb_geom::local_text_rotation(fp, degrees).map_err(geometry)?;
        }
        FootprintTextEdit::Height(height) => {
            if !(height.is_finite() && height > 0.0) {
                return Err(FootprintTextRefusal::BadHeight);
            }
            property.font_size = Some(height);
        }
        FootprintTextEdit::Justify(justify) => property.justify = justify,
    }
    if property == fp.properties[index] {
        return Ok(None);
    }
    Ok(Some(BoardCommand::SetFootprintProperty {
        footprint,
        index,
        property,
    }))
}

#[cfg(test)]
#[path = "footprint_text_edit_tests.rs"]
mod tests;
