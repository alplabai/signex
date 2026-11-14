//! Properties panel of the active PCB (#2353): with nothing selected, the
//! board itself (a header with search and tabs, then Selection Filter, Snap
//! Options, Board Information, Grid Manager, Guide Manager and Other); the
//! Component view when only footprints are selected (`pcb_component`); the
//! track, via or pad panel when only one such kind is selected
//! (`pcb_primitive_properties`); a card
//! naming the board for any other selection. A pure view over data the panel context copies from the board
//! and the editor settings.
//!
//! What the board model cannot store, or the editor cannot act on, yet is
//! drawn disabled with a tooltip saying why.

mod chrome;
mod filter;
mod grids;
mod header;
mod health;
mod info;
mod other;
mod snap;

// The PCB Active Bar's Selection Filter names the kinds as the chips do.
pub(crate) use filter::kind_label as pick_kind_label;

use std::path::PathBuf;

use iced::widget::{Column, container};
use iced::{Element, Length, Widget as _};
use pcb_geom::{BoardStats, PickFilter, PickKind};
use signex_engine::pcb::snap::SnapObject;
use signex_i18n::{ids, t};
use signex_types::coord::Unit;
use signex_types::pcb::PcbBoard;

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
    /// The header's Selection Filter menu is open.
    pub filter_menu_open: bool,
    /// The snap grid step being typed, until Enter applies it.
    pub grid_draft: Option<String>,
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
    FilterMenuToggled,
    /// A Selection Filter chip.
    FilterKindToggled(PickKind),
    /// The `All - On` chip: every kind on, or off when all are on.
    FilterAllToggled,
    SnapToGridsToggled,
    SnapScopePicked(SnapScope),
    SnapObjectToggled(SnapObject),
    SnapDistancePicked(SnapDistance),
    /// A components count was clicked: select those components.
    ComponentsSelected(ComponentSide),
    /// The DRC violations count was clicked.
    DrcResultsOpened,
    GridStepTyped(String),
    /// Enter in the grid step field: apply the typed step.
    GridStepSubmitted,
}

/// The board facts and editor settings the panel shows.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PcbBoardSummary {
    /// The document's name, as its tab shows it.
    pub name: String,
    /// Width and height of the outline's extent in mm; `None` without an
    /// outline.
    pub size_mm: Option<(f64, f64)>,
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
    /// The board opened read-only (a newer format).
    pub read_only: bool,
    pub filter: PickFilter,
    pub snap: PcbSnapOptions,
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
}

impl PcbBoardSummary {
    pub fn of(board: &PcbBoard, name: String, selected: usize) -> Self {
        Self::with_stats(board, name, selected, BoardStats::of(board))
    }

    /// The summary of `board` with totals already counted.
    pub fn with_stats(board: &PcbBoard, name: String, selected: usize, stats: BoardStats) -> Self {
        let size_mm = board.outline.split_first().map(|(first, rest)| {
            let (mut lo, mut hi) = ((first.x, first.y), (first.x, first.y));
            for p in rest {
                lo = (lo.0.min(p.x), lo.1.min(p.y));
                hi = (hi.0.max(p.x), hi.1.max(p.y));
            }
            (hi.0 - lo.0, hi.1 - lo.1)
        });
        Self {
            name,
            size_mm,
            copper_layers: signex_renderer::pcb::board_copper_layers(&board.layers).len(),
            layers: board.layers.len(),
            thickness_mm: board.thickness,
            stats,
            grid_mm: board
                .setup
                .as_ref()
                .map(|_| signex_engine::pcb::placement::grid_step_mm(board)),
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
    let form = Form::new(ctx, &board.view.search);
    let mut col: Column<Element<'a, PanelMsg>> = Column::new()
        .width(Length::Fill)
        .push(header::view(&form, board));
    if board.view.tab == BoardTab::HealthCheck {
        return col.push(health::view(&form)).boxed();
    }
    let sections = [
        filter::section(&form, board),
        snap::section(&form, board),
        info::section(&form, board),
        grids::grid_section(&form, board),
        grids::guide_section(&form),
        other::section(&form),
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
