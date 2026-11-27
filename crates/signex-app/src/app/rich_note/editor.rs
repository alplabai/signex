//! The in-place editor of a rich note (mockup row 203): open it on a note of
//! the sheet in view (double-click, Edit… in Properties, a search hit or
//! `edit_rich_note`), type with the preview recompiled after each pause,
//! wrap the selection from the toolbar, Apply as one undo step or Cancel.
//! And the note's links: Ctrl+click opens a file of the project or a web
//! page, in a task.

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use iced::Task;
use iced::widget::text_editor::{Action, Edit};
use serde_json::{Value, json};
use signex_notebook::note::{NoteKey, PlacedNote};
use signex_types::schematic::{SelectedItem, SelectedKind, TextNote, TextNoteFormat};

use super::compile::NoteJob;
use super::{NoteOutcome, RichNoteEditor, RichNoteMsg, Wrap};
use crate::app::command::PlacedRefusal;
use crate::app::{Message, Signex};

/// The typing pause after which the preview compiles.
pub(crate) const TYPING_PAUSE: Duration = Duration::from_millis(150);

/// The editor's source box, for focus.
pub(crate) fn editor_id() -> iced::widget::Id {
    iced::widget::Id::new("rich-note-editor")
}

/// What a note's link opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkTarget {
    /// An `http(s)` page, in the browser.
    Web(String),
    /// A file inside the project folder, in its viewer.
    ProjectFile(PathBuf),
}

/// What `target`, as written in a note, opens: an `http(s)` address, or a
/// relative path that stays inside `folder` (the project's). Anything else
/// (another scheme, an absolute path, a `..`) is refused with the reason.
pub fn resolve_link(target: &str, folder: &Path) -> Result<LinkTarget, String> {
    let target = target.trim();
    let lower = target.to_ascii_lowercase();
    if lower.starts_with("https://") || lower.starts_with("http://") {
        return Ok(LinkTarget::Web(target.to_string()));
    }
    if target.is_empty() || target.contains(':') {
        return Err(format!(
            "`{target}` is neither a web address nor a file of the project"
        ));
    }
    let relative = Path::new(target);
    let inside = relative
        .components()
        .all(|part| matches!(part, Component::Normal(_) | Component::CurDir));
    if !inside {
        return Err(format!("`{target}` leaves the project folder"));
    }
    Ok(LinkTarget::ProjectFile(folder.join(relative)))
}

impl Signex {
    /// The rich note `note` on the sheet in view.
    fn rich_note_in_view(&self, note: uuid::Uuid) -> Option<&TextNote> {
        let engine = self.document_state.active_engine()?;
        engine
            .document()
            .text_notes
            .iter()
            .find(|n| n.uuid == note && n.format == TextNoteFormat::Typst)
    }

    /// Open the editor on `note`, the cursor at byte `at` of its source. An
    /// editor already open on another note applies what was typed in it
    /// first, as a click away from the plain editor does.
    pub(crate) fn request_rich_note_editor(
        &mut self,
        note: uuid::Uuid,
        at: usize,
    ) -> Result<Task<Message>, String> {
        if let Some(reason) = self.refuse_if_active_read_only() {
            return Err(reason.to_string());
        }
        let Some(found) = self.rich_note_in_view(note) else {
            return Err(format!("no rich text note {note} on the sheet in view"));
        };
        let text = found.text.clone();
        let lock = self.document_state.active_engine().and_then(|engine| {
            engine.lock_refusal(&signex_engine::Command::UpdateText {
                target: signex_engine::TextTarget::TextNote(note),
                value: text.clone(),
            })
        });
        if let Some(reason) = lock {
            return Err(reason);
        }
        let owed = (self.ui_state.rich_notes.editor.as_ref())
            .is_some_and(|open| open.note != note && open.is_edited());
        if owed {
            self.apply_rich_note_editor()
                .map_err(|refusal| refusal.reason)?;
        }
        if let Some(plain) = self.interaction_state.editing_text.take() {
            self.commit_text_edit(plain);
        }
        let sheet = self.document_state.active_path.clone();
        let editor = RichNoteEditor::open(sheet, note, &text, at);
        self.ui_state.rich_notes.editor = Some(editor);
        let compile = self.compile_editor_text();
        let focus = iced::widget::operation::focus(editor_id());
        Ok(Task::batch([compile, focus]))
    }

