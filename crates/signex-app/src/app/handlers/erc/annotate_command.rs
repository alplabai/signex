//! `annotate_schematic_quietly document=..` and
//! `force_annotate_all_schematics document=..` (#2470): the project-wide
//! numbering run on the project of the sheet named, whatever tab is active.
//! It is the menu's run (`begin_run_at`, the same read, the same pass and
//! the same undo group); the caller is answered when the run ends, with every
//! designator it changed, sheet by sheet. Nothing is written to disk.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use iced::Task;
use serde_json::{Value, json};
use signex_engine::AnnotateMode;
use signex_types::schematic::Symbol;
use uuid::Uuid;

use super::annotate_job::PassKind;
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::document::DocumentKey;
use crate::app::project_sheets::ProjectSheetSet;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

type Ran = (CommandOutcome, Task<Message>);

fn refused(reason: impl Into<String>) -> Ran {
    (annotate_refused(reason), Task::none())
}

/// An annotate run's refusal, the reason under the catalog's "did not run"
/// line; the run that ends in `annotate_job` shares it.
pub(super) fn annotate_refused(why: impl Into<String>) -> CommandOutcome {
    let reason = signex_i18n::Translatable::new(signex_i18n::ids::DIALOG_ANNOTATE_COMMAND_REFUSED)
        .arg("why", why.into());
    CommandOutcome::Disabled { reason }
}

/// `annotate_schematic_quietly` from its operands: number the `?` parts.
pub(crate) fn annotate_quietly_by_operands(app: &mut Signex, args: &CommandArgs) -> Ran {
    annotate_by_operands(
        app,
        args,
        "annotate_schematic_quietly",
        AnnotateMode::Incremental,
    )
}

/// `force_annotate_all_schematics` from its operands: reset and renumber.
pub(crate) fn force_annotate_by_operands(app: &mut Signex, args: &CommandArgs) -> Ran {
    annotate_by_operands(
        app,
        args,
        "force_annotate_all_schematics",
        AnnotateMode::ResetAndRenumber,
    )
}

/// The loaded sheet `document` names; `None` when the operand is left out.
fn named_sheet(app: &Signex, args: &CommandArgs, id: &str) -> Result<Option<PathBuf>, String> {
    let Some(OperandValue::Document(target)) = args.operand("document") else {
        return Ok(None);
    };
    let entry = app.resolve_document_target(target)?;
    match entry.key {
        Some(DocumentKey::Schematic(path)) if app.document_state.engines.contains_key(&path) => {
            Ok(Some(path))
        }
        Some(DocumentKey::Schematic(_)) | None => {
            Err(format!("{} is not loaded; open it first", entry.name))
        }
        Some(_) => Err(format!(
            "`{id}` numbers sheets, and {} is a {}",
            entry.name,
            entry.kind.as_str()
        )),
    }
}

fn annotate_by_operands(app: &mut Signex, args: &CommandArgs, id: &str, mode: AnnotateMode) -> Ran {
    let command = match AppCommandId::new(id) {
        Ok(command) => command,
        Err(why) => return refused(why),
    };
    let anchor = match named_sheet(app, args, id) {
        Ok(anchor) => anchor,
        Err(why) => return refused(why),
    };
    if anchor.is_none() && app.document_state.active_path.is_none() {
        return refused(format!(
            "`{id}` needs the operand `document` or a sheet in view"
        ));
    }
    let waiting = app.wait_for_command(&command, args);
    let ticket = waiting.ticket;
    match app.begin_run_at(PassKind::Annotate(mode), None, anchor, Some(waiting)) {
        Ok(task) => (CommandOutcome::Deferred { ticket }, task),
        Err(why) => refused(why),
    }
}

/// Each sheet's symbols with their designators.
pub(super) type SheetDesignators = BTreeMap<PathBuf, Vec<(Uuid, String)>>;

fn designators(symbols: &[Symbol]) -> Vec<(Uuid, String)> {
    symbols
        .iter()
        .map(|symbol| (symbol.uuid, symbol.reference.clone()))
        .collect()
}

impl Signex {
    /// The designators of every sheet of `set` (open ones read from their
    /// engines), and of `anchor` when the set does not hold it.
    pub(super) fn designators_before(
        &self,
        set: &ProjectSheetSet,
        anchor: Option<&Path>,
    ) -> SheetDesignators {
        let mut found: SheetDesignators = set
            .sheets
            .iter()
            .map(|(path, sheet)| (path.clone(), designators(&sheet.symbols)))
            .collect();
        if let Some(anchor) = anchor
            && !found.contains_key(anchor)
            && let Some(engine) = self.document_state.engines.get(anchor)
        {
            found.insert(
                anchor.to_path_buf(),
                designators(&engine.document().symbols),
            );
        }
        found
    }

