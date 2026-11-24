//! The Polygon Manager's model (mockup row 178): one row per polygon pour of
//! the board, read by the app from the board and its fills, and the view
//! state the tab and the PCB panel's Polygons mode share (DECISIONS
//! 2026-11-18, "Tab + compact panel pair"): the sub-tab, the filters, the
//! ticked rows and the text typed and not applied yet. Nothing here is saved.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use signex_i18n::{MessageId, ids};
use uuid::Uuid;

/// Where a pour stands. A pour's copper is computed from the board as it is
/// now, so there is no Out of date state yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PourState {
    Poured,
    /// Outline and settings kept, no copper (#2523).
    Shelved,
    /// The fill could not be made; the row says why.
    NotFilled,
}

impl PourState {
    pub const ALL: [Self; 3] = [Self::Poured, Self::Shelved, Self::NotFilled];

    pub fn message(self) -> MessageId {
        match self {
            Self::Poured => ids::POLYGON_MANAGER_STATE_POURED,
            Self::Shelved => ids::PCB_PRIM_SHELVED_PILL,
            Self::NotFilled => ids::POLYGON_MANAGER_STATE_NOT_FILLED,
        }
    }

    /// What the panel's Needs attention chip keeps.
    pub fn needs_attention(self) -> bool {
        !matches!(self, Self::Poured)
    }
}

/// A cell typed in place and applied with Enter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PourField {
    /// mm; 0 follows the board's clearance.
    Clearance,
    /// mm.
    MinWidth,
    /// mm²; 0 keeps every island.
    IslandsBelow,
}

impl PourField {
    pub const ALL: [Self; 3] = [Self::Clearance, Self::MinWidth, Self::IslandsBelow];

    /// The column title, and the label of the undo step an edit makes (the
    /// one Properties gives the same field).
    pub fn message(self) -> MessageId {
        match self {
            Self::Clearance => ids::PCB_PRIM_CLEARANCE,
            Self::MinWidth => ids::PCB_PRIM_MIN_THICKNESS,
            Self::IslandsBelow => ids::PCB_PRIM_MIN_ISLAND_AREA,
        }
    }
}

/// One pour of the board.
#[derive(Debug, Clone, PartialEq)]
pub struct PourRow {
    pub zone: Uuid,
    /// The name the board's pour naming scheme gives it.
    pub name: String,
    /// The net's name; empty for a pour of no net.
    pub net: String,
    /// The layer's name, as the Layers panel names it.
    pub layer: String,
    /// Where the pour's layer sits among the board's copper layers, from 1;
    /// the table groups by it.
    pub layer_place: usize,
    /// Its place in the fill order of its layer, from 1; `None` while
    /// shelved, since a shelved pour does not fill.
    pub order: Option<usize>,
    pub state: PourState,
    /// Why it has no fill, when it has none (a `pcb-pour-unfilled-*` id).
    pub unfilled: Option<MessageId>,
    pub clearance_mm: f64,
    pub min_width_mm: f64,
    pub islands_below_mm2: f64,
    pub dead_copper: bool,
    /// What the fill holds: copper area, pieces and the islands its rules
    /// dropped; `None` without a fill.
    pub area_mm2: Option<f64>,
    pub pieces: Option<usize>,
    pub islands_removed: Option<usize>,
}

impl PourRow {
    /// The stored value of a typed cell.
    pub fn value(&self, field: PourField) -> f64 {
        match field {
            PourField::Clearance => self.clearance_mm,
            PourField::MinWidth => self.min_width_mm,
            PourField::IslandsBelow => self.islands_below_mm2,
        }
    }
}

/// What the manager shows of the board in view.
#[derive(Debug, Clone, PartialEq)]
pub struct PolygonFacts {
    pub board: PathBuf,
    pub read_only: bool,
    /// Grouped by layer in copper order, each layer in pour order with its
    /// shelved pours last.
    pub rows: Vec<PourRow>,
}

impl PolygonFacts {
    /// The layers that hold a pour, in table order, each once.
    pub fn layers(&self) -> Vec<String> {
        let mut layers: Vec<String> = Vec::new();
        for row in &self.rows {
            if !layers.contains(&row.layer) {
                layers.push(row.layer.clone());
            }
        }
        layers
    }

    /// The nets of the pours, sorted, each once; no net is left out.
    pub fn nets(&self) -> Vec<String> {
        let nets: BTreeSet<&String> = self
            .rows
            .iter()
            .map(|row| &row.net)
            .filter(|net| !net.is_empty())
            .collect();
        nets.into_iter().cloned().collect()
    }

    pub fn count(&self, state: PourState) -> usize {
        self.rows.iter().filter(|row| row.state == state).count()
    }

    pub fn row(&self, zone: Uuid) -> Option<&PourRow> {
        self.rows.iter().find(|row| row.zone == zone)
    }
}

/// The page of the tab in view. Out of date and Pour order (mockup row 178)
/// are not drawn yet: a pour's copper always follows the board, and pours
/// are reordered through their Priority in Properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PolygonSubTab {
    #[default]
    Pours,
    Shelved,
    BulkEdit,
}

