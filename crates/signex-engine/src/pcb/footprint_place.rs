//! Footprints put at absolute places, by id (#2936): the board edits of
//! `set_footprint_position`, a whole list in one undo step.
//!
//! Pure: [`position_commands`] reads a board and returns the commands, which
//! the caller runs as one undo step. Each footprint moves as a drag moves it
//! ([`placement::move_commands`]): the track ends on its pads follow, bent
//! at the corner rule. A list is built one footprint at a time on a copy of
//! the board, so a track between two moved footprints follows both.

use std::collections::BTreeSet;

use signex_types::corner::CornerRule;
use signex_types::object_id::IdSource;
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use super::command::BoardCommand;
use super::error::BoardCommandError;
use super::placement::{self, PlacementRefusal};

/// One footprint and the board point its origin goes to, in the model's
/// frame (mm, Y down).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FootprintPlace {
    pub footprint: Uuid,
    pub to: Point,
}

/// Why a list of places did nothing, beyond a placement refusal.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum PlaceRefusal {
    #[error(transparent)]
    Placement(#[from] PlacementRefusal),
    #[error("footprint {0} is named twice")]
    Repeated(Uuid),
}

impl From<BoardCommandError> for PlaceRefusal {
    fn from(error: BoardCommandError) -> Self {
        Self::Placement(PlacementRefusal::Command(error))
    }
}

/// The commands that put each footprint of `places` at its point, the track
/// ends on its pads following at `corner`. Refused whole, nothing built,
/// when a footprint is missing, locked or named twice. A footprint already
/// at its point gets no command.
pub fn position_commands(
    board: &PcbBoard,
    places: &[FootprintPlace],
    corner: CornerRule,
    new_ids: &mut dyn IdSource,
) -> Result<Vec<BoardCommand>, PlaceRefusal> {
    let mut named = BTreeSet::new();
    for place in places {
        if !named.insert(place.footprint) {
            return Err(PlaceRefusal::Repeated(place.footprint));
        }
    }
    placement::editable_footprints(board, &named)?;
    let mut scratch = board.clone();
    let mut commands = Vec::new();
    for place in places {
        let at = scratch
            .footprints
            .iter()
            .find(|fp| fp.uuid == place.footprint)
            .map(|fp| fp.position)
            .ok_or(BoardCommandError::FootprintNotFound(place.footprint))?;
        let delta = Point::new(place.to.x - at.x, place.to.y - at.y);
        let one = BTreeSet::from([place.footprint]);
        let mut built = placement::move_commands(&scratch, &one, delta, corner, new_ids)?;
        for command in &mut built {
            // The point as given: `x + (t - x)` need not be `t` in `f64`.
            if let BoardCommand::MoveFootprint { footprint, to } = command
                && *footprint == place.footprint
            {
                *to = place.to;
            }
        }
        for command in &built {
            command.clone().apply(&mut scratch)?;
        }
        commands.extend(built);
    }
    Ok(commands)
}

#[cfg(test)]
#[path = "footprint_place_tests.rs"]
mod tests;
