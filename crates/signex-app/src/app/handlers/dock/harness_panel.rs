//! The harness Properties and the harness definitions view (#1280): the
//! facts they show, gathered once per selection refresh, and their edits.
//! Connector edits are one `UpdateHarnessConnector` each (typing in one
//! connector is one undo step); definition edits are one step each on the
//! project's history (`ProjectSectionCommand::SetHarnessDefinitions`); a
//! type rename renames it in every connector of the open sheets, one step
//! per sheet.

use iced::Task;
use signex_i18n::{ids, t};

use super::super::super::*;
use crate::panels::{HarnessField, HarnessMismatchRow, HarnessPanel, HarnessPanelMsg};
use signex_types::schematic::{
    HarnessDefinition, HarnessDefinitionEntry, SelectedKind, harness_type_uses,
};

/// The ERC rule whose findings drive the mismatch banner.
const MISMATCH_RULE: &str = "builtin::harness_entry_mismatch";

impl Signex {
    /// What the harness panels show for the current selection. Keeps the
    /// definitions view open and its pick across refreshes.
    pub(crate) fn harness_panel_facts(&self) -> HarnessPanel {
        let old = &self.document_state.panel_ctx.harness;
        let mut facts = HarnessPanel {
            definitions_open: old.definitions_open,
            selected_definition: old.selected_definition,
            definitions: self.harness_definitions_in_view(),
            ..HarnessPanel::default()
        };
        let project = self
            .project_in_view()
            .and_then(|id| self.document_state.project_by_id(id));
        facts.project_name = project.map(|p| p.data.name.clone()).unwrap_or_default();
        let engines = &self.document_state.engines;
        facts.uses = facts
            .definitions
            .iter()
            .map(|d| harness_type_uses(engines.values().map(|e| &e.document().harness), &d.name))
            .collect();
        let Some(engine) = self.document_state.active_engine() else {
            return facts;
        };
        let sheet = engine.document();
        facts.sheet_name = self
            .document_state
            .active_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let (Some(id), Some(kind)) = (
            self.document_state.panel_ctx.selected_uuid,
            self.document_state.panel_ctx.selected_kind,
        ) else {
            return facts;
        };
        match kind {
            SelectedKind::HarnessConnector | SelectedKind::HarnessEntry => {
                let connector = sheet
                    .harness
                    .connector(id)
                    .or_else(|| sheet.harness.entry(id).map(|(c, _)| c))
                    .cloned();
                if let Some(c) = &connector {
                    facts.entry_nets = entry_nets(sheet, c);
                    facts.mismatches = self.harness_mismatch_rows(c);
                }
                facts.connector = connector;
            }
            SelectedKind::HarnessWire => {
                facts.wire_ends = sheet.harness.wire(id).map(|w| (w.start, w.end));
                facts.wire_type = signex_net::harness_bundles(sheet)
                    .into_iter()
                    .find(|b| b.wires.contains(&id))
                    .and_then(|b| b.net_type().map(str::to_string));
            }
            SelectedKind::SheetPin => {
                facts.sheet_entry_type = sheet.harness.entry_type(id).map(str::to_string);
            }
            _ => {}
        }
        facts
    }

    /// The far-side mismatches of `c` from the sheet's ERC findings: their
    /// tag is `far:<sheet symbol>:<expected entry>`.
    fn harness_mismatch_rows(
        &self,
        c: &signex_types::schematic::HarnessConnector,
    ) -> Vec<HarnessMismatchRow> {
        self.ui_state
            .erc_violations
            .iter()
            .filter(|v| v.rule_id.as_str() == MISMATCH_RULE)
            .filter_map(|v| {
                let entry = v
                    .primary
                    .filter(|p| p.kind == SelectedKind::HarnessEntry)?
                    .uuid;
                let index = c.entry_index(entry)?;
                let (far, expects) = v.tag.as_deref()?.strip_prefix("far:")?.rsplit_once(':')?;
                Some(HarnessMismatchRow {
                    entry,
                    has: c.entries[index].name.clone(),
                    expects: expects.to_string(),
                    far: far.to_string(),
                })
            })
            .collect()
    }

