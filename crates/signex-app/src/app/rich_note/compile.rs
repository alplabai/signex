//! Compiling rich notes off the UI thread (#1114). After every update the
//! open sheets' rich notes are matched against the cache by key: a note whose
//! key is cached is shown at once, a missing key is compiled in one task with
//! the others missing, and a key being compiled is not asked for twice. A
//! compile that fails keeps the note's last good drawing on the sheet, marked,
//! and says the error once in Messages. The editor's typing is debounced
//! (`editor.rs`). Rules: `docs/architecture/snxsch-note-format.md`, "Drawing".

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use iced::Task;
use signex_notebook::note::{NoteInput, NoteKey, compile_note, note_key};
use signex_types::schematic::{HAlign, SelectedItem, SelectedKind, TextNote, TextNoteFormat};

use super::{NoteOutcome, RichNoteMsg};
use crate::app::{Message, Signex};
use crate::schematic_runtime::rich_note::ShownNote;

/// What one compile reads: an owned copy of a note's [`NoteInput`].
#[derive(Debug, Clone, PartialEq)]
pub struct NoteJob {
    pub key: NoteKey,
    pub source: String,
    pub width_mm: Option<f64>,
    pub text_size_mm: f64,
    pub align: HAlign,
}

impl NoteJob {
    /// The job of `note` as stored.
    #[must_use]
    pub fn of(note: &TextNote) -> Self {
        Self::of_text(note, &note.text)
    }

    /// The job of `note` with `text` as its source (the editor's typing).
    #[must_use]
    pub fn of_text(note: &TextNote, text: &str) -> Self {
        let input = NoteInput {
            source: text,
            ..NoteInput::of(note)
        };
        Self {
            key: note_key(&input),
            source: text.to_string(),
            width_mm: input.width_mm,
            text_size_mm: input.text_size_mm,
            align: input.align,
        }
    }

    /// Compile. Blocking: run it on a worker.
    #[must_use]
    pub fn run(&self) -> NoteOutcome {
        let started = Instant::now();
        let compiled = compile_note(&NoteInput {
            source: &self.source,
            width_mm: self.width_mm,
            text_size_mm: self.text_size_mm,
            align: self.align,
        });
        let elapsed = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        NoteOutcome::of(compiled, elapsed)
    }
}

/// Compile `jobs` in order. Blocking.
#[must_use]
pub fn run_jobs(jobs: &[NoteJob]) -> Vec<(NoteKey, Arc<NoteOutcome>)> {
    jobs.iter()
        .map(|job| (job.key, Arc::new(job.run())))
        .collect()
}

/// The outcome of a compile whose worker stopped.
pub(super) fn stopped(why: &str) -> NoteOutcome {
    NoteOutcome {
        drawing: None,
        diagnostics: vec![signex_notebook::Diagnostic {
            severity: signex_notebook::DiagnosticSeverity::Error,
            file: signex_notebook::note::NOTE_FILE.to_string(),
            line: 0,
            column: 0,
            message: why.to_string(),
            hints: Vec::new(),
        }],
        elapsed_ms: 0,
    }
}

/// Run `jobs` on a worker; the outcomes land as one message.
pub(super) fn compile_task(jobs: Vec<NoteJob>) -> Task<Message> {
    let keys: Vec<NoteKey> = jobs.iter().map(|job| job.key).collect();
    Task::perform(
        async move { tokio::task::spawn_blocking(move || run_jobs(&jobs)).await },
        move |result| {
            let outcomes = result.unwrap_or_else(|error| {
                let failed = Arc::new(stopped(&error.to_string()));
                keys.iter().map(|key| (*key, failed.clone())).collect()
            });
            Message::from(RichNoteMsg::Compiled {
                outcomes: Arc::new(outcomes),
            })
        },
    )
}

