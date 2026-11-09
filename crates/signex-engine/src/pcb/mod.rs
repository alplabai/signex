//! Pure board edit commands (#2177).
//!
//! Each [`BoardCommand`] is applied to a [`PcbBoard`](signex_types::pcb::PcbBoard)
//! by [`BoardCommand::apply`], which returns the command that undoes it and a
//! [`BoardPatch`] naming what changed. Nothing here does IO. [`BoardEngine`] hosts a board and its
//! undo history; it is the only way an edit reaches a board (#661 slice 7).
//!
//! Commands carry absolute targets (a position, a rotation, an outline), never
//! deltas: `x + d - d` is not `x` in `f64`, and the inverse must restore the
//! board bit for bit.

mod batch;
mod command;
mod engine;
mod error;
mod footprint;
mod footprint_content;
mod items;

#[cfg(test)]
mod content_tests;
#[cfg(test)]
mod engine_tests;
#[cfg(test)]
mod tests;

pub use batch::{BatchRefusal, check_batch};
pub use command::{Applied, BoardCommand, BoardPatch, BoardPatchKind};
pub use engine::BoardEngine;
pub use error::BoardCommandError;
pub use footprint::FootprintFace;
pub use footprint_content::FootprintText;
pub use items::{BoardItem, BoardItemId, RemovedItem};
