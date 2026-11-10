//! The change list "Update PCB from schematic" shows: one row per change,
//! each keyed so the user's ticks survive a Refresh.

use std::collections::BTreeSet;

use signex_types::part::{FootprintResolveError, FootprintSourceRef};
use signex_types::pcb::{Footprint, PadNet, SymbolLink};
use signex_types::schematic::InstancePath;
use uuid::Uuid;

/// A row's identity. Built from what the row is about (a footprint uuid, a
/// part's sheet and anchor unit, a net number, a pad), never from its
/// position in the list, so a recomputed list finds the same keys.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EcoKey {
    /// A part with no footprint, by its sheet and anchor (smallest) unit.
    AddFootprint {
        sheet: InstancePath,
        part: Uuid,
    },
    RemoveFootprint(Uuid),
    ChangeFootprint(Uuid),
    SetDesignator(Uuid),
    SetValue(Uuid),
    Link(Uuid),
    /// A board net the change adds, by its name: the number it takes can
    /// shift when other nets come or go.
    AddNet(String),
    RemoveNet(u32),
    RenameNet(u32),
    /// A pad whose net changes, by footprint and pad uuid.
    PadNet {
        footprint: Uuid,
        pad: Uuid,
    },
}

/// The groups the dialog shows rows in, in this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EcoGroup {
    AddFootprints,
    RemoveFootprints,
    ChangeFootprints,
    Designators,
    Values,
    Links,
    Nets,
    PadNets,
}

impl EcoGroup {
    pub const fn label(self) -> &'static str {
        match self {
            Self::AddFootprints => "Add footprints",
            Self::RemoveFootprints => "Remove footprints",
            Self::ChangeFootprints => "Change footprints",
            Self::Designators => "Designators",
            Self::Values => "Values",
            Self::Links => "Links",
            Self::Nets => "Nets",
            Self::PadNets => "Pad nets",
        }
    }
}

/// Why a footprint is replaced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeReason {
    /// The part now binds a different footprint.
    Footprint,
    /// Same footprint, another library version (versions are not ordered
    /// here): listed, not ticked.
    OtherVersion,
}

/// What a row does.
#[derive(Debug, Clone, PartialEq)]
pub enum EcoChange {
    /// Place a new footprint (already staged, linked and named).
    AddFootprint {
        footprint: Box<Footprint>,
    },
    RemoveFootprint {
        footprint: Uuid,
        reference: String,
    },
    ChangeFootprint {
        footprint: Uuid,
        reference: String,
        from: String,
        to: FootprintSourceRef,
        reason: ChangeReason,
        /// The library footprint, its pads already given their board uuids.
        with: Box<Footprint>,
    },
    SetDesignator {
        footprint: Uuid,
        from: String,
        to: String,
    },
    SetValue {
        footprint: Uuid,
        reference: String,
        from: String,
        to: String,
    },
    /// Store (or update) the footprint's link to its part.
    Link {
        footprint: Uuid,
        reference: String,
        link: SymbolLink,
    },
    AddNet {
        number: u32,
        name: String,
    },
    RemoveNet {
        number: u32,
        name: String,
    },
    RenameNet {
        number: u32,
        from: String,
        to: String,
    },
    PadNet {
        footprint: Uuid,
        pad: Uuid,
        reference: String,
        pad_number: String,
        from: Option<PadNet>,
        to: Option<PadNet>,
        /// The pad's net lock after the change.
        locked: bool,
    },
}

/// Why a row cannot be ticked.
#[derive(Debug, Clone, PartialEq)]
pub enum EcoBlock {
    /// The part's footprint could not be resolved.
    MissingFootprint(FootprintResolveError),
    /// The part is not annotated (an empty designator or one with `?`).
    NeedsAnnotation,
    /// Another part carries the same designator.
    DuplicateDesignator(String),
    /// A pad number shrinks and a pad past the new count holds a net or a
    /// lock that would be lost.
    PadsCannotCarry { number: String },
    /// The footprint cannot be measured for staging (a non-finite value).
    Unplaceable(String),
    /// The part's sheet or first unit could not be read back, so nothing
    /// about it can be compared.
    PartNotRead,
}