impl Signex {
    /// Show the cached compile of every rich note of the open sheets and
    /// compile the ones the cache does not hold yet. Run after every update;
    /// nothing is compiled here, and a note's source is copied only when it
    /// must be compiled.
    pub(crate) fn sync_rich_notes(&mut self) -> Task<Message> {
        let open_editor = self.open_rich_note_editor_owed();
        let mut seen: Vec<(uuid::Uuid, NoteKey)> = Vec::new();
        let mut jobs: Vec<NoteJob> = Vec::new();
        let rich = &self.ui_state.rich_notes;
        for engine in self.document_state.engines.values() {
            let notes = (engine.document().text_notes.iter())
                .filter(|note| note.format == TextNoteFormat::Typst);
            for note in notes {
                let key = note_key(&NoteInput::of(note));
                seen.push((note.uuid, key));
                let wanted = !rich.compiled.contains_key(&key)
                    && !rich.pending.contains(&key)
                    && jobs.iter().all(|job| job.key != key);
                if wanted {
                    jobs.push(NoteJob::of(note));
                }
            }
        }
        let rich = &mut self.ui_state.rich_notes;
        rich.pending.extend(jobs.iter().map(|job| job.key));
        let mut changed = false;
        for (note, key) in &seen {
            let cached = self.ui_state.rich_notes.compiled.get(key).cloned();
            if let Some(outcome) = cached {
                changed |= self.show_rich_note(*note, *key, &outcome);
            }
        }
        let live: HashSet<uuid::Uuid> = seen.iter().map(|(note, _)| *note).collect();
        let renders = &mut self.ui_state.rich_notes.renders;
        let before = renders.revision();
        renders.retain(|id| live.contains(&id));
        changed |= renders.revision() != before;
        if changed {
            self.repaint_rich_notes();
        }
        let compile = if jobs.is_empty() {
            Task::none()
        } else {
            self.ui_state.rich_notes.compiles += jobs.len() as u64;
            compile_task(jobs)
        };
        Task::batch([open_editor, compile])
    }

    /// Show `outcome` (compiled from `key`) for `note`: its drawing, or the
    /// last good one marked when it failed. `true` when the canvas changes.
    fn show_rich_note(&mut self, note: uuid::Uuid, key: NoteKey, outcome: &NoteOutcome) -> bool {
        let rich = &mut self.ui_state.rich_notes;
        let previous = rich.renders.get(note).and_then(|s| s.drawing.clone());
        let shown = ShownNote {
            drawing: outcome.drawing.clone().or(previous),
            key,
            failed: outcome.drawing.is_none(),
        };
        let changed = rich.renders.set(note, shown);
        if outcome.drawing.is_some() {
            rich.reported.remove(&note);
        } else if rich.reported.get(&note) != Some(&key) {
            rich.reported.insert(note, key);
            self.report_rich_note_error(note, outcome);
        }
        changed
    }

    /// Say a note's compile error in Messages, with Show on the note.
    fn report_rich_note_error(&self, note: uuid::Uuid, outcome: &NoteOutcome) {
        let Some(error) = outcome.first_error() else {
            return;
        };
        let found = self
            .document_state
            .engines
            .iter()
            .find_map(|(path, engine)| {
                let at = engine
                    .document()
                    .text_notes
                    .iter()
                    .find(|n| n.uuid == note)?;
                Some((path.clone(), at.position))
            });
        let Some((sheet, position)) = found else {
            return;
        };
        let line = self.ui_state.i18n.message_plain(
            signex_i18n::ids::OVERLAY_RICH_NOTE_COMPILE_ERROR,
            &[
                ("x", signex_i18n::Arg::from(format!("{:.2}", position.x))),
                ("y", signex_i18n::Arg::from(format!("{:.2}", position.y))),
                ("line", signex_i18n::Arg::from(error.line)),
                ("message", signex_i18n::Arg::from(error.message.clone())),
            ],
        );
        crate::diagnostics::log_warning_at(
            line,
            sheet,
            Some(SelectedItem::new(note, SelectedKind::TextNote)),
        );
    }

    /// Compiles landed: keep them and show the notes they are for.
    pub(super) fn rich_notes_compiled(
        &mut self,
        outcomes: &[(NoteKey, Arc<NoteOutcome>)],
    ) -> Task<Message> {
        for (key, outcome) in outcomes {
            self.ui_state.rich_notes.keep(*key, outcome.clone());
        }
        self.sync_rich_notes()
    }

    /// The canvases draw the renders again: content and hit boxes.
    fn repaint_rich_notes(&mut self) {
        for canvas in self.interaction_state.all_schematic_canvases_mut() {
            canvas.clear_content_cache();
            canvas.clear_overlay_cache();
        }
    }
}
