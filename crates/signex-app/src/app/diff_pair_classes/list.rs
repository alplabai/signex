//! `list_diff_pairs` (#2952): every differential pair of a project's open
//! sheets, found by a directive or by name, with its class and where it was
//! found, so the pairs can be checked before Update PCB. A read-only query;
//! the list is `signex_net::listed_pairs`. Answer shape: `docs/CLI.md`,
//! "Differential pairs".

use serde_json::{Value, json};
use signex_net::{ListedPair, PairFoundBy};
use signex_types::schematic::SchematicSheet;

use crate::app::Signex;
use crate::app::command::CommandArgs;
use crate::app::state::ProjectId;

impl Signex {
    /// `list_diff_pairs document=..`: the pairs of the project `document`
    /// is (or belongs to). Pairs on a sheet that is not open are not
    /// listed; `unread_sheets` says when there are such sheets.
    pub(crate) fn list_diff_pairs(&mut self, args: &CommandArgs) -> Result<Value, String> {
        let id = self.rules_project_named(args)?;
        let project = self
            .document_state
            .project_by_id(id)
            .ok_or("the project is no longer open")?;
        let classes = &project.data.schematic_design.diff_pair_classes;
        let unread_sheets = self.project_has_closed_sheets(project);
        let pairs = signex_net::listed_pairs(&self.project_open_sheets(id), classes);
        Ok(json!({
            "project": project.path.display().to_string(),
            "count": pairs.len(),
            "unread_sheets": unread_sheets,
            "pairs": pairs.iter().map(pair_json).collect::<Vec<_>>(),
        }))
    }

    /// The open sheets of project `id`, by file name, in file-name order.
    pub(super) fn project_open_sheets(&self, id: ProjectId) -> Vec<(String, &SchematicSheet)> {
        let mut sheets: Vec<(String, &SchematicSheet)> = self
            .document_state
            .engines
            .iter()
            .filter(|(path, _)| {
                self.document_state
                    .document_project(path)
                    .is_some_and(|p| p.id == id)
            })
            .map(|(path, engine)| {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                (name, engine.document())
            })
            .collect();
        sheets.sort_by(|a, b| a.0.cmp(&b.0));
        sheets
    }
}

/// One pair of the answer. A missing half is `null`.
fn pair_json(pair: &ListedPair) -> Value {
    let (found_by, directive) = match pair.found_by {
        PairFoundBy::Directive(uuid) => ("directive", Some(uuid.to_string())),
        PairFoundBy::Suffix => ("suffix", None),
    };
    json!({
        "name": pair.name,
        "positive": pair.positive,
        "negative": pair.negative,
        "class": pair.class,
        "found_by": found_by,
        "directive": directive,
        "sheet": pair.sheet,
    })
}

#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;
