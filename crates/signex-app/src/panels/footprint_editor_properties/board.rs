//! The Board section of a board sketch entity (#2234): board outline,
//! mounting hole and the hole's drill, plating and net. Shown only while a
//! board is in sketch mode; keepouts and cutouts are the Role picker's.

use iced::widget::{Column, container};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_widgets::text;

use super::super::{BoardSketchAttrMsg, FootprintEditorPanelContext, PanelMsg, PanelPalette};
use crate::panels::{form_check_row, form_edit_row, form_label};
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

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
            &t!(i18n, ids::BOARD_SKETCH_OUTLINE).into_string(),
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
            &t!(i18n, ids::BOARD_SKETCH_HOLE).into_string(),
            is_hole,
            edit(M::MountingHoleToggled { id, on: !is_hole }),
            muted,
            surface,
        ));
    }
    if let Some(hole) = board.hole.as_ref() {
        col = col.push(form_edit_row(
            &t!(i18n, ids::BOARD_SKETCH_HOLE_DRILL).into_string(),
            &hole.drill,
            muted,
            move |value| PanelMsg::BoardSketch(M::HoleDrillEdited { id, value }),
        ));
        col = col.push(form_check_row(
            &t!(i18n, ids::BOARD_SKETCH_HOLE_PLATED).into_string(),
            hole.plated,
            edit(M::HolePlatedToggled {
                id,
                on: !hole.plated,
            }),
            muted,
            surface,
        ));
        col = col.push(form_edit_row(
            &t!(i18n, ids::BOARD_SKETCH_HOLE_NET).into_string(),
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
