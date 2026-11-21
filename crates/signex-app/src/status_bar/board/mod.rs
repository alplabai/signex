//! The board counters of the status bar and their popovers (mockup row 164, #2460).
//!
//! On a board tab the bar adds Routed, Unrouted, DRC and Selected before the
//! zoom; the layer pill, the Grid read-out and every counter open a popover
//! above themselves. The data comes in as [`BoardReadout`] and
//! [`BoardPopoverModel`], built in `update`'s state by the app; this module
//! only draws it. Layout and wording: `docs/DECISIONS.md`, "Board status bar
//! counters".

use std::time::SystemTime;

use iced::Color;
use pcb_geom::{ItemRef, PadName, PickKind};
use signex_i18n::Localized;
use signex_types::pcb_layer::PcbLayer;

mod anchor;
mod counters;
mod drc_popover;
mod grid_popover;
mod kit;
mod layer_dropdown;
mod layer_popover;
mod selection_popover;
mod unrouted_popover;

pub(crate) use anchor::{AnchorBar, popover_left};
pub(crate) use counters::grid_value as board_grid_value;
pub(super) use counters::{board_counters, board_grid_readout, layer_chevron, layer_swatch};
pub(crate) use grid_popover::custom_pitch;
pub(crate) use kit::popover_view;
pub(crate) use layer_dropdown::{LayerDropdownGroup, group_layers};

/// A board counter of the status bar; each opens a popover.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardCounter {
    /// A click on the active layer pill: the layer list of the switcher.
    Layer,
    /// A right-click on the pill: the Active layer popover, under either
    /// switcher.
    ActiveLayer,
    /// The Grid read-out.
    Grid,
    Routed,
    Unrouted,
    Drc,
    Selected,
}

impl BoardCounter {
    /// The popover the counter opens: Routed and Unrouted share one.
    pub fn popover(self) -> BoardPopoverKind {
        match self {
            Self::Layer => BoardPopoverKind::Layer,
            Self::ActiveLayer => BoardPopoverKind::ActiveLayer,
            Self::Grid => BoardPopoverKind::Grid,
            Self::Routed | Self::Unrouted => BoardPopoverKind::Unrouted,
            Self::Drc => BoardPopoverKind::Drc,
            Self::Selected => BoardPopoverKind::Selection,
        }
    }
}

/// One of the board popovers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardPopoverKind {
    /// The pill's click: the dropdown under the Dropdown switcher, else the
    /// Active layer popover.
    Layer,
    /// The pill's right-click: the Active layer popover.
    ActiveLayer,
    Grid,
    Unrouted,
    Drc,
    Selection,
}

/// The open board popover: the window whose bar opened it, which one, and
/// the Grid popover's Custom… field.
#[derive(Debug, Clone, PartialEq)]
pub struct BoardStatusPopover {
    pub window: iced::window::Id,
    pub kind: BoardPopoverKind,
    /// What the Custom… field holds.
    pub custom_grid: String,
    /// The field's last Enter was refused (not a pitch above 0): it shows
    /// in the error ink until edited.
    pub custom_refused: bool,
}

impl BoardStatusPopover {
    pub fn new(window: iced::window::Id, kind: BoardPopoverKind) -> Self {
        Self {
            window,
            kind,
            custom_grid: String::new(),
            custom_refused: false,
        }
    }
}

/// The board counters' own state: the open popover and the time the bar
/// reads the DRC age against, stored by `update` (a run landing, the 30 s
/// tick) so the view never reads the clock.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoardStatusUi {
    pub popover: Option<BoardStatusPopover>,
    pub now: Option<SystemTime>,
}

/// What a board popover reports.
#[derive(Debug, Clone, PartialEq)]
pub enum BoardPopoverEvent {
    /// Esc or a click outside closed it.
    Dismissed,
    /// A row of the Active layer popover was picked.
    LayerPicked(PcbLayer),
    /// A layer set row of the layer dropdown was picked.
    LayerSetMenu(crate::app::LayerSetMenuMsg),
    LayersPanelOpened,
    /// A grid preset row was picked, in mm.
    GridPresetPicked(f32),
    CustomGridEdited(String),
    /// Enter in the Custom… field.
    CustomGridSubmitted,
    SnapToggled,
    /// Grids…: the board's grid settings.
    GridSettingsOpened,
    /// A net's Zoom: frame its ratsnest lines.
    NetZoomed(u32),
    /// Route next: frame the first unrouted connection and arm Interactive
    /// Routing.
    RouteNextRequested,
    /// A violation's Zoom, by its waiver key.
    ViolationZoomed(String),
    DrcRunRequested,
    MessagesOpened,
    /// A selected object's Zoom.
    SelectedItemZoomed(ItemRef),
    PropertiesOpened,
    /// Select in schematic: the selection's counterpart on the sheets.
    SchematicSelectRequested,
    SelectionCleared,
}

