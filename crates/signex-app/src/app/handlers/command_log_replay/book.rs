//! What each Command Log row took, for Replay. App state beside the log: an
//! editor's input is a GUI message, which `app::core` cannot name.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use pcb_geom::ItemRef;
use serde_json::Value;
use signex_engine::Command;
use signex_engine::pcb::BoardCommand;
use signex_types::pcb::Point;
use signex_types::schematic::SelectedItem;
use uuid::Uuid;

use super::editors::EditorTargets;
use super::scope::ContinuesAs;
use crate::library::messages::PrimitiveEdit;

/// Longest text of a recorded input in a row's arguments.
const SUMMARY_CHARS: usize = 600;

/// The finished input of one operation.
#[derive(Debug, Clone)]
pub(super) enum ReplayInput {
    /// Engine commands on a sheet, and the items they act on.
    Sheet {
        commands: Vec<Command>,
        targets: Vec<SelectedItem>,
    },
    /// Board commands, and the items they act on.
    Board {
        commands: Vec<BoardCommand>,
        targets: Vec<ItemRef>,
    },
    /// Footprints carried by an offset.
    BoardMove {
        footprints: BTreeSet<Uuid>,
        delta: Point,
        leaves_tracks: bool,
    },
    /// Messages to an editor document, on its recorded selection.
    Editor {
        path: PathBuf,
        targets: EditorTargets,
        messages: Vec<PrimitiveEdit>,
    },
    /// The operation made new objects from values only its dialog or tool
    /// held; its commands name ids that now exist.
    MadeNewObjects,
}

impl ReplayInput {
    /// The input as the row's detail shows it.
    pub(super) fn summary(&self) -> Value {
        let text = match self {
            Self::Sheet { commands, .. } => format!("{commands:?}"),
            Self::Board { commands, .. } => format!("{commands:?}"),
            Self::BoardMove {
                footprints, delta, ..
            } => format!(
                "{} footprints by ({}, {})",
                footprints.len(),
                delta.x,
                delta.y
            ),
            Self::Editor { messages, .. } => format!("{messages:?}"),
            Self::MadeNewObjects => "new objects".to_string(),
        };
        Value::String(text.chars().take(SUMMARY_CHARS).collect())
    }

    /// `self` and `next`, run in one dispatch, as one input.
    fn merged(self, next: Self) -> Self {
        match (self, next) {
            (
                Self::Sheet {
                    mut commands,
                    mut targets,
                },
                Self::Sheet {
                    commands: more,
                    targets: more_targets,
                },
            ) => {
                commands.extend(more);
                targets.extend(more_targets);
                targets.sort_by_key(|item| item.uuid);
                targets.dedup();
                Self::Sheet { commands, targets }
            }
            (
                Self::Board {
                    mut commands,
                    mut targets,
                },
                Self::Board {
                    commands: more,
                    targets: more_targets,
                },
            ) => {
                commands.extend(more);
                targets.extend(more_targets);
                targets.sort();
                targets.dedup();
                Self::Board { commands, targets }
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
                },
                Self::Sheet {
                    commands: b,
                    targets: bt,
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
            _ => false,
        }
    }
}

/// Whether both are one edit of the same pin's number, or of its name.
fn same_pin_field(a: &[PrimitiveEdit], b: &[PrimitiveEdit]) -> bool {
    use crate::library::messages::SymbolEditorMsg as M;
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
    pub(super) takes: ContinuesAs,
    pub(super) document: Option<PathBuf>,
}

/// The recorded inputs and the recording in progress.
#[derive(Debug, Clone, Default)]
pub(crate) struct ReplayBook {
    inputs: BTreeMap<u64, Arc<ReplayInput>>,
    /// Inside a logged dispatch: the edits it made.
    pub(super) capturing: Option<Vec<ReplayInput>>,
    pub(super) pending: Option<PendingTool>,
    /// A Replay is running: what it does is not recorded again.
    pub(super) replaying: bool,
    /// The offset of the board drop about to run.
    pub(super) board_move: Option<ReplayInput>,
    /// The next board edit is a Properties panel edit.
    pub(super) board_properties: bool,
    /// The last point of a symbol editor drag.
    pub(super) symbol_drag: Option<(PathBuf, f64, f64)>,
    /// The newest row made by a panel edit, and when.
    pub(super) last_gesture: Option<(u64, Instant)>,
}

impl ReplayBook {
    pub(super) fn input_of(&self, seq: u64) -> Option<Arc<ReplayInput>> {
        self.inputs.get(&seq).cloned()
    }

    /// Keep `input` for row `seq`, dropping rows older than `oldest`.
    pub(super) fn keep(&mut self, seq: u64, input: Arc<ReplayInput>, oldest: Option<u64>) {
        if let Some(oldest) = oldest {
            self.inputs = self.inputs.split_off(&oldest);
        }
        self.inputs.insert(seq, input);
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
        self.board_move = None;
        self.board_properties = false;
    }

    /// The next board edit comes from a Properties panel.
    pub(crate) fn mark_board_properties_edit(&mut self) {
        self.board_properties = true;
    }

    /// The board drop about to run moves `footprints` by `delta`.
    pub(crate) fn note_board_drop(
        &mut self,
        footprints: BTreeSet<Uuid>,
        delta: Point,
        leaves_tracks: bool,
    ) {
        self.board_move = Some(ReplayInput::BoardMove {
            footprints,
            delta,
            leaves_tracks,
        });
    }
}
