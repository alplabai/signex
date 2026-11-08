//! Pure board edit commands (#2177).
//!
//! Each [`BoardCommand`] is applied to a [`PcbBoard`](signex_types::pcb::PcbBoard)
//! by [`BoardCommand::apply`], which returns the command that undoes it and a
//! [`BoardPatch`] naming what changed. Nothing here does IO or owns history;
//! hosting the commands on an undo stack waits for the Document model (#661).
//!
//! Commands carry absolute targets (a position, a rotation, an outline), never
//! deltas: `x + d - d` is not `x` in `f64`, and the inverse must restore the
//! board bit for bit.

mod command;
mod error;
mod footprint;
mod items;

#[cfg(test)]
mod tests;

pub use command::{Applied, BoardCommand, BoardPatch, BoardPatchKind};
pub use error::BoardCommandError;
pub use footprint::FootprintFace;
pub use items::{BoardItem, BoardItemId, RemovedItem};
