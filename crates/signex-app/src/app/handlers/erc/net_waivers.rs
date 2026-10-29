//! Waive / restore a net-level ERC finding (#1581).
//!
//! Waivers live in the project's rule set (`ProjectRules::erc_net_waivers`),
//! keyed by rule id + net name. A toggle is one undoable rule-set edit (the
//! same history as the severity edits) that flips the project dirty; the
//! explicit save writes it through the project writer.

use iced::Task;
use signex_types::project::ProjectRules;

use super::super::super::*;

impl Signex {
    /// The panel rows for the active project's waivers: every stored
    /// (rule id, net) with how many findings the newest run hid for it.
    pub(crate) fn erc_waiver_entries(&self) -> Vec<crate::panels::ErcWaiverEntry> {
        let Some(rules) = self
            .document_state
            .active_document_project()
            .and_then(|p| p.data.rules.as_ref())
        else {
            return Vec::new();
        };
        rules
            .erc_net_waivers
            .iter()
            .flat_map(|(rule_id, nets)| {
                nets.iter().map(move |net| crate::panels::ErcWaiverEntry {
                    rule_id: rule_id.clone(),
                    net: net.clone(),
                    hidden: self
                        .ui_state
                        .erc_waived_hidden
                        .get(&(rule_id.clone(), net.clone()))
                        .copied()
                        .unwrap_or(0),
                })
            })
            .collect()
    }

    /// Waive (`waive`) or restore one (rule id, net) pair, then re-run ERC so
    /// the list matches. A project that stores no rules first snapshots this
    /// machine's preferences into the project, as "Store ERC rules" does, so
    /// the waiver has a place to live.
    pub(crate) fn handle_erc_net_waiver_set(
        &mut self,
        rule_id: &str,
        net: &str,
        waive: bool,
    ) -> Task<Message> {
        let Some(mut rules) = self.rules_for_waiver_edit() else {
            return Task::none();
        };
        if !signex_erc::waivers::set_waived(&mut rules.erc_net_waivers, rule_id, net, waive) {
            return Task::none();
        }
        self.replace_project_rules(Some(rules), true);
        self.handle_run_erc()
    }

    /// The active project's rule set to add a waiver to: the stored one, or a
    /// snapshot of this machine's preferences when it stores none (said in
    /// the Messages panel, since the project's rules now travel with it). `None`
    /// (logged) without a project, or when the project holds a rule set from
    /// a newer build that storing would replace.
    pub(super) fn rules_for_waiver_edit(&self) -> Option<ProjectRules> {
        let project = self.document_state.active_document_project()?;
        match project.data.rules.clone() {
            Some(rules) => Some(rules),
            None if self.active_project_has_unreadable_rules() => {
                crate::diagnostics::log_warning(
                    "Not waiving: this project holds a rule set from a newer Signex that \
                     storing would replace.",
                );
                None
            }
            None => {
                crate::diagnostics::log_info(format!(
                    "ERC: project {} now stores its own ERC rules, copied from this \
                     machine's preferences, so the waiver has a place to live.",
                    project.data.name
                ));
                Some(self.seeded_project_rules())
            }
        }
    }

    /// The rule set "Store ERC rules in this project" would write: this
    /// machine's preferences, with the shipped profile named as provenance
    /// only when the snapshot is exactly that profile.
    pub(super) fn seeded_project_rules(&self) -> ProjectRules {
        let erc = signex_erc::project_rules::seed_from_overrides(
            &self.ui_state.erc_severity_override,
            &self.ui_state.pin_matrix_overrides,
        );
        let shipped = signex_erc::project_rules::profile::builtin_profile();
        let profile = (erc.sha256_hex() == shipped.content_sha256()).then(|| {
            signex_erc::project_rules::profile::profile_ref(
                &shipped,
                signex_erc::project_rules::profile::SOURCE_BUILTIN,
            )
        });
        ProjectRules {
            erc,
            profile,
            ..ProjectRules::default()
        }
    }
}
