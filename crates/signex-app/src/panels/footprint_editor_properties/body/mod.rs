//! The footprint Properties body for every state but a selected pad in Pads
//! mode and pad placement (mockup row 124): its own object header, then the body.
//!
//! - Nothing selected: the pad stack check, then the Footprint, Snap & Grid
//!   and sketch sections in one scroll.
//! - A sketch entity: its role sub-form or header, the Entity section, then
//!   Snap Options and the sketch sections as before.
//! - A silk graphic: the silk form, then Snap Options.
//! - A board sketch with nothing selected: the Sketch DOF, Parameters and
//!   Bake as sections, the board summary and the sections the board showed
//!   before (#2234; its redesign is mockup row 110). Under a board
//!   selection, the Sketch DOF section follows the selection's header and
//!   Parameters and Bake as its section (mockup row 179).
//! - A board sketch with its bake preview open: the Bake panel (mockup row
//!   179), above any selection. A footprint sketch's open preview shows its
//!   Bake panel the same way (mockup row 190).
//!
//! Un-scrolled: the parent wraps the column in the scrollable.

mod footprint_illustration;
mod footprint_sections;
mod generated_sections;
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
    BoardBakePreview, BoardSketchProps, CollapsedSections, FootprintEditorPanelContext,
    FootprintModeKind, FootprintPadSummary, FootprintSelectedSilkSummary,
    FootprintSketchEntitySummary, PanelMsg, PanelPalette,
};
use crate::library::editor::standalone::sketch_exit::FootprintBakeCard;
use crate::styles::PillTone;

pub(super) use header::{
    banner, caption, check_cell, check_grid, full_width_segment, gated, ghost, is_open, kv_row,
    link, mini_head, object_header, pill, rule, section, section_shown, section_toggle,
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
    /// A board sketch with its bake preview open (mockup row 179): the Bake
    /// panel, whatever is selected; a selection only highlights on the canvas.
    Bake(&'a BoardSketchProps, &'a BoardBakePreview),
    /// A footprint sketch with its bake preview open (mockup row 190): the
    /// Bake panel with the card, whatever is selected.
    FootprintBake(&'a FootprintBakeCard),
    SketchEntity(&'a FootprintSketchEntitySummary),
    SelectedPad(&'a FootprintPadSummary),
    Silk(&'a FootprintSelectedSilkSummary),
    Board(&'a BoardSketchProps),
    NothingSelected,
}

fn body_state(fp: &FootprintEditorPanelContext) -> BodyState<'_> {
    if fp.mode_kind == FootprintModeKind::Sketch
        && let Some(board) = fp.board.as_ref()
        && let Some(preview) = board.bake.as_ref()
    {
        return BodyState::Bake(board, preview);
    }
    if fp.mode_kind == FootprintModeKind::Sketch
        && let Some(card) = fp.footprint_bake.as_ref()
    {
        return BodyState::FootprintBake(card);
    }
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
    let col = match fp.bake_outcome.as_ref() {
        Some(outcome) => super::bake_outcome::view_bake_outcome(col, cx, outcome),
        None => col,
    };
    match body_state(fp) {
        BodyState::Bake(board, preview) => {
            let col = super::bake_panel::view_bake_panel(col, cx, board, preview);
            // Bake as: one role picker per closed profile (mockup row 179).
            let col = super::roles::view_board_bake_as(col, cx);
            super::bake_panel::view_bake_panel_tail(col, cx)
        }
        BodyState::FootprintBake(card) => {
            super::bake_panel::view_footprint_bake_panel(col, cx, card)
        }
        BodyState::SketchEntity(entity) => {
            let col = super::selection::view_sketch_entity(col, cx, entity);
            selection_tail(col, cx, super::sections::leads_with_solve(cx))
        }
        BodyState::SelectedPad(pad) => super::selection::view_selected_pad(col, cx, pad),
        BodyState::Silk(silk) => {
            let col = super::silk::view_silk_selection(col, cx, silk);
            selection_tail(col, cx, false)
        }
        BodyState::Board(board) => view_board(col, cx, board),
        BodyState::NothingSelected => view_nothing_selected(col, cx),
    }
}

/// Below a selection: the board section, the pad stack check, Snap Options
/// and, in Sketch mode, the sketch sections, as they showed before. When the
/// selection already led with the solve section (`solve_shown`, a board
/// sketch), Parameters and Bake as come first and only Solve warnings stays
/// at the bottom (mockup row 179).
fn selection_tail<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
    solve_shown: bool,
) -> Column<Element<'a, PanelMsg>> {
    if solve_shown {
        col = super::sections::view_board_parameters_and_bake_as(col, cx);
    }
    col = super::board::view_board_section(col, cx.fp, cx.palette, cx.i18n);
    col = super::pad_stack_check::view_pad_stack_check(col, cx.fp, cx.i18n);
    col = super::snap_options::view_snap_options(col, cx, true);
    col = sketch_sections_below(col, cx, solve_shown);
    col.push(hint(cx))
}

/// In Sketch mode, the sketch sections; only Solve warnings when the body
/// already showed the solve section, Parameters and Bake as near the top.
fn sketch_sections_below<'a>(
    col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
    solve_shown: bool,
) -> Column<Element<'a, PanelMsg>> {
    match (cx.fp.mode_kind == FootprintModeKind::Sketch, solve_shown) {
        (false, _) => col,
        (true, true) => super::sections::view_sketch_warnings(col, cx),
        (true, false) => super::sections::view_sketch_sections(col, cx),
    }
}

/// A board sketch with nothing selected: the board header, a failing
/// Outline checks section (mockup row 179d), then the Sketch DOF, Parameters
/// and Bake as sections (mockup row 179), the board summary, then the
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
    // A failing outline check and its fix lead (mockup row 179d).
    col = super::board::view_failing_outline_checks(col, board, cx.palette, cx.collapsed, i18n);
    let leads = super::sections::leads_with_solve(cx);
    if leads {
        col = super::sections::view_sketch_solve_section(col, cx);
        col = super::sections::view_board_parameters_and_bake_as(col, cx);
    }
    col = super::board::view_board_summary(
        col,
        cx.fp,
        board,
        cx.palette,
        cx.collapsed,
        cx.unit,
        i18n,
    );
    col = super::pad_stack_check::view_pad_stack_check(col, cx.fp, cx.i18n);
    col = super::snap_options::view_snap_options(col, cx, false);
    col = super::managers::view_grid_manager(col, cx);
    col = col.push(footprint_sections::units_row(cx));
    col = sketch_sections_below(col, cx, leads);
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
    col = super::roles::view_roles(col, cx);
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
