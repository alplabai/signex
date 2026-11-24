//! Board sketch mode (#2234): the working copy of one board's sketch while
//! the user edits it. Nothing here touches the board or the disk; Bake
//! bakes the copy and commits it as one board command, Discard changes
//! drops it (#2518). See `docs/PCB_SKETCH.md`, "Sketch mode".

use std::path::{Path, PathBuf};
use std::sync::Arc;

use signex_bake::BoardBakeError;
use signex_library::{Footprint, FootprintFile};
use signex_sketch::SketchData;
use signex_types::pcb::PcbBoard;
use signex_types::pcb_layer::{CopperLayer, MechanicalLayer, PcbLayer};
use signex_types::pcb_origin::BoardOrigin;
use signex_types::pcb_sketch::BakedBoard;
use signex_types::sketch::SketchHost;
use signex_types::sketch::board_attr::SketchBoardItems;

use crate::app::FootprintDocument;
use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
use crate::library::editor::footprint::sketch_mode::{BakePreview, SketchEdit};
use crate::library::editor::footprint::{BoardBackdrop, EditorMode};
use crate::panels::{BoardTargetProps, ComponentChoice, LayerChoice};

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
    /// Why the open preview's bake refuses, with the entity it names: the
    /// preview card selects it on a click (mockup row 190).
    pub preview_error: Option<BoardBakeError>,
    /// The open Import DXF dialog (mockup row 179c); the sketch is unchanged
    /// until its Import.
    pub dxf_dialog: Option<Box<super::SketchDxfDialog>>,
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
            preview_error: None,
            dxf_dialog: None,
            start,
        }
    }

    /// Follow the board's origin and whether its view hides the marker.
    pub fn follow_board_origin(&mut self, origin: BoardOrigin, hidden: bool) {
        let Some(backdrop) = self.editor.state.board_backdrop.as_mut() else {
            return;
        };
        if backdrop.origin == origin && backdrop.hides_origin_marker == hidden {
            return;
        }
        let backdrop = Arc::make_mut(backdrop);
        backdrop.origin = origin;
        backdrop.hides_origin_marker = hidden;
        self.editor.canvas_cache.clear();
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

    /// Bake the copy for the preview against `board` (its nets and placed
    /// components), when the preview is open; close it otherwise. Pure: the
    /// board is not touched. The bake also goes to the canvas, which draws
    /// it (mockup row 190).
    pub fn refresh_preview(&mut self, board: Option<&PcbBoard>) {
        let open = self.editor.state.sketch_exit.preview;
        let bake = open.then(|| preview_bake(&self.working_sketch(), board));
        let drawn = self.editor.state.sketch_exit.board_result.is_some();
        self.preview = bake.as_ref().map(board_preview);
        self.preview_error = bake.as_ref().and_then(|bake| bake.as_ref().err().cloned());
        let (baked, items) = match bake {
            Some(Ok((baked, items))) => (Some(Arc::new(baked)), Some(Arc::new(items))),
            _ => (None, None),
        };
        self.editor.state.sketch_exit.board_result = baked;
        self.editor.state.sketch_exit.board_items = items;
        if open || drawn {
            self.editor.canvas_cache.clear();
        }
    }

    /// The board target rows of the selected entity (decision 2), with the
    /// choices `board` offers: its copper and mechanical layers and its
    /// placed components.
    pub fn target_props(&self, board: &PcbBoard) -> BoardTargetProps {
        let editor = &self.editor;
        let sketch = editor.primitive().sketch.as_ref();
        let entity = editor
            .state
            .selected_sketch
            .and_then(|id| sketch?.entities.iter().find(|e| e.id == id));
        let (Some(sketch), Some(entity)) = (sketch, entity) else {
            return BoardTargetProps::None;
        };
        if entity.board_slot.is_some() {
            let size = editor.state.last_solve.as_ref().and_then(|solve| {
                signex_bake::board::slot_size(sketch, solve, entity.id)
                    .map(|slot| (slot.length, slot.width))
            });
            return BoardTargetProps::Slot { size };
        }
        if let Some(pour) = entity.pour.as_ref() {
            return BoardTargetProps::Pour {
                layer: pour.layer,
                layers: copper_choices(board),
            };
        }
        if let Some(drawing) = entity.mechanical {
            return BoardTargetProps::Mechanical {
                layer: drawing.layer,
                layers: mechanical_choices(board),
            };
        }
        if let Some(anchor) = entity.component_anchor {
            let mut components: Vec<ComponentChoice> = board
                .footprints
                .iter()
                .map(|fp| ComponentChoice {
                    footprint: fp.uuid,
                    reference: fp.reference.clone(),
                })
                .collect();
            components.sort_by(|a, b| a.reference.cmp(&b.reference));
            return BoardTargetProps::Anchor {
                picked: anchor.footprint,
                components,
            };
        }
        BoardTargetProps::None
    }

    /// The board took the copy and stored it as `stored`, which also records
    /// what the bake owns (`baked_kinds`): it is the new starting point.
    /// `board` is the board after the commit: the backdrop and the outline
    /// facts in Properties follow its new outline.
    pub fn mark_committed(&mut self, stored: SketchData, board: &PcbBoard) {
        let hidden = self
            .editor
            .state
            .board_backdrop
            .as_ref()
            .is_some_and(|backdrop| backdrop.hides_origin_marker);
        self.editor.state.board_backdrop = Some(Arc::new(BoardBackdrop {
            hides_origin_marker: hidden,
            ..BoardBackdrop::from_board(board)
        }));
        self.editor.primitive_mut().sketch = Some(stored.clone());
        self.editor.mark_saved();
        self.editor.canvas_cache.clear();
        self.start = stored;
        self.refusal = None;
    }
}

