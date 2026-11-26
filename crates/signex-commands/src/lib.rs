//! The command vocabulary shared by the application core and every front end
//! (ADR-0006, epic #2326): the types a command, its shortcut and its operands
//! are described with. UI-free: front ends convert their toolkit's input into
//! these types on their side.

pub mod catalog;
mod command_id;
mod document_target;
#[cfg(test)]
mod modifier_tests;
mod outcome;
mod pcb_targets;
mod platform;
mod shortcut;
#[cfg(test)]
mod spelling_tests;

pub use command_id::{AppCommandId, EXTENSION_ID_PREFIX, is_extension_id, is_extension_name};
pub use document_target::DocumentTarget;
pub use outcome::CommandOutcome;
pub use pcb_targets::PcbTargets;
pub use platform::Platform;
pub use shortcut::{Modifiers, ShortcutContext};