/// What the bar shows of a board beyond the w3a segments.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct BoardReadout {
    /// `None` until the connections of the board's current revision are
    /// counted.
    pub connections: Option<ConnectionTally>,
    pub drc: DrcTally,
    pub selected: usize,
    /// The popover open in this window, so its counter reads as pressed.
    pub open: Option<BoardPopoverKind>,
    /// The grid in effect is one of the board's extra grids.
    pub grid_extra: bool,
}

/// Routed and unrouted connections.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ConnectionTally {
    pub routed: usize,
    pub total: usize,
    pub unrouted: usize,
}

impl ConnectionTally {
    /// Percent routed, to the nearest whole: 38 of 46 is 83. It reads 100
    /// only when every connection is routed, and a board with nothing to
    /// connect is all routed.
    pub fn percent(self) -> usize {
        if self.total == 0 || self.routed >= self.total {
            return 100;
        }
        ((self.routed * 100 + self.total / 2) / self.total).min(99)
    }
}

/// The DRC counter: the last batch run's violations and how long ago it ran.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum DrcTally {
    #[default]
    NotRun,
    Ran {
        violations: usize,
        age: DrcAge,
    },
}

/// How long ago the last run landed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DrcAge {
    /// Under a minute, or the clock went back.
    JustNow,
    Minutes(u64),
}

impl DrcAge {
    /// The age of a run that landed at `finished` when it is `now`.
    pub fn between(finished: SystemTime, now: SystemTime) -> Self {
        match now.duration_since(finished) {
            Ok(age) if age.as_secs() >= 60 => Self::Minutes(age.as_secs() / 60),
            _ => Self::JustNow,
        }
    }
}

/// Grid pitches the Grid popover lists, in mm.
pub(crate) const BOARD_GRID_PRESETS_MM: [f32; 4] = [0.1, 0.25, 0.5, 1.27];

/// What a board popover lists, read from the app state in `view` without
/// computing anything heavy (the counts are cached when the board changes).
#[derive(Debug, Clone)]
pub(crate) enum BoardPopoverModel {
    Layer(LayerList),
    /// The Layer pill's list under the Dropdown layer switcher.
    LayerDropdown(Vec<LayerDropdownGroup>),
    Grid(GridChoice),
    Unrouted(UnroutedList),
    Drc(DrcList),
    Selection(SelectionList),
}

impl BoardPopoverModel {
    pub fn kind(&self) -> BoardPopoverKind {
        match self {
            Self::Layer(_) | Self::LayerDropdown(_) => BoardPopoverKind::Layer,
            Self::Grid(_) => BoardPopoverKind::Grid,
            Self::Unrouted(_) => BoardPopoverKind::Unrouted,
            Self::Drc(_) => BoardPopoverKind::Drc,
            Self::Selection(_) => BoardPopoverKind::Selection,
        }
    }
}

/// The layers the next-layer key steps through, and its keys.
#[derive(Debug, Clone)]
pub(crate) struct LayerList {
    pub rows: Vec<LayerChoice>,
    pub next_key: String,
    pub prev_key: String,
}

#[derive(Debug, Clone)]
pub(crate) struct LayerChoice {
    pub layer: PcbLayer,
    pub name: Localized,
    pub colour: Color,
    pub active: bool,
}

/// The Grid popover's state.
#[derive(Debug, Clone)]
pub(crate) struct GridChoice {
    pub current_mm: f32,
    pub custom: String,
    pub refused: bool,
    pub snap: bool,
    pub cycle_key: String,
}

/// The unrouted connections by net.
#[derive(Debug, Clone)]
pub(crate) struct UnroutedList {
    pub unrouted: usize,
    pub nets: Vec<NetRow>,
    pub route_key: String,
}

#[derive(Debug, Clone)]
pub(crate) struct NetRow {
    pub net: u32,
    pub name: String,
    /// The pads each unrouted connection joins.
    pub pairs: Vec<(PadName, PadName)>,
    pub pour_will_connect: bool,
}

impl NetRow {
    /// The net's unrouted connections.
    pub fn pair_count(&self) -> usize {
        self.pairs.len()
    }
}

/// The last DRC run of the board.
#[derive(Debug, Clone)]
pub(crate) struct DrcList {
    /// When it landed (local `HH:MM:SS`) and the board's edits since;
    /// `None` before the first run.
    pub last_run: Option<(String, u64)>,
    pub running: bool,
    pub rows: Vec<crate::panels::DrcRow>,
    /// Violations past the rows listed.
    pub more: usize,
}

/// The selected objects.
#[derive(Debug, Clone)]
pub(crate) struct SelectionList {
    pub count: usize,
    pub rows: Vec<SelectedRow>,
    /// The board is in a project, so the selection has a schematic side.
    pub linked: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct SelectedRow {
    pub item: ItemRef,
    pub what: SelectedWhat,
}

#[derive(Debug, Clone)]
pub(crate) enum SelectedWhat {
    Part {
        designator: String,
        value: String,
        footprint: String,
    },
    Kind(PickKind),
}

#[cfg(test)]
mod tests;
