//! The footprint Properties body for every state but a selected pad in Pads
//! mode and pad placement (mockup row 124): its own object header, then the body.
//!
//! - Nothing selected: the pad stack check, then the Footprint, Snap & Grid
//!   and sketch sections in one scroll.
//! - A sketch entity: its role sub-form or header, the Entity section, then
//!   Snap Options and the sketch sections as before.
//! - A silk graphic: the silk form, then Snap Options.
//! - A board sketch with nothing selected: the board summary and the
//!   sections the board showed before (#2234; its redesign is mockup row 110).
//!
//! Un-scrolled: the parent wraps the column in the scrollable.

mod footprint_illustration;
mod footprint_sections;
mod header;
mod sketch_sections;
mod snap_illustration;
mod snap_sections;

#[cfg(test)]
mod tests;

use iced::widget::Column;
use iced::{Element, Length};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::Label;

use super::super::{
    BoardSketchProps, CollapsedSections, FootprintEditorPanelContext, FootprintModeKind,
    FootprintPadSummary, FootprintSelectedSilkSummary, FootprintSketchEntitySummary, PanelMsg,
    PanelPalette,
};
use crate::styles::PillTone;

pub(super) use header::{
    banner, caption, check_cell, check_grid, full_width_segment, gated, ghost, is_open, kv_row,
    mini_head, object_header, pill, rule, section,
};

/// The field ids that link the illustrations to their rows.
pub(super) mod fields {
    use signex_widgets::property_illustration::FieldId;

    /// The generated courtyard (its enable header and its outline).
    pub const COURTYARD: FieldId = FieldId::new("fp.courtyard");
    pub const EXPANSION: FieldId = FieldId::new("fp.courtyard.expansion");
    /// Snapping is not Off.
    pub const SNAPPING_ON: FieldId = FieldId::new("fp.snap.on");
    pub const AXES: FieldId = FieldId::new("fp.snap.axes");
    pub const SNAP_DISTANCE: FieldId = FieldId::new("fp.snap.distance");
    pub const AXIS_SNAP_RANGE: FieldId = FieldId::new("fp.snap.axis-range");
    /// The active grid's step.
    pub const GRID_STEP: FieldId = FieldId::new("fp.snap.grid-step");
}

/// What the body draws from: the panel context and the panel's look.
#[derive(Clone, Copy)]
pub(super) struct Ctx<'a, 'i> {
    pub fp: &'a FootprintEditorPanelContext,
    pub palette: PanelPalette,
    pub tokens: &'a ThemeTokens,
    pub collapsed: &'a CollapsedSections,
    pub unit: signex_types::coord::Unit,
    pub i18n: &'i I18n,
}

/// Which body the panel shows.
#[derive(Debug, Clone, Copy)]
enum BodyState<'a> {
    SketchEntity(&'a FootprintSketchEntitySummary),
    SelectedPad(&'a FootprintPadSummary),
    Silk(&'a FootprintSelectedSilkSummary),
    Board(&'a BoardSketchProps),
    NothingSelected,
}

fn body_state(fp: &FootprintEditorPanelContext) -> BodyState<'_> {
    if fp.mode_kind == FootprintModeKind::Sketch
        && let Some(entity) = fp.selected_sketch_entity.as_ref()
    {
        return BodyState::SketchEntity(entity);
    }
    if fp.mode_kind == FootprintModeKind::Pads
        && !fp.placement_active
        && !fp.placement_paused
        && let Some(pad) = fp.selected_pad.as_ref()
    {
        return BodyState::SelectedPad(pad);
    }
    if let Some(silk) = fp.selected_silk_summary.as_ref() {
        return BodyState::Silk(silk);
    }
    if let Some(board) = fp.board.as_ref() {
        return BodyState::Board(board);
    }
    BodyState::NothingSelected
}

/// The header and body of the footprint Properties panel.
pub(super) fn view_body<'a>(
    fp: &'a FootprintEditorPanelContext,
    palette: PanelPalette,
    tokens: &'a ThemeTokens,
    collapsed_sections: &'a CollapsedSections,
    unit: signex_types::coord::Unit,
    i18n: &signex_i18n::I18n,
) -> Column<Element<'a, PanelMsg>> {
    let cx = Ctx {
        fp,
        palette,
        tokens,
        collapsed: collapsed_sections,
        unit,
        i18n,
    };
    let col = Column::new().spacing(0).width(Length::Fill);
    match body_state(fp) {
        BodyState::SketchEntity(entity) => {
            let col = super::selection::view_sketch_entity(col, cx, entity);
            selection_tail(col, cx)
        }
        BodyState::SelectedPad(pad) => super::selection::view_selected_pad(col, cx, pad),
        BodyState::Silk(silk) => {
            let col = super::silk::view_silk_selection(col, cx, silk);
            selection_tail(col, cx)
        }
        BodyState::Board(board) => view_board(col, cx, board),
        BodyState::NothingSelected => view_nothing_selected(col, cx),
    }
}

