//! The engines of PCB tabs closed while dirty; `state/pcb_board.rs` reads and
//! writes them.

use std::path::PathBuf;

use signex_engine::pcb::BoardEngine;

/// The engines of closed dirty PCB tabs, by path.
pub(crate) type ParkedBoards = std::collections::HashMap<PathBuf, BoardEngine>;
