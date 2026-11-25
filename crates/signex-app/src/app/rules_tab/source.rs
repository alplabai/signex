//! Where the Rules tab and the `rules_*` commands read and write rules:
//! the project's stored rule text (or this machine's, which Run DRC reads
//! while the project stores none), its board, and the one write path, an
//! undo step on the project's rule history.

use std::path::PathBuf;

use iced::Task;
use signex_drc::{DrcRules, WrittenRuleEdit};
use signex_drc_dsl::table::{self, RuleField};
use signex_types::pcb::PcbBoard;
use signex_types::project::DrcRuleSet;

use super::RulesText;
use crate::app::command::{CommandArgs, OperandValue};
use crate::app::document::{DocumentKey, UndoTarget};
use crate::app::document_ids::DocumentKind;
use crate::app::state::ProjectId;
use crate::app::state::rules_history::RuleState;
use crate::app::{Message, Signex, TabKind};

/// Why an undo, redo or a switch to the table waits: the text view holds
/// typed text not applied, which it would drop.
pub(crate) const TEXT_UNAPPLIED: &str =
    "the text view holds typed text that is not applied: Apply or Discard it first";

/// The severity entry edit that follows setting `field` of rule `rule` (a
/// name or `user::<name>`) to `value`, for the tab's cells and `rules_set`.
pub(crate) fn field_follow<'a>(
    rule: &'a str,
    field: RuleField,
    value: &'a str,
) -> Option<WrittenRuleEdit<'a>> {
    let name = rule_name(rule);
    match field {
        RuleField::Severity => Some(WrittenRuleEdit::SeveritySet(name)),
        RuleField::Name => Some(WrittenRuleEdit::Renamed {
            from: name,
            to: value.trim(),
        }),
        RuleField::Enabled => table::parse_bool(value.trim())
            .filter(|on| *on)
            .map(|_| WrittenRuleEdit::SwitchedOn(name)),
        _ => None,
    }
}

/// The name in a rule id: `user::<name>` or the name itself.
pub(crate) fn rule_name(rule: &str) -> &str {
    rule.strip_prefix(table::RULE_ID_PREFIX).unwrap_or(rule)
}

/// Where the rule text shown comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RulesOrigin {
    /// The project stores its rules (`rules.drc.dsl` in the `.snxprj`).
    Project,
    /// The project stores none: Run DRC reads this machine's profile, and
    /// the first edit stores the rules in the project.
    Machine,
}

impl RulesOrigin {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            RulesOrigin::Project => "project",
            RulesOrigin::Machine => "machine",
        }
    }
}

impl Signex {
    /// The DRC rule text of `id` as Run DRC reads it, and where it is from.
    pub(crate) fn rules_text_of(&self, id: ProjectId) -> Option<(String, RulesOrigin)> {
        let project = self.document_state.project_by_id(id)?;
        Some(match &project.data.rules {
            Some(rules) => (
                rules.drc.dsl.clone().unwrap_or_default(),
                RulesOrigin::Project,
            ),
            None => (
                self.ui_state
                    .machine_drc_rules
                    .dsl
                    .clone()
                    .unwrap_or_default(),
                RulesOrigin::Machine,
            ),
        })
    }

    /// The path of `id`'s board: its `pcb_file`, else an open board tab of
    /// the project.
    pub(crate) fn rules_board_path(&self, id: ProjectId) -> Option<PathBuf> {
        let docs = &self.document_state;
        let project = docs.project_by_id(id)?;
        let named = project
            .data
            .pcb_file
            .as_ref()
            .map(|file| project.dir().join(file));
        if let Some(path) = named.filter(|path| docs.pcb_board(path).is_some()) {
            return Some(path);
        }
        docs.tabs
            .iter()
            .find(|tab| matches!(tab.kind, TabKind::Pcb) && tab.project_id == Some(id))
            .map(|tab| tab.path.clone())
            .or_else(|| {
                project
                    .data
                    .pcb_file
                    .as_ref()
                    .map(|file| project.dir().join(file))
            })
    }

    /// `id`'s board when it is open (or parked with edits).
    pub(crate) fn rules_board(&self, id: ProjectId) -> Option<(PathBuf, &PcbBoard)> {
        let path = self.rules_board_path(id)?;
        let board = self.document_state.pcb_board(&path)?;
        Some((path, board))
    }

