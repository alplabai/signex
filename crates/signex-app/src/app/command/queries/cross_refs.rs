//! `query_cross_references document=<sheet>` (#1273): every Global label of
//! the sheet with the pages and frame zones its net continues on, as the
//! canvas and the PDF print them beside the label. Read from the project's
//! net index; when it is not current the query waits for the build and is
//! answered when it lands. A sheet in no project is one page, so its labels
//! list none. See `docs/COMMANDS.md`.

use std::path::Path;

use iced::Task;
use serde_json::{Value, json};
use signex_i18n::{Translatable, ids};
use signex_output::cross_ref_text;
use signex_types::schematic::LabelType;

use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::document::DocumentKey;
use crate::app::document_ids::DocumentKind;
use crate::app::state::net_nav::ProjectNetData;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

impl Signex {
    pub(super) fn query_cross_references(
        &mut self,
        command: &AppCommandId,
        args: &CommandArgs,
    ) -> (CommandOutcome, Task<Message>) {
        let path = match self.cross_ref_sheet(args) {
            Ok(path) => path,
            Err(reason) => return (CommandOutcome::Disabled { reason }, Task::none()),
        };
        match self.document_state.document_project(&path).map(|p| p.id) {
            Some(project) => self.start_cross_ref_query(command, args, project, path),
            None => (
                CommandOutcome::Answered(labels_answer(self, &path, None)),
                Task::none(),
            ),
        }
    }

    /// The open sheet `args` names.
    fn cross_ref_sheet(&self, args: &CommandArgs) -> Result<std::path::PathBuf, Translatable> {
        let entry = self.document_operand(args).map_err(|reason| {
            Translatable::new(ids::PROJECT_CROSS_REFS_DOCUMENT).arg("reason", reason)
        })?;
        match &entry.key {
            Some(DocumentKey::Schematic(path))
                if self.document_state.engines.contains_key(path) =>
            {
                Ok(path.clone())
            }
            _ if entry.kind == DocumentKind::Schematic => {
                Err(Translatable::new(ids::PROJECT_CROSS_REFS_NOT_OPEN)
                    .arg("name", entry.name.as_str()))
            }
            _ => Err(Translatable::new(ids::PROJECT_CROSS_REFS_NOT_A_SHEET)),
        }
    }

    /// The answer for `document` from the project index `data`.
    pub(crate) fn cross_refs_answer(&self, document: &Path, data: &ProjectNetData) -> Value {
        labels_answer(self, document, Some(data))
    }
}

/// Every Global label of `document`, in sheet order, each with the cross
/// references of every placement of the sheet that has one.
fn labels_answer(app: &Signex, document: &Path, data: Option<&ProjectNetData>) -> Value {
    let pages: Vec<_> = data
        .map(|data| {
            data.cross_refs
                .pages()
                .iter()
                .filter(|page| page.path == document)
                .collect()
        })
        .unwrap_or_default();
    let labels: Vec<Value> = app
        .document_state
        .engines
        .get(document)
        .map(|engine| {
            engine
                .document()
                .labels
                .iter()
                .filter(|label| label.label_type == LabelType::Global)
                .map(|label| {
                    let placements: Vec<Value> = pages
                        .iter()
                        .filter_map(|page| {
                            let (_, targets) =
                                page.labels.iter().find(|(id, _)| *id == label.uuid)?;
                            Some(json!({
                                "placement": page.placement.to_string(),
                                "cross_reference": cross_ref_text(targets),
                                "targets": targets.iter().map(|t| json!({
                                    "page": t.page,
                                    "zone": t.zone,
                                    "position": [t.x_mm, t.y_mm],
                                })).collect::<Vec<_>>(),
                            }))
                        })
                        .collect();
                    json!({
                        "id": label.uuid.to_string(),
                        "text": label.text,
                        "position": [label.position.x, label.position.y],
                        "placements": placements,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    json!({ "count": labels.len(), "labels": labels })
}
