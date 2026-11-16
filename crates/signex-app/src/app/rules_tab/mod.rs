//! The Rules document tab (#2398): the project's written DRC rules as a
//! table, one row per rule of the rule text, and the same rules as text.
//! The text stays the format: editing a cell rewrites only that rule's
//! text (`signex_drc_dsl::table`), as one undo step on the project's rule
//! history. Resolution, flags and the per-rule results come from
//! `signex-drc`; this module only shows them. Behaviour:
//! `docs/DRC_RULES.md`, "Rules manager".

mod analysis;
mod commands;
mod json;
mod open;
mod source;
mod update;
mod view;

#[cfg(test)]
mod tests;

pub(crate) use analysis::RulesAnalysis;
pub(crate) use commands::{rules_add, rules_enable, rules_remove, rules_set};
pub(crate) use source::RulesOrigin;

use iced::widget::text_editor;
use signex_drc_dsl::table::{RuleField, RuleKind};

use crate::app::state::ProjectId;

/// A sub-tab: the rule kinds it lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RulesDomain {
    #[default]
    Clearance,
    Routing,
}

impl RulesDomain {
    pub const ALL: [RulesDomain; 2] = [RulesDomain::Clearance, RulesDomain::Routing];

    pub fn of(kind: RuleKind) -> Self {
        match kind {
            RuleKind::Clearance => RulesDomain::Clearance,
            RuleKind::Track | RuleKind::Via => RulesDomain::Routing,
        }
    }

    pub const fn kinds(self) -> &'static [RuleKind] {
        match self {
            RulesDomain::Clearance => &[RuleKind::Clearance],
            RulesDomain::Routing => &[RuleKind::Track, RuleKind::Via],
        }
    }

    /// Stable name in answers.
    pub const fn as_str(self) -> &'static str {
        match self {
            RulesDomain::Clearance => "clearance",
            RulesDomain::Routing => "routing",
        }
    }
}

/// Which rows the table shows besides the search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RulesFilter {
    #[default]
    All,
    /// Rules with a finding in the last run.
    Violations,
    /// Rules with a flag (shadowed, conflict, lowers a class, no nets) or
    /// a compile problem.
    Flagged,
    /// Rules switched off.
    Disabled,
}

impl RulesFilter {
    pub const ALL: [RulesFilter; 4] = [
        RulesFilter::All,
        RulesFilter::Violations,
        RulesFilter::Flagged,
        RulesFilter::Disabled,
    ];
}

/// A cell being typed in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellEdit {
    pub rule: String,
    pub field: RuleField,
    pub text: String,
}

/// The text view's editor. Cloning copies the text (the cursor starts
/// over); the tab state is cloned only with the project tab state.
pub struct RulesText {
    pub content: text_editor::Content,
    /// Typed in since it was opened or applied.
    pub edited: bool,
}

impl RulesText {
    pub fn new(text: &str) -> Self {
        RulesText {
            content: text_editor::Content::with_text(text),
            edited: false,
        }
    }
}

impl Clone for RulesText {
    fn clone(&self) -> Self {
        RulesText {
            content: text_editor::Content::with_text(&self.content.text()),
            edited: self.edited,
        }
    }
}

impl std::fmt::Debug for RulesText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RulesText")
            .field("text", &self.content.text())
            .field("edited", &self.edited)
            .finish()
    }
}

/// Per-project Rules tab state, kept with the project's tab state for as
/// long as the project is loaded.
#[derive(Debug, Clone, Default)]
pub struct RulesTabState {
    pub domain: RulesDomain,
    pub filter: RulesFilter,
    pub search: String,
    /// The rule whose detail is shown, by name.
    pub selected: Option<String>,
    pub edit: Option<CellEdit>,
    /// Why the last edit was refused, until the next edit.
    pub refused: Option<String>,
    /// Some while the text view is shown.
    pub text: Option<RulesText>,
    /// Flags and scope counts, refreshed in `update` when the rules, the
    /// board or the net classes change.
    pub analysis: Option<RulesAnalysis>,
}

/// Messages of a Rules tab, under `ProjectMsg::Rules`. Each names its
/// project, so an edit never lands on a project that replaced it.
#[derive(Debug, Clone)]
pub enum RulesTabMsg {
    /// Design ▸ Rules, the palette or a button: open or focus the tab of
    /// `project`, or of the active document's project.
    OpenRequested {
        project: Option<ProjectId>,
    },
    DomainPicked {
        project_id: ProjectId,
        domain: RulesDomain,
    },
    FilterPicked {
        project_id: ProjectId,
        filter: RulesFilter,
    },
    SearchEdited {
        project_id: ProjectId,
        text: String,
    },
    RowSelected {
        project_id: ProjectId,
        rule: String,
    },
    CellOpened {
        project_id: ProjectId,
        rule: String,
        field: RuleField,
    },
    CellEdited {
        project_id: ProjectId,
        text: String,
    },
    CellCommitted {
        project_id: ProjectId,
    },
    CellCancelled {
        project_id: ProjectId,
    },
    EnabledToggled {
        project_id: ProjectId,
        rule: String,
        on: bool,
    },
    RuleAdded {
        project_id: ProjectId,
        kind: RuleKind,
    },
    RuleRemoved {
        project_id: ProjectId,
        rule: String,
    },
    TextViewToggled {
        project_id: ProjectId,
        on: bool,
    },
    TextActed {
        project_id: ProjectId,
        action: text_editor::Action,
    },
    TextApplied {
        project_id: ProjectId,
    },
    /// Show the rule's objects on the board: open its tab, select them.
    ScopeShown {
        project_id: ProjectId,
        rule: String,
    },
    /// Run DRC on the project's board, from the tab.
    DrcRequested {
        project_id: ProjectId,
    },
}