type PreviewBake = Result<(BakedBoard, SketchBoardItems), BoardBakeError>;

/// The preview's bake: with the board, its pours, drawings and anchors
/// too; without one (it is closing), the sketch's own geometry.
fn preview_bake(sketch: &SketchData, board: Option<&PcbBoard>) -> PreviewBake {
    match board {
        Some(board) => signex_bake::board::bake_board_with_items(sketch, board),
        None => signex_bake::bake_board(sketch, &[]).map(|baked| (baked, Default::default())),
    }
}

/// What a board bake writes: a count per board target, or why the bake
/// refuses.
fn board_preview(bake: &PreviewBake) -> BakePreview {
    use signex_bake::board::{baked_count, baked_item_count};
    match bake {
        Ok((baked, items)) => BakePreview::Counts(
            SketchHost::Board
                .bake_targets()
                .iter()
                .filter_map(|&kind| {
                    let count = baked_count(baked, kind).or_else(|| baked_item_count(items, kind));
                    count.map(|n| (kind, n))
                })
                .collect(),
        ),
        Err(error) => BakePreview::Refused(error.to_string()),
    }
}

/// The copper layers a board pour may take: the board's own, by its names,
/// or top and bottom for a board without a layer list.
fn copper_choices(board: &PcbBoard) -> Vec<LayerChoice> {
    let listed: Vec<LayerChoice> = board
        .layers
        .iter()
        .filter_map(|def| {
            let layer = def.pcb_layer().filter(|layer| layer.is_copper())?;
            Some(LayerChoice {
                layer,
                name: def.display_label().to_string(),
            })
        })
        .collect();
    if !listed.is_empty() {
        return listed;
    }
    [CopperLayer::Top, CopperLayer::Bottom]
        .into_iter()
        .map(|copper| {
            let layer = PcbLayer::Copper(copper);
            LayerChoice {
                layer,
                name: signex_types::format::pcb_layer_token(layer),
            }
        })
        .collect()
}

/// The mechanical layers a drawing may take: those the board's layer list
/// names, else every one, 1 to [`MechanicalLayer::MAX`].
fn mechanical_choices(board: &PcbBoard) -> Vec<MechanicalLayer> {
    let mut listed: Vec<MechanicalLayer> = board
        .layers
        .iter()
        .filter_map(|def| match def.pcb_layer() {
            Some(PcbLayer::Mechanical(layer)) => Some(layer),
            _ => None,
        })
        .collect();
    if listed.is_empty() {
        listed = (1..=MechanicalLayer::MAX)
            .filter_map(MechanicalLayer::new)
            .collect();
    }
    listed.sort();
    listed.dedup();
    listed
}
