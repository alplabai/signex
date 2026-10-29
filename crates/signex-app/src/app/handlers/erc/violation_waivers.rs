//! Waive / restore one ERC finding with a reason (#1285).
//!
//! Waivers live in the project's rule set (`ProjectRules::erc_waivers`). The
//! reason and author are typed into an inline editor in the ERC panel, held
//! as `ui_state.erc_waive_draft`; confirming is one undoable rule-set edit,
//! the same path as net waivers, then ERC re-runs.

use iced::Task;

use super::super::super::*;
use crate::panels::{ErcViolationWaiverEntry, ErcWaiveDraft, ErcWaiverMsg, ErcWaiverStatus};

impl Signex {
    /// The panel rows for the active project's per-finding waivers, with
    /// what the newest run made of each.
    pub(crate) fn erc_violation_waiver_entries(&self) -> Vec<ErcViolationWaiverEntry> {
        let Some(rules) = self
            .document_state
            .active_document_project()
            .and_then(|p| p.data.rules.as_ref())
        else {
            return Vec::new();
        };
        rules
            .erc_waivers
            .iter()
            .map(|w| {
                let waived = self
                    .ui_state
                    .erc_waived_findings
                    .iter()
                    .filter(|f| f.waiver.same_identity(&w.rule_id, &w.objects))
                    .count();
                let identity = (w.rule_id.clone(), w.objects.clone());
                let status = if !w.is_complete() {
                    ErcWaiverStatus::Incomplete
                } else if waived > 0 {
                    ErcWaiverStatus::Waiving(waived)
                } else if self.ui_state.erc_lapsed_waivers.contains(&identity) {
                    ErcWaiverStatus::Lapsed
                } else {
                    ErcWaiverStatus::Unmatched
                };
                ErcViolationWaiverEntry {
                    rule_id: w.rule_id.clone(),
                    objects: w.objects.clone(),
                    message: w.message.clone(),
                    reason: w.reason.clone(),
                    author: w.author.clone(),
                    date: w.date.clone(),
                    status,
                }
            })
            .collect()
    }

    pub(crate) fn handle_erc_waiver(&mut self, msg: &ErcWaiverMsg) -> Task<Message> {
        let follow = match msg {
            ErcWaiverMsg::Started(index) => {
                self.start_erc_waiver(*index);
                Task::none()
            }
            ErcWaiverMsg::ReasonChanged(reason) => {
                if let Some(draft) = self.ui_state.erc_waive_draft.as_mut() {
                    draft.reason = reason.clone();
                    draft.error = None;
                }
                Task::none()
            }
            ErcWaiverMsg::AuthorChanged(author) => {
                if let Some(draft) = self.ui_state.erc_waive_draft.as_mut() {
                    draft.author = author.clone();
                    draft.error = None;
                }
                Task::none()
            }
            ErcWaiverMsg::Cancelled => {
                self.ui_state.erc_waive_draft = None;
                Task::none()
            }
            ErcWaiverMsg::Confirmed => self.confirm_erc_waiver(),
            ErcWaiverMsg::Removed { rule_id, objects } => self.remove_erc_waiver(rule_id, objects),
        };
        self.refresh_panel_ctx();
        follow
    }

    /// Open the editor for row `index`. The author defaults to the last one
    /// used, then to the OS login name.
    fn start_erc_waiver(&mut self, index: usize) {
        let entries = self.build_erc_diagnostic_entries();
        let Some((entry, key)) = entries
            .get(index)
            .and_then(|e| e.waiver_key.clone().map(|k| (e, k)))
        else {
            return;
        };
        let author = self
            .ui_state
            .erc_waive_draft
            .as_ref()
            .map(|d| d.author.clone())
            .filter(|a| !a.trim().is_empty())
            .unwrap_or_else(default_author);
        self.ui_state.erc_waive_draft = Some(ErcWaiveDraft {
            key,
            message: entry.message.clone(),
            reason: String::new(),
            author,
            error: None,
        });
    }

    fn confirm_erc_waiver(&mut self) -> Task<Message> {
        let Some(draft) = self.ui_state.erc_waive_draft.clone() else {
            return Task::none();
        };
        let date = chrono::Local::now().format("%Y-%m-%d").to_string();
        let waiver = match signex_erc::waivers::violation::new_waiver(
            &draft.key,
            &draft.message,
            &draft.reason,
            &draft.author,
            &date,
        ) {
            Ok(waiver) => waiver,
            Err(why) => {
                if let Some(d) = self.ui_state.erc_waive_draft.as_mut() {
                    d.error = Some(why.to_string());
                }
                return Task::none();
            }
        };
        let Some(mut rules) = self.rules_for_waiver_edit() else {
            return Task::none();
        };
        rules.erc_waivers = signex_erc::waivers::violation::with_waiver(&rules.erc_waivers, waiver);
        self.ui_state.erc_waive_draft = None;
        crate::diagnostics::log_info(format!(
            "ERC: waived {} ({}): {}",
            draft.key.rule_id,
            draft.message,
            draft.reason.trim()
        ));
        self.replace_project_rules(Some(rules), true);
        self.handle_run_erc()
    }

    fn remove_erc_waiver(&mut self, rule_id: &str, objects: &[String]) -> Task<Message> {
        let Some(mut rules) = self
            .document_state
            .active_document_project()
            .and_then(|p| p.data.rules.clone())
        else {
            return Task::none();
        };
        let kept =
            signex_erc::waivers::violation::without_waiver(&rules.erc_waivers, rule_id, objects);
        if kept.len() == rules.erc_waivers.len() {
            return Task::none();
        }
        rules.erc_waivers = kept;
        self.replace_project_rules(Some(rules), true);
        self.handle_run_erc()
    }
}

/// The OS login name, or empty when the environment names none.
fn default_author() -> String {
    ["USER", "USERNAME", "LOGNAME"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.trim().is_empty()))
        .unwrap_or_default()
}