    /// The DRC rule layer `id`'s rules are read from: the project's, else
    /// this machine's. Its `severity` entries outrank a rule's own word.
    pub(crate) fn rules_drc_layer(&self, id: ProjectId) -> Option<&DrcRuleSet> {
        let project = self.document_state.project_by_id(id)?;
        let (profile, layer) = crate::app::handlers::pcb_drc::drc_layers(
            project.data.rules.as_ref(),
            &self.ui_state.machine_drc_rules,
        );
        layer.or(profile)
    }

    /// The rules Run DRC would check `id`'s board against, resolved
    /// against the board's nets, or why there are none.
    pub(crate) fn rules_resolved(&self, id: ProjectId) -> Result<DrcRules, String> {
        let (_, board) = self
            .rules_board(id)
            .ok_or("the project's board is not open")?;
        let project = self
            .document_state
            .project_by_id(id)
            .ok_or("the project is no longer open")?;
        let classes = &project.data.net_classes;
        let (profile, layer) = crate::app::handlers::pcb_drc::drc_layers(
            project.data.rules.as_ref(),
            &self.ui_state.machine_drc_rules,
        );
        let written = signex_drc_dsl::compile_layers(profile, layer);
        let rules = DrcRules::for_board(board, classes)
            .map_err(|e| format!("the board's clearance rules are not usable: {e}"))?;
        Ok(rules.with_user_rules(board, classes, written.rules))
    }

