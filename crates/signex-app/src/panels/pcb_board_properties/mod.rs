//! Properties panel of the active PCB (#2353): with nothing selected, the
//! board itself (the header row, the tabs, Search, the board illustration
//! with its size row, then Selection Filter, Snap Options, Board
//! Information, Grid Manager, Guide Manager, Panel and Other; #2523); the
//! Component view when only footprints are selected (`pcb_component`); the
//! track, via or pad panel when only one such kind is selected
//! (`pcb_primitive_properties`); a card naming the board for any other
//! selection. A pure view over data the panel context copies from the board
//! and the editor settings. Guides, extra grids, the sheet and the naming
//! options are board options (#2374).
//!
//! What the board model cannot store, or the editor cannot act on, yet is
//! drawn disabled with a tooltip saying why.

mod cards;
mod chrome;
mod drawing;
mod fields;
mod filter;
mod grids;
mod guides;
mod header;
mod health;
mod info;
mod other;
mod panel;
mod snap;

pub use drawing::{BOARD_HEIGHT, BOARD_WIDTH, SNAP_STEP, VISIBLE_GRID, grid_field, size_readout};
// The PCB Active Bar's Selection Filter names the kinds as the chips do.
pub(crate) use filter::kind_label as pick_kind_label;
use panel::PanelFacts;

use std::collections::BTreeMap;
use std::path::PathBuf;

use iced::widget::{Column, container};
use iced::{Element, Length, Widget as _};
use pcb_geom::{BoardStats, PickFilter, PickKind};
use signex_engine::pcb::snap::SnapObject;
use signex_i18n::{ids, t};
use signex_types::coord::Unit;
use signex_types::pcb::PcbBoard;
use signex_types::pcb_options::{BoardOptions, DesignatorDisplay, GuideKind, PolygonNaming};
use signex_types::pcb_panel::{PanelField, PanelSeparation};
use signex_widgets::property_illustration::{IllustrationHover, IllustrationMsg};
use uuid::Uuid;

use super::{PanelContext, PanelMsg};
use crate::pcb_canvas::{PcbSnapOptions, SnapDistance, SnapScope};
use crate::styles::Spacing;
use chrome::Form;

const SP: Spacing = Spacing::DEFAULT;

/// The tabs under the panel title.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BoardTab {
    #[default]
    General,
    Parameters,
    HealthCheck,
}

/// The panel's own view state: kept for the session, never in the board.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PcbBoardPanelView {
    pub tab: BoardTab,
    /// Text in the search field; rows whose label holds it stay.
    pub search: String,
    /// The snap grid step being typed, until Enter applies it.
    pub grid_draft: Option<String>,
    /// Other fields being typed, until Enter applies them (#2374).
    pub drafts: BTreeMap<BoardField, String>,
    /// The grid card whose ⋯ menu is open (#2386).
    pub grid_menu: Option<GridCard>,
    /// The grid card whose fields are shown under it.
    pub grid_editing: Option<GridCard>,
    /// The guide card whose ⋯ menu is open.
    pub guide_menu: Option<Uuid>,
    /// The Add Guide dropdown is open.
    pub guide_add_open: bool,
    /// The Add Guide dropdown places the guide with a click on the board
    /// instead of adding it through the origin.
    pub guide_place: bool,
    /// The field under the pointer, in a row or on the board illustration
    /// (#2523): each side lights the other.
    pub illustration_hover: IllustrationHover,
}

/// One card of the Grid Manager: the board's own grid, or an extra grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridCard {
    Global,
    Extra(Uuid),
}

/// A text field of the panel whose draft is kept until Enter (#2374).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BoardField {
    AxisRange,
    GuideX(Uuid),
    GuideY(Uuid),
    GridName(Uuid),
    GridStep(Uuid),
    GridOriginX(Uuid),
    GridOriginY(Uuid),
    SheetX,
    SheetY,
    SheetWidth,
    SheetHeight,
    /// A setting of the board's panel (#2410).
    Panel(PanelField),
}

/// Which of the board's components a Board Information count selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentSide {
    All,
    Top,
    Bottom,
}