/// Below a selection: the board section, the pad stack check, Snap Options
/// and, in Sketch mode, the sketch sections, as they showed before.
fn selection_tail<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
) -> Column<Element<'a, PanelMsg>> {
    col = super::board::view_board_section(col, cx.fp, cx.palette, cx.i18n);
    col = super::pad_stack_check::view_pad_stack_check(col, cx.fp, cx.i18n);
    col = super::snap_options::view_snap_options(col, cx, true);
    if cx.fp.mode_kind == FootprintModeKind::Sketch {
        col = super::sections::view_sketch_sections(col, cx);
    }
    col.push(hint(cx))
}

/// A board sketch with nothing selected: the board summary, then the
/// sections the board showed before (#2234).
fn view_board<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
    board: &'a BoardSketchProps,
) -> Column<Element<'a, PanelMsg>> {
    let i18n = cx.i18n;
    col = col.push(object_header(
        t!(i18n, ids::BOARD_SKETCH_SECTION),
        Some(t!(i18n, ids::BOARD_SKETCH_TITLE).into()),
        cx.tokens,
    ));
    col = super::board::view_board_summary(col, cx.fp, board, cx.palette, cx.collapsed, i18n);
    col = super::pad_stack_check::view_pad_stack_check(col, cx.fp, cx.i18n);
    col = super::snap_options::view_snap_options(col, cx, false);
    col = super::managers::view_grid_manager(col, cx);
    col = col.push(footprint_sections::units_row(cx));
    if cx.fp.mode_kind == FootprintModeKind::Sketch {
        col = super::sections::view_sketch_sections(col, cx);
    }
    col.push(hint(cx))
}

/// Nothing selected: the Footprint header, the pad stack check, then every
/// section in one scroll (Footprint, Snap & Grid, Sketch), each
/// collapsible: one scroll, not pages (Caner, 2026-11-20, mockup row 124).
fn view_nothing_selected<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
) -> Column<Element<'a, PanelMsg>> {
    let i18n = cx.i18n;
    col = col.push(object_header(
        t!(i18n, ids::EDITORS_FP_PROPS_TITLE),
        Some(footprint_status(cx.fp, i18n)),
        cx.tokens,
    ));
    col = super::pad_stack_check::view_pad_stack_check(col, cx.fp, i18n);
    col = footprint_sections::view(col, cx);
    col = snap_sections::view(col, cx);
    col = sketch_sections::view(col, cx);
    col.push(hint(cx))
}

/// "8 pads", and the solve state once the sketch was solved.
fn footprint_status(fp: &FootprintEditorPanelContext, i18n: &I18n) -> Label<'static> {
    let pads = fp.editor_pad_count;
    if fp.sketch_state.tone == PillTone::Neutral {
        return t!(i18n, ids::EDITORS_FP_PROPS_STATUS, "pads" => pads).into();
    }
    t!(
        i18n,
        ids::EDITORS_FP_PROPS_STATUS_SOLVED,
        "pads" => pads,
        "state" => t!(i18n, fp.sketch_state.label).into_string()
    )
    .into()
}

/// The hint at the bottom of the panel, by editor mode.
fn hint<'a>(cx: Ctx<'a, '_>) -> Element<'a, PanelMsg> {
    let id = match cx.fp.mode_kind {
        FootprintModeKind::Pads => ids::EDITORS_FP_PROPS_HINT_PADS,
        FootprintModeKind::Sketch => ids::EDITORS_FP_PROPS_HINT_SKETCH,
        FootprintModeKind::View3d => ids::EDITORS_FP_PROPS_HINT_VIEW3D,
    };
    caption(t!(cx.i18n, id), cx.tokens)
}
