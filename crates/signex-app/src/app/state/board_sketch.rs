//! Board sketch mode (#2234): the working copy of one board's sketch while
//! the user edits it. Nothing here touches the board or the disk; leaving
//! the mode or Apply bakes the copy and commits it as one board command.
//! See `docs/PCB_SKETCH.md`, "Sketch mode".

use std::path::{Path, PathBuf};
use std::sync::Arc;

use signex_library::{Footprint, FootprintFile};
use signex_sketch::SketchData;
use signex_types::pcb::PcbBoard;

use crate::app::FootprintDocument;
use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
use crate::library::editor::footprint::sketch_mode::SketchEdit;
use crate::library::editor::footprint::state::{BoardBackdrop, EditorMode};

/// One board in sketch mode. The sketch is edited by the footprint
/// editor's sketch tool, hosted on a footprint that exists only here: its
/// one footprint holds the working sketch, and its history is the mode's
/// undo. The footprint is never saved or baked.
pub struct BoardSketchSession {
    /// The board being sketched; also the key the editor's messages carry.
    pub board_path: PathBuf,
    /// The sketch editor over the working copy.
    pub editor: Box<FootprintDocument>,
    /// Why the last commit was refused, shown in the mode's bar until the
    /// next commit.
    pub refusal: Option<String>,
    /// The sketch as the board last took it (or had it at entry).
    start: SketchData,
}

impl BoardSketchSession {
    /// Start sketch mode on `board`: a working copy of its sketch (an empty
    /// one when it has none), solved once so the DOF row has a state.
    pub fn open(board_path: &Path, board: &PcbBoard) -> Self {
        let sketch = board
            .sketch
            .as_ref()
            .map(|stored| stored.sketch.clone())
            .unwrap_or_default();
        let name = board_path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut host = Footprint::empty(name);
        host.sketch = Some(sketch);
        let mut editor = FootprintDocument::reloaded(
            board_path.to_path_buf(),
            FootprintFile::from_footprint(host),
        );
        editor.state.mode = EditorMode::Sketch;
        editor.state.board_backdrop = Some(Arc::new(BoardBackdrop::from_board(board)));
        editor.with_parts(|state, primitive| {
            apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
        });
        // The first solve is not an edit: the copy starts equal to the board.
        editor.mark_saved();
        let start = editor.primitive().sketch.clone().unwrap_or_default();
        Self {
            board_path: board_path.to_path_buf(),
            editor: Box::new(editor),
            refusal: None,
            start,
        }
    }

    /// The sketch as the user has it now.
    pub fn working_sketch(&self) -> SketchData {
        self.editor.primitive().sketch.clone().unwrap_or_default()
    }

    /// Whether the copy differs from what the board last took. Compared by
    /// content, so picking a tool or undoing back to the start is no change.
    pub fn has_changes(&self) -> bool {
        self.editor.primitive().sketch.as_ref() != Some(&self.start)
    }

    /// The board took the copy and stored it as `stored`, which also records
    /// what the bake owns (`baked_kinds`): it is the new starting point.
    pub fn mark_committed(&mut self, stored: SketchData) {
        self.editor.primitive_mut().sketch = Some(stored.clone());
        self.editor.mark_saved();
        self.editor.canvas_cache.clear();
        self.start = stored;
        self.refusal = None;
    }
}