impl PolygonSubTab {
    pub const ALL: [Self; 3] = [Self::Pours, Self::Shelved, Self::BulkEdit];

    pub fn message(self) -> MessageId {
        match self {
            Self::Pours => ids::POLYGON_MANAGER_SUB_POURS,
            Self::Shelved => ids::POLYGON_MANAGER_SUB_SHELVED,
            Self::BulkEdit => ids::POLYGON_MANAGER_SUB_BULK,
        }
    }
}

/// An event of the Polygon Manager tab or the PCB panel's Polygons mode.
/// Each but the first names the board it was shown for, so a tab change in
/// between cannot send it to another board.
#[derive(Debug, Clone, PartialEq)]
pub enum PolygonMsg {
    /// Open or focus the board's Polygon Manager tab (Tools ▸ Polygon
    /// Manager..., a pour's right-click, the panel's link), with the pours
    /// selected on the board ticked.
    ManagerOpened,
    SubTabPicked {
        board: PathBuf,
        tab: PolygonSubTab,
    },
    LayerFilterPicked {
        board: PathBuf,
        layer: Option<String>,
    },
    NetFilterPicked {
        board: PathBuf,
        net: Option<String>,
    },
    StateFilterPicked {
        board: PathBuf,
        state: Option<PourState>,
    },
    /// The panel's All / Needs attention chips.
    AttentionPicked {
        board: PathBuf,
        on: bool,
    },
    /// A row was clicked: it alone is ticked and its detail shows.
    RowClicked {
        board: PathBuf,
        zone: Uuid,
    },
    RowTicked {
        board: PathBuf,
        zone: Uuid,
        on: bool,
    },
    /// The header's tick box: the rows shown are ticked, or none is.
    ShownTicked {
        board: PathBuf,
        zones: Vec<Uuid>,
        on: bool,
    },
    FieldTyped {
        board: PathBuf,
        zone: Uuid,
        field: PourField,
        text: String,
    },
    /// Enter in a cell: its draft is applied to that pour.
    FieldSubmitted {
        board: PathBuf,
        zone: Uuid,
        field: PourField,
    },
    DeadCopperToggled {
        board: PathBuf,
        zone: Uuid,
        on: bool,
    },
    BulkTyped {
        board: PathBuf,
        field: PourField,
        text: String,
    },
    /// Enter in a bulk field: applied to every ticked pour, one undo step.
    BulkSubmitted {
        board: PathBuf,
        field: PourField,
    },
    BulkDeadCopperToggled {
        board: PathBuf,
        on: bool,
    },
    RepourAllPressed {
        board: PathBuf,
    },
    RepourSelectedPressed {
        board: PathBuf,
    },
    RepourPressed {
        board: PathBuf,
        zone: Uuid,
    },
    ShelveAllPressed {
        board: PathBuf,
    },
    UnshelveAllPressed {
        board: PathBuf,
    },
    ShelveSelectedPressed {
        board: PathBuf,
    },
    ShelveToggled {
        board: PathBuf,
        zone: Uuid,
        shelved: bool,
    },
}

impl PolygonMsg {
    /// The board the event was shown for; `None` for Open.
    pub fn board(&self) -> Option<&Path> {
        use PolygonMsg as M;
        match self {
            M::ManagerOpened => None,
            M::SubTabPicked { board, .. }
            | M::LayerFilterPicked { board, .. }
            | M::NetFilterPicked { board, .. }
            | M::StateFilterPicked { board, .. }
            | M::AttentionPicked { board, .. }
            | M::RowClicked { board, .. }
            | M::RowTicked { board, .. }
            | M::ShownTicked { board, .. }
            | M::FieldTyped { board, .. }
            | M::FieldSubmitted { board, .. }
            | M::DeadCopperToggled { board, .. }
            | M::BulkTyped { board, .. }
            | M::BulkSubmitted { board, .. }
            | M::BulkDeadCopperToggled { board, .. }
            | M::RepourAllPressed { board }
            | M::RepourSelectedPressed { board }
            | M::RepourPressed { board, .. }
            | M::ShelveAllPressed { board }
            | M::UnshelveAllPressed { board }
            | M::ShelveSelectedPressed { board }
            | M::ShelveToggled { board, .. } => Some(board.as_path()),
        }
    }
}

/// The view state of the manager, for the board it was made on.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PolygonsUi {
    pub board: Option<PathBuf>,
    pub sub_tab: PolygonSubTab,
    pub layer: Option<String>,
    pub net: Option<String>,
    pub state: Option<PourState>,
    /// The panel's Needs attention chip is on.
    pub attention: bool,
    /// The ticked rows.
    pub selected: BTreeSet<Uuid>,
    /// The row whose detail shows: the last one clicked or ticked.
    pub current: Option<Uuid>,
    pub drafts: BTreeMap<(Uuid, PourField), String>,
    pub bulk_drafts: BTreeMap<PourField, String>,
}

impl PolygonsUi {
    /// Start over for `board` when the state was made for another one.
    pub fn follow(&mut self, board: &Path) {
        if !self.is_for(board) {
            *self = Self {
                board: Some(board.to_path_buf()),
                ..Self::default()
            };
        }
    }

