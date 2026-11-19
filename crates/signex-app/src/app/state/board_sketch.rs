//! Board sketch mode (#2234): the working copy of one board's sketch while
//! the user edits it. Nothing here touches the board or the disk; Bake
//! bakes the copy and commits it as one board command, Discard changes
//! drops it (#2518). See `docs/PCB_SKETCH.md`, "Sketch mode".

use std::path::{Path, PathBuf};
use std::sync::Arc;

use signex_library::{Footprint, FootprintFile};
use signex_sketch::SketchData;
use signex_types::pcb::{NetDef, PcbBoard};
use signex_types::sketch::SketchHost;

use crate::app::FootprintDocument;
use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
use crate::library::editor::footprint::sketch_mode::{BakePreview, SketchEdit};
use crate::library::editor::footprint::{BoardBackdrop, EditorMode};

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
    /// next commit; rendered in the language of the moment it is shown.
    pub refusal: Option<signex_i18n::Translatable>,
    /// What a bake of the copy writes, while the bake preview is open.
    pub preview: Option<BakePreview>,
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
        debug_assert_eq!(editor.state.sketch_host(), SketchHost::Board);
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
            preview: None,
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

    /// Bake the copy for the preview against the board's `nets`, when the
    /// preview is open; close it otherwise. Pure: the board is not touched.
    pub fn refresh_preview(&mut self, nets: &[NetDef]) {
        self.preview = self
            .editor
            .state
            .sketch_exit
            .preview
            .then(|| board_preview(&self.working_sketch(), nets));
    }

    /// The board took the copy and stored it as `stored`, which also records
    /// what the bake owns (`baked_kinds`): it is the new starting point.
    /// `board` is the board after the commit: the backdrop and the outline
    /// facts in Properties follow its new outline.
    pub fn mark_committed(&mut self, stored: SketchData, board: &PcbBoard) {
        self.editor.state.board_backdrop = Some(Arc::new(BoardBackdrop::from_board(board)));
        self.editor.primitive_mut().sketch = Some(stored.clone());
        self.editor.mark_saved();
        self.editor.canvas_cache.clear();
        self.start = stored;
        self.refusal = None;
    }
}

/// What a board bake of `sketch` writes: a count per board target, or why
/// the bake refuses.
fn board_preview(sketch: &SketchData, nets: &[NetDef]) -> BakePreview {
    match signex_bake::bake_board(sketch, nets) {
        Ok(baked) => BakePreview::Counts(
            SketchHost::Board
                .bake_targets()
                .iter()
                .filter_map(|&kind| {
                    signex_bake::board::baked_count(&baked, kind).map(|n| (kind, n))
                })
                .collect(),
        ),
        Err(error) => BakePreview::Refused(error.to_string()),
    }
}
