//! Replay of the footprint and symbol editors' Properties panel edits, and
//! of a footprint pad drag by its offset.

use std::path::{Path, PathBuf};

use iced::Task;
use signex_i18n::Translatable;

use super::book::ReplayInput;
use super::editors::{EditorTargets, footprint_targets, symbol_targets};
use super::run::{document_closed, no_change, targets_gone, tool_active};
use super::scope::EditFamily;
use super::selection::Shown;
use crate::app::command::CommandOutcome;
use crate::app::{Message, Signex};
use crate::dock::DockMessage;
use crate::library::messages::{FootprintEditorMsg, PrimitiveEdit};
use crate::panels::PanelMsg;

/// An editor panel message about to run.
pub(crate) struct PanelEditProbe {
    path: PathBuf,
    history: Option<(u64, usize)>,
    targets: EditorTargets,
    message: Box<PanelMsg>,
}

/// Whether `msg` is one of the editors' own Properties messages. The two
/// families share their prefix; a new variant joins by its name.
fn is_editor_panel_message(msg: &PanelMsg) -> bool {
    let name = format!("{msg:?}");
    name.starts_with("FpEditor") || name.starts_with("SymEditor")
}

impl Signex {
    /// The editor document in the active tab, with its selection.
    fn active_editor_targets(&self) -> Option<(PathBuf, EditorTargets)> {
        let docs = &self.document_state;
        let kind = &docs.tabs.get(docs.active_tab)?.kind;
        if let Some(path) = kind.as_footprint_editor() {
            let editor = docs.footprint_editors.get(path)?;
            return Some((path.clone(), footprint_targets(editor)));
        }
        let path = kind.as_symbol_editor()?;
        let editor = docs.symbol_editors.get(path)?;
        Some((path.clone(), symbol_targets(editor)))
    }

    /// Read what the panel message `msg` would record, before it runs.
    pub(crate) fn probe_panel_edit(&mut self, msg: &PanelMsg) -> Option<PanelEditProbe> {
        if !self.records_replay_input() {
            return None;
        }
        let (path, targets) = self.active_editor_targets()?;
        if !is_editor_panel_message(msg) {
            return None;
        }
        self.ui_state.command_replay.panel_edit_running = true;
        Some(PanelEditProbe {
            history: self.editor_history(&path),
            path,
            targets,
            message: Box::new(msg.clone()),
        })
    }

    /// The panel message ran: when it made an undo step, record it.
    pub(crate) fn finish_panel_edit(&mut self, probe: Option<PanelEditProbe>) {
        let Some(probe) = probe else {
            return;
        };
        self.ui_state.command_replay.panel_edit_running = false;
        if self.editor_history(&probe.path) == probe.history {
            return;
        }
        let input = ReplayInput::EditorPanel {
            path: probe.path.clone(),
            targets: probe.targets,
            message: probe.message,
        };
        self.note_finished_edit(
            probe.path,
            EditFamily::Properties,
            Some("edit_object_properties"),
            input,
        );
    }

    /// Why a panel edit of the editor at `path` cannot run now. The panel
    /// acts on the tab in view, so a tab behind it needs a switch.
    pub(super) fn editor_panel_refusal(
        &self,
        path: &Path,
        targets: &EditorTargets,
    ) -> Option<Translatable> {
        let Some(missing) = self.missing_editor_targets(path, targets) else {
            return Some(document_closed(path));
        };
        if !missing.is_empty() {
            return Some(targets_gone(path, &missing));
        }
        let Some(shown) = self.where_shown(path) else {
            return Some(document_closed(path));
        };
        (matches!(shown, Shown::Behind { .. }) && self.tool_in_progress())
            .then(|| tool_active(path))
    }

    /// Run a panel edit on its recorded selection, in the tab or window that
    /// shows its editor; the person's selection and tab come back.
    pub(super) fn run_editor_panel(
        &mut self,
        path: &Path,
        targets: &EditorTargets,
        message: &PanelMsg,
    ) -> (CommandOutcome, Task<Message>) {
        let before = self.editor_history(path);
        let send = |app: &mut Self| {
            app.refresh_panel_ctx();
            app.update(Message::Dock(DockMessage::Panel(message.clone())))
        };
        let task = match self.where_shown(path) {
            Some(Shown::Main) => self.on_editor_targets(path, targets, send),
            Some(Shown::Window(window)) => self
                .run_in_window_document(window, |app| app.on_editor_targets(path, targets, send)),
            Some(Shown::Behind { back }) => {
                let held = self.take_main_selection();
                if !self.focus_open_sheet(path) {
                    self.give_back_main_selection(held);
                    return (CommandOutcome::disabled(no_change()), Task::none());
                }
                let task = self.on_editor_targets(path, targets, send);
                self.return_to_tab(back.as_ref());
                self.give_back_main_selection(held);
                task
            }
            None => Task::none(),
        };
        self.refresh_panel_ctx();
        let outcome = if self.editor_history(path) == before {
            CommandOutcome::disabled(no_change())
        } else {
            CommandOutcome::Ran
        };
        (outcome, task)
    }

    /// Run `edit` with the editor at `path` holding `targets`.
    fn on_editor_targets(
        &mut self,
        path: &Path,
        targets: &EditorTargets,
        edit: impl FnOnce(&mut Self) -> Task<Message>,
    ) -> Task<Message> {
        let Some(held) = self.put_editor_targets(path, targets) else {
            return Task::none();
        };
        let task = edit(self);
        self.give_back_editor_selection(path, held);
        task
    }

    /// Move pad `idx` by `delta` from where it is now, on the recorded
    /// selection, so a group drag moves the same pads.
    pub(super) fn run_pad_drag(
        &mut self,
        path: &Path,
        targets: &EditorTargets,
        idx: usize,
        delta: (f64, f64),
    ) -> (CommandOutcome, Task<Message>) {
        let Some(at) = self
            .document_state
            .footprint_editors
            .get(path)
            .and_then(|editor| editor.state.pads.get(idx))
            .map(|pad| pad.position_mm)
        else {
            return (CommandOutcome::disabled(no_change()), Task::none());
        };
        let message = PrimitiveEdit::Footprint(FootprintEditorMsg::MovePad {
            idx,
            x_mm: at.0 + delta.0,
            y_mm: at.1 + delta.1,
        });
        self.run_editor_input(path, targets, std::slice::from_ref(&message))
    }
}
