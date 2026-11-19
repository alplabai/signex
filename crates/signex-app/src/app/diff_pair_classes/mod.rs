//! Design ▸ Differential Pair Classes… (#1489): the dialog that edits the
//! project's differential pair classes (impedance, width and gap, pairing
//! by suffix). It works on a copy: Cancel and Esc drop it, OK writes it to
//! `ProjectData.schematic_design.diff_pair_classes` as one step of the
//! project's General history. Max skew and max uncoupled length are rules
//! of the Rules tab (High Speed), shown read-only here (SCH-06).
//! Design note: `docs/SCHEMATIC_DIRECTIVES.md`, "Differential pairs".

mod update;
pub(crate) use update::NAME_INPUT_ID;

#[cfg(test)]
mod tests;

use signex_types::diff_pair_class::DiffPairClass;

use crate::app::state::ProjectId;

impl From<DiffPairClassesMsg> for crate::app::Message {
    fn from(msg: DiffPairClassesMsg) -> Self {
        Self::DiffPairClasses(msg)
    }
}

/// A typed field of the selected class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PairClassField {
    Impedance,
    Tolerance,
    Width,
    Gap,
}

/// What happened in the Differential Pair Classes dialog, namespaced under
/// `Message::DiffPairClasses` and routed to
/// `dispatch_diff_pair_classes_message`.
#[derive(Debug, Clone, PartialEq)]
pub enum DiffPairClassesMsg {
    /// `open_diff_pair_classes` ran.
    Requested,
    ClassSelected(usize),
    /// A class row's menu opened (`Some`) or closed.
    ClassMenuToggled(Option<usize>),
    ClassRenameStarted(usize),
    ClassDuplicated(usize),
    ClassDeleted(usize),
    ClassAdded,
    /// The selected class's name as typed.
    NameEdited(String),
    /// A value of the selected class as typed.
    FieldEdited(PairClassField, String),
    PairBySuffixToggled(bool),
    /// "Edit in Rules ▸ High Speed": the Rules tab opens.
    RulesLinkClicked,
    CancelRequested,
    OkRequested,
}

/// One pair of the project, as the Members table lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairMemberRow {
    pub class: String,
    pub pair: String,
    pub positive: String,
    pub negative: String,
    /// The sheet file the directive is on.
    pub sheet: String,
}

/// The open dialog: a copy of the project's classes, edited until OK.
#[derive(Debug, Clone, PartialEq)]
pub struct DiffPairClassesState {
    pub project: ProjectId,
    pub project_name: String,
    pub classes: Vec<DiffPairClass>,
    pub selected: usize,
    /// The class row whose menu is open.
    pub menu: Option<usize>,
    /// The pairs of the project's open sheets, by class.
    pub members: Vec<PairMemberRow>,
    /// Text typed in a value field, kept while it is typed.
    pub drafts: Vec<(PairClassField, String)>,
    /// Why the last OK did not apply (a name used twice or left blank).
    pub problem: Option<PairClassProblem>,
}

/// Why OK was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairClassProblem {
    BlankName,
    DuplicateName,
}

/// A fresh class: pairs by suffix, nothing else set.
pub fn new_class(name: String) -> DiffPairClass {
    DiffPairClass::named(name)
}

/// The first free name `base`, `base_2`, `base_3`, … among `classes`.
pub fn free_name(classes: &[DiffPairClass], base: &str) -> String {
    let taken = |name: &str| classes.iter().any(|c| c.name == name);
    if !taken(base) {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base}_{n}"))
        .find(|name| !taken(name))
        .unwrap_or_else(|| base.to_string())
}

impl DiffPairClassesState {
    pub fn selected_class(&self) -> Option<&DiffPairClass> {
        self.classes.get(self.selected)
    }

    /// How many pairs class `index` has.
    pub fn pair_count(&self, index: usize) -> usize {
        self.classes.get(index).map_or(0, |class| {
            self.members
                .iter()
                .filter(|m| m.class == class.name)
                .count()
        })
    }

    /// Whether class `index` may be deleted: only while it has no pairs.
    pub fn can_delete(&self, index: usize) -> bool {
        index < self.classes.len() && self.pair_count(index) == 0
    }

    /// The draft of `field`, if one is typed.
    pub fn draft(&self, field: PairClassField) -> Option<&str> {
        self.drafts
            .iter()
            .find(|(f, _)| *f == field)
            .map(|(_, text)| text.as_str())
    }

    /// The problem OK would meet: a blank name or one used twice.
    pub fn problem(&self) -> Option<PairClassProblem> {
        if self.classes.iter().any(|c| c.name.trim().is_empty()) {
            return Some(PairClassProblem::BlankName);
        }
        let mut names: Vec<&str> = self.classes.iter().map(|c| c.name.trim()).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        (names.len() != before).then_some(PairClassProblem::DuplicateName)
    }

    /// The classes OK writes: names trimmed.
    pub fn applied(&self) -> Vec<DiffPairClass> {
        self.classes
            .iter()
            .cloned()
            .map(|mut c| {
                c.name = c.name.trim().to_string();
                c
            })
            .collect()
    }
}

/// A stored value of `class` for `field`.
pub fn field_value(class: &DiffPairClass, field: PairClassField) -> Option<f64> {
    match field {
        PairClassField::Impedance => class.impedance_ohm,
        PairClassField::Tolerance => class.impedance_tolerance_pct,
        PairClassField::Width => class.width_mm,
        PairClassField::Gap => class.gap_mm,
    }
}

/// Set `field` of `class`; `None` clears it.
pub fn set_field(class: &mut DiffPairClass, field: PairClassField, value: Option<f64>) {
    let slot = match field {
        PairClassField::Impedance => &mut class.impedance_ohm,
        PairClassField::Tolerance => &mut class.impedance_tolerance_pct,
        PairClassField::Width => &mut class.width_mm,
        PairClassField::Gap => &mut class.gap_mm,
    };
    *slot = value;
}
