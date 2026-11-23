//! Replay in the footprint and symbol editors. Their items are named by
//! index, so a target is recorded with the pad or pin number it held: an
//! index that now holds another number counts as gone.

use std::path::Path;
use std::time::{Duration, Instant};

use iced::Task;
use signex_sketch::id::SketchEntityId;

use super::book::ReplayInput;
use super::run::{no_change, refused};
use super::scope::EditFamily;
use crate::app::command::CommandOutcome;
use crate::app::{FootprintDocument, Message, Signex, SymbolDocument};
use crate::library::editor::footprint::EditorPad;
use crate::library::editor::symbol::SymbolSelection;
use crate::library::messages::{FootprintEditorMsg, PrimitiveEdit, SymbolEditorMsg};

/// Drag ticks this close together are one drag and one row.
const DRAG_RUN: Duration = Duration::from_secs(2);

/// An editor document's selection as recorded.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum EditorTargets {
    Footprint {
        footprint: usize,
        primary: Option<usize>,
        extra: Vec<usize>,
        pads: Vec<(usize, String)>,
        /// The silk item selected, and how many there were: its index means
        /// the same item only while the count holds.
        silk: Option<(usize, usize)>,
        sketch: Option<SketchEntityId>,
        sketch_secondary: Option<SketchEntityId>,
        sketch_extra: Vec<SketchEntityId>,
    },
    Symbol {
        symbol: usize,
        selection: Option<SymbolSelection>,
        pins: Vec<(usize, String)>,
        graphics: Vec<usize>,
    },
}

/// An editor message about to run that may finish an edit.
pub(crate) struct EditorEditProbe {
    history: (u64, usize),
    input: ReplayInput,
    family: EditFamily,
    row: &'static str,
}

/// The editor selection set aside while Replay runs.
pub(super) enum HeldEditor {
    Footprint {
        primary: Option<usize>,
        extra: Vec<(usize, String)>,
        primary_number: Option<String>,
        silk: Option<usize>,
        sketch: Option<SketchEntityId>,
        sketch_secondary: Option<SketchEntityId>,
        sketch_extra: Vec<SketchEntityId>,
    },
    Symbol {
        selection: Option<SymbolSelection>,
        pins: usize,
        graphics: usize,
    },
}

/// What a footprint message records: the row it is named after, its family
/// and whether it acts on the selection.
fn footprint_edit(msg: &FootprintEditorMsg) -> Option<(&'static str, EditFamily, bool)> {
    use FootprintEditorMsg as M;
    Some(match msg {
        M::DeleteSelected | M::DeleteSilkF => ("delete_selection", EditFamily::Other, true),
        M::ActiveBarRotateSelection => ("rotate_selection", EditFamily::Other, true),
        M::ActiveBarFlipSelection => ("flip_selection", EditFamily::Other, true),
        M::ActiveBarNudgeSelection | M::ActiveBarAlignSelectionToGrid | M::MoveByConfirm => {
            ("move_selection", EditFamily::Move, true)
        }
        M::MovePad { .. } | M::SketchMovePoint { .. } | M::SketchMoveLine { .. } => {
            ("move_selection", EditFamily::Move, false)
        }
        M::SketchAddConstraintForSelection(_) | M::SketchMakePadFromProfile => {
            ("edit_object_properties", EditFamily::Properties, true)
        }
        M::SketchSetRole { .. }
        | M::SketchUnlinkCornerRadius { .. }
        | M::SketchResizeRoundPad { .. }
        | M::SketchCloseGapRequested { .. }
        | M::SketchAddFilletsRequested
        | M::SketchProfileRolePicked { .. }
        | M::SketchEditParameter { .. } => {
            ("edit_object_properties", EditFamily::Properties, false)
        }
        _ => return None,
    })
}

