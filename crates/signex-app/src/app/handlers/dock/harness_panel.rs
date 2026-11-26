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

#[cfg(test)]
thread_local! {
    /// Test probe: how many times this thread built the harness facts.
    pub(crate) static HARNESS_FACT_BUILDS: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
}

impl Signex {
    /// Bring the harness panels up to date. They are rebuilt only when what
    /// they read changed (see [`Self::harness_facts_key`]). A rebuild drops
    /// the typed draft; refused text changes nothing, so it stays in its box.
    pub(crate) fn refresh_harness_panel(&mut self) {
        let key = self.harness_facts_key();
        if self.document_state.panel_ctx.harness.built_from == Some(key) {
            return;
        }
        let mut facts = self.harness_panel_facts();
        facts.built_from = Some(key);
        *self.document_state.panel_ctx.harness = facts;
    }

    /// A stamp of everything [`Self::harness_panel_facts`] reads: the
    /// project in view (its name and harness definitions), every open
    /// sheet's revision, the sheet in view, the selection and the harness
    /// mismatch findings.
    fn harness_facts_key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let docs = &self.document_state;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        let project = self.project_in_view().and_then(|id| docs.project_by_id(id));
        if let Some(project) = project {
            (project.id, &project.data.name).hash(&mut hasher);
            project
                .data
                .has_unreadable_schematic_design()
                .hash(&mut hasher);
            for d in &project.data.schematic_design.harness_definitions {
                (&d.name, &d.description).hash(&mut hasher);
                for e in &d.entries {
                    (&e.name, &e.harness).hash(&mut hasher);
                }
                d.entries.len().hash(&mut hasher);
            }
        }
        // Order-independent: the open sheets and their revisions.
        let sheets = docs
            .engines
            .iter()
            .map(|(path, engine)| {
                let mut one = std::collections::hash_map::DefaultHasher::new();
                (path, engine.revision()).hash(&mut one);
                one.finish()
            })
            .fold(0u64, u64::wrapping_add);
        (sheets, docs.engines.len()).hash(&mut hasher);
        docs.active_engine()
            .map(signex_engine::Engine::revision)
            .hash(&mut hasher);
        docs.active_path.hash(&mut hasher);
        (docs.panel_ctx.selected_uuid, docs.panel_ctx.selected_kind).hash(&mut hasher);
        for v in &self.ui_state.erc_violations {
            if v.rule_id.as_str() == MISMATCH_RULE {
                (v.primary, &v.tag).hash(&mut hasher);
            }
        }
        hasher.finish()
    }

    /// What the harness panels show for the current selection. Keeps the
    /// definitions view open and its pick across refreshes.
    fn harness_panel_facts(&self) -> HarnessPanel {
        #[cfg(test)]
        HARNESS_FACT_BUILDS.with(|n| n.set(n.get() + 1));
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
        facts.are_definitions_locked =
            project.is_some_and(|p| p.data.has_unreadable_schematic_design());
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

    /// The far-side mismatches of `c` from the sheet's ERC findings.
    fn harness_mismatch_rows(
        &self,
        c: &signex_types::schematic::HarnessConnector,
    ) -> Vec<HarnessMismatchRow> {
        self.ui_state
            .erc_violations
            .iter()
            .filter_map(|v| {
                let (entry, far, expects) = far_side_mismatch(v)?;
                let index = c.entry_index(entry)?;
                Some(HarnessMismatchRow {
                    entry,
                    has: c.entries[index].name.clone(),
                    expects: expects.to_string(),
                    far: far.to_string(),
                })
            })
            .collect()
    }

    /// The caption the canvas draws under the marker of a far-side mismatch
    /// on a harness entry (mockup row 152): `expects CS_ADC`. `None` for any
    /// other finding.
    pub(crate) fn erc_marker_caption(&self, v: &signex_erc::Violation) -> Option<String> {
        let (_, _, expects) = far_side_mismatch(v)?;
        Some(self.ui_state.i18n.message_plain(
            ids::PANELS_HARNESS_EXPECTS,
            &[("expects", expects.to_string().into())],
        ))
    }

    /// The harness entry the canvas tints for a far-side mismatch (mockup
    /// row 152); `None` for any other finding.
    pub(crate) fn erc_marker_harness_entry(v: &signex_erc::Violation) -> Option<uuid::Uuid> {
        far_side_mismatch(v).map(|(entry, _, _)| entry)
    }

    /// The sheet entry the canvas badges for a harness mismatch with the far
    /// side: the finding's peer, the entry the harness leaves through
    /// (decision of 2026-11-25). `None` for any other finding.
    pub(crate) fn erc_marker_sheet_entry(v: &signex_erc::Violation) -> Option<uuid::Uuid> {
        if v.rule_id.as_str() != MISMATCH_RULE {
            return None;
        }
        v.peer
            .filter(|p| p.kind == SelectedKind::SheetPin)
            .map(|p| p.uuid)
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
        let mut refused_text = None;
        match msg.clone() {
            HarnessPanelMsg::TypePicked {
                connector,
                harness_type,
            } => self.retype_harness_connector(connector, harness_type),
            HarnessPanelMsg::FieldTyped {
                connector,
                field,
                text,
            } => refused_text = self.type_harness_field(connector, field, text),
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
                    task = self.open_sheet_through_symbol(symbol, &file);
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
            HarnessPanelMsg::AddEntryClicked { connector } => {
                self.add_harness_entry(connector);
            }
        }
        self.refresh_harness_panel();
        // A rebuild for an earlier change must not take the refused text.
        if refused_text.is_some() {
            self.document_state.panel_ctx.harness.draft = refused_text;
        }
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

    /// A keystroke in a placement box. A number the field can hold is applied
    /// at once (typing in one connector is one undo step). Anything else
    /// changes nothing and is returned, as the draft its box keeps.
    fn type_harness_field(
        &mut self,
        id: uuid::Uuid,
        field: HarnessField,
        text: String,
    ) -> Option<(HarnessField, String)> {
        let parsed = crate::numeric_field::parse_field_f64(&text);
        let Some(value) = parsed.filter(|value| harness_field_holds(field, *value)) else {
            return Some((field, text));
        };
        self.document_state.panel_ctx.harness.draft = None;
        let (mut c, stored_y) = self.document_state.edit_engine().and_then(|e| {
            let sheet = e.document();
            Some((sheet.harness.connector(id).cloned()?, sheet.page_y(value)))
        })?;
        match field {
            HarnessField::X => c.position.x = value,
            HarnessField::Y => c.position.y = stored_y,
            HarnessField::Width => c.width = value,
            HarnessField::EntryPitch => c.entry_pitch = value,
        }
        self.update_harness_connector(c);
        None
    }

    /// The + of the Entries header: one entry after the last, named as the
    /// Harness Entry tool names one, as one undo step of its own.
    fn add_harness_entry(&mut self, id: uuid::Uuid) -> bool {
        let definitions = self.harness_definitions_in_view();
        let Some((entry, index)) = self.document_state.edit_engine().and_then(|e| {
            let c = e.document().harness.connector(id)?;
            let entry = signex_types::schematic::HarnessEntry {
                uuid: uuid::Uuid::now_v7(),
                name: signex_types::schematic::next_entry_name(c, &definitions),
            };
            Some((entry, c.entries.len()))
        }) else {
            return false;
        };
        self.end_directive_edit_run();
        let done = self.apply_engine_command(
            signex_engine::Command::PlaceHarnessEntry {
                connector_id: id,
                entry,
                index,
            },
            false,
            true,
        );
        self.end_directive_edit_run();
        done
    }

    /// Apply `edit` to the project's definitions as one step of the project's
    /// history. `false` when there is no project or its settings section is
    /// unreadable, so nothing that follows from the edit should run.
    fn edit_harness_definitions(&mut self, edit: impl FnOnce(&mut Vec<HarnessDefinition>)) -> bool {
        let Some(id) = self.project_in_view() else {
            crate::diagnostics::log_warning(
                t!(self.ui_state.i18n, ids::HARNESS_MSG_DEFS_NO_PROJECT).into_string(),
            );
            return false;
        };
        if self.refuse_schematic_design_edit(id) {
            return false;
        }
        let mut definitions = self.harness_definitions_in_view();
        edit(&mut definitions);
        let command = signex_engine::ProjectSectionCommand::SetHarnessDefinitions { definitions };
        if let Err(problem) = self.apply_project_section(id, &command) {
            crate::diagnostics::log_warning(
                t!(self.ui_state.i18n, ids::HARNESS_MSG_DEFS_REFUSED, "why" => problem.to_string())
                    .into_string(),
            );
        }
        true
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
        let edited = self.edit_harness_definitions(|defs| defs[index].name = name.clone());
        if !edited || old == name {
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

/// A far-side mismatch finding located on a harness entry: the entry, the
/// sheet symbol whose child expects another name, and that name. The rule
/// tags it `far:<sheet symbol>:<expected entry>`.
fn far_side_mismatch(v: &signex_erc::Violation) -> Option<(uuid::Uuid, &str, &str)> {
    if v.rule_id.as_str() != MISMATCH_RULE {
        return None;
    }
    let entry = v
        .primary
        .filter(|p| p.kind == SelectedKind::HarnessEntry)?
        .uuid;
    let (far, expects) = v.tag.as_deref()?.strip_prefix("far:")?.rsplit_once(':')?;
    Some((entry, far, expects))
}

/// Whether `field` can hold the finite `value`: a size is above zero, as the
/// engine requires of a connector; a position is any number.
fn harness_field_holds(field: HarnessField, value: f64) -> bool {
    match field {
        HarnessField::X | HarnessField::Y => true,
        HarnessField::Width | HarnessField::EntryPitch => value > 0.0,
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

#[cfg(test)]
mod marker_caption_tests {
    use super::*;
    use signex_erc::{RuleId, RuleKind, Severity, Violation};
    use signex_types::schematic::{Point, SelectedItem};

    fn finding(kind: RuleKind, tag: &str) -> Violation {
        let mut v = Violation::new(
            RuleId::builtin(kind),
            kind,
            Severity::Warning,
            "finding",
            Point::new(10.0, 20.0),
        );
        v.primary = Some(SelectedItem::new(
            uuid::Uuid::from_u128(11),
            SelectedKind::HarnessEntry,
        ));
        v.tag = Some(tag.to_string());
        v
    }

    /// Mockup row 152, Entry mismatch: the marker of a far-side mismatch on
    /// an entry carries `expects CS_ADC` to the canvas; an unpaired entry and
    /// any other rule carry no caption.
    #[test]
    fn a_far_side_mismatch_marker_carries_what_the_far_side_expects() {
        let (mut app, _boot) = Signex::new();
        let path = std::path::PathBuf::from("mcu.snxsch");
        let findings = vec![
            finding(RuleKind::HarnessEntryMismatch, "far:sensor:CS_ADC"),
            finding(RuleKind::HarnessEntryMismatch, "extra:sensor:CS2"),
            finding(RuleKind::HarnessTypeConflict, "far:sensor:CS_ADC"),
        ];
        app.ui_state
            .erc_violations_by_path
            .insert(path.clone(), findings);
        app.refresh_active_erc_from_cache(Some(&path));
        let captions: Vec<Option<String>> = app
            .interaction_state
            .active_canvas()
            .erc_markers
            .iter()
            .map(|m| m.caption.clone())
            .collect();
        let expects = app
            .ui_state
            .i18n
            .message_plain(ids::PANELS_HARNESS_EXPECTS, &[("expects", "CS_ADC".into())]);
        assert_eq!(expects, "expects CS_ADC");
        assert_eq!(captions, [Some(expects), None, None]);
        let tinted: Vec<Option<uuid::Uuid>> = app
            .interaction_state
            .active_canvas()
            .erc_markers
            .iter()
            .map(|m| m.harness_entry)
            .collect();
        assert_eq!(tinted, [Some(uuid::Uuid::from_u128(11)), None, None]);
    }

    /// The sheet entry a far-side mismatch names as its peer is the one the
    /// canvas badges (Caner, 2026-11-25); another rule's peer, or a mismatch
    /// with no sheet entry, badges nothing.
    #[test]
    fn a_far_side_mismatch_marker_carries_its_sheet_entry() {
        let (mut app, _boot) = Signex::new();
        let path = std::path::PathBuf::from("mcu.snxsch");
        let entry = SelectedItem::new(uuid::Uuid::from_u128(21), SelectedKind::SheetPin);
        let through = |mut v: Violation| {
            v.peer = Some(entry);
            v
        };
        let findings = vec![
            through(finding(RuleKind::HarnessEntryMismatch, "far:sensor:CS_ADC")),
            through(finding(RuleKind::HarnessEntryMismatch, "extra:sensor:CS2")),
            finding(RuleKind::HarnessEntryMismatch, "definition:CS_ADC"),
            through(finding(RuleKind::HarnessTypeConflict, "far:sensor:CS_ADC")),
        ];
        app.ui_state
            .erc_violations_by_path
            .insert(path.clone(), findings);
        app.refresh_active_erc_from_cache(Some(&path));
        let badged: Vec<Option<uuid::Uuid>> = app
            .interaction_state
            .active_canvas()
            .erc_markers
            .iter()
            .map(|m| m.sheet_entry)
            .collect();
        assert_eq!(badged, [Some(entry.uuid), Some(entry.uuid), None, None]);
    }
}