    /// Double-click on a rich note: its editor instead of the one-line box.
    /// `None` for anything else.
    pub(crate) fn open_rich_note_editor_on(&mut self, item: SelectedItem) -> Option<Task<Message>> {
        if item.kind != SelectedKind::TextNote {
            return None;
        }
        self.rich_note_in_view(item.uuid)?;
        Some(
            self.request_rich_note_editor(item.uuid, 0)
                .unwrap_or_else(|why| {
                    crate::diagnostics::log_warning(why);
                    Task::none()
                }),
        )
    }

    /// A search hit inside a rich note: open its editor at byte `at` of its
    /// source once its sheet is the one in view.
    pub(crate) fn open_rich_note_at_search_hit(&mut self, note: uuid::Uuid, at: usize) {
        self.ui_state.rich_notes.open_at = Some((note, at));
    }

    /// The editor a search hit owes, once the hit's sheet shows.
    pub(super) fn open_rich_note_editor_owed(&mut self) -> Task<Message> {
        let Some((note, at)) = self.ui_state.rich_notes.open_at else {
            return Task::none();
        };
        let shown = self.document_state.active_engine().and_then(|engine| {
            let found = engine
                .document()
                .text_notes
                .iter()
                .find(|n| n.uuid == note)?;
            Some(found.format == TextNoteFormat::Typst)
        });
        match shown {
            // Its sheet is not the one in view yet.
            None => return Task::none(),
            // A plain note has no rich editor: the hit only selects it.
            Some(false) => {
                self.ui_state.rich_notes.open_at = None;
                return Task::none();
            }
            Some(true) => {}
        }
        self.ui_state.rich_notes.open_at = None;
        self.request_rich_note_editor(note, at)
            .unwrap_or_else(|why| {
                crate::diagnostics::log_warning(why);
                Task::none()
            })
    }

    /// A keystroke or a cursor move in the source: an edit starts the typing
    /// pause, after which the preview compiles.
    pub(super) fn rich_note_editor_acted(&mut self, action: Action) -> Task<Message> {
        let Some(editor) = self.ui_state.rich_notes.editor.as_mut() else {
            return Task::none();
        };
        let edits = action.is_edit();
        editor.content.perform(action);
        if !edits {
            return Task::none();
        }
        pause(editor.edited())
    }

    /// The typing paused: compile the text if no key came since.
    pub(super) fn rich_note_editor_paused(&mut self, generation: u64) -> Task<Message> {
        let current = self
            .ui_state
            .rich_notes
            .editor
            .as_ref()
            .map(|e| e.generation);
        if current != Some(generation) {
            return Task::none();
        }
        self.compile_editor_text()
    }

    /// Compile the editor's text: from the cache at once, else in a task.
    fn compile_editor_text(&mut self) -> Task<Message> {
        let Some(editor) = self.ui_state.rich_notes.editor.as_ref() else {
            return Task::none();
        };
        let generation = editor.generation;
        let text = editor.text();
        let Some(note) = self.rich_note_in_view(editor.note) else {
            return Task::none();
        };
        let job = NoteJob::of_text(note, &text);
        if let Some(outcome) = self.ui_state.rich_notes.compiled.get(&job.key).cloned() {
            self.rich_note_editor_compiled(generation, job.key, outcome);
            return Task::none();
        }
        let key = job.key;
        self.ui_state.rich_notes.compiles += 1;
        Task::perform(
            async move {
                let job = job;
                tokio::task::spawn_blocking(move || job.run()).await
            },
            move |result| {
                let outcome = result.unwrap_or_else(|error| NoteOutcome {
                    diagnostics: vec![signex_notebook::Diagnostic {
                        severity: signex_notebook::DiagnosticSeverity::Error,
                        file: signex_notebook::note::NOTE_FILE.to_string(),
                        line: 0,
                        column: 0,
                        message: error.to_string(),
                        hints: Vec::new(),
                    }],
                    ..NoteOutcome::default()
                });
                Message::from(RichNoteMsg::EditorCompiled {
                    generation,
                    key,
                    outcome: Arc::new(outcome),
                })
            },
        )
    }

    /// The compile of the typed text of `generation` landed: cached, and
    /// shown when it is still the newest text. An error keeps the last good
    /// drawing in the preview.
    pub(super) fn rich_note_editor_compiled(
        &mut self,
        generation: u64,
        key: NoteKey,
        outcome: Arc<NoteOutcome>,
    ) {
        let rich = &mut self.ui_state.rich_notes;
        rich.keep(key, outcome.clone());
        let Some(editor) = rich.editor.as_mut() else {
            return;
        };
        if editor.generation != generation {
            return;
        }
        if let Some(drawing) = &outcome.drawing {
            editor.last_good = Some(drawing.clone());
        }
        editor.preview = Some(outcome);
    }

