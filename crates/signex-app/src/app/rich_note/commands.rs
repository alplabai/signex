//! The rich note editor's catalog commands (#1114, mockup row 203), the CLI
//! and agent surface of what the editor's buttons and keys do, on a note of
//! the sheet in view:
//!
//! - `edit_rich_note id [line] [column]` opens the editor, the cursor there.
//! - `wrap_rich_note_selection id style` is a toolbar button.
//! - `cancel_rich_note_edit id` is Esc.
//! - `open_rich_note_link id target` is Ctrl+click on a link.
//! - `compile_rich_note id` compiles the note off the UI thread and answers
//!   with its size, links and problems.
//!
//! Apply and the Frame and Size sections are document edits:
//! `app/command/rich_note_operands.rs`. Answers: `docs/CLI.md`.

use std::sync::Arc;

use iced::Task;
use serde_json::{Value, json};
use signex_notebook::note::NoteKey;
use signex_types::schematic::TextNoteFormat;

use super::compile::NoteJob;
use super::editor::{LinkTarget, resolve_link};
use super::{NoteOutcome, RichNoteMsg, Wrap};
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::notebook_project::{because, refusal};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

type Ran = (CommandOutcome, Task<Message>);

fn text<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.as_str()),
        _ => None,
    }
}

/// A refusal with the reason another part of Signex gave as text, through
/// the notebook commands' catalog line (no new legacy English).
fn refused(reason: impl Into<String>) -> Ran {
    refusal(because(reason))
}

/// The rich note `id` names on the sheet in view, its uuid and its source.
fn rich_note(app: &Signex, args: &CommandArgs) -> Result<(uuid::Uuid, String), String> {
    let id = text(args, "id").ok_or("operand `id` is missing")?;
    let uuid = uuid::Uuid::parse_str(id.trim())
        .map_err(|_| format!("`id` must be an object id (a uuid), got `{id}`"))?;
    let engine = (app.document_state.active_engine()).ok_or("no schematic is in view")?;
    let note = engine
        .document()
        .text_notes
        .iter()
        .find(|note| note.uuid == uuid)
        .ok_or_else(|| format!("no text note {uuid} on the sheet in view"))?;
    if note.format != TextNoteFormat::Typst {
        return Err(format!(
            "the note {uuid} is plain text; set its format to `typst` first"
        ));
    }
    Ok((uuid, note.text.clone()))
}

/// A 1-based line or column operand.
fn position(args: &CommandArgs, name: &str) -> Result<Option<usize>, String> {
    let Some(value) = text(args, name) else {
        return Ok(None);
    };
    value
        .trim()
        .parse::<usize>()
        .ok()
        .filter(|n| *n > 0)
        .map(Some)
        .ok_or_else(|| format!("`{name}` is a whole number from 1, got `{value}`"))
}

/// The byte of `source` at 1-based `line` and `column` (columns in
/// characters), clamped to the line's end.
fn byte_at(source: &str, line: usize, column: usize) -> usize {
    let mut start = 0;
    for (index, text) in source.split('\n').enumerate() {
        if index + 1 == line {
            let within = text
                .char_indices()
                .nth(column - 1)
                .map_or(text.len(), |(at, _)| at);
            return start + within;
        }
        start += text.len() + 1;
    }
    source.len()
}

/// The editor of `note`'s answer: its source and cursor.
fn editor_answer(app: &Signex, note: uuid::Uuid) -> Value {
    match app.ui_state.rich_notes.editor.as_ref() {
        Some(editor) if editor.note == note => {
            let (line, index) = editor.cursor();
            json!({
                "id": note.to_string(),
                "editing": true,
                "source": editor.text(),
                "edited": editor.is_edited(),
                "cursor": { "line": line + 1, "byte": index },
            })
        }
        _ => json!({ "id": note.to_string(), "editing": false }),
    }
}

/// `edit_rich_note id=.. [line=..] [column=..]`.
pub(crate) fn edit_rich_note(app: &mut Signex, args: &CommandArgs) -> Ran {
    let (note, source) = match rich_note(app, args) {
        Ok(found) => found,
        Err(why) => return refused(why),
    };
    let at = match (position(args, "line"), position(args, "column")) {
        (Ok(line), Ok(column)) => byte_at(&source, line.unwrap_or(1), column.unwrap_or(1)),
        (Err(why), _) | (_, Err(why)) => return refused(why),
    };
    match app.request_rich_note_editor(note, at) {
        Ok(task) => (CommandOutcome::Answered(editor_answer(app, note)), task),
        Err(why) => refused(why),
    }
}

/// `cancel_rich_note_edit id=..`: the typing is dropped; nothing is written.
pub(crate) fn cancel_rich_note_edit(app: &mut Signex, args: &CommandArgs) -> Ran {
    let (note, _) = match rich_note(app, args) {
        Ok(found) => found,
        Err(why) => return refused(why),
    };
    let rich = &mut app.ui_state.rich_notes;
    let open = rich.editor.as_ref().is_some_and(|e| e.note == note);
    if open {
        rich.editor = None;
    }
    let answer = json!({ "id": note.to_string(), "cancelled": open });
    (CommandOutcome::Answered(answer), Task::none())
}