/// What the user did in the board Properties panel.
#[derive(Debug, Clone, PartialEq)]
pub enum PcbBoardMsg {
    TabPicked(BoardTab),
    SearchChanged(String),
    /// The header's filter button: open or close the Active Bar's Selection
    /// Filter dropdown.
    FilterMenuToggled,
    /// A Selection Filter chip.
    FilterKindToggled(PickKind),
    /// The `All - On` chip: every kind on, or off when all are on.
    FilterAllToggled,
    /// The filter dropdown's All (`true`) or None (`false`) cell.
    FilterAllSet(bool),
    /// Save the chips that are on as a new preset (#2383).
    FilterPresetAdded,
    /// A preset was clicked: its kinds become the filter, and it is the one
    /// the panel edits.
    FilterPresetApplied(usize),
    FilterPresetRenamed(usize, String),
    /// Save the chips that are on into this preset.
    FilterPresetUpdated(usize),
    FilterPresetRemoved(usize),
    SnapToGridsToggled,
    SnapScopePicked(SnapScope),
    SnapObjectToggled(SnapObject),
    SnapDistancePicked(SnapDistance),
    /// A components count was clicked: select those components.
    ComponentsSelected(ComponentSide),
    /// The DRC violations count was clicked.
    DrcResultsOpened,
    /// The Reports button: Board Reports opens on the BOM (#2404).
    ReportsOpened,
    /// The pointer entered or left a field row or a part of the board
    /// illustration (#2523).
    IllustrationHovered(IllustrationMsg),
    /// Edit board shape…: runs `board_sketch`, as Design ▸ Board Sketch
    /// does (#2523; the board size follows the outline).
    BoardShapeEditPressed,
    /// Other's board materials link: runs `board_layer_stack`, where the
    /// board's materials and finish live (#2523).
    LayerStackOpened,
    GridStepTyped(String),
    /// Enter in the grid step field: apply the typed step.
    GridStepSubmitted,
    SnapToGuidesToggled,
    SnapToAxesToggled,
    /// Text typed in one of the panel's fields (#2374).
    FieldTyped(BoardField, String),
    /// Enter in that field: apply what was typed.
    FieldSubmitted(BoardField),
    /// The Guide Manager's Add: a guide of this kind through the origin.
    GuideAdded(GuideKind),
    /// The Guide Manager's Place: the next click on the board sets a guide
    /// of this kind there.
    GuidePlaceArmed(GuideKind),
    GuideEnabledToggled(Uuid),
    /// The guide's swatch was clicked: its next colour.
    GuideColorStepped(Uuid),
    /// The guide card's ⋯ menu Delete.
    GuideDeleted(Uuid),
    /// Open or close a guide card's ⋯ menu.
    GuideMenuToggled(Uuid),
    /// Open or close the Add Guide dropdown.
    GuideAddMenuToggled,
    /// The Add Guide dropdown's Place on Board row.
    GuidePlaceModeToggled,
    /// The Grid Manager's Add: a new grid at the top priority.
    GridAdded,
    /// Open or close a grid card's ⋯ menu.
    GridMenuToggled(GridCard),
    /// The ⋯ menu's Edit: show or hide the card's fields.
    GridEditToggled(GridCard),
    /// The ⋯ menu's Move Up or Move Down: one place in priority
    /// (`true`: up).
    GridMoved(Uuid, bool),
    /// The ⋯ menu's Delete.
    GridDeleted(Uuid),
    GridComponentsToggled(Uuid),
    GridNonComponentsToggled(Uuid),
    GridColorStepped(Uuid),
    PolygonNamingPicked(PolygonNaming),
    DesignatorDisplayPicked(DesignatorDisplay),
    /// A Fast Grids button: set the board's snap step to that fast grid.
    FastGridApplied(usize),
    /// Get Size From Sheet Layer.
    SheetFromBoardToggled,
    /// The Panel section's Panelize Board (#2410): runs `panelize_board`.
    PanelizeClicked,
    /// Its Remove Panel: runs `remove_board_panel`.
    PanelRemoveClicked,
    /// Its Mouse Bites | V-Score control.
    PanelSeparationPicked(PanelSeparation),
}

