//! What each Command Log row took, for Replay. App state beside the log: an
//! editor's input is a GUI message, which `app::core` cannot name.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use pcb_geom::ItemRef;
use serde_json::Value;
use signex_engine::Command;
use signex_engine::pcb::{BoardCommand, BoardReplayGuard};
use signex_types::corner::CornerRule;
use signex_types::pcb::{Footprint, Point};
use signex_types::schematic::SelectedItem;
use uuid::Uuid;

use super::editors::EditorTargets;
use super::scope::ContinuesAs;
use super::sheet_guard::{SheetTargetGuard, TargetGuard};
use crate::library::messages::{FootprintEditorMsg, PrimitiveEdit, SymbolEditorMsg};
use crate::panels::PanelMsg;

/// Longest text of a recorded input in a row's arguments.
const SUMMARY_CHARS: usize = 600;

/// What a board drop carried and how (#2513): Replay rebuilds the same move
/// from it, the tracks that follow bent at the same corner.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BoardDrop {
    pub(crate) footprints: BTreeSet<Uuid>,
    pub(crate) vias: BTreeSet<Uuid>,
    /// Tracks, arcs, areas, strings, dimensions and drawings (#2870).
    pub(crate) items: BTreeSet<Uuid>,
    pub(crate) rooms: BTreeSet<Uuid>,
    pub(crate) delta: Point,
    pub(crate) leaves_tracks: bool,
    /// The corner the tracks that follow bend at, so a replay bends them as
    /// the drop did whatever the router's rule is by then.
    pub(crate) corner: CornerRule,
}

impl BoardDrop {
    /// A drop of `footprints` alone.
    pub(crate) fn of_footprints(
        footprints: BTreeSet<Uuid>,
        delta: Point,
        leaves_tracks: bool,
        corner: CornerRule,
    ) -> Self {
        Self {
            footprints,
            vias: BTreeSet::new(),
            items: BTreeSet::new(),
            rooms: BTreeSet::new(),
            delta,
            leaves_tracks,
            corner,
        }
    }

    /// How many objects of each kind it carried, for the row's detail.
    fn summary(&self) -> String {
        let counts = [
            (self.footprints.len(), "footprints"),
            (self.vias.len(), "vias"),
            (self.items.len(), "objects"),
            (self.rooms.len(), "rooms"),
        ];
        let carried: Vec<String> = (counts.iter())
            .filter(|(n, _)| *n > 0)
            .map(|(n, kind)| format!("{n} {kind}"))
            .collect();
        let carried = if carried.is_empty() {
            "0 footprints".to_string()
        } else {
            carried.join(", ")
        };
        format!("{carried} by ({}, {})", self.delta.x, self.delta.y)
    }
}

/// The finished input of one operation.
#[derive(Debug, Clone)]
pub(super) enum ReplayInput {
    /// Engine commands on a sheet, and the items they act on.
    Sheet {
        commands: Vec<Command>,
        targets: Vec<SelectedItem>,
        /// What the commands that replace a whole object replaced, oldest
        /// first: Replay carries only what they changed (`rebase`).
        pre_images: Vec<Command>,
        /// For a move: its targets before and after.
        guard: Option<SheetTargetGuard>,
    },
    /// Board commands, and the items they act on.
    Board {
        commands: Vec<BoardCommand>,
        targets: Vec<ItemRef>,
        /// The touched targets before and after: Replay refuses once one
        /// holds anything else.
        guard: BoardReplayGuard,
    },
    /// Board objects carried by an offset: a part, via, track or mixed drop.
    BoardMove {
        drop: BoardDrop,
        /// What the move wrote, before and after; set once it has run.
        guard: Option<BoardReplayGuard>,
    },
    /// A reuse block placed around an anchor; a replay makes new objects.
    ReuseBlock {
        name: String,
        anchor: Uuid,
        degrees: f64,
    },
    /// Footprints given a library footprint. A replay runs the swap again,
    /// with its linked sheets, as one undo step per document.
    FootprintSwap {
        footprints: Vec<Uuid>,
        template: Box<Footprint>,
    },
    /// Messages to an editor document, on its recorded selection.
    Editor {
        path: PathBuf,
        targets: EditorTargets,
        messages: Vec<PrimitiveEdit>,
    },
    /// A footprint editor pad dragged from `start` to `to`.
    PadDrag {
        path: PathBuf,
        targets: EditorTargets,
        idx: usize,
        start: (f64, f64),
        to: (f64, f64),
    },
    /// A Properties panel edit of an editor document.
    EditorPanel {
        path: PathBuf,
        targets: EditorTargets,
        message: Box<PanelMsg>,
    },
    /// The operation made new objects from values only its tool held, and
    /// has no recorded form that can mint new ids.
    MadeNewObjects,
}