/// `wrap_rich_note_selection id=.. style=bold|italic|heading|list|math|link|table`.
pub(crate) fn wrap_rich_note_selection(app: &mut Signex, args: &CommandArgs) -> Ran {
    let (note, _) = match rich_note(app, args) {
        Ok(found) => found,
        Err(why) => return refused(why),
    };
    let word = text(args, "style").unwrap_or_default().trim();
    let Some(wrap) = Wrap::from_word(word) else {
        let words: Vec<&str> = Wrap::ALL.iter().map(|w| w.as_str()).collect();
        return refused(format!(
            "`style` is one of {}, got `{word}`",
            words.join(", ")
        ));
    };
    if app.ui_state.rich_notes.editor.as_ref().map(|e| e.note) != Some(note) {
        return refused(format!(
            "no editor is open on the note {note}; open it with `edit_rich_note`"
        ));
    }
    match app.wrap_rich_note_editor(wrap) {
        Ok(task) => {
            let mut answer = editor_answer(app, note);
            answer["style"] = json!(wrap.as_str());
            (CommandOutcome::Answered(answer), task)
        }
        Err(why) => refused(why),
    }
}

/// `open_rich_note_link id=.. target=..`: a link the note's source makes.
pub(crate) fn open_rich_note_link(app: &mut Signex, args: &CommandArgs) -> Ran {
    let (note, source) = match rich_note(app, args) {
        Ok(found) => found,
        Err(why) => return refused(why),
    };
    let target = text(args, "target").unwrap_or_default().trim().to_string();
    if target.is_empty() || !source.contains(&format!("\"{target}\"")) {
        return refused(format!("the note {note} has no link to `{target}`"));
    }
    let resolved = app
        .document_state
        .active_path
        .as_deref()
        .and_then(std::path::Path::parent)
        .map(|folder| resolve_link(&target, folder));
    let kind = match resolved {
        Some(Ok(LinkTarget::Web(_))) => "web",
        Some(Ok(LinkTarget::ProjectFile(_))) => "file",
        Some(Err(why)) => return refused(why),
        None => return refused("no sheet is in view"),
    };
    match app.launch_note_link(&target) {
        Ok(task) => {
            let answer = json!({ "id": note.to_string(), "target": target, "opens": kind });
            (CommandOutcome::Answered(answer), task)
        }
        Err(why) => refused(why),
    }
}

/// The answer of a compile of `note` from `key`.
fn compile_answer(note: uuid::Uuid, key: NoteKey, outcome: &NoteOutcome, cached: bool) -> Value {
    let drawing = outcome.drawing.as_deref();
    let problems: Vec<Value> = outcome
        .diagnostics
        .iter()
        .map(|d| {
            json!({
                "severity": d.severity,
                "line": d.line,
                "column": d.column,
                "message": d.message,
            })
        })
        .collect();
    let links: Vec<Value> = drawing
        .map(|d| d.links.iter().map(|l| json!(l.target)).collect())
        .unwrap_or_default();
    json!({
        "id": note.to_string(),
        "key": format!("{:016x}", key.0),
        "compiled": drawing.is_some(),
        "cached": cached,
        "width_mm": drawing.map(|d| d.width_mm),
        "height_mm": drawing.map(|d| d.height_mm),
        "shapes": drawing.map_or(0, |d| d.shapes.len()),
        "links": links,
        "problems": problems,
        "compile_ms": outcome.elapsed_ms,
    })
}

/// `compile_rich_note id=..`: from the cache at once, else compiled in a
/// task and answered when it lands.
pub(crate) fn compile_rich_note(app: &mut Signex, args: &CommandArgs) -> Ran {
    let (note, _) = match rich_note(app, args) {
        Ok(found) => found,
        Err(why) => return refused(why),
    };
    let (Some(sheet), Some(engine)) = (
        app.document_state.active_path.clone(),
        app.document_state.active_engine(),
    ) else {
        return refused("no schematic is in view");
    };
    let Some(found) = engine.document().text_notes.iter().find(|n| n.uuid == note) else {
        return refused(format!("no text note {note} on the sheet in view"));
    };
    let job = NoteJob::of(found);
    if let Some(outcome) = app.ui_state.rich_notes.compiled.get(&job.key) {
        let answer = compile_answer(note, job.key, outcome, true);
        return (CommandOutcome::Answered(answer), Task::none());
    }
    let command = match AppCommandId::new("compile_rich_note") {
        Ok(command) => command,
        Err(why) => return refused(why.to_string()),
    };
    let waiting = app.wait_for_command(&command, args);
    let ticket = waiting.ticket;
    let key = job.key;
    app.ui_state.rich_notes.compiles += 1;
    let task = Task::perform(
        async move { tokio::task::spawn_blocking(move || job.run()).await },
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
            Message::from(RichNoteMsg::CommandCompiled {
                waiting: Box::new(waiting),
                sheet,
                note,
                key,
                outcome: Arc::new(outcome),
            })
        },
    );
    (CommandOutcome::Deferred { ticket }, task)
}

impl Signex {
    /// `compile_rich_note`'s compile landed: cache it and answer its caller.
    pub(super) fn rich_note_command_compiled(
        &mut self,
        waiting: crate::app::agent::WaitingCommand,
        sheet: &std::path::Path,
        note: uuid::Uuid,
        key: NoteKey,
        outcome: Arc<NoteOutcome>,
    ) -> Task<Message> {
        self.ui_state.rich_notes.keep(key, outcome.clone());
        let answer = compile_answer(note, key, &outcome, false);
        let refresh = self.sync_rich_notes();
        let answered =
            self.finish_waiting_command(waiting, sheet, |_| CommandOutcome::Answered(answer));
        Task::batch([refresh, answered])
    }
}
