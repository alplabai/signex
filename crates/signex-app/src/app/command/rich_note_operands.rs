//! A rich text note's Frame and Size sections and Apply of its editor, from
//! operands (#1114, mockup row 203): `set_text_note_frame`,
//! `set_text_note_size` and `apply_rich_note_edit`, each with an optional
//! `document`. A Properties row and an operand run call the same `Signex`
//! method, so a typed value, a CLI call and an agent's edit end in one engine
//! command and one undo step. Rules: `docs/CLI.md`, "Rich text notes".

use iced::Task;
use serde_json::{Value, json};
use signex_types::schematic::{
    NoteFrame, NoteSize, RichNoteLayout, SelectedItem, SelectedKind, TextNoteFormat,
};

use super::placed_operands::PlacedRefusal;
use super::placement_operands::{run_edit, text};
use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::{Message, Signex};

type Ran = (CommandOutcome, Task<Message>);

/// The text of the optional operand `name`, trimmed.
fn optional(args: &CommandArgs, name: &str) -> Option<String> {
    match args.operand(name) {
        Some(OperandValue::Text(value)) => Some(value.trim().to_string()),
        _ => None,
    }
}

/// A length in mm at or above zero, from the operand `name`'s `text`.
pub(crate) fn length_mm(text: &str, name: &str) -> Result<f64, String> {
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|mm| mm.is_finite() && *mm >= 0.0)
        .ok_or_else(|| format!("`{name}` is a length in mm at or above zero, got `{text}`"))
}

/// A border width: `auto` (the sheet's line width) or a length in mm.
pub(crate) fn border_mm(text: &str) -> Result<Option<f64>, String> {
    if text.trim().eq_ignore_ascii_case("auto") || text.trim().is_empty() {
        return Ok(None);
    }
    length_mm(text, "border").map(Some)
}

/// `set_text_note_frame id=.. frame=none|border|filled [border=mm|auto]
/// [padding=mm] [document=..]`: the Frame section.
pub(super) fn set_text_note_frame(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| {
        let item = app.placed_item_operand(args)?;
        let word = text(args, "frame")?;
        let frame = NoteFrame::from_word(word.trim())
            .ok_or_else(|| format!("`frame` is `none`, `border` or `filled`, got `{word}`"))?;
        let mut layout = app.text_note_layout(item)?;
        layout.frame = frame;
        if let Some(border) = optional(args, "border") {
            layout.border_mm = border_mm(&border)?;
        }
        if let Some(padding) = optional(args, "padding") {
            layout.padding_mm = length_mm(&padding, "padding")?;
        }
        answered(app.set_text_note_layout(item, layout))
    })
}

/// `set_text_note_size id=.. sizing=fit_text|fixed_width|fixed_box
/// [width=mm] [height=mm] [document=..]`: the Size section.
pub(super) fn set_text_note_size(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| {
        let item = app.placed_item_operand(args)?;
        let word = text(args, "sizing")?;
        let size = NoteSize::from_word(word.trim()).ok_or_else(|| {
            format!("`sizing` is `fit_text`, `fixed_width` or `fixed_box`, got `{word}`")
        })?;
        let mut layout = app.text_note_layout(item)?;
        layout.size = size;
        if let Some(width) = optional(args, "width") {
            layout.width_mm = length_mm(&width, "width")?;
        }
        if let Some(height) = optional(args, "height") {
            layout.height_mm = length_mm(&height, "height")?;
        }
        answered(app.set_text_note_layout(item, layout))
    })
}

/// `apply_rich_note_edit id=.. source=.. [document=..]`: the note's text
/// becomes `source`, one undo step. A call without `source` is refused and
/// reads nothing from the editor: only the editor's own Apply (its button
/// and key) applies the person's typing (Caner, 2026-11-27).
pub(super) fn apply_rich_note_edit(app: &mut Signex, args: &CommandArgs) -> Ran {
    let Some(OperandValue::Text(source)) = args.operand("source") else {
        let reason =
            signex_i18n::Translatable::new(signex_i18n::ids::OVERLAY_RICH_NOTE_NEEDS_SOURCE);
        return (CommandOutcome::Disabled { reason }, Task::none());
    };
    let source = source.clone();
    run_edit(app, args, |app| {
        let item = app.placed_item_operand(args)?;
        answered(app.set_rich_note_source(item, &source))
    })
}

fn answered(placed: Result<Value, PlacedRefusal>) -> Result<CommandOutcome, String> {
    placed
        .map(CommandOutcome::Answered)
        .map_err(|refusal| refusal.reason)
}

impl Signex {
    /// The Frame and Size sections of the rich note `item` on the sheet an
    /// edit acts on.
    pub(crate) fn text_note_layout(&self, item: SelectedItem) -> Result<RichNoteLayout, String> {
        let engine = (self.document_state.edit_engine()).ok_or("no schematic is open")?;
        engine
            .document()
            .text_notes
            .iter()
            .find(|note| item.kind == SelectedKind::TextNote && note.uuid == item.uuid)
            .map(|note| note.rich)
            .ok_or_else(|| format!("{} is not a text note", item.uuid))
    }

    /// Set the Frame and Size sections of the text note `item`: one undo
    /// step. The answer adds the sections as written.
    pub(crate) fn set_text_note_layout(
        &mut self,
        item: SelectedItem,
        layout: RichNoteLayout,
    ) -> Result<Value, PlacedRefusal> {
        let props = signex_engine::TextNoteProps {
            rich: Some(layout),
            ..Default::default()
        };
        let mut answer = self.set_text_note_props(item, props)?;
        answer["frame"] = json!(layout.frame.as_str());
        answer["border_mm"] = json!(layout.border_mm);
        answer["padding_mm"] = json!(layout.padding_mm);
        answer["sizing"] = json!(layout.size.as_str());
        answer["width_mm"] = json!(layout.width_mm);
        answer["height_mm"] = json!(layout.height_mm);
        Ok(answer)
    }

    /// The rich note `item`'s text becomes `source`: one undo step. An editor
    /// open on the note closes, since its text is no longer the note's.
    pub(crate) fn set_rich_note_source(
        &mut self,
        item: SelectedItem,
        source: &str,
    ) -> Result<Value, PlacedRefusal> {
        let rich = (self.document_state.edit_engine())
            .and_then(|engine| {
                engine
                    .document()
                    .text_notes
                    .iter()
                    .find(|note| note.uuid == item.uuid)
            })
            .is_some_and(|note| note.format == TextNoteFormat::Typst);
        if !rich {
            return Err(PlacedRefusal::new(
                "only a rich text note has a Typst source; set its format to `typst` first",
            ));
        }
        self.apply_placed_edit(signex_engine::Command::UpdateText {
            target: signex_engine::TextTarget::TextNote(item.uuid),
            value: source.to_string(),
        })?;
        let rich_notes = &mut self.ui_state.rich_notes;
        if rich_notes
            .editor
            .as_ref()
            .is_some_and(|e| e.note == item.uuid)
        {
            rich_notes.editor = None;
        }
        let mut answer = self.placed_answer(item)?;
        answer["text"] = json!(source);
        answer["applied"] = json!(true);
        Ok(answer)
    }
}
