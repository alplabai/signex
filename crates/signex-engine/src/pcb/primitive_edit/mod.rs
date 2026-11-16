//! The command behind one field of a track's, via's, pad's, area's or
//! drawing's Properties (#2355, #2393, #2424): the edit applied to every
//! selected item at once, built as one
//! [`BoardCommand::ReplaceItems`] or [`BoardCommand::ReplacePads`], so it is
//! one undo step. Each builder returns `None` when every item already reads
//! so. Targets are absolute per item, as for every board command.

mod graphic;
mod pad;
mod track;
mod via;
mod zone;

#[cfg(test)]
mod graphic_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod zone_tests;

pub use graphic::{GraphicEdit, apply_graphic_edit, graphic_edit_command};
pub use pad::{PadEdit, pad_edit_command};
pub use track::{TrackEdit, track_edit_command, track_location};
pub use via::{ViaEdit, via_edit_command};
pub use zone::{ZoneEdit, apply_zone_edit, zone_edit_command, zone_layer_allowed};

/// Why a field edit was not turned into a command. The board is unchanged.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum PrimitiveEditRefusal {
    #[error("an item of the selection is no longer on the board")]
    Gone,
    #[error("nothing is selected")]
    Empty,
    #[error("{0} is locked")]
    Locked(String),
    #[error("the pads and graphics of {0} are locked")]
    PrimitivesLocked(String),
    #[error("pad {0} has no hole")]
    NoHole(String),
    #[error("the value must be a finite number")]
    NonFinite,
    #[error("the new position is outside the board's range")]
    OutOfRange,
    #[error("the value must not be below zero")]
    Negative,
    #[error("the area cannot be on that layer")]
    LayerNotAllowed,
    #[error("the value must be above zero")]
    NotPositive,
    #[error("a drawing goes on a non-copper layer other than a keep-out")]
    GraphicLayerNotAllowed,
}

fn finite(value: f64) -> Result<f64, PrimitiveEditRefusal> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(PrimitiveEditRefusal::NonFinite)
    }
}