    /// Handle an edit or click of the harness panels; `None` for any other
    /// panel message. Go to answers with the task that opens the sheet.
    pub(super) fn handle_dock_harness_message(
        &mut self,
        panel_msg: &crate::panels::PanelMsg,
    ) -> Option<Task<Message>> {
        let crate::panels::PanelMsg::Harness(msg) = panel_msg else {
            return None;
        };
        let mut task = Task::none();
        match msg.clone() {
            HarnessPanelMsg::TypePicked {
                connector,
                harness_type,
            } => self.retype_harness_connector(connector, harness_type),
            HarnessPanelMsg::FieldTyped {
                connector,
                field,
                text,
            } => self.type_harness_field(connector, field, text),
            HarnessPanelMsg::RenameFixClicked {
                connector,
                entry,
                to,
            } => {
                self.rename_harness_entry(connector, entry, &to);
            }
            HarnessPanelMsg::GoToSheetClicked { far } => {
                let target = self.document_state.active_engine().and_then(|e| {
                    e.document()
                        .child_sheets
                        .iter()
                        .find(|cs| cs.name == far)
                        .map(|cs| (cs.uuid, cs.filename.clone()))
                });
                if let Some((symbol, file)) = target {
                    task = self.descend_through_sheet_symbol(symbol, &file);
                }
            }
            HarnessPanelMsg::DefinitionsOpened => {
                self.document_state.panel_ctx.harness.definitions_open = true;
            }
            HarnessPanelMsg::DefinitionsClosed => {
                self.document_state.panel_ctx.harness.definitions_open = false;
            }
            HarnessPanelMsg::DefinitionPicked(index) => {
                self.document_state.panel_ctx.harness.selected_definition = index;
            }
            HarnessPanelMsg::DefinitionNameTyped { index, text } => {
                self.rename_harness_type(index, text);
            }
            HarnessPanelMsg::DefinitionDescriptionTyped { index, text } => {
                self.edit_harness_definitions(|defs| {
                    if let Some(d) = defs.get_mut(index) {
                        d.description = text;
                    }
                });
            }
            HarnessPanelMsg::DefinitionEntryTyped { index, entry, text } => {
                self.edit_harness_definitions(|defs| {
                    if let Some(e) = defs.get_mut(index).and_then(|d| d.entries.get_mut(entry)) {
                        e.name = text;
                    }
                });
            }
            HarnessPanelMsg::NewTypeClicked => {
                let mut count = 0;
                self.edit_harness_definitions(|defs| {
                    let name = (1..)
                        .map(|n| format!("HARNESS_{n}"))
                        .find(|n| defs.iter().all(|d| &d.name != n))
                        .unwrap_or_default();
                    defs.push(HarnessDefinition {
                        name,
                        description: String::new(),
                        entries: vec![HarnessDefinitionEntry {
                            name: "SIGNAL_1".into(),
                            harness: None,
                        }],
                    });
                    count = defs.len();
                });
                self.document_state.panel_ctx.harness.selected_definition = count.saturating_sub(1);
            }
            HarnessPanelMsg::SheetEntryTypePicked { pin, harness_type } => {
                self.set_harness_entry_type(pin, harness_type);
            }
        }
        self.document_state.panel_ctx.harness = self.harness_panel_facts();
        Some(task)
    }

    /// Give the connector `id` the type `harness_type`, as one undo step.
    fn retype_harness_connector(&mut self, id: uuid::Uuid, harness_type: String) {
        let Some(mut c) = self
            .document_state
            .edit_engine()
            .and_then(|e| e.document().harness.connector(id).cloned())
        else {
            return;
        };
        c.harness_type = harness_type.clone();
        self.end_directive_edit_run();
        if self.update_harness_connector(c) {
            self.interaction_state.last_harness_type = Some(harness_type);
        }
        self.end_directive_edit_run();
    }

