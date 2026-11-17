//! The board Net Classes panel's own state (#2400): the cell typed and not
//! applied yet and why the last edit was refused, both for one board. It
//! lives in the Model, so a panel-context rebuild never drops it.

use std::path::{Path, PathBuf};

use crate::panels::BoardNetClassDraft;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoardNetClassPanelState {
    board: Option<PathBuf>,
    pub draft: Option<BoardNetClassDraft>,
    pub notice: Option<String>,
}

impl BoardNetClassPanelState {
    /// The state of `board`; another board's draft and notice are dropped.
    pub fn for_board(&mut self, board: &Path) -> &mut Self {
        if self.board.as_deref() != Some(board) {
            *self = Self {
                board: Some(board.to_path_buf()),
                ..Self::default()
            };
        }
        self
    }

    /// The draft and notice, when they are `board`'s.
    pub fn of(&self, board: &Path) -> (Option<BoardNetClassDraft>, Option<String>) {
        if self.board.as_deref() == Some(board) {
            (self.draft.clone(), self.notice.clone())
        } else {
            (None, None)
        }
    }
}
