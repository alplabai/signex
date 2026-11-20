//! The Rules document tab (#2398, #2530): the project's written DRC rules
//! as a constraint grid (domain tabs, the object hierarchy as rows, one
//! column per limit) and as text. The text stays the format: editing a
//! cell rewrites or adds one rule (`signex_drc_dsl::table::grid`), as one
//! undo step on the project's rule history. Resolution, flags and the
//! per-rule results come from `signex-drc`; this module only shows them.
//! Behaviour: `docs/DRC_RULES.md`, "Rules manager".

mod analysis;
mod commands;
mod drawing;
mod grid;
mod json;
mod matrix;
mod open;
mod presets;
mod search;
mod sets;
mod source;
mod update;
mod update_grid;
mod update_matrix;
mod view;

#[cfg(test)]
mod matrix_preview_tests;
#[cfg(test)]
mod matrix_tests;
#[cfg(test)]
mod tests;

pub(crate) use analysis::RulesAnalysis;
pub(crate) use commands::{rules_add, rules_enable, rules_remove, rules_set};
pub(crate) use grid::RulesGrid;
pub(crate) use matrix::rules_matrix_set;
pub(crate) use presets::rules_preset_apply;
pub(crate) use sets::{rules_set_apply, rules_set_create, rules_set_edit};
pub(crate) use source::{RulesOrigin, TEXT_UNAPPLIED};

use iced::widget::text_editor;
use signex_drc_dsl::table::catalog::Domain;
use signex_drc_dsl::table::matrix::CellKey;
use signex_drc_dsl::table::presets::RowChange;
use signex_drc_dsl::table::{RuleField, RuleKind, RuleTable};
use signex_types::project::DrcRuleSet;
use signex_widgets::constraint_grid::GridMessage;

use crate::app::state::ProjectId;
use drawing::{PreviewMsg, PreviewTarget, RulePreview};

/// How the Clearance domain shows its rules (#2398): the grid, or the
/// class-to-class matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClearanceView {
    /// The constraint grid.
    #[default]
    List,
    /// The class-to-class matrix.
    Matrix,
}

/// A panel the tab shows with its rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RulesPanel {
    /// Constraint sets: cards under the toolbar, with the set form.
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

/// A field of the constraint-set form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetFormField {
    Name,
    Values,
    Severity,
    Scope,
}

/// What is typed in the constraint-set form.
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
    /// The domain tab shown and its grid: rows, columns and the grid's
    /// view state (collapsed, selected, typed in). Rebuilt in `update` when
    /// its rows change.
    pub grid: RulesGrid,
    /// Inherited values are shown muted; off, only values set on a row or
    /// by a constraint set show.
    pub hide_inherited: bool,
    /// Only rows with a value of their own in the domain are listed.
    pub overridden_only: bool,
    /// The find-a-constraint box.
    pub find: grid::FindState,
    /// The drawing of the selected cell: its label being typed, a drag.
    pub(crate) drawing: RulePreview,
    /// What the drawing shows: the selected cell's rule kind with its row's
    /// values, or the hovered or pinned matrix pair. Refreshed in `update`.
    pub(crate) drawing_target: Option<PreviewTarget>,
    /// The rule whose detail is shown, by name: the rule that governs the
    /// selected cell, or a scoped rule's row.
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
    /// The class matrix widget's view state: the classes in order, the
    /// pinned cell and the cell being typed in.
    pub matrix: signex_widgets::constraint_grid::ClassMatrix,
    /// The matrix cell or object pair under the pointer, for the preview.
    pub matrix_hover: Option<CellKey>,
    /// The matrix cell or object pair the preview stays on; a value typed
    /// on the drawing writes it.
    pub matrix_pin: Option<CellKey>,
}

impl RulesTabState {
    /// The Clearance domain shows its class matrix, not the grid.
    pub fn is_matrix_shown(&self) -> bool {
        self.grid.domain() == Domain::Clearance && self.clearance_view == ClearanceView::Matrix
    }
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
        domain: Domain,
    },
    /// Something happened in the constraint grid: a row or group toggled,
    /// a cell pressed, typed in, submitted or reset, the find box typed in.
    GridActed {
        project_id: ProjectId,
        msg: GridMessage,
    },
    /// Something happened on the selected cell's drawing: a label typed
    /// in or committed, a handle dragged.
    PreviewActed {
        project_id: ProjectId,
        msg: PreviewMsg,
    },
    /// The Constraint sets button: the set cards shown or hidden.
    SetsToggled {
        project_id: ProjectId,
    },
    /// The Show inherited values chip.
    InheritedToggled {
        project_id: ProjectId,
    },
    /// The Overridden rows only chip.
    OverriddenOnlyToggled {
        project_id: ProjectId,
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
    /// The Clearance domain switched between its grid and the matrix.
    ClearanceViewPicked {
        project_id: ProjectId,
        view: ClearanceView,
    },
    /// Enter in the object-pair row being typed in.
    MatrixCellCommitted {
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
    /// The class matrix widget spoke: a cell pressed, typed in, submitted,
    /// cancelled or cleared.
    MatrixActed {
        project_id: ProjectId,
        msg: signex_widgets::constraint_grid::MatrixMessage,
    },
    /// The pointer moved onto a matrix cell or an object-pair row; `None`
    /// once it left them.
    MatrixHovered {
        project_id: ProjectId,
        key: Option<CellKey>,
    },
    /// An object-pair row was clicked: the preview stays on it.
    MatrixPairPinned {
        project_id: ProjectId,
        key: CellKey,
    },
    /// Text was typed in an object-pair row of the open cell.
    MatrixPairTyped {
        project_id: ProjectId,
        key: CellKey,
        text: String,
    },
}
