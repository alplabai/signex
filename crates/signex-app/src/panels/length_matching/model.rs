//! What the Length Matching tab and panel show (mockup row 107): the
//! board's groups with each member's length against the group's band, and
//! the views' own state: the filter, the folded groups, the group picked
//! for editing, the member picked in it and the typed text. Nothing here
//! edits the board.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use signex_engine::pcb::length_match::MemberStatus;
use signex_i18n::{MessageId, ids};
use signex_types::pcb_length::LengthTopology;
use uuid::Uuid;

/// One member of a group, all from the board.
#[derive(Debug, Clone, PartialEq)]
pub struct LengthMemberRow {
    pub net: String,
    /// Routed length in mm; `None` for a net the board lacks.
    pub length_mm: Option<f64>,
    /// Length minus the group's target; `None` without a routed length.
    pub delta_mm: Option<f64>,
    pub status: MemberStatus,
    /// The differential pair the net is one half of, by name.
    pub pair: Option<String>,
}

/// One group with its members, in member order.
#[derive(Debug, Clone, PartialEq)]
pub struct LengthGroupRow {
    pub uuid: Uuid,
    pub name: String,
    /// The High Speed rule the group stands for (`MatchedLengths_<name>`).
    pub rule: String,
    pub target_mm: f64,
    pub tolerance_mm: f64,
    pub topology: LengthTopology,
    pub members: Vec<LengthMemberRow>,
}

impl LengthGroupRow {
    /// Members longer or shorter than the band.
    pub fn out_of_band(&self) -> usize {
        self.members
            .iter()
            .filter(|m| m.status.is_out_of_band())
            .count()
    }
}

/// The groups of the board in view.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LengthMatchingFacts {
    pub board: PathBuf,
    /// A board from a newer build takes no edit.
    pub read_only: bool,
    pub groups: Vec<LengthGroupRow>,
    /// The board's net names, sorted: what Add net offers.
    pub nets: Vec<String>,
}

impl LengthMatchingFacts {
    pub fn member_count(&self) -> usize {
        self.groups.iter().map(|g| g.members.len()).sum()
    }

    pub fn out_of_band(&self) -> usize {
        self.groups.iter().map(LengthGroupRow::out_of_band).sum()
    }

    pub fn group(&self, uuid: Uuid) -> Option<&LengthGroupRow> {
        self.groups.iter().find(|g| g.uuid == uuid)
    }
}

/// Which groups the views list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LengthFilter {
    #[default]
    All,
    /// Only groups with a member out of band.
    OutOfBand,
}

impl LengthFilter {
    pub const ALL: [LengthFilter; 2] = [LengthFilter::All, LengthFilter::OutOfBand];

    pub fn message(self) -> MessageId {
        match self {
            LengthFilter::All => ids::LENGTH_MATCHING_FILTER_ALL,
            LengthFilter::OutOfBand => ids::LENGTH_MATCHING_FILTER_OUT,
        }
    }

    /// Whether `group` passes.
    pub fn shows(self, group: &LengthGroupRow) -> bool {
        match self {
            LengthFilter::All => true,
            LengthFilter::OutOfBand => group.out_of_band() > 0,
        }
    }
}

/// A typed field of the tab: the picked group's, or a new group's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LengthField {
    Name,
    Target,
    Tolerance,
    NewName,
    NewTarget,
    NewTolerance,
}

impl LengthField {
    /// A field of the picked group, not of a new one.
    pub fn is_group_field(self) -> bool {
        matches!(
            self,
            LengthField::Name | LengthField::Target | LengthField::Tolerance
        )
    }
}

/// The name of `topology` in the list.
pub fn topology_message(topology: LengthTopology) -> MessageId {
    match topology {
        LengthTopology::Star => ids::LENGTH_MATCHING_TOPOLOGY_STAR,
        LengthTopology::Chain => ids::LENGTH_MATCHING_TOPOLOGY_CHAIN,
        LengthTopology::FlyBy => ids::LENGTH_MATCHING_TOPOLOGY_FLY_BY,
        LengthTopology::T => ids::LENGTH_MATCHING_TOPOLOGY_T,
    }
}