/// The selection of the footprint editor `editor`, as recorded.
pub(super) fn footprint_targets(editor: &FootprintDocument) -> EditorTargets {
    let state = &editor.state;
    let pads = state
        .selected_pad_indices()
        .into_iter()
        .map(|i| (i, state.pads[i].number.clone()))
        .collect();
    EditorTargets::Footprint {
        footprint: editor.active_idx,
        primary: state.selected_pad,
        extra: state.selected_pads_extra.clone(),
        pads,
        silk: state
            .selected_silk_f
            .map(|i| (i, editor.primitive().silk_f.len())),
        sketch: state.selected_sketch,
        sketch_secondary: state.selected_sketch_secondary,
        sketch_extra: state.selected_sketch_extra.clone(),
    }
}

/// The selection of the symbol editor `editor`, as recorded.
pub(super) fn symbol_targets(editor: &SymbolDocument) -> EditorTargets {
    let (pins, graphics) = symbol_selection_items(editor, editor.selected.as_ref());
    EditorTargets::Symbol {
        symbol: editor.active_idx,
        selection: editor.selected.clone(),
        pins,
        graphics,
    }
}

impl EditorTargets {
    /// Whether nothing is selected.
    fn is_empty(&self) -> bool {
        match self {
            Self::Footprint {
                pads,
                silk,
                sketch,
                sketch_secondary,
                sketch_extra,
                ..
            } => {
                pads.is_empty()
                    && silk.is_none()
                    && sketch.is_none()
                    && sketch_secondary.is_none()
                    && sketch_extra.is_empty()
            }
            Self::Symbol { selection, .. } => selection.is_none(),
        }
    }
}

/// The recorded input of the footprint message `msg`.
fn footprint_input(
    path: &Path,
    editor: &FootprintDocument,
    mut targets: EditorTargets,
    msg: &FootprintEditorMsg,
) -> Option<ReplayInput> {
    use FootprintEditorMsg as M;
    let messages = match msg {
        M::MovePad { idx, x_mm, y_mm } => {
            let pad = editor.state.pads.get(*idx)?;
            if let EditorTargets::Footprint { pads, .. } = &mut targets
                && !pads.iter().any(|(i, _)| i == idx)
            {
                pads.push((*idx, pad.number.clone()));
            }
            return Some(ReplayInput::PadDrag {
                path: path.to_path_buf(),
                targets,
                idx: *idx,
                start: pad.position_mm,
                to: (*x_mm, *y_mm),
            });
        }
        M::MoveByConfirm => {
            let modal = editor.state.move_by_modal.as_ref()?;
            vec![
                M::MoveByOpen,
                M::MoveBySetX(modal.dx_buf.clone()),
                M::MoveBySetY(modal.dy_buf.clone()),
                M::MoveByConfirm,
            ]
        }
        other => vec![other.clone()],
    };
    Some(ReplayInput::Editor {
        path: path.to_path_buf(),
        targets,
        messages: messages.into_iter().map(PrimitiveEdit::Footprint).collect(),
    })
}

/// The same for a symbol message: `true` when it needs a selection.
fn symbol_edit(msg: &SymbolEditorMsg) -> Option<(&'static str, EditFamily, bool)> {
    use SymbolEditorMsg as M;
    Some(match msg {
        M::DeleteSelected => ("delete_selection", EditFamily::Other, true),
        M::RotateSelected { .. } => ("rotate_selection", EditFamily::Other, true),
        M::AlignSelectedToGrid => ("move_selection", EditFamily::Move, true),
        M::DragCommit => ("move_selection", EditFamily::Move, true),
        M::MoveAll { .. } => ("move_selection", EditFamily::Move, false),
        M::SetPinNumber { .. } | M::SetPinName { .. } => {
            ("edit_object_properties", EditFamily::Properties, false)
        }
        _ => return None,
    })
}

impl Signex {
    /// Read what `msg` would record, before it runs on the footprint editor
    /// at `path`.
    pub(crate) fn probe_footprint_edit(
        &self,
        path: &Path,
        msg: &FootprintEditorMsg,
    ) -> Option<EditorEditProbe> {
        if !self.records_replay_input() || self.ui_state.command_replay.panel_edit_running {
            return None;
        }
        let (row, family, needs_selection) = footprint_edit(msg)?;
        let editor = self.document_state.footprint_editors.get(path)?;
        let targets = footprint_targets(editor);
        if needs_selection && targets.is_empty() {
            return None;
        }
        Some(EditorEditProbe {
            history: (editor.history.position() as u64, editor.history.undo_len()),
            input: footprint_input(path, editor, targets, msg)?,
            family,
            row,
        })
    }

