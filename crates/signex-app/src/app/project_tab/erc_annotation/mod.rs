//! ERC and annotation section of the Project tab (#1692): where the project's
//! ERC rules come from (stored in the project, this machine's Preferences, or
//! an unreadable newer set), how many rules are set to each severity, "Store
//! ERC rules in this project", and the project's Annotate undo choice. The
//! look (Graphite mockup m34) is in `view`.
//!
//! Migration: Preferences values seed a new project and are the default of a
//! project that stores none; they never overwrite a project's own values.
//! Storing the rules is a step of the project's rule history (shared with the
//! ERC dialog and the Rules tab); the annotate choice is a General step.

mod view;

#[cfg(test)]
mod tests;

use signex_erc::Severity;
use signex_types::project::AnnotateUndo;

use crate::app::Signex;
use crate::app::state::{LoadedProject, ProjectId};

pub(super) use view::view;

/// Where a project's ERC rules come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ErcSource {
    /// The project stores its own rule set.
    Project,
    /// The project stores none: this machine's Preferences apply.
    Machine,
    /// The project holds a rule set this build cannot read.
    Unreadable,
}

impl ErcSource {
    pub(crate) fn of(project: &LoadedProject) -> Self {
        if project.data.rules.is_some() {
            Self::Project
        } else if project.data.has_unreadable_rules() {
            Self::Unreadable
        } else {
            Self::Machine
        }
    }

    /// The token of the `project_settings` answer.
    pub(crate) const fn token(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Machine => "preferences",
            Self::Unreadable => "unreadable",
        }
    }
}

/// What the section shows, read from the Model by [`Signex::erc_section_facts`].
pub(crate) struct ErcFacts {
    pub source: ErcSource,
    /// Rules per severity, Error, Warning, Info, Off.
    pub counts: [(Severity, usize); 4],
    /// This machine's Annotate undo preference.
    pub machine_annotate_undo: AnnotateUndo,
}

impl ErcFacts {
    /// How many rules there are, whatever their severity.
    pub(crate) fn total(&self) -> usize {
        self.counts.iter().map(|(_, count)| count).sum()
    }
}

impl Signex {
    /// The section's facts: where the rules come from, the severities they
    /// give, and the Preferences default.
    pub(crate) fn erc_section_facts(&self, project: &LoadedProject) -> ErcFacts {
        let source = ErcSource::of(project);
        let overrides = match project.data.rules.as_ref() {
            Some(rules) => signex_erc::project_rules::resolve(&rules.erc).severity,
            None => self.ui_state.erc_severity_override.clone(),
        };
        ErcFacts {
            source,
            counts: signex_erc::project_rules::severity_counts(&overrides),
            machine_annotate_undo: self.ui_state.annotate_undo,
        }
    }

    /// The Annotate undo of the project rooted at `root`: its own choice, else
    /// this machine's Preferences value (#1692). A loose sheet follows
    /// Preferences.
    pub(crate) fn annotate_undo_of(&self, root: Option<&std::path::Path>) -> AnnotateUndo {
        root.and_then(|root| self.document_state.document_project(root))
            .and_then(|project| project.data.annotate_undo())
            .unwrap_or(self.ui_state.annotate_undo)
    }

    /// The Annotate undo radio: one General step; `None` follows Preferences.
    pub(super) fn pick_project_annotate_undo(
        &mut self,
        project_id: ProjectId,
        choice: Option<AnnotateUndo>,
    ) {
        let command = signex_engine::ProjectSectionCommand::SetAnnotateUndo { choice };
        if let Err(problem) = self.apply_project_section(project_id, &command) {
            crate::diagnostics::log_warning(format!("Project tab: {problem}"));
        }
    }

    /// "Store ERC rules in this project" for `project_id`: a copy of this
    /// machine's rules, one step of the project's rule history. Refused when
    /// the project already stores rules or holds an unreadable newer set,
    /// which storing would replace. `Ok(false)` when it already stores some.
    pub(crate) fn store_erc_rules_of(&mut self, project_id: ProjectId) -> Result<bool, String> {
        let project = self
            .document_state
            .project_by_id(project_id)
            .ok_or_else(|| "the project is no longer open".to_string())?;
        match ErcSource::of(project) {
            ErcSource::Project => Ok(false),
            ErcSource::Unreadable => Err(
                "this project holds a rule set from a newer Signex, which storing rules would \
                 replace"
                    .to_string(),
            ),
            ErcSource::Machine => {
                let rules = self.seeded_project_rules();
                self.replace_rules_of(project_id, Some(rules), true);
                Ok(true)
            }
        }
    }

    pub(super) fn store_erc_rules_in(&mut self, project_id: ProjectId) {
        if let Err(problem) = self.store_erc_rules_of(project_id) {
            crate::diagnostics::log_warning(format!("Project tab: {problem}"));
        }
    }
}