    pub fn is_for(&self, board: &Path) -> bool {
        self.board.as_deref() == Some(board)
    }

    /// Apply the view-state part of `msg`; the app carries out the rest.
    pub fn apply(&mut self, msg: &PolygonMsg) {
        let Some(board) = msg.board() else {
            return;
        };
        self.follow(board);
        self.apply_view(msg);
    }

    fn apply_view(&mut self, msg: &PolygonMsg) {
        use PolygonMsg as M;
        match msg {
            M::SubTabPicked { tab, .. } => self.sub_tab = *tab,
            M::LayerFilterPicked { layer, .. } => self.layer.clone_from(layer),
            M::NetFilterPicked { net, .. } => self.net.clone_from(net),
            M::StateFilterPicked { state, .. } => self.state = *state,
            M::AttentionPicked { on, .. } => self.attention = *on,
            M::RowClicked { zone, .. } => {
                self.selected = BTreeSet::from([*zone]);
                self.current = Some(*zone);
            }
            M::RowTicked { zone, on, .. } => self.tick(*zone, *on),
            M::ShownTicked { zones, on, .. } => {
                for zone in zones {
                    self.tick(*zone, *on);
                }
            }
            M::FieldTyped {
                zone, field, text, ..
            } => {
                self.drafts.insert((*zone, *field), text.clone());
            }
            M::BulkTyped { field, text, .. } => {
                self.bulk_drafts.insert(*field, text.clone());
            }
            M::ManagerOpened
            | M::FieldSubmitted { .. }
            | M::DeadCopperToggled { .. }
            | M::BulkSubmitted { .. }
            | M::BulkDeadCopperToggled { .. }
            | M::RepourAllPressed { .. }
            | M::RepourSelectedPressed { .. }
            | M::RepourPressed { .. }
            | M::ShelveAllPressed { .. }
            | M::UnshelveAllPressed { .. }
            | M::ShelveSelectedPressed { .. }
            | M::ShelveToggled { .. } => {}
        }
    }

    fn tick(&mut self, zone: Uuid, on: bool) {
        if on {
            self.selected.insert(zone);
            self.current = Some(zone);
        } else {
            self.selected.remove(&zone);
            if self.current == Some(zone) {
                self.current = self.selected.iter().next().copied();
            }
        }
    }

    /// The ticked pours still on the board of `facts`, in table order;
    /// none when this state was made for another board.
    pub fn selected_in(&self, facts: &PolygonFacts) -> Vec<Uuid> {
        if !self.is_for(&facts.board) {
            return Vec::new();
        }
        facts
            .rows
            .iter()
            .filter(|row| self.selected.contains(&row.zone))
            .map(|row| row.zone)
            .collect()
    }

    /// The pour whose detail shows, while it is on the board.
    pub fn current_in<'f>(&self, facts: &'f PolygonFacts) -> Option<&'f PourRow> {
        let zone = self.current.filter(|_| self.is_for(&facts.board))?;
        facts.row(zone)
    }

    /// The rows the tab shows: its page and its filters keep them.
    pub fn tab_rows<'f>(&self, facts: &'f PolygonFacts) -> Vec<&'f PourRow> {
        let ours = self.is_for(&facts.board);
        let shelved_page = ours && self.sub_tab == PolygonSubTab::Shelved;
        facts
            .rows
            .iter()
            .filter(|row| !shelved_page || row.state == PourState::Shelved)
            .filter(|row| !ours || self.keeps(row))
            .collect()
    }

    /// The rows the panel lists: every pour, or with Needs attention on
    /// those that are not poured.
    pub fn panel_rows<'f>(&self, facts: &'f PolygonFacts) -> Vec<&'f PourRow> {
        let attention = self.attention && self.is_for(&facts.board);
        facts
            .rows
            .iter()
            .filter(|row| !attention || row.state.needs_attention())
            .collect()
    }

    fn keeps(&self, row: &PourRow) -> bool {
        self.layer.as_ref().is_none_or(|layer| *layer == row.layer)
            && self.net.as_ref().is_none_or(|net| *net == row.net)
            && self.state.is_none_or(|state| state == row.state)
    }

    /// The draft of a cell, if one is typed.
    pub fn draft(&self, zone: Uuid, field: PourField) -> Option<&str> {
        self.drafts.get(&(zone, field)).map(String::as_str)
    }
}

/// The value every row of `rows` shares for `field`; `None` when they
/// differ (shown "mixed") or there is no row.
pub fn shared_value(rows: &[&PourRow], field: PourField) -> Option<f64> {
    let first = rows.first()?.value(field);
    rows.iter()
        .all(|row| row.value(field) == first)
        .then_some(first)
}

/// Whether every row of `rows` removes dead copper, or none does; `None`
/// when they differ or there is no row.
pub fn shared_dead_copper(rows: &[&PourRow]) -> Option<bool> {
    let first = rows.first()?.dead_copper;
    rows.iter()
        .all(|row| row.dead_copper == first)
        .then_some(first)
}