    /// Read what `msg` would record, before it runs on the symbol editor at
    /// `path`. A drag's last point is kept for the commit that ends it.
    pub(crate) fn probe_symbol_edit(
        &mut self,
        path: &Path,
        msg: &SymbolEditorMsg,
    ) -> Option<EditorEditProbe> {
        if !self.records_replay_input() || self.ui_state.command_replay.panel_edit_running {
            return None;
        }
        if let SymbolEditorMsg::MoveSelected { x, y } = msg {
            self.ui_state.command_replay.symbol_drag = Some((path.to_path_buf(), *x, *y));
            return None;
        }
        let (row, family, needs_selection) = symbol_edit(msg)?;
        let drag = match msg {
            SymbolEditorMsg::DragCommit => self
                .ui_state
                .command_replay
                .symbol_drag
                .take()
                .filter(|(of, _, _)| of == path),
            _ => None,
        };
        if matches!(msg, SymbolEditorMsg::DragCommit) && drag.is_none() {
            return None;
        }
        let editor = self.document_state.symbol_editors.get(path)?;
        if needs_selection && editor.selected.is_none() {
            return None;
        }
        let mut messages = Vec::new();
        if let Some((_, x, y)) = drag {
            messages.push(PrimitiveEdit::Symbol(SymbolEditorMsg::MoveSelected {
                x,
                y,
            }));
        }
        messages.push(PrimitiveEdit::Symbol(msg.clone()));
        Some(EditorEditProbe {
            history: (editor.history_generation, 0),
            input: ReplayInput::Editor {
                path: path.to_path_buf(),
                targets: symbol_targets(editor),
                messages,
            },
            family,
            row,
        })
    }

    /// The message ran: when it made an undo step, record it. A later tick
    /// of a drag joins the row of its first.
    pub(crate) fn finish_editor_edit(&mut self, path: &Path, probe: Option<EditorEditProbe>) {
        let Some(probe) = probe else {
            return;
        };
        if probe.input.is_drag_tick() && self.continue_drag_row(path, probe.row, &probe.input) {
            return;
        }
        let now = self.editor_history(path);
        if now.is_none() || now == Some(probe.history) {
            return;
        }
        self.note_finished_edit(
            path.to_path_buf(),
            probe.family,
            Some(probe.row),
            probe.input,
        );
    }

    /// `tick` continues the drag of the newest row: that row takes it.
    fn continue_drag_row(&mut self, path: &Path, row: &str, tick: &ReplayInput) -> bool {
        let book = &self.ui_state.command_replay;
        let Some((seq, at)) = book.last_gesture else {
            return false;
        };
        let newest = self.ui_state.command_log.log.rows().next_back();
        let is_newest = newest.is_some_and(|new| {
            new.seq == seq && new.command == row && new.document.as_deref() == Some(path)
        });
        if !is_newest || at.elapsed() >= DRAG_RUN {
            return false;
        }
        let Some(joined) = book.input_of(seq).and_then(|i| i.drag_continued_by(tick)) else {
            return false;
        };
        self.ui_state.command_replay.last_gesture = Some((seq, Instant::now()));
        self.set_replay_input(seq, joined);
        true
    }

