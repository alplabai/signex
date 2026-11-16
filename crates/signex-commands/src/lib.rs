//! The command vocabulary shared by the application core and every front end
//! (ADR-0006, epic #2326): the types a command, its shortcut and its operands
//! are described with. UI-free: front ends convert their toolkit's input into
//! these types on their side.

mod command_id;
mod document_target;
#[cfg(test)]
mod modifier_tests;
mod pcb_targets;
mod platform;
mod shortcut;
#[cfg(test)]
mod spelling_tests;

pub use command_id::AppCommandId;
pub use document_target::DocumentTarget;
pub use pcb_targets::PcbTargets;
pub use platform::Platform;
pub use shortcut::{Modifiers, ShortcutContext};