    /// A keystroke in a placement box: the text stays as a draft; a number
    /// is applied at once (typing in one connector is one undo step).
    fn type_harness_field(&mut self, id: uuid::Uuid, field: HarnessField, text: String) {
        self.document_state.panel_ctx.harness.draft = Some((field, text.clone()));
        let Ok(value) = text.trim().parse::<f64>() else {
            return;
        };
        let Some(mut c) = self
            .document_state
            .edit_engine()
            .and_then(|e| e.document().harness.connector(id).cloned())
        else {
            return;
        };
        match field {
            HarnessField::X => c.position.x = value,
            HarnessField::Y => c.position.y = value,
            HarnessField::Width => c.width = value,
            HarnessField::EntryPitch => c.entry_pitch = value,
        }
        self.update_harness_connector(c);
    }

    /// Apply `edit` to the project's definitions as one step of the project's
    /// history.
    fn edit_harness_definitions(&mut self, edit: impl FnOnce(&mut Vec<HarnessDefinition>)) {
        let Some(id) = self.project_in_view() else {
            crate::diagnostics::log_warning(
                t!(self.ui_state.i18n, ids::HARNESS_MSG_DEFS_NO_PROJECT).into_string(),
            );
            return;
        };
        let mut definitions = self.harness_definitions_in_view();
        edit(&mut definitions);
        let command = signex_engine::ProjectSectionCommand::SetHarnessDefinitions { definitions };
        if let Err(problem) = self.apply_project_section(id, &command) {
            crate::diagnostics::log_warning(
                t!(self.ui_state.i18n, ids::HARNESS_MSG_DEFS_REFUSED, "why" => problem.to_string())
                    .into_string(),
            );
        }
    }

    /// Rename the definition `index` to `name`, and the type of every
    /// connector of the open sheets that used the old name (one undo step
    /// per sheet).
    fn rename_harness_type(&mut self, index: usize, name: String) {
        let Some(old) = self
            .harness_definitions_in_view()
            .get(index)
            .map(|d| d.name.clone())
        else {
            return;
        };
        self.edit_harness_definitions(|defs| defs[index].name = name.clone());
        if old == name {
            return;
        }
        let active = self.document_state.active_path.clone();
        let paths: Vec<std::path::PathBuf> = self.document_state.engines.keys().cloned().collect();
        for path in paths {
            let Some(engine) = self.document_state.engines.get_mut(&path) else {
                continue;
            };
            let commands: Vec<signex_engine::Command> = engine
                .document()
                .harness
                .connectors
                .iter()
                .filter(|c| c.harness_type == old)
                .map(|c| signex_engine::Command::UpdateHarnessConnector {
                    uuid: c.uuid,
                    connector: signex_types::schematic::HarnessConnector {
                        harness_type: name.clone(),
                        ..c.clone()
                    },
                })
                .collect();
            if commands.is_empty() {
                continue;
            }
            engine.end_field_edit_run();
            if active.as_ref() == Some(&path) {
                // The sheet in view goes through the gateway, so it redraws.
                self.apply_engine_commands(commands, true, true);
                continue;
            }
            if let Err(error) = engine.execute_batch(commands) {
                crate::diagnostics::log_warning(
                    t!(
                        self.ui_state.i18n,
                        ids::HARNESS_MSG_RENAME_FAILED,
                        "file" => path.display().to_string(),
                        "why" => error.to_string()
                    )
                    .into_string(),
                );
                continue;
            }
            engine.end_field_edit_run();
            self.document_state.sync_history_dirty(&path);
        }
    }
}

/// Per entry of `c`: the name of the net its wire meets on `sheet`, empty
/// when no wire ends on it.
fn entry_nets(
    sheet: &signex_types::schematic::SchematicSheet,
    c: &signex_types::schematic::HarnessConnector,
) -> Vec<String> {
    let netlist = signex_net::build_netlist(sheet);
    (0..c.entries.len())
        .map(|i| {
            let at = c.entry_point(i).um_key();
            let wire = sheet
                .wires
                .iter()
                .find(|w| w.start.um_key() == at || w.end.um_key() == at)
                .map(|w| w.uuid);
            wire.and_then(|w| netlist.nets.iter().find(|n| n.wires.contains(&w)))
                .map(|n| n.name.clone())
                .unwrap_or_default()
        })
        .collect()
}
