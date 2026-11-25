//! The board accessors of `DocumentState`, reached from the tests under
//! `tests/`. The accessors stay `pub(crate)`, so they are not signex-app API;
//! this trait exists only in test builds (#2449). Inside the crate the
//! inherent methods win, so unit tests never see it.

use std::path::Path;

use signex_engine::pcb::BoardEngine;

use crate::app::DocumentState;

/// Test-only access to the PCB board engines a `DocumentState` holds.
pub trait BoardEngines {
    /// See `DocumentState::board_engine_mut`.
    fn board_engine_mut(&mut self, path: &Path) -> Option<&mut BoardEngine>;
    /// See `DocumentState::tab_board_engine`.
    fn tab_board_engine(&self, idx: usize) -> Option<&BoardEngine>;
    /// See `DocumentState::first_tab_board_engine`.
    fn first_tab_board_engine(&self, path: &Path) -> Option<&BoardEngine>;
}

impl BoardEngines for DocumentState {
    fn board_engine_mut(&mut self, path: &Path) -> Option<&mut BoardEngine> {
        DocumentState::board_engine_mut(self, path)
    }

    fn tab_board_engine(&self, idx: usize) -> Option<&BoardEngine> {
        DocumentState::tab_board_engine(self, idx)
    }

    fn first_tab_board_engine(&self, path: &Path) -> Option<&BoardEngine> {
        DocumentState::first_tab_board_engine(self, path)
    }
}