impl ReplayInput {
    /// The input as the row's detail shows it.
    pub(super) fn summary(&self) -> Value {
        let text = match self {
            Self::Sheet { commands, .. } => format!("{commands:?}"),
            Self::Board { commands, .. } => format!("{commands:?}"),
            Self::BoardMove { drop, .. } => drop.summary(),
            Self::ReuseBlock {
                name,
                anchor,
                degrees,
            } => format!("block {name} on {anchor} at {degrees}"),
            Self::FootprintSwap {
                footprints,
                template,
            } => format!(
                "{} footprints to {}",
                footprints.len(),
                template.footprint_id
            ),
            Self::Editor { messages, .. } => format!("{messages:?}"),
            Self::PadDrag { idx, start, to, .. } => {
                format!("pad {idx} by ({}, {})", to.0 - start.0, to.1 - start.1)
            }
            Self::EditorPanel { message, .. } => format!("{message:?}"),
            Self::MadeNewObjects => "new objects".to_string(),
        };
        Value::String(text.chars().take(SUMMARY_CHARS).collect())
    }

    /// The catalog command a row of this input is named after, when the
    /// input says it.
    pub(super) fn row_command(&self) -> Option<&'static str> {
        match self {
            Self::BoardMove { .. } => Some("move_selection"),
            Self::ReuseBlock { .. } => Some("place_board_reuse_block"),
            Self::FootprintSwap { .. } => Some("change_footprint"),
            _ => None,
        }
    }

    /// `self` and `next`, run in one dispatch, as one input.
    fn merged(self, next: Self) -> Self {
        match (self, next) {
            (
                Self::Sheet {
                    mut commands,
                    mut targets,
                    mut pre_images,
                    guard,
                },
                Self::Sheet {
                    commands: more,
                    targets: more_targets,
                    pre_images: more_images,
                    guard: more_guard,
                },
            ) => {
                commands.extend(more);
                targets.extend(more_targets);
                targets.sort_by_key(|item| item.uuid);
                targets.dedup();
                pre_images.extend(more_images);
                Self::Sheet {
                    commands,
                    targets,
                    pre_images,
                    guard: guard.zip(more_guard).map(|(a, b)| a.merged(b)),
                }
            }
            (
                Self::Board {
                    mut commands,
                    mut targets,
                    guard,
                },
                Self::Board {
                    commands: more,
                    targets: more_targets,
                    guard: more_guard,
                },
            ) => {
                commands.extend(more);
                targets.extend(more_targets);
                targets.sort();
                targets.dedup();
                Self::Board {
                    commands,
                    targets,
                    guard: guard.merged(more_guard),
                }
            }
            _ => Self::MadeNewObjects,
        }
    }

    /// Merge what one dispatch recorded.
    pub(super) fn merge_all(inputs: Vec<Self>) -> Option<Self> {
        inputs.into_iter().reduce(Self::merged)
    }

    /// Whether `other` is the next keystroke of the same field edit.
    pub(super) fn same_edit_as(&self, other: &Self) -> bool {
        let kinds = |commands: &[Command]| -> Vec<std::mem::Discriminant<Command>> {
            commands.iter().map(std::mem::discriminant).collect()
        };
        match (self, other) {
            (
                Self::Sheet {
                    commands: a,
                    targets: at,
                    ..
                },
                Self::Sheet {
                    commands: b,
                    targets: bt,
                    ..
                },
            ) => at == bt && kinds(a) == kinds(b),
            (
                Self::Editor {
                    path: a,
                    messages: am,
                    ..
                },
                Self::Editor {
                    path: b,
                    messages: bm,
                    ..
                },
            ) => a == b && same_pin_field(am, bm),
            (
                Self::EditorPanel {
                    path: a,
                    targets: at,
                    message: am,
                },
                Self::EditorPanel {
                    path: b,
                    targets: bt,
                    message: bm,
                },
            ) => {
                a == b
                    && at == bt
                    && std::mem::discriminant(am.as_ref()) == std::mem::discriminant(bm.as_ref())
            }
            _ => false,
        }
    }

    /// `self`, the next keystroke of `earlier`'s edit, keeping what stood
    /// before the first keystroke.
    pub(super) fn continuing(self, earlier: &Self) -> Self {
        match (self, earlier) {
            (
                Self::Sheet {
                    commands,
                    targets,
                    pre_images,
                    guard,
                },
                Self::Sheet {
                    pre_images: first, ..
                },
            ) => {
                let mut kept = first.clone();
                kept.extend(pre_images);
                Self::Sheet {
                    commands,
                    targets,
                    pre_images: kept,
                    guard,
                }
            }
            (input, _) => input,
        }
    }

    /// `next`, the next tick of the drag `self` recorded, as one input from
    /// the drag's start. `None` when it is not the same drag.
    pub(super) fn drag_continued_by(&self, next: &Self) -> Option<Self> {
        match (self, next) {
            (
                Self::PadDrag {
                    path,
                    targets,
                    idx,
                    start,
                    ..
                },
                Self::PadDrag {
                    path: next_path,
                    idx: next_idx,
                    to,
                    ..
                },
            ) if path == next_path && idx == next_idx => Some(Self::PadDrag {
                path: path.clone(),
                targets: targets.clone(),
                idx: *idx,
                start: *start,
                to: *to,
            }),
            (
                Self::Editor {
                    path,
                    targets,
                    messages,
                },
                Self::Editor {
                    path: next_path,
                    messages: next_messages,
                    ..
                },
            ) if path == next_path => {
                let total = Self::joined_sketch_move(messages, next_messages)?;
                Some(Self::Editor {
                    path: path.clone(),
                    targets: targets.clone(),
                    messages: vec![PrimitiveEdit::Footprint(total)],
                })
            }
            _ => None,
        }
    }

    /// Two sketch drag ticks on the same entity as one move.
    fn joined_sketch_move(
        first: &[PrimitiveEdit],
        next: &[PrimitiveEdit],
    ) -> Option<FootprintEditorMsg> {
        use FootprintEditorMsg as M;
        match (first, next) {
            (
                [PrimitiveEdit::Footprint(M::SketchMovePoint { id, dx, dy })],
                [
                    PrimitiveEdit::Footprint(M::SketchMovePoint {
                        id: next,
                        dx: more_x,
                        dy: more_y,
                    }),
                ],
            ) if id == next => Some(M::SketchMovePoint {
                id: *id,
                dx: dx + more_x,
                dy: dy + more_y,
            }),
            (
                [PrimitiveEdit::Footprint(M::SketchMoveLine { id, dx, dy })],
                [
                    PrimitiveEdit::Footprint(M::SketchMoveLine {
                        id: next,
                        dx: more_x,
                        dy: more_y,
                    }),
                ],
            ) if id == next => Some(M::SketchMoveLine {
                id: *id,
                dx: dx + more_x,
                dy: dy + more_y,
            }),
            _ => None,
        }
    }

    /// Whether this is one tick of a drag, which may add no undo step of its
    /// own after the first.
    pub(super) fn is_drag_tick(&self) -> bool {
        use FootprintEditorMsg as M;
        match self {
            Self::PadDrag { .. } => true,
            Self::Editor { messages, .. } => matches!(
                messages.as_slice(),
                [PrimitiveEdit::Footprint(
                    M::SketchMovePoint { .. } | M::SketchMoveLine { .. }
                )]
            ),
            _ => false,
        }
    }
}

