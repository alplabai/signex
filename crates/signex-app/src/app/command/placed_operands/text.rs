//! `set_object_text` (#2953): a placed part's designator or value, a label's
//! name or a text note's text, as one engine edit and one undo step. The
//! Object List's inline edit and an operand run call
//! [`Signex::set_object_text`]; it sends the engine commands the Properties
//! rows send for the same field (`handlers/dock/property_editor.rs`), so the
//! three agree. Rules: `docs/CLI.md`, "Editing placed objects".

use serde_json::json;
use signex_engine::TextTarget;
use signex_types::schematic::{SchematicSheet, SelectedItem, SelectedKind};

use super::super::CommandArgs;
use super::super::placement_operands::{run_edit, text};
use super::{Placed, PlacedRefusal, Ran, answered};
use crate::app::Signex;
use crate::app::mutation_gateway::MutationOutcome as Applied;

/// The `field` operand's words, in the order the docs list them.
pub(crate) const TEXT_FIELDS: [&str; 3] = ["designator", "value", "text"];

/// The text `field` of `item` names on `sheet`: `designator` or `value` of a
/// part (a power port's value is its name), `text` of a label or a note.
pub(crate) fn text_target(
    sheet: &SchematicSheet,
    item: SelectedItem,
    field: &str,
) -> Result<TextTarget, String> {
    let power = || {
        sheet
            .symbols
            .iter()
            .any(|s| s.uuid == item.uuid && s.is_power)
    };
    match (item.kind, field.trim()) {
        (SelectedKind::Symbol, "designator") if power() => {
            Err("a power port has no designator; its name is its `value`".into())
        }
        (SelectedKind::Symbol, "designator") => Ok(TextTarget::SymbolReference(item.uuid)),
        (SelectedKind::Symbol, "value") => Ok(TextTarget::SymbolValue(item.uuid)),
        (SelectedKind::Label, "text") => Ok(TextTarget::Label(item.uuid)),
        (SelectedKind::TextNote, "text") => Ok(TextTarget::TextNote(item.uuid)),
        (_, word) if !TEXT_FIELDS.contains(&word) => Err(format!(
            "`field` is one of {}, got `{word}`",
            TEXT_FIELDS.join(", ")
        )),
        (_, word) => Err(format!("this object has no `{word}` to set")),
    }
}

/// The field word of `target`, as the answer names it.
fn field_word(target: TextTarget) -> &'static str {
    match target {
        TextTarget::SymbolReference(_) => "designator",
        TextTarget::SymbolValue(_) => "value",
        TextTarget::Label(_) | TextTarget::TextNote(_) => "text",
    }
}

/// `set_object_text id=.. field=designator|value|text [value=..] [document=..]`.
pub(in crate::app::command) fn set_object_text(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| {
        let item = app.placed_item_operand(args)?;
        let field = text(args, "field")?;
        let value = text(args, "value").unwrap_or_default();
        let target = {
            let engine = app
                .document_state
                .edit_engine()
                .ok_or("no schematic is open")?;
            text_target(engine.document(), item, &field)?
        };
        answered(app.set_object_text(item, target, &value))
    })
}

impl Signex {
    /// Set the text `target` of `item` on the sheet an edit acts on to
    /// `value`: one engine edit, one undo step that no typing run of the
    /// same field joins. A designator is written as the Designator row
    /// writes it, for the placement the sheet in view shows (#1151), so it
    /// is set on the sheet in view only.
    pub(crate) fn set_object_text(
        &mut self,
        item: SelectedItem,
        target: TextTarget,
        value: &str,
    ) -> Placed {
        let commands = match target {
            TextTarget::SymbolReference(symbol) => {
                let ds = &self.document_state;
                if ds.edit_path() != ds.active_path.as_deref() {
                    return Err(PlacedRefusal::new(
                        "a designator is set on the sheet in view, whose placement it names",
                    ));
                }
                let commands = self.designator_edit_commands(symbol, value.to_string());
                if commands.is_empty() {
                    // `designator_edit_commands` said why in Messages.
                    return Err(PlacedRefusal {
                        reason: "the designator cannot be written on this sheet".into(),
                        logged: true,
                    });
                }
                commands
            }
            _ => vec![signex_engine::Command::UpdateText {
                target,
                value: value.to_string(),
            }],
        };
        self.end_parameter_typing_run();
        let applied = self.apply_placed_edits(commands);
        self.end_parameter_typing_run();
        applied?;
        let mut answer = self.placed_answer(item)?;
        let stored = self.stored_object_text(target);
        answer["text"] = json!({ "field": field_word(target), "value": stored });
        Ok(answer)
    }

    /// The text `target` holds on the sheet an edit acts on, a designator
    /// as the sheet in view shows it (its placement, #1151).
    pub(crate) fn stored_object_text(&self, target: TextTarget) -> Option<String> {
        let ds = &self.document_state;
        let path = ds.edit_path()?;
        let placement = (ds.active_path.as_deref() == Some(path))
            .then(|| self.shown_placement(path))
            .flatten();
        let engine = ds.edit_engine()?;
        signex_engine::stored_text(engine.document(), target, placement.as_ref())
    }

    /// [`Signex::apply_placed_edit`] for edits that go together: refused,
    /// with nothing sent, when a lock holds what any of them touches.
    fn apply_placed_edits(
        &mut self,
        commands: Vec<signex_engine::Command>,
    ) -> Result<(), PlacedRefusal> {
        let engine = self
            .document_state
            .edit_engine()
            .ok_or_else(|| PlacedRefusal::new("no schematic is open"))?;
        if let Some(reason) = commands.iter().find_map(|c| engine.lock_refusal(c)) {
            return Err(PlacedRefusal::new(reason));
        }
        let (outcome, reason) = self.apply_engine_commands_reporting(commands, true, true);
        match outcome {
            Applied::Changed | Applied::Unchanged => Ok(()),
            Applied::Failed => Err(PlacedRefusal {
                reason: reason.unwrap_or_else(|| "the engine refused the edit".into()),
                logged: true,
            }),
        }
    }
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