    /// A toolbar button: wrap the selection (or insert the empty markup).
    pub(crate) fn wrap_rich_note_editor(&mut self, wrap: Wrap) -> Result<Task<Message>, String> {
        let Some(editor) = self.ui_state.rich_notes.editor.as_mut() else {
            return Err("no rich text note is being edited".to_string());
        };
        let selected = editor.content.selection();
        let text = wrap.around(selected.as_deref());
        editor
            .content
            .perform(Action::Edit(Edit::Paste(Arc::new(text))));
        Ok(pause(editor.edited()))
    }

    /// Apply: the typed text becomes the note's, one undo step. The editor
    /// closes; an unchanged text closes it with no step.
    pub(crate) fn apply_rich_note_editor(&mut self) -> Result<Value, PlacedRefusal> {
        let Some(editor) = self.ui_state.rich_notes.editor.clone() else {
            return Err(PlacedRefusal::new("no rich text note is being edited"));
        };
        if self.document_state.active_path != editor.sheet {
            return Err(PlacedRefusal::new(
                "the note's sheet is not the one in view; open it to apply the edit",
            ));
        }
        let text = editor.text();
        let item = SelectedItem::new(editor.note, SelectedKind::TextNote);
        if editor.is_edited() {
            self.apply_placed_edit(signex_engine::Command::UpdateText {
                target: signex_engine::TextTarget::TextNote(editor.note),
                value: text.clone(),
            })?;
        }
        self.ui_state.rich_notes.editor = None;
        let mut answer = self.placed_answer(item)?;
        answer["text"] = json!(text);
        answer["applied"] = json!(editor.is_edited());
        Ok(answer)
    }

    /// Ctrl+click on the sheet: the link of a rich note under `(x, y)` mm,
    /// opened. `None` when no link is there, so the click selects as usual.
    pub(crate) fn rich_note_link_at(&mut self, x: f64, y: f64) -> Option<Task<Message>> {
        let engine = self.document_state.active_engine()?;
        let renders = &self.ui_state.rich_notes.renders;
        let target = engine.document().text_notes.iter().find_map(|note| {
            let drawing = renders.drawing_of(note)?;
            let local = PlacedNote::of(note, Some(drawing)).sheet_to_text([x, y]);
            drawing
                .links
                .iter()
                .find(|link| link.contains(local[0], local[1]))
                .map(|link| link.target.clone())
        })?;
        Some(self.open_note_link(&target))
    }

    /// The folder a note's file links are read in: the project's of the
    /// sheet in view, else the sheet's own.
    fn link_folder(&self) -> Option<PathBuf> {
        let sheet = self.document_state.active_path.as_deref()?;
        match self.document_state.document_project(sheet) {
            Some(project) => project.path.parent().map(Path::to_path_buf),
            None => sheet.parent().map(Path::to_path_buf),
        }
    }

    /// Open `target` in a task; a refusal goes to Messages.
    pub(crate) fn open_note_link(&mut self, target: &str) -> Task<Message> {
        match self.launch_note_link(target) {
            Ok(task) => task,
            Err(why) => {
                crate::diagnostics::log_warning(why);
                Task::none()
            }
        }
    }

    /// The task that opens `target`, or why it may not be opened.
    pub(crate) fn launch_note_link(&self, target: &str) -> Result<Task<Message>, String> {
        let folder = self
            .link_folder()
            .ok_or_else(|| "no sheet is in view".to_string())?;
        let link = resolve_link(target, &folder)?;
        let opener = self.ui_state.io.opener;
        let shown = target.to_string();
        Ok(Task::perform(
            async move {
                tokio::task::spawn_blocking(move || match link {
                    LinkTarget::Web(url) => opener.browse(&url),
                    LinkTarget::ProjectFile(path) => opener.view(&path),
                })
                .await
            },
            move |result| {
                let failure = match result {
                    Ok(Ok(())) => None,
                    Ok(Err(error)) => Some(error.to_string()),
                    Err(error) => Some(error.to_string()),
                };
                Message::from(RichNoteMsg::LinkOpened {
                    target: shown,
                    failure,
                })
            },
        ))
    }
}

/// The task that reports the typing pause of `generation`.
fn pause(generation: u64) -> Task<Message> {
    Task::perform(tokio::time::sleep(TYPING_PAUSE), move |()| {
        Message::from(RichNoteMsg::EditorPaused { generation })
    })
}