/// Whether both are one edit of the same pin's number, or of its name.
fn same_pin_field(a: &[PrimitiveEdit], b: &[PrimitiveEdit]) -> bool {
    use SymbolEditorMsg as M;
    match (a, b) {
        (
            [PrimitiveEdit::Symbol(M::SetPinNumber { idx: x, .. })],
            [PrimitiveEdit::Symbol(M::SetPinNumber { idx: y, .. })],
        )
        | (
            [PrimitiveEdit::Symbol(M::SetPinName { idx: x, .. })],
            [PrimitiveEdit::Symbol(M::SetPinName { idx: y, .. })],
        ) => x == y,
        _ => false,
    }
}

/// A row whose tool is still running, waiting for its finished input.
#[derive(Debug, Clone)]
pub(super) struct PendingTool {
    pub(super) seq: u64,
    pub(super) command: String,
    pub(super) takes: ContinuesAs,
    pub(super) document: Option<PathBuf>,
}

/// The recorded inputs and the recording in progress.
#[derive(Debug, Clone, Default)]
pub(crate) struct ReplayBook {
    inputs: BTreeMap<u64, Arc<ReplayInput>>,
    /// What each row with operands wrote on a board, for its Replay.
    operand_guards: BTreeMap<u64, Arc<BoardReplayGuard>>,
    /// What each Delete or Duplicate row found on its recorded targets.
    target_guards: BTreeMap<u64, Arc<TargetGuard>>,
    /// Inside a logged dispatch: the edits it made.
    pub(super) capturing: Option<Vec<ReplayInput>>,
    pub(super) pending: Option<PendingTool>,
    /// A Replay is running: what it does is not recorded again.
    pub(super) replaying: bool,
    /// What the board edit about to run is, when its commands do not say.
    pub(super) board_input: Option<ReplayInput>,
    /// The next board edit is a Properties panel edit.
    pub(super) board_properties: bool,
    /// The last point of a symbol editor drag.
    pub(super) symbol_drag: Option<(PathBuf, f64, f64)>,
    /// The newest row made by a panel edit or drag, and when.
    pub(super) last_gesture: Option<(u64, Instant)>,
    /// A Properties panel edit is running: it is the one recorded.
    pub(super) panel_edit_running: bool,
    /// Change Footprint is between its picker and its read.
    pub(super) swap_in_transit: bool,
    /// The sheet of each paste still waiting for its designator numbers.
    waiting_pastes: Vec<PathBuf>,
    /// How deep `Signex::update` is nested.
    update_depth: u32,
}