    /// The targets no longer in the editor at `path`; `None` when it is not
    /// open.
    pub(super) fn missing_editor_targets(
        &self,
        path: &Path,
        targets: &EditorTargets,
    ) -> Option<Vec<String>> {
        match targets {
            EditorTargets::Footprint {
                footprint,
                pads,
                silk,
                sketch,
                sketch_secondary,
                sketch_extra,
                ..
            } => {
                let editor = self.document_state.footprint_editors.get(path)?;
                let same = editor.active_idx == *footprint;
                let mut gone = gone_pads(editor, same, pads);
                let shape = editor.primitive();
                if let Some((index, count)) = silk
                    && (!same || shape.silk_f.len() != *count)
                {
                    gone.push(format!("Silk {index}"));
                }
                let ids = sketch.iter().chain(sketch_secondary).chain(sketch_extra);
                gone.extend(gone_sketch_entities(editor, same, ids));
                Some(gone)
            }
            EditorTargets::Symbol {
                symbol,
                pins,
                graphics,
                ..
            } => {
                let editor = self.document_state.symbol_editors.get(path)?;
                let same = editor.active_idx == *symbol;
                Some(gone_symbol_items(editor, same, pins, graphics))
            }
        }
    }

    /// Run `messages` on the recorded selection; the person's comes back.
    pub(super) fn run_editor_input(
        &mut self,
        path: &Path,
        targets: &EditorTargets,
        messages: &[PrimitiveEdit],
    ) -> (CommandOutcome, Task<Message>) {
        let before = self.editor_history(path);
        let Some(held) = self.put_editor_targets(path, targets) else {
            return (refused(no_change()), Task::none());
        };
        let tasks: Vec<Task<Message>> = messages
            .iter()
            .cloned()
            .map(|message| self.handle_primitive_editor_event(path.to_path_buf(), message))
            .collect();
        self.give_back_editor_selection(path, held);
        self.refresh_panel_ctx();
        let outcome = if self.editor_history(path) == before {
            refused(no_change())
        } else {
            CommandOutcome::Ran
        };
        (outcome, Task::batch(tasks))
    }

    pub(super) fn editor_history(&self, path: &Path) -> Option<(u64, usize)> {
        let docs = &self.document_state;
        docs.footprint_editors
            .get(path)
            .map(|e| (e.history.position() as u64, e.history.undo_len()))
            .or_else(|| {
                docs.symbol_editors
                    .get(path)
                    .map(|e| (e.history_generation, 0))
            })
    }

    pub(super) fn put_editor_targets(
        &mut self,
        path: &Path,
        targets: &EditorTargets,
    ) -> Option<HeldEditor> {
        match targets {
            EditorTargets::Footprint {
                primary,
                extra,
                silk,
                sketch,
                sketch_secondary,
                sketch_extra,
                ..
            } => {
                let state = &mut self.document_state.footprint_editors.get_mut(path)?.state;
                let held = HeldEditor::Footprint {
                    primary: state.selected_pad,
                    primary_number: state.selected_pad.and_then(|i| pad_number(&state.pads, i)),
                    extra: state
                        .selected_pads_extra
                        .iter()
                        .filter_map(|&i| Some((i, pad_number(&state.pads, i)?)))
                        .collect(),
                    silk: state.selected_silk_f,
                    sketch: state.selected_sketch,
                    sketch_secondary: state.selected_sketch_secondary,
                    sketch_extra: state.selected_sketch_extra.clone(),
                };
                state.selected_pad = *primary;
                state.selected_pads_extra = extra.clone();
                state.selected_silk_f = silk.map(|(index, _)| index);
                state.selected_sketch = *sketch;
                state.selected_sketch_secondary = *sketch_secondary;
                state.selected_sketch_extra = sketch_extra.clone();
                Some(held)
            }
            EditorTargets::Symbol { selection, .. } => {
                let editor = self.document_state.symbol_editors.get_mut(path)?;
                let held = HeldEditor::Symbol {
                    selection: editor.selected.clone(),
                    pins: editor.primitive().pins.len(),
                    graphics: editor.primitive().graphics.len(),
                };
                editor.selected = selection.clone();
                Some(held)
            }
        }
    }

