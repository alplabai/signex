//! What the component picker modal is doing for a placed part: Change
//! Component (#1260, mockup row 155) and the datasheet list (#1301). The
//! default is the plain Place Component list.

use std::collections::{HashMap, HashSet};

use signex_engine::ComponentChangePreview;
use uuid::Uuid;

use crate::library::messages::{PlacedPartTarget, ResolvedPlacement};
use crate::library::placed_part::DatasheetTarget;

#[derive(Debug, Clone, Default)]
pub enum PickerMode {
    /// Pick a part to place.
    #[default]
    Place,
    /// Change Component with no replacement read yet: the part list, and an
    /// empty comparison beside it.
    ChangeComponent(Box<ChangeSession>),
    /// Change Component with the picked replacement read and checked: the
    /// same list, and the comparison of the placed part with it.
    ChangePreview(Box<ChangePreview>),
    /// Pick which of the part's documents to open.
    Datasheets(DatasheetChoice),
}

/// What Change Component knows of the placed part while the user picks,
/// kept across every pick of the one dialog.
#[derive(Debug, Clone)]
pub struct ChangeSession {
    pub target: PlacedPartTarget,
    /// How messages and the dialog name the part ("R5", or "the part").
    pub reference: String,
    /// The sheet's file name, for the header note.
    pub sheet_name: String,
    pub placed: PlacedNow,
    pub filters: ChangeFilters,
    /// Rows the switched-on filters leave out; rebuilt with `filters`.
    pub hidden: HashSet<Uuid>,
    /// Pin count of each listed row's drawing, read off the update thread;
    /// a row not in it yet shows no pin tag.
    pub row_pins: HashMap<Uuid, usize>,
}

impl ChangeSession {
    /// A session with nothing known of the placed part beyond its name.
    pub fn new(target: PlacedPartTarget, reference: String) -> Self {
        Self {
            sheet_name: target
                .sheet
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
            target,
            reference,
            placed: PlacedNow::default(),
            filters: ChangeFilters::default(),
            hidden: HashSet::new(),
            row_pins: HashMap::new(),
        }
    }

    /// The pin tag of row `row_id`: `None` while its count is not read.
    pub fn pin_match(&self, row_id: Uuid) -> Option<PinMatch> {
        let placed = self.placed.pins?;
        let row = *self.row_pins.get(&row_id)?;
        Some(if row == placed {
            PinMatch::Same
        } else {
            PinMatch::Differs { row, placed }
        })
    }
}

/// The placed part as the list compares rows with it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlacedNow {
    /// The part number the "Placed now" card shows (MPN, else the value).
    pub part: String,
    pub footprint: String,
    /// The drawing's `"<library uuid>:<symbol uuid>"`, for Same symbol.
    pub lib_id: String,
    /// The value parameter of the part's class and its placed value, for
    /// Same value; `None` when the part's row or class is not known.
    pub value: Option<(String, String)>,
    /// Distinct pins of the placed drawing; `None` when it has no drawing.
    pub pins: Option<usize>,
}

/// The list's filter chips. Same symbol is on when the dialog opens, as
/// mockup row 155 draws it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChangeFilters {
    pub same_symbol: bool,
    pub same_value: bool,
    /// Only rows a distributor last reported in stock (the cached prices).
    pub in_stock: bool,
}

impl Default for ChangeFilters {
    fn default() -> Self {
        Self {
            same_symbol: true,
            same_value: false,
            in_stock: false,
        }
    }
}

/// One filter chip of the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeFilter {
    SameSymbol,
    SameValue,
    InStock,
}

/// A listed row's pins against the placed part's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinMatch {
    Same,
    /// A row with another pin count is greyed and cannot be picked.
    Differs {
        row: usize,
        placed: usize,
    },
}

/// Which placed parts the change applies to. Mockup row 155 also draws
/// the same part in every channel and every part of the same MPN in the
/// project; those wait on a maintainer decision.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ChangeScope {
    /// The part the change started on.
    #[default]
    ThisPart,
}

/// The replacement part, read and checked against the sheet, and what the
/// user chose about it.
#[derive(Debug, Clone)]
pub struct ChangePreview {
    pub session: ChangeSession,
    pub resolved: ResolvedPlacement,
    pub preview: ComponentChangePreview,
    /// The Now / After table.
    pub rows: Vec<CompareRow>,
    /// User parameters whose Keep now box is cleared: the swap clears them.
    /// Every other user parameter is kept.
    pub dropped: Vec<String>,
    pub scope: ChangeScope,
}

impl ChangePreview {
    /// How many parts Change applies to.
    pub fn part_count(&self) -> usize {
        match self.scope {
            ChangeScope::ThisPart => 1,
        }
    }
}

/// One line of the Now / After table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompareRow {
    pub field: CompareField,
    pub now: String,
    /// Empty for a user parameter the new part does not have.
    pub after: String,
    pub kind: CompareKind,
}

/// What a line of the table compares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompareField {
    Value,
    Footprint,
    Datasheet,
    /// A part field or parameter, by its name.
    Named(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareKind {
    /// The new part gives the same value.
    Same,
    /// Library data the new part replaces; it cannot be kept
    /// (DECISIONS 2026-11-19).
    Replaced,
    /// A user or override parameter the new part does not supply: its
    /// Keep now box decides.
    User,
}

#[derive(Debug, Clone)]
pub struct DatasheetChoice {
    pub reference: String,
    pub targets: Vec<DatasheetTarget>,
}