    /// Store `text` as `id`'s DRC rule text: one undo step on the project's
    /// rule history, the project dirty, nothing written to disk (Save does).
    /// The rule layer's severity entries follow `follows` in the same step.
    /// A project that stores no rules has them stored first, from this
    /// machine, in the same step. Unchanged rules record no step.
    pub(crate) fn commit_rules_text(
        &mut self,
        id: ProjectId,
        text: String,
        follows: &[WrittenRuleEdit<'_>],
    ) -> Result<(), String> {
        self.commit_drc_section(id, |drc| {
            drc.dsl = (!text.trim().is_empty()).then_some(text);
            for edit in follows {
                signex_drc::follow_written_rule_edit(drc, *edit);
            }
        })
    }

    /// `id`'s DRC rules as the first edit stores them: the project's, else
    /// this machine's (what Run DRC reads while the project stores none).
    pub(crate) fn rules_drc_of(&self, id: ProjectId) -> Option<DrcRuleSet> {
        let project = self.document_state.project_by_id(id)?;
        Some(match &project.data.rules {
            Some(rules) => rules.drc.clone(),
            None => self.ui_state.machine_drc_rules.clone(),
        })
    }

    /// Store `id`'s DRC rules as `edit` leaves them (rule text, severity
    /// entries, rule sets): one undo step on the project's rule history,
    /// the project dirty, nothing written to disk (Save does). A project
    /// that stores no rules has them stored first, from this machine, in
    /// the same step. Unchanged rules record no step.
    pub(crate) fn commit_drc_section(
        &mut self,
        id: ProjectId,
        edit: impl FnOnce(&mut DrcRuleSet),
    ) -> Result<(), String> {
        self.commit_fab_section(id, edit, None, None)
    }

    /// [`Self::commit_drc_section`] that also sets the project's router
    /// tool to `router` mm and its min slot to `min_slot` mm when each is
    /// `Some`, in the same undo step: a fab tier or profile preset (mockup
    /// row 179d). Unchanged rules, router and min slot record no step.
    pub(crate) fn commit_fab_section(
        &mut self,
        id: ProjectId,
        edit: impl FnOnce(&mut DrcRuleSet),
        router: Option<f64>,
        min_slot: Option<f64>,
    ) -> Result<(), String> {
        let project = self.rules_writable(id)?;
        let before = project.data.rules.clone();
        let router_before = project.data.router_tool_diameter();
        let router_after = router.or(router_before);
        let mut after = before
            .clone()
            .unwrap_or_else(|| self.seeded_project_rules());
        edit(&mut after.drc);
        if min_slot.is_some() {
            after.set_min_slot(min_slot);
        }
        if before.as_ref() == Some(&after) && router_after == router_before {
            return Ok(());
        }
        let state = RuleState {
            rules: Some(after),
            router_tool_diameter: router_after,
        };
        self.replace_rule_state_of(id, state, true);
        Ok(())
    }

    /// `id` when its stored rules can take a write, else why not. A caller
    /// with a second step to land asks first, so it can refuse with nothing
    /// applied.
    pub(crate) fn rules_writable(
        &self,
        id: ProjectId,
    ) -> Result<&crate::app::state::LoadedProject, String> {
        let project = self
            .document_state
            .project_by_id(id)
            .ok_or_else(|| "the project is no longer open".to_string())?;
        if project.data.has_unreadable_rules() {
            return Err(
                "this project holds a rule set from a newer Signex, which storing \
                        rules would replace"
                    .to_string(),
            );
        }
        Ok(project)
    }

    /// Whether `id`'s text view holds typed text not applied yet.
    pub(crate) fn rules_text_unapplied(&self, id: ProjectId) -> bool {
        self.document_state
            .project_tabs
            .get(&id)
            .and_then(|tab| tab.rules.text.as_ref())
            .is_some_and(|text| text.edited)
    }

    /// Ctrl+Z / Ctrl+Y in a Rules tab: step `id`'s rule history (shared
    /// with the ERC dialog). Refused while the text view holds typed text
    /// not applied, which the step would replace. Whether a step moved.
    pub(crate) fn step_rules_history(&mut self, id: ProjectId, undo: bool) -> bool {
        if self.rules_text_unapplied(id) {
            self.rules_tab_mut(id).refused = Some(TEXT_UNAPPLIED.to_string());
            return false;
        }
        if !self.step_rules_of(id, undo) {
            return false;
        }
        // A cell shows the stored rules again; the text view stays open on
        // their new text.
        let text = self.rules_text_of(id).map(|(text, _)| text);
        let tab = self.rules_tab_mut(id);
        tab.edit = None;
        tab.matrix_edit = None;
        tab.refused = None;
        if let (Some(view), Some(text)) = (tab.text.as_mut(), text) {
            *view = RulesText::new(&text);
        }
        true
    }

    /// Undo / Redo of `id`'s Rules tab, then ERC again when it has results
    /// the step may outdate (the step can be an ERC dialog edit).
    pub(crate) fn step_rules_tab(&mut self, id: ProjectId, undo: bool) -> Task<Message> {
        let moved = self.step_rules_history(id, undo);
        self.rerun_erc_after_rules_edit(moved)
    }

    /// Edit ▸ Undo / Redo in the main window: a Rules tab, or a Project tab
    /// whose next step is a rule step, steps the rule history as
    /// [`Self::step_rules_tab`]; any other tab as before.
    pub(crate) fn step_main_window_history(&mut self, undo: bool) -> Task<Message> {
        match self.undo_target(None) {
            UndoTarget::RulesTab(id) => return self.step_rules_tab(id, undo),
            UndoTarget::Document(DocumentKey::Project(id))
                if self.project_step_is_rules(id, undo) =>
            {
                self.close_text_edit(if undo {
                    "the document was undone"
                } else {
                    "the document was redone"
                });
                return self.step_rules_tab(id, undo);
            }
            _ => {}
        }
        if undo {
            self.handle_undo_requested();
        } else {
            self.handle_redo_requested();
        }
        Task::none()
    }

    /// The project a `rules_*` command names with `document`: the project
    /// itself, or any document of it.
    pub(crate) fn rules_project_named(&mut self, args: &CommandArgs) -> Result<ProjectId, String> {
        let Some(OperandValue::Document(target)) = args.operand("document") else {
            return Err("needs the operand `document`: the project or one of its documents".into());
        };
        self.reconcile_document_ids();
        let entry = self.resolve_document_target(target)?;
        let path = match entry.kind {
            DocumentKind::Project => entry.path.clone(),
            _ => entry.project.as_ref().map(|project| project.path.clone()),
        };
        let path = path.ok_or_else(|| format!("{} is not part of an open project", entry.name))?;
        self.document_state
            .projects
            .iter()
            .find(|project| project.path == path)
            .map(|project| project.id)
            .ok_or_else(|| format!("{} is not open", path.display()))
    }
}