impl ReplayBook {
    /// A paste on `sheet` started reading its numbers: it places later.
    pub(crate) fn note_paste_waiting(&mut self, sheet: PathBuf) {
        self.waiting_pastes.push(sheet);
    }

    /// The numbers of one paste on `sheet` came back, used or dropped.
    pub(crate) fn note_paste_answered(&mut self, sheet: Option<&std::path::Path>) {
        let at = (self.waiting_pastes.iter()).position(|path| Some(path.as_path()) == sheet);
        if let Some(at) = at {
            self.waiting_pastes.remove(at);
        }
    }

    pub(super) fn has_waiting_paste(&self, sheet: &std::path::Path) -> bool {
        self.waiting_pastes.iter().any(|path| path == sheet)
    }

    pub(super) fn input_of(&self, seq: u64) -> Option<Arc<ReplayInput>> {
        self.inputs.get(&seq).cloned()
    }

    /// Whether row `seq` holds a recorded input.
    #[cfg(test)]
    pub(in crate::app::handlers) fn has_input(&self, seq: u64) -> bool {
        self.input_of(seq).is_some()
    }

    /// Keep `input` for row `seq`, dropping rows older than `oldest`.
    pub(super) fn keep(&mut self, seq: u64, input: Arc<ReplayInput>, oldest: Option<u64>) {
        if let Some(oldest) = oldest {
            self.inputs = self.inputs.split_off(&oldest);
        }
        self.inputs.insert(seq, input);
    }

