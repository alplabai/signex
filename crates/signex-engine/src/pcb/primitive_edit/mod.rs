//! The command behind one field of a track's, via's, pad's, area's or
//! drawing's Properties (#2355, #2393, #2424): the edit applied to every
//! selected item at once, built as one
//! [`BoardCommand::ReplaceItems`] or [`BoardCommand::ReplacePads`], so it is
//! one undo step. Each builder returns `None` when every item already reads
//! so. Targets are absolute per item, as for every board command.

mod dimension_style;
mod free;
mod graphic;
mod pad;
mod track;
mod via;
mod zone;

#[cfg(test)]
mod dimension_style_tests;
#[cfg(test)]
mod free_tests;
#[cfg(test)]
mod graphic_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod zone_tests;

pub use dimension_style::{DimensionStyleEdit, apply_style};
pub use free::{
    ArcEdit, ArcSpan, DIMENSION_MAX_PRECISION, DimensionEdit, TextEdit, apply_arc, apply_dimension,
    apply_free_pad, apply_text, arc_edit_command, dimension_edit_command, free_pad_edit_command,
    text_edit_command,
};
pub use graphic::{GraphicEdit, apply_graphic_edit, graphic_edit_command};
pub use pad::{PadEdit, pad_edit_command};
pub use track::{TrackEdit, track_edit_command, track_location};
pub(crate) use via::edit_vias;
pub use via::{ViaEdit, via_edit_command};
pub use zone::{
    ZoneEdit, apply_zone_edit, zone_edit_command, zone_edits_command, zone_layer_allowed,
};

use signex_types::pcb::MaskExpansion;

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
    #[error("the value must be a whole number")]
    NotWhole,
    #[error("the area cannot be on that layer")]
    LayerNotAllowed,
    /// A selected track or via is locked and the edit is not its unlock.
    #[error("a selected track or via is locked")]
    ItemLocked,
    /// An arc edit that leaves no arc: a zero radius or turn (#2373).
    #[error("the arc would have no radius or turn")]
    NoArc,
    /// A dimension edit that puts both its ends on one point (#2373).
    #[error("the dimension would measure nothing")]
    NoLength,
    #[error("the value must be above zero")]
    NotPositive,
    #[error("a drawing goes on any layer but a keep-out")]
    GraphicLayerNotAllowed,
    /// A trapezoid or custom pad the edit would leave with no copper (#2242).
    #[error("{0}")]
    PadShape(signex_types::pcb_pad_shape::PadShapeFault),
    /// A pad thermal relief value no pour can use (#2593).
    #[error("{0}")]
    PadThermal(signex_types::pad_thermal::PadThermalFault),
}

fn finite(value: f64) -> Result<f64, PrimitiveEditRefusal> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(PrimitiveEditRefusal::NonFinite)
    }
}

fn finite_mask(mask: MaskExpansion) -> Result<MaskExpansion, PrimitiveEditRefusal> {
    if mask.is_finite() {
        Ok(mask)
    } else {
        Err(PrimitiveEditRefusal::NonFinite)
    }
}