/// The board facts and editor settings the panel shows.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PcbBoardSummary {
    /// The document's name, as its tab shows it.
    pub name: String,
    /// Width and height of the outline's extent in mm; `None` without an
    /// outline.
    pub size_mm: Option<(f64, f64)>,
    /// The outline's extent in model mm, `(min x, min y, max x, max y)`,
    /// y down; `None` without an outline.
    pub extent_mm: Option<(f64, f64, f64, f64)>,
    /// The outline is an axis-aligned rectangle, so its extent is its size;
    /// otherwise the size row reads as a bounding box.
    pub outline_is_rectangle: bool,
    pub copper_layers: usize,
    pub layers: usize,
    pub thickness_mm: f64,
    /// The board's totals.
    pub stats: BoardStats,
    /// The board and revision `stats` were counted for; a refresh at the
    /// same revision reuses them.
    pub stats_key: Option<(PathBuf, u64)>,
    /// Nets with a connection left to route; `None` until the ratsnest of
    /// this revision is worked out.
    pub unrouted_nets: Option<usize>,
    /// Nets whose ratsnest is hidden.
    pub hidden_nets: usize,
    /// The snap grid step in use, mm; `None` when the board has no setup
    /// to hold one.
    pub grid_mm: Option<f64>,
    /// The board's guides, extra grids, sheet and naming options (#2374).
    pub options: BoardOptions,
    /// What the board's panel comes to (#2410); `None` without a panel,
    /// the reason when it does not lay out.
    pub panel: Option<Result<PanelFacts, String>>,
    /// The board opened read-only (a newer format).
    pub read_only: bool,
    pub filter: PickFilter,
    /// The filter's saved presets and the one the panel edits (#2383).
    pub presets: Vec<crate::active_bar::CustomFilterPreset<PickKind>>,
    pub preset_tab: usize,
    pub snap: PcbSnapOptions,
    /// The fast grid preferences (#1672) the Grid Manager offers.
    pub fast_grids_mm: [f32; 2],
    pub view: PcbBoardPanelView,
    /// Board items selected in the main window.
    pub selected: usize,
    /// The one selected footprint, when the selection is exactly one.
    pub footprint: Option<super::PcbFootprintFacts>,
    /// The selected footprints, when the selection holds nothing else.
    pub component: Option<super::PcbComponentFacts>,
    /// The selected tracks, vias or pads, when the selection is only those
    /// of one kind (#2355).
    pub primitive: Option<super::PcbPrimitiveFacts>,
    /// The one selected room (#2457).
    pub room: Option<super::pcb_room_properties::RoomFacts>,
}

impl PcbBoardSummary {
    pub fn of(board: &PcbBoard, name: String, selected: usize) -> Self {
        Self::with_stats(board, name, selected, BoardStats::of(board))
    }

    /// The summary of `board` with totals already counted.
    pub fn with_stats(board: &PcbBoard, name: String, selected: usize, stats: BoardStats) -> Self {
        let extent_mm = board.outline.split_first().map(|(first, rest)| {
            let (mut lo, mut hi) = ((first.x, first.y), (first.x, first.y));
            for p in rest {
                lo = (lo.0.min(p.x), lo.1.min(p.y));
                hi = (hi.0.max(p.x), hi.1.max(p.y));
            }
            (lo.0, lo.1, hi.0, hi.1)
        });
        let size_mm = extent_mm.map(|(x0, y0, x1, y1)| (x1 - x0, y1 - y0));
        Self {
            name,
            size_mm,
            extent_mm,
            outline_is_rectangle: extent_mm
                .is_some_and(|extent| is_rectangle(&board.outline, extent)),
            copper_layers: signex_renderer::pcb::board_copper_layers(&board.layers).len(),
            layers: board.layers.len(),
            thickness_mm: board.thickness,
            stats,
            grid_mm: board
                .setup
                .as_ref()
                .map(|_| signex_engine::pcb::placement::grid_step_mm(board)),
            options: board.options.clone(),
            panel: PanelFacts::of(board),
            read_only: board.unknown.newer_minor_source.is_some(),
            selected,
            ..Self::default()
        }
    }

