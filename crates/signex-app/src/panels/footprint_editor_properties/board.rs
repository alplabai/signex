//! The Board sections of board sketch mode: with nothing selected, the
//! board's name and outline facts (#2356); for a selected entity, board
//! outline, mounting hole and the hole's drill, plating and net (#2234).
//! Shown only while a board is in sketch mode; keepouts and cutouts are the
//! Role picker's.

use iced::widget::{Column, container};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_widgets::text;

use super::super::{
    BoardSketchAttrMsg, BoardSketchProps, CollapsedSections, FootprintEditorPanelContext, PanelMsg,
    PanelPalette,
};
use super::rows::value_row;
use crate::panels::{
    collapsible_section_header, form_check_row, form_edit_row, form_label, is_section_collapsed,
};
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The Board section with nothing selected (#2356): the board's name, its
/// outline's size and area, and the sketch's counts. It stands where a
/// footprint shows its own fields, none of which apply to a board.
pub(super) fn view_board_summary<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    fp: &'a FootprintEditorPanelContext,
    board: &'a BoardSketchProps,
    palette: PanelPalette,
    collapsed_sections: &'a CollapsedSections,
    i18n: &I18n,
) -> Column<Element<'a, PanelMsg>> {
    col = col.push(
        collapsible_section_header(
            "fp_board",
            &t!(i18n, ids::BOARD_SKETCH_SECTION).into_string(),
            collapsed_sections,
            palette.primary,
            palette.border,
        )
        .boxed(),
    );
    if is_section_collapsed("fp_board", collapsed_sections) {
        return col;
    }
    let size = match board.board_size_mm {
        Some((w, h)) => t!(
            i18n, ids::PCB_PROPERTIES_SIZE_VALUE,
            "width" => format!("{w:.2}"), "height" => format!("{h:.2}")
        ),
        None => t!(i18n, ids::PCB_PROPERTIES_NO_OUTLINE),
    };
    let mut rows = vec![
        (ids::BOARD_SKETCH_NAME, board.board_name.clone()),
        (ids::PCB_PROPERTIES_SIZE, size.into_string()),
    ];
    if let Some(area) = board.board_area_mm2 {
        let area = t!(i18n, ids::BOARD_SKETCH_AREA_VALUE, "area" => format!("{area:.2}"));
        rows.push((ids::BOARD_SKETCH_AREA, area.into_string()));
    }
    rows.push((
        ids::BOARD_SKETCH_ENTITIES,
        i18n.count(fp.sketch_entity_count as i64),
    ));
    rows.push((
        ids::BOARD_SKETCH_CONSTRAINTS,
        i18n.count(fp.sketch_constraint_count as i64),
    ));
    for (key, value) in rows {
        col = col.push(value_row(
            &t!(i18n, key).into_string(),
            &value,
            palette.muted,
            palette.input_bg,
        ));
    }
    col
}

pub(super) fn view_board_section<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    fp: &'a FootprintEditorPanelContext,
    palette: PanelPalette,
    i18n: &I18n,
) -> Column<Element<'a, PanelMsg>> {
    use BoardSketchAttrMsg as M;
    let Some(board) = fp.board.as_ref() else {
        return col;
    };
    let Some(id) = board.selected else {
        return col;
    };
    let muted = palette.muted;
    let surface = palette.panel_bg;
    let edit = PanelMsg::BoardSketch;

    col = col.push(form_label(t!(i18n, ids::BOARD_SKETCH_SECTION)));
    if board.is_edge {
        col = col.push(form_check_row(
            t!(i18n, ids::BOARD_SKETCH_OUTLINE),
            board.outline,
            edit(M::OutlineToggled {
                id,
                on: !board.outline,
            }),
            muted,
            surface,
        ));
    }
    if board.is_circle {
        let is_hole = board.hole.is_some();
        col = col.push(form_check_row(
            t!(i18n, ids::BOARD_SKETCH_HOLE),
            is_hole,
            edit(M::MountingHoleToggled { id, on: !is_hole }),
            muted,
            surface,
        ));
    }
    if let Some(hole) = board.hole.as_ref() {
        col = col.push(form_edit_row(
            t!(i18n, ids::BOARD_SKETCH_HOLE_DRILL),
            &hole.drill,
            muted,
            move |value| PanelMsg::BoardSketch(M::HoleDrillEdited { id, value }),
        ));
        col = col.push(form_check_row(
            t!(i18n, ids::BOARD_SKETCH_HOLE_PLATED),
            hole.plated,
            edit(M::HolePlatedToggled {
                id,
                on: !hole.plated,
            }),
            muted,
            surface,
        ));
        col = col.push(form_edit_row(
            t!(i18n, ids::BOARD_SKETCH_HOLE_NET),
            &hole.net,
            muted,
            move |value| PanelMsg::BoardSketch(M::HoleNetEdited { id, value }),
        ));
    }
    col.push(
        container(
            text(t!(i18n, ids::BOARD_SKETCH_ROLES_HINT))
                .size(TS.caption)
                .color(muted),
        )
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .boxed(),
    )
}