    /// Put the person's selection back. Pads are found again by number, as
    /// a delete shifts indices; a symbol selection is dropped if the replay
    /// changed the number of pins or graphics its indices count in.
    pub(super) fn give_back_editor_selection(&mut self, path: &Path, held: HeldEditor) {
        match held {
            HeldEditor::Footprint {
                primary,
                primary_number,
                extra,
                silk,
                sketch,
                sketch_secondary,
                sketch_extra,
            } => {
                let Some(editor) = self.document_state.footprint_editors.get_mut(path) else {
                    return;
                };
                let state = &mut editor.state;
                state.selected_pad = primary
                    .zip(primary_number)
                    .and_then(|(i, number)| find_pad(&state.pads, i, &number));
                state.selected_pads_extra = extra
                    .iter()
                    .filter_map(|(i, number)| find_pad(&state.pads, *i, number))
                    .collect();
                state.selected_silk_f = silk;
                state.selected_sketch = sketch;
                state.selected_sketch_secondary = sketch_secondary;
                state.selected_sketch_extra = sketch_extra;
            }
            HeldEditor::Symbol {
                selection,
                pins,
                graphics,
            } => {
                let Some(editor) = self.document_state.symbol_editors.get_mut(path) else {
                    return;
                };
                let same = editor.primitive().pins.len() == pins
                    && editor.primitive().graphics.len() == graphics;
                editor.selected = selection.filter(|_| same);
            }
        }
    }
}

/// The recorded pads no longer at their index with their number.
fn gone_pads(editor: &FootprintDocument, same: bool, pads: &[(usize, String)]) -> Vec<String> {
    let all = &editor.state.pads;
    pads.iter()
        .filter(|(i, number)| !same || all.get(*i).is_none_or(|pad| pad.number != *number))
        .map(|(_, number)| format!("Pad {number}"))
        .collect()
}

/// The recorded sketch entities no longer in the footprint.
fn gone_sketch_entities<'a>(
    editor: &FootprintDocument,
    same: bool,
    ids: impl Iterator<Item = &'a SketchEntityId>,
) -> Vec<String> {
    let shape = editor.primitive();
    let entities = shape.sketch.as_ref().map(|s| s.entities.as_slice());
    let has =
        |id: &SketchEntityId| same && entities.is_some_and(|all| all.iter().any(|e| e.id == *id));
    ids.filter(|id| !has(id))
        .map(|id| format!("Sketch entity {id:?}"))
        .collect()
}

/// The recorded pins and graphics no longer in the symbol.
fn gone_symbol_items(
    editor: &SymbolDocument,
    same: bool,
    pins: &[(usize, String)],
    graphics: &[usize],
) -> Vec<String> {
    let shape = editor.primitive();
    let gone_pins = pins
        .iter()
        .filter(|(i, number)| !same || shape.pins.get(*i).is_none_or(|pin| pin.number != *number))
        .map(|(_, number)| format!("Pin {number}"));
    let gone_graphics = graphics
        .iter()
        .filter(|i| !same || **i >= shape.graphics.len())
        .map(|i| format!("Graphic {i}"));
    gone_pins.chain(gone_graphics).collect()
}

fn pad_number(pads: &[EditorPad], index: usize) -> Option<String> {
    pads.get(index).map(|pad| pad.number.clone())
}

/// The pad holding `number`: at `index` when it still is, else the first.
fn find_pad(pads: &[EditorPad], index: usize, number: &str) -> Option<usize> {
    if pads.get(index).is_some_and(|pad| pad.number == number) {
        return Some(index);
    }
    pads.iter().position(|pad| pad.number == number)
}

/// The pins (with their numbers) and graphics `selection` names.
fn symbol_selection_items(
    editor: &SymbolDocument,
    selection: Option<&SymbolSelection>,
) -> (Vec<(usize, String)>, Vec<usize>) {
    let shape = editor.primitive();
    let pin = |i: usize| shape.pins.get(i).map(|pin| (i, pin.number.clone()));
    match selection {
        Some(SymbolSelection::Pin(i)) => (pin(*i).into_iter().collect(), Vec::new()),
        Some(SymbolSelection::Graphic(i)) => (Vec::new(), vec![*i]),
        Some(SymbolSelection::Multiple {
            pin_indices,
            graphic_indices,
        }) => (
            pin_indices.iter().filter_map(|i| pin(*i)).collect(),
            graphic_indices.clone(),
        ),
        Some(SymbolSelection::All | SymbolSelection::Field(_)) | None => (Vec::new(), Vec::new()),
    }
}
