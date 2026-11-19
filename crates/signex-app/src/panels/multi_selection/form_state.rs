//! View state of the selection Properties forms (#1699): the multi-selection
//! kind chips, the ERC rule filter of a directive, the No-ERC rule list being
//! picked and the hover link between a property illustration and its fields.
//!
//! None of it is document data. The chips only choose which sections the
//! panel shows; they never change the canvas selection.

use signex_engine::SelectionGroups;
use signex_types::schematic::{SelectedItem, SelectedKind};
use signex_widgets::property_illustration::{IllustrationHover, IllustrationMsg};

/// A kind chip of the multi-selection form, in chip order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MultiKind {
    Parts,
    Labels,
    Wires,
}

impl MultiKind {
    pub const ALL: [Self; 3] = [Self::Parts, Self::Labels, Self::Wires];

    /// How many objects of this kind the form edits; `None` when the
    /// selection holds none, so the chip and the section are left out.
    pub fn count_in(self, groups: &SelectionGroups) -> Option<usize> {
        match self {
            Self::Parts => groups.symbols.as_ref().map(|g| g.count),
            Self::Labels => groups.labels.as_ref().map(|g| g.count),
            Self::Wires => groups.wires.as_ref().map(|g| g.count),
        }
    }
}

/// The order kinds are listed in a footer or the nothing-shared card.
const KIND_ORDER: [SelectedKind; 15] = [
    SelectedKind::Symbol,
    SelectedKind::SymbolRefField,
    SelectedKind::SymbolValField,
    SelectedKind::Label,
    SelectedKind::Wire,
    SelectedKind::Bus,
    SelectedKind::BusEntry,
    SelectedKind::Junction,
    SelectedKind::NoConnect,
    SelectedKind::NoErcDirective,
    SelectedKind::SheetPin,
    SelectedKind::TextNote,
    SelectedKind::ChildSheet,
    SelectedKind::Drawing,
    SelectedKind::Directive,
];

/// How many objects of each kind a selection holds, in [`KIND_ORDER`]; a
/// kind with none is left out.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SelectionKinds(Vec<(SelectedKind, usize)>);

impl SelectionKinds {
    pub fn of(items: &[SelectedItem]) -> Self {
        Self(
            KIND_ORDER
                .into_iter()
                .map(|kind| (kind, items.iter().filter(|i| i.kind == kind).count()))
                .filter(|(_, n)| *n > 0)
                .collect(),
        )
    }

    pub fn iter(&self) -> impl Iterator<Item = (SelectedKind, usize)> + '_ {
        self.0.iter().copied()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// The view state of the selection forms; one per Properties panel.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SelectionForm {
    /// The kind chip that is on; `None` is All.
    pub kind_filter: Option<MultiKind>,
    /// The text typed in a directive's rule filter.
    pub rule_query: String,
    /// The No-ERC directive whose All rules box was unticked: its rule list
    /// is open for picking while the directive still silences every rule.
    pub picking_rules_of: Option<uuid::Uuid>,
    /// The kinds of the current selection, for the footer and the
    /// nothing-shared card.
    pub kinds: SelectionKinds,
    /// The field or callout under the pointer.
    pub hover: IllustrationHover,
}

/// Messages of the selection forms, namespaced under
/// `PanelMsg::SelectionForm`.
#[derive(Debug, Clone, PartialEq)]
pub enum SelectionFormMsg {
    /// A kind chip was pressed; `None` is All.
    KindChipPicked(Option<MultiKind>),
    /// The rule filter was typed in.
    RuleQueryChanged(String),
    /// All rules was unticked on a No-ERC directive that silences every
    /// rule: its list opens for picking, the document is unchanged.
    AllRulesUnticked(uuid::Uuid),
    /// All rules was ticked again on a No-ERC directive whose list was open
    /// but still names no rule: picking ends, the document is unchanged.
    AllRulesTicked(uuid::Uuid),
    /// The pointer moved over a field row or the illustration.
    IllustrationHovered(IllustrationMsg),
    /// Browse… beside a sheet symbol's File was pressed.
    SheetFileBrowseClicked(uuid::Uuid),
    /// The file dialog closed; `None` when it was cancelled.
    SheetFileBrowsed(uuid::Uuid, Option<std::path::PathBuf>),
}

impl SelectionForm {
    /// Applies a message that only changes the view state; false for the
    /// ones the app handles (they touch files or the document).
    pub fn update(&mut self, message: &SelectionFormMsg) -> bool {
        match message {
            SelectionFormMsg::KindChipPicked(kind) => self.kind_filter = *kind,
            SelectionFormMsg::RuleQueryChanged(query) => self.rule_query = query.clone(),
            SelectionFormMsg::AllRulesUnticked(id) => self.picking_rules_of = Some(*id),
            SelectionFormMsg::AllRulesTicked(id) => {
                if self.picking_rules_of == Some(*id) {
                    self.picking_rules_of = None;
                }
            }
            SelectionFormMsg::IllustrationHovered(hover) => self.hover.update(hover.clone()),
            SelectionFormMsg::SheetFileBrowseClicked(_)
            | SelectionFormMsg::SheetFileBrowsed(..) => return false,
        }
        true
    }

    /// The chip in effect: the one that is on, or All when the selection
    /// holds none of its kind any more.
    pub fn effective_filter(&self, groups: &SelectionGroups) -> Option<MultiKind> {
        self.kind_filter
            .filter(|kind| kind.count_in(groups).is_some())
    }

    /// Whether the section of `kind` is shown.
    pub fn shows(&self, kind: MultiKind, groups: &SelectionGroups) -> bool {
        kind.count_in(groups).is_some() && self.effective_filter(groups).is_none_or(|on| on == kind)
    }

    /// Whether a No-ERC directive's All rules box reads as on: it names no
    /// rule and its list was not opened for picking.
    pub fn no_erc_all_rules_on(&self, id: uuid::Uuid, listed: &[String]) -> bool {
        listed.is_empty() && self.picking_rules_of != Some(id)
    }
}

/// Whether a rule named `name` passes the filter `query` (case-insensitive,
/// surrounding blanks ignored).
pub fn rule_matches(name: &str, query: &str) -> bool {
    let query = query.trim();
    query.is_empty() || name.to_lowercase().contains(&query.to_lowercase())
}