/// An event of the tab or the panel.
#[derive(Debug, Clone, PartialEq)]
pub enum LengthMatchingMsg {
    /// "Open as tab", or Length Matching... on the board in view.
    TabOpened,
    /// The tab's "Show as panel": the compact panel docks beside the board.
    PanelShown,
    FilterPicked(LengthFilter),
    /// A panel group's header: fold or unfold it.
    GroupToggled(Uuid),
    /// A tab group's header: edit it.
    GroupPicked(Uuid),
    MemberClicked {
        group: Uuid,
        net: String,
    },
    FieldTyped {
        field: LengthField,
        text: String,
    },
    /// Enter in a field: the picked group's field is applied; a new
    /// group's waits for Add Group.
    FieldSubmitted(LengthField),
    TopologyPicked {
        group: Uuid,
        topology: LengthTopology,
    },
    NetAdded {
        group: Uuid,
        net: String,
    },
    MemberRemoved {
        group: Uuid,
        net: String,
    },
    GroupAdded,
    GroupDeleted(Uuid),
}

/// The views' own state, kept across panel context rebuilds.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LengthMatchingUi {
    /// The board this state was made on; another board starts afresh.
    pub board: Option<PathBuf>,
    pub filter: LengthFilter,
    /// Panel groups folded shut.
    pub closed: BTreeSet<Uuid>,
    /// The group the tab edits.
    pub current: Option<Uuid>,
    /// The member picked in the current group, by net name.
    pub selected: Option<String>,
    pub drafts: BTreeMap<LengthField, String>,
}

impl LengthMatchingUi {
    /// Start afresh when `board` is not the board this state was made on.
    pub fn follow(&mut self, board: &Path) {
        if self.board.as_deref() != Some(board) {
            *self = Self {
                board: Some(board.to_path_buf()),
                ..Self::default()
            };
        }
    }

    /// Whether this state was made on `board`.
    pub fn is_for(&self, board: &Path) -> bool {
        self.board.as_deref() == Some(board)
    }

    /// The text shown in `field`: the typed draft, else `stored`.
    pub fn shown(&self, field: LengthField, stored: impl FnOnce() -> String) -> String {
        self.drafts.get(&field).cloned().unwrap_or_else(stored)
    }

    /// The view part of `msg`; the board edits are the app's.
    pub fn apply(&mut self, msg: &LengthMatchingMsg) {
        use LengthMatchingMsg as M;
        match msg {
            M::FilterPicked(filter) => self.filter = *filter,
            M::GroupToggled(group) => {
                if !self.closed.remove(group) {
                    self.closed.insert(*group);
                }
            }
            M::GroupPicked(group) => self.pick(*group),
            M::MemberClicked { group, net } => {
                self.pick(*group);
                self.selected = Some(net.clone());
            }
            M::FieldTyped { field, text } => {
                self.drafts.insert(*field, text.clone());
            }
            M::GroupDeleted(group) if self.current == Some(*group) => {
                self.current = None;
                self.selected = None;
                self.drafts.retain(|field, _| !field.is_group_field());
            }
            M::MemberRemoved { net, .. } if self.selected.as_ref() == Some(net) => {
                self.selected = None;
            }
            M::TabOpened
            | M::PanelShown
            | M::FieldSubmitted(_)
            | M::TopologyPicked { .. }
            | M::NetAdded { .. }
            | M::MemberRemoved { .. }
            | M::GroupAdded
            | M::GroupDeleted(_) => {}
        }
    }

    /// Edit `group`: the text typed for another group goes.
    fn pick(&mut self, group: Uuid) {
        if self.current != Some(group) {
            self.current = Some(group);
            self.selected = None;
            self.drafts.retain(|field, _| !field.is_group_field());
        }
    }

    /// Forget the text typed in `fields`: it was applied.
    pub fn drop_drafts(&mut self, fields: &[LengthField]) {
        self.drafts.retain(|field, _| !fields.contains(field));
    }
}
