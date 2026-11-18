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
mod matrix;
mod open;
mod presets;
mod search;
mod sets;
mod source;
mod update;
mod update_matrix;
mod view;

#[cfg(test)]
mod matrix_tests;
#[cfg(test)]
mod tests;

pub(crate) use analysis::RulesAnalysis;
pub(crate) use commands::{rules_add, rules_enable, rules_remove, rules_set};
pub(crate) use matrix::rules_matrix_set;
pub(crate) use presets::rules_preset_apply;
pub(crate) use sets::{rules_set_apply, rules_set_create, rules_set_edit};
pub(crate) use source::{RulesOrigin, TEXT_UNAPPLIED};

use iced::widget::text_editor;
use signex_drc_dsl::table::matrix::CellKey;
use signex_drc_dsl::table::presets::RowChange;
use signex_drc_dsl::table::{RuleField, RuleKind, RuleTable};
use signex_types::project::DrcRuleSet;

use crate::app::state::ProjectId;

/// A sub-tab: the rule kinds it lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RulesDomain {
    #[default]
    Clearance,
    Routing,
    /// Hole size, annular ring and hole spacing (#2435).
    Holes,
    /// Solder mask and paste.
    Mask,
    /// Silkscreen and courtyards.
    Assembly,
}

impl RulesDomain {
    pub const ALL: [RulesDomain; 5] = [
        RulesDomain::Clearance,
        RulesDomain::Routing,
        RulesDomain::Holes,
        RulesDomain::Mask,
        RulesDomain::Assembly,
    ];

    pub fn of(kind: RuleKind) -> Self {
        match kind {
            RuleKind::Clearance | RuleKind::Edge => RulesDomain::Clearance,
            RuleKind::Track | RuleKind::Via => RulesDomain::Routing,
            RuleKind::Hole => RulesDomain::Holes,
            RuleKind::Mask | RuleKind::Paste => RulesDomain::Mask,
            RuleKind::Silk | RuleKind::Courtyard => RulesDomain::Assembly,
        }
    }

    pub const fn kinds(self) -> &'static [RuleKind] {
        match self {
            RulesDomain::Clearance => &[RuleKind::Clearance, RuleKind::Edge],
            RulesDomain::Routing => &[RuleKind::Track, RuleKind::Via],
            RulesDomain::Holes => &[RuleKind::Hole],
            RulesDomain::Mask => &[RuleKind::Mask, RuleKind::Paste],
            RulesDomain::Assembly => &[RuleKind::Silk, RuleKind::Courtyard],
        }
    }

    /// Stable name in answers.
    pub const fn as_str(self) -> &'static str {
        match self {
            RulesDomain::Clearance => "clearance",
            RulesDomain::Routing => "routing",
            RulesDomain::Holes => "holes",
            RulesDomain::Mask => "mask",
            RulesDomain::Assembly => "assembly",
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

/// How the Clearance sub-tab shows its rules (#2398).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClearanceView {
    #[default]
    List,
    /// The class-to-class matrix.
    Matrix,
}

/// A panel the tab shows beside its rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RulesPanel {
    /// Named rule sets.
    Sets,
    /// Generic tiers and rule profiles, applied after a diff.
    Presets,
}

/// A matrix cell being typed in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatrixEdit {
    pub key: CellKey,
    pub text: String,
}

/// A field of the rule-set form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetFormField {
    Name,
    Values,
    Severity,
    Scope,
}

/// What is typed in the rule-set form.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SetForm {
    pub name: String,
    /// `min_clearance=0.8mm, min_width=0.3mm`.
    pub values: String,
    pub severity: String,
    /// The scope Apply writes the set's rules for.
    pub scope: String,
}

/// A preset picked: what applying it changes, and the DRC rules it leaves,
/// from the rules it was computed against.
#[derive(Debug, Clone, PartialEq)]
pub struct PresetPreview {
    /// `tier:standard` or `profile:<name>`.
    pub preset: String,
    /// The tier or profile name, as shown.
    pub label: String,
    pub changes: Vec<RowChange>,
    /// Severity, rule-set and other changes, one line each.
    pub other: Vec<String>,
    /// Whose limits the preset copies and from where (#2311): a tier's
    /// own, or a fab profile's; `None` for a profile that names none.
    pub fab: Option<signex_types::project::FabProfileInfo>,
    /// The project's DRC rules the preview was computed from.
    pub base: DrcRuleSet,
    /// The DRC rules after Apply.
    pub result: DrcRuleSet,
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
    /// What the compiler says about the text as typed, read again in
    /// `update` after each edit, not in `view`.
    pub checked: RuleTable,
}

impl RulesText {
    pub fn new(text: &str) -> Self {
        RulesText {
            content: text_editor::Content::with_text(text),
            edited: false,
            checked: signex_drc_dsl::table::table(text),
        }
    }

    /// Perform `action`; an edit marks the text typed in and checks it
    /// again.
    pub fn perform(&mut self, action: text_editor::Action) {
        let edits = action.is_edit();
        self.content.perform(action);
        if edits {
            self.edited = true;
            self.checked = signex_drc_dsl::table::table(&self.content.text());
        }
    }
}

impl Clone for RulesText {
    fn clone(&self) -> Self {
        RulesText {
            content: text_editor::Content::with_text(&self.content.text()),
            edited: self.edited,
            checked: self.checked.clone(),
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
    pub clearance_view: ClearanceView,
    pub panel: Option<RulesPanel>,
    pub matrix_edit: Option<MatrixEdit>,
    /// The class pair whose object sub-grid is shown.
    pub matrix_open: Option<(String, String)>,
    pub set_form: SetForm,
    pub preview: Option<PresetPreview>,
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
    /// The text view's typed text was dropped: it shows the stored text.
    TextDiscarded {
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
    /// The Clearance sub-tab switched between its list and the matrix.
    ClearanceViewPicked {
        project_id: ProjectId,
        view: ClearanceView,
    },
    /// A matrix cell was clicked: type in it and show its object sub-grid.
    MatrixCellOpened {
        project_id: ProjectId,
        key: CellKey,
    },
    MatrixCellEdited {
        project_id: ProjectId,
        text: String,
    },
    MatrixCellCommitted {
        project_id: ProjectId,
    },
    MatrixCellCancelled {
        project_id: ProjectId,
    },
    /// Clear a cell back to what its two classes ask without it.
    MatrixCellCleared {
        project_id: ProjectId,
        key: CellKey,
    },
    /// The object sub-grid was closed.
    MatrixCollapsed {
        project_id: ProjectId,
    },
    /// A side panel was opened, or closed with `None`.
    PanelPicked {
        project_id: ProjectId,
        panel: Option<RulesPanel>,
    },
    SetFormEdited {
        project_id: ProjectId,
        field: SetFormField,
        text: String,
    },
    /// Create the set the form names, with its limits.
    SetCreated {
        project_id: ProjectId,
    },
    /// Give set `set` the form's limits and severity.
    SetRedefined {
        project_id: ProjectId,
        set: String,
    },
    /// Apply set `set` to the form's scope.
    SetApplied {
        project_id: ProjectId,
        set: String,
    },
    /// Read the rule profile library, for its profiles as presets.
    PresetsListRequested {
        project_id: ProjectId,
    },
    /// A preset was picked: show what applying it changes.
    PresetPicked {
        project_id: ProjectId,
        preset: String,
    },
    /// Apply the previewed preset as one undo step.
    PresetApplied {
        project_id: ProjectId,
    },
    PresetDismissed {
        project_id: ProjectId,
    },
}
