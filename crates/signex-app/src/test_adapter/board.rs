//! The board accessors of `DocumentState`, reached from the tests under
//! `tests/`. The accessors stay `pub(crate)`, so they are not signex-app API;
//! this trait exists only in test builds (#2449). Inside the crate the
//! inherent methods win, so unit tests never see it.

use std::path::{Path, PathBuf};

use signex_engine::pcb::BoardEngine;

use crate::app::{DocumentState, TabInfo, TabKind};

/// A tab of `kind` at `path` that holds no board, for a test to push. A
/// `TabKind::Pcb` one is a PCB tab whose board is absent (as after a failed
/// History restore): a board lives only in `DocumentState::boards`, which
/// this does not touch. Open a real board with `open_pcb_tab`.
pub fn boardless_tab(title: impl Into<String>, path: impl Into<PathBuf>, kind: TabKind) -> TabInfo {
    TabInfo {
        title: title.into(),
        path: path.into(),
        project_id: None,
        kind,
    }
}

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