/// Something the user should know that is not a row.
#[derive(Debug, Clone, PartialEq)]
pub enum EcoNote {
    /// A sheet could not be read, so nothing linked under it is removed.
    UnreadableSheet(InstancePath),
    /// A locked pad's number is not on the new footprint: the pad goes, and
    /// its lock with it.
    PadLosesLock {
        footprint: Uuid,
        reference: String,
        pad_number: String,
    },
    /// A pad's number is not on the new footprint: the pad goes, and its net
    /// with it.
    PadLosesNet {
        footprint: Uuid,
        reference: String,
        pad_number: String,
        net: String,
    },
    /// A part whose sheet or first unit could not be read back: its footprint
    /// is left as it is.
    PartNotRead { designator: String },
    /// A placed part whose footprint could not be resolved (a library not
    /// open, a missing row or footprint): its pin map is unknown, so its
    /// pads are left out of net sync.
    FootprintUnresolved {
        footprint: Uuid,
        reference: String,
        error: FootprintResolveError,
    },
    /// A footprint with no link and no part of its designator; it is kept.
    KeptUnlinked { footprint: Uuid, reference: String },
    /// Net sync left a pad or terminal as it was (from `assign_pad_nets`).
    Net(String),
}

/// One row of the list.
#[derive(Debug, Clone, PartialEq)]
pub struct EcoRow {
    pub key: EcoKey,
    pub group: EcoGroup,
    pub change: EcoChange,
    /// Whether the row starts ticked.
    pub default_on: bool,
    /// Whether the row is ticked now. Always `false` while blocked.
    pub on: bool,
    pub blocked: Option<EcoBlock>,
    /// Rows this one only makes sense with: it is applied only while each of
    /// them is ticked too (a pad net row needs its footprint added).
    pub needs: Vec<EcoKey>,
}

impl EcoRow {
    pub(crate) fn new(key: EcoKey, group: EcoGroup, change: EcoChange, default_on: bool) -> Self {
        Self {
            key,
            group,
            change,
            default_on,
            on: default_on,
            blocked: None,
            needs: Vec::new(),
        }
    }

    pub(crate) fn blocked(mut self, why: EcoBlock) -> Self {
        self.blocked = Some(why);
        self.on = false;
        self
    }

    pub(crate) fn needing(mut self, keys: Vec<EcoKey>) -> Self {
        self.needs = keys;
        self
    }
}

/// The tick state of one group: `on` of its `open` (not blocked) rows are
/// ticked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupTicks {
    pub on: usize,
    pub open: usize,
}

impl GroupTicks {
    /// Every open row is ticked (and there is one).
    pub fn all(self) -> bool {
        self.open > 0 && self.on == self.open
    }

    /// Some open rows are ticked and some are not.
    pub fn mixed(self) -> bool {
        self.on > 0 && self.on < self.open
    }
}

/// The whole list, for the board at `revision`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EcoChangeList {
    /// The board revision the list was computed for; Execute is refused once
    /// the board has moved on.
    pub revision: u64,
    /// Footprint rows, then net rows, each in a fixed order.
    pub rows: Vec<EcoRow>,
    pub notes: Vec<EcoNote>,
}

impl EcoChangeList {
    pub fn row(&self, key: &EcoKey) -> Option<&EcoRow> {
        self.rows.iter().find(|r| &r.key == key)
    }

    /// Tick or untick a row. A blocked row stays unticked. Returns whether a
    /// footprint row changed, after which the net rows must be recomputed
    /// ([`super::refresh_nets`]).
    pub fn set_on(&mut self, key: &EcoKey, on: bool) -> bool {
        let Some(row) = self.rows.iter_mut().find(|r| &r.key == key) else {
            return false;
        };
        let on = on && row.blocked.is_none();
        let changed = row.on != on;
        row.on = on;
        changed && row.group < EcoGroup::Nets
    }

    /// Whether `row` takes effect: ticked, not blocked (`on` is public), and
    /// every row it needs takes effect in turn. Needs may form a cycle (a rename and the pad rows
    /// that follow it need each other): a row met again on the way counts
    /// as taking effect if it is ticked.
    pub fn applies(&self, row: &EcoRow) -> bool {
        let mut seen = BTreeSet::new();
        self.applies_within(row, &mut seen)
    }

