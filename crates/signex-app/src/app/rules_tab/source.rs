//! Where the Rules tab and the `rules_*` commands read and write rules:
//! the project's stored rule text (or this machine's, which Run DRC reads
//! while the project stores none), its board, and the one write path, an
//! undo step on the project's rule history.

use std::path::PathBuf;

use signex_drc::DrcRules;
use signex_types::pcb::PcbBoard;

use crate::app::command::{CommandArgs, OperandValue};
use crate::app::document_ids::DocumentKind;
use crate::app::state::ProjectId;
use crate::app::{Signex, TabKind};

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

    /// The rules Run DRC would check `id`'s board against, resolved
    /// against the board's nets; `None` with no open board.
    pub(crate) fn rules_resolved(&self, id: ProjectId) -> Option<DrcRules> {
        let (_, board) = self.rules_board(id)?;
        let project = self.document_state.project_by_id(id)?;
        let classes = &project.data.net_classes;
        let (profile, layer) = crate::app::handlers::pcb_drc::drc_layers(
            project.data.rules.as_ref(),
            &self.ui_state.machine_drc_rules,
        );
        let written = signex_drc_dsl::compile_layers(profile, layer);
        let rules = DrcRules::for_board(board, classes).ok()?.with_user_rules(
            board,
            classes,
            written.rules,
        );
        Some(rules)
    }

    /// Store `text` as `id`'s DRC rule text: one undo step on the project's
    /// rule history, the project dirty, nothing written to disk (Save does).
    /// A project that stores no rules has them stored first, from this
    /// machine, in the same step. Unchanged text records no step.
    pub(crate) fn commit_rules_text(&mut self, id: ProjectId, text: String) -> Result<(), String> {
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
        let before = project.data.rules.clone();
        let mut after = before
            .clone()
            .unwrap_or_else(|| self.seeded_project_rules());
        after.drc.dsl = (!text.trim().is_empty()).then_some(text);
        if before.as_ref() == Some(&after) {
            return Ok(());
        }
        self.invalidate_pending_profile_update();
        let docs = &mut self.document_state;
        docs.note_project_clean_state(id);
        docs.rules_history.entry(id).or_default().record(before);
        if let Some(project) = docs.project_by_id_mut(id) {
            project.data.rules = Some(after);
        }
        docs.sync_project_history_dirty(id, true);
        self.refresh_panel_ctx();
        Ok(())
    }

    /// Ctrl+Z / Ctrl+Y in a Rules tab: step `id`'s rule history. Whether a
    /// step moved.
    pub(crate) fn step_rules_history(&mut self, id: ProjectId, undo: bool) -> bool {
        let Some(current) = self
            .document_state
            .project_by_id(id)
            .map(|p| p.data.rules.clone())
        else {
            return false;
        };
        let restored = self
            .document_state
            .rules_history
            .get_mut(&id)
            .and_then(|history| {
                if undo {
                    history.undo(current)
                } else {
                    history.redo(current)
                }
            });
        let Some(restored) = restored else {
            return false;
        };
        self.invalidate_pending_profile_update();
        let docs = &mut self.document_state;
        if let Some(project) = docs.project_by_id_mut(id) {
            project.data.rules = restored;
        }
        docs.sync_project_history_dirty(id, true);
        // A cell or the text view shows the stored rules again.
        if let Some(tab) = docs.project_tabs.get_mut(&id) {
            tab.rules.edit = None;
            tab.rules.text = None;
        }
        self.refresh_panel_ctx();
        true
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