    /// This summary showing `footprint`'s properties.
    pub fn with_footprint(self, footprint: Option<super::PcbFootprintFacts>) -> Self {
        Self { footprint, ..self }
    }

    /// This summary showing the selected components.
    pub fn with_component(self, component: Option<super::PcbComponentFacts>) -> Self {
        Self { component, ..self }
    }

    /// This summary showing the selected tracks', vias' or pads' panel.
    pub fn with_primitive(self, primitive: Option<super::PcbPrimitiveFacts>) -> Self {
        Self { primitive, ..self }
    }

    /// This summary showing the selected room (#2457).
    pub fn with_room(self, room: Option<super::pcb_room_properties::RoomFacts>) -> Self {
        Self { room, ..self }
    }
}

/// Whether `outline` is the rectangle `extent`: four corners, each on a
/// corner of the extent (a repeated closing vertex is ignored).
fn is_rectangle(outline: &[signex_types::pcb::Point], extent: (f64, f64, f64, f64)) -> bool {
    // One nanometre, the file's resolution.
    const SAME: f64 = 1e-6;
    let ring = match outline {
        [first, .., last] if first == last => &outline[..outline.len() - 1],
        _ => outline,
    };
    let (x0, y0, x1, y1) = extent;
    let on = |v: f64, a: f64, b: f64| (v - a).abs() < SAME || (v - b).abs() < SAME;
    ring.len() == 4 && ring.iter().all(|p| on(p.x, x0, x1) && on(p.y, y0, y1))
}

/// The unit the panel shows and reads lengths in: mm or mil, as its Units
/// toggle offers; any other unit reads as mm.
pub fn shown_unit(unit: Unit) -> Unit {
    match unit {
        Unit::Mil => Unit::Mil,
        _ => Unit::Mm,
    }
}

/// The panel body for `board`.
pub fn view<'a>(ctx: &'a PanelContext, board: &'a PcbBoardSummary) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    if let Some(room) = board.room.as_ref() {
        return super::pcb_room_properties::view(ctx, room);
    }
    if let Some(component) = board.component.as_ref() {
        return super::pcb_component::view(ctx, component, board.footprint.as_ref());
    }
    if let Some(primitive) = board.primitive.as_ref() {
        return super::pcb_primitive_properties::view(ctx, primitive);
    }
    if board.selected > 0 {
        return container(super::empty_state::empty_state_card(
            &ctx.tokens,
            t!(i18n, ids::PCB_PROPERTIES_TITLE),
            t!(
                i18n, ids::PCB_PROPERTIES_SELECTION,
                "count" => board.selected, "board" => board.name.clone()
            ),
            Some(t!(i18n, ids::PCB_PROPERTIES_SELECTION_HINT)),
        ))
        .padding(SP.m)
        .width(Length::Fill)
        .boxed();
    }
    let form = Form::new(ctx, &board.view.search, &board.view.illustration_hover);
    // One order on every PCB Properties panel (#2523, mockup row 93): the
    // header row and the tabs, Search, the illustration, then the sections.
    let mut col: Column<Element<'a, PanelMsg>> = Column::new()
        .width(Length::Fill)
        .push(header::view(&form, board));
    if board.view.tab == BoardTab::HealthCheck {
        return col.push(health::view(&form)).boxed();
    }
    col = col
        .push(header::search(&form, board))
        .push(drawing::view(&form, board));
    let sections = [
        filter::section(&form, board),
        snap::section(&form, board),
        info::section(&form, board),
        grids::section(&form, board),
        guides::section(&form, board),
        panel::section(&form, board),
        other::section(&form, board),
    ];
    let mut shown = 0;
    for section in sections.into_iter().flatten() {
        col = col.push(section);
        shown += 1;
    }
    if shown == 0 {
        col = col.push(form.note(t!(i18n, ids::PCB_BOARD_SEARCH_EMPTY)));
    }
    col.boxed()
}

#[cfg(test)]
mod tests;