    /// The designators the sheets of `before` have now: from the engine that
    /// holds each (a sheet the run changed is parked in one), else unchanged.
    pub(super) fn designators_now(&self, before: &SheetDesignators) -> SheetDesignators {
        before
            .iter()
            .map(|(path, was)| {
                let now = self
                    .document_state
                    .engines
                    .get(path)
                    .map_or_else(|| was.clone(), |e| designators(&e.document().symbols));
                (path.clone(), now)
            })
            .collect()
    }
}

/// A symbol of `sheet` whose designator went from `old` to `new`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DesignatorChange {
    pub sheet: PathBuf,
    pub symbol: Uuid,
    pub old: String,
    pub new: String,
}

/// What changed between `before` and `after`, sheet by sheet in path order,
/// each sheet's symbols in their order.
pub(super) fn designator_changes(
    before: &SheetDesignators,
    after: &SheetDesignators,
) -> Vec<DesignatorChange> {
    let mut changes = Vec::new();
    for (sheet, was) in before {
        let Some(now) = after.get(sheet) else {
            continue;
        };
        let now: HashMap<Uuid, &str> = now.iter().map(|(id, r)| (*id, r.as_str())).collect();
        for (symbol, old) in was {
            if let Some(new) = now.get(symbol)
                && *new != old.as_str()
            {
                changes.push(DesignatorChange {
                    sheet: sheet.clone(),
                    symbol: *symbol,
                    old: old.clone(),
                    new: (*new).to_string(),
                });
            }
        }
    }
    changes
}

fn mode_name(mode: AnnotateMode) -> &'static str {
    match mode {
        AnnotateMode::Incremental => "incremental",
        AnnotateMode::ResetAndRenumber => "reset_and_renumber",
        AnnotateMode::ResetOnly => "reset_only",
    }
}

/// The answer: the run's mode, each change and how many sheets changed.
pub(super) fn answer(mode: Option<AnnotateMode>, changes: &[DesignatorChange]) -> Value {
    let sheets: BTreeSet<&PathBuf> = changes.iter().map(|change| &change.sheet).collect();
    let rows: Vec<Value> = changes
        .iter()
        .map(|change| {
            json!({
                "sheet": change.sheet.display().to_string(),
                "symbol": change.symbol.to_string(),
                "old": change.old,
                "new": change.new,
            })
        })
        .collect();
    json!({
        "mode": mode.map(mode_name),
        "changes": rows,
        "sheets_changed": sheets.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet(refs: &[(u128, &str)]) -> Vec<(Uuid, String)> {
        refs.iter()
            .map(|(id, r)| (Uuid::from_u128(*id), (*r).to_string()))
            .collect()
    }

    #[test]
    fn only_changed_designators_are_listed_sheet_by_sheet() {
        let before = SheetDesignators::from([
            ("a.snxsch".into(), sheet(&[(1, "R?"), (2, "C4")])),
            ("b.snxsch".into(), sheet(&[(3, "U?")])),
        ]);
        let after = SheetDesignators::from([
            ("a.snxsch".into(), sheet(&[(1, "R1"), (2, "C4")])),
            ("b.snxsch".into(), sheet(&[(3, "U1")])),
        ]);
        let changes = designator_changes(&before, &after);
        let listed: Vec<_> = changes
            .iter()
            .map(|c| (c.sheet.clone(), c.old.as_str(), c.new.as_str()))
            .collect();
        assert_eq!(
            listed,
            [
                (PathBuf::from("a.snxsch"), "R?", "R1"),
                (PathBuf::from("b.snxsch"), "U?", "U1"),
            ]
        );
        let value = answer(Some(AnnotateMode::Incremental), &changes);
        assert_eq!(value["mode"], "incremental");
        assert_eq!(value["sheets_changed"], 2);
        assert_eq!(
            value["changes"][0]["symbol"],
            Uuid::from_u128(1).to_string()
        );
    }

    #[test]
    fn a_symbol_gone_since_is_not_a_change() {
        let before = SheetDesignators::from([("a.snxsch".into(), sheet(&[(1, "R?")]))]);
        let after = SheetDesignators::from([("a.snxsch".into(), sheet(&[]))]);
        assert!(designator_changes(&before, &after).is_empty());
    }
}
