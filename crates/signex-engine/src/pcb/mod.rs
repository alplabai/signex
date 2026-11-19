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

pub mod align;
mod batch;
mod board_sketch;
pub mod board_tools;
mod clipboard;
mod command;
pub mod component;
pub mod component_edit;
pub mod designators;
pub mod draw;
mod draw_free;
#[cfg(test)]
mod draw_free_tests;
pub mod dxf;
pub mod eco;
mod engine;
mod error;
pub mod find;
mod footprint;
mod footprint_content;
pub mod footprint_swap;
pub mod footprint_text_edit;
pub mod graphic;
mod graphics;
mod items;
pub mod lock;
mod options;
pub mod pad_swap;
mod panel;
pub mod part_place;
pub mod placement;
pub mod pour_edit;
pub mod primitive_edit;
mod replace;
pub mod reuse_block;
pub mod route;
pub mod similar;
pub mod snap;
mod snap_points;
mod stack;
pub mod stack_edit;
pub mod via_types;

#[cfg(test)]
mod board_sketch_tests;
#[cfg(test)]
mod content_tests;
#[cfg(test)]
mod engine_tests;
#[cfg(test)]
mod grid_tests;
#[cfg(test)]
mod pad_shape_tests;
#[cfg(test)]
mod replace_tests;
#[cfg(test)]
mod tests;

pub use batch::BatchRefusal;
pub use clipboard::BoardClipboard;
pub use command::{Applied, BoardCommand, BoardPatch, BoardPatchKind};
pub use engine::BoardEngine;
pub use error::BoardCommandError;
pub use footprint::FootprintFace;
pub use footprint_content::FootprintText;
pub use items::{BoardItem, BoardItemId, RemovedItem};
pub use replace::FootprintPad;
pub use stack::{ItemLayerSizes, StackSnapshot};