    pub(super) fn operand_guard_of(&self, seq: u64) -> Option<Arc<BoardReplayGuard>> {
        self.operand_guards.get(&seq).cloned()
    }

    /// Keep `guard` for row `seq`, dropping rows older than `oldest`.
    pub(super) fn keep_operand_guard(
        &mut self,
        seq: u64,
        guard: Arc<BoardReplayGuard>,
        oldest: Option<u64>,
    ) {
        if let Some(oldest) = oldest {
            self.operand_guards = self.operand_guards.split_off(&oldest);
        }
        self.operand_guards.insert(seq, guard);
    }

    pub(super) fn target_guard_of(&self, seq: u64) -> Option<Arc<TargetGuard>> {
        self.target_guards.get(&seq).cloned()
    }

    /// Keep `guard` for row `seq`, dropping rows older than `oldest`.
    pub(super) fn keep_target_guard(
        &mut self,
        seq: u64,
        guard: Arc<TargetGuard>,
        oldest: Option<u64>,
    ) {
        if let Some(oldest) = oldest {
            self.target_guards = self.target_guards.split_off(&oldest);
        }
        self.target_guards.insert(seq, guard);
    }

    /// A Move was cancelled: no later drag belongs to its row.
    pub(crate) fn drop_pending_move(&mut self) {
        if self
            .pending
            .as_ref()
            .is_some_and(|tool| tool.takes == ContinuesAs::Move)
        {
            self.pending = None;
        }
    }

    /// The board edit these marks were for did not land.
    pub(crate) fn forget_board_marks(&mut self) {
        self.board_input = None;
        self.board_properties = false;
    }

    /// The next board edit comes from a Properties panel.
    pub(crate) fn mark_board_properties_edit(&mut self) {
        self.board_properties = true;
    }

    /// The board drop about to run moves `footprints` by `delta`, the
    /// tracks on their pads bent at `corner`.
    pub(crate) fn note_board_drop(
        &mut self,
        footprints: BTreeSet<Uuid>,
        delta: Point,
        leaves_tracks: bool,
        corner: CornerRule,
    ) {
        self.note_drop(BoardDrop::of_footprints(
            footprints,
            delta,
            leaves_tracks,
            corner,
        ));
    }

    /// The board drop about to run carries `drop`: a via, track or mixed
    /// drop as well as a part's (#2513).
    pub(crate) fn note_drop(&mut self, drop: BoardDrop) {
        self.board_input = Some(ReplayInput::BoardMove { drop, guard: None });
    }

    /// The board edit about to run places the reuse block `name`.
    pub(crate) fn note_reuse_block(&mut self, name: &str, anchor: Uuid, degrees: f64) {
        self.board_input = Some(ReplayInput::ReuseBlock {
            name: name.trim().to_string(),
            anchor,
            degrees,
        });
    }

    /// The swap about to run gives `footprints` the footprint `template`.
    pub(crate) fn note_footprint_swap(&mut self, footprints: Vec<Uuid>, template: Box<Footprint>) {
        self.swap_in_transit = false;
        self.board_input = Some(ReplayInput::FootprintSwap {
            footprints,
            template,
        });
    }

    /// Change Footprint's picker is about to open, or its pick is being read.
    pub(crate) fn note_swap_in_transit(&mut self, in_transit: bool) {
        self.swap_in_transit = in_transit;
    }

    /// `Signex::update` starts; whether it is the outermost.
    pub(crate) fn enter_update(&mut self) {
        self.update_depth += 1;
    }

    /// `Signex::update` ends; whether it was the outermost.
    pub(crate) fn leave_update(&mut self) -> bool {
        self.update_depth = self.update_depth.saturating_sub(1);
        self.update_depth == 0
    }
}