    fn applies_within(&self, row: &EcoRow, seen: &mut BTreeSet<EcoKey>) -> bool {
        if !row.on || row.blocked.is_some() {
            return false;
        }
        if !seen.insert(row.key.clone()) {
            return true;
        }
        row.needs.iter().all(|k| {
            self.row(k)
                .is_some_and(|needed| self.applies_within(needed, seen))
        })
    }

    /// How many rows of `group` can be ticked (not blocked), and how many of
    /// them are: the group checkbox is on when all are, off when none is,
    /// mixed between.
    pub fn group_ticks(&self, group: EcoGroup) -> GroupTicks {
        let open = self
            .rows
            .iter()
            .filter(|r| r.group == group && r.blocked.is_none());
        let (on, open) = open.fold((0, 0), |(on, all), r| (on + usize::from(r.on), all + 1));
        GroupTicks { on, open }
    }

    /// Tick or untick every row of `group` that is not blocked. Returns
    /// whether a footprint row changed, as [`Self::set_on`] does.
    pub fn set_group_on(&mut self, group: EcoGroup, on: bool) -> bool {
        let keys: Vec<EcoKey> = self
            .rows
            .iter()
            .filter(|r| r.group == group)
            .map(|r| r.key.clone())
            .collect();
        let mut footprint_changed = false;
        for key in &keys {
            footprint_changed |= self.set_on(key, on);
        }
        footprint_changed
    }

    /// The keys ticked now.
    pub fn ticked(&self) -> BTreeSet<EcoKey> {
        self.rows
            .iter()
            .filter(|r| r.on)
            .map(|r| r.key.clone())
            .collect()
    }

    /// Carry the user's choices over from `previous` (the list before a
    /// Refresh of the same design, `input`): a row found there by key, whose
    /// change is still the same choice, keeps its tick where it can be
    /// ticked. The net rows are then recomputed for the carried footprint
    /// ticks, so they never describe a board the ticks no longer make, and
    /// their own ticks are carried the same way.
    pub fn keep_choices_from(&mut self, previous: &EcoChangeList, input: &super::EcoInput) {
        self.carry_ticks(previous);
        super::refresh_nets(self, input);
        self.carry_ticks(previous);
    }

    fn carry_ticks(&mut self, previous: &EcoChangeList) {
        for row in &mut self.rows {
            if let Some(old) = previous.row(&row.key)
                && old.change.same_choice(&row.change)
            {
                row.on = old.on && row.blocked.is_none();
            }
        }
    }
}

impl EcoChange {
    /// Whether a tick given to `self` still means the same thing for
    /// `other`. An added footprint may be staged elsewhere; a change of
    /// footprint is the same only for the same source, target and reason;
    /// every other row must match exactly.
    pub(crate) fn same_choice(&self, other: &EcoChange) -> bool {
        match (self, other) {
            (Self::AddFootprint { footprint: a }, Self::AddFootprint { footprint: b }) => {
                a.library_ref == b.library_ref
                    && a.footprint_id == b.footprint_id
                    && a.reference == b.reference
                    && a.value == b.value
            }
            (
                Self::ChangeFootprint {
                    from: fa,
                    to: ta,
                    reason: ra,
                    ..
                },
                Self::ChangeFootprint {
                    from: fb,
                    to: tb,
                    reason: rb,
                    ..
                },
            ) => fa == fb && ta == tb && ra == rb,
            // A new net's number can shift; its name is what was chosen.
            (Self::AddNet { name: a, .. }, Self::AddNet { name: b, .. }) => a == b,
            (
                Self::PadNet {
                    from: fa,
                    to: ta,
                    locked: la,
                    ..
                },
                Self::PadNet {
                    from: fb,
                    to: tb,
                    locked: lb,
                    ..
                },
            ) => {
                let name = |n: &Option<PadNet>| n.as_ref().map(|n| n.name.clone());
                fa == fb && la == lb && name(ta) == name(tb)
            }
            (a, b) => a == b,
        }
    }
}
