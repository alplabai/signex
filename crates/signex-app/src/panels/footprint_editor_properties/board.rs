//! The Board sections of board sketch mode: with nothing selected, the
//! board's name and outline facts (#2356); for a selected entity, board
//! outline, mounting hole and the hole's drill, plating and net (#2234).
//! Shown only while a board is in sketch mode; keepouts and cutouts are the
//! Role picker's.

use iced::widget::{Column, container};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_types::coord::Unit;
use signex_types::measure::length_field_text;
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

// Declared here, not in the folder's `mod.rs`: the section ships with the
// Board summary it follows.
#[path = "outline_checks.rs"]
mod outline_checks;

// The Slot, Pour, Mechanical layer and Component anchor rows (decision 2).
#[path = "board_targets.rs"]
mod board_targets;

/// The Board section with nothing selected (#2356): the board's name, its
/// outline's size and area, and the sketch's counts. It stands where a
/// footprint shows its own fields, none of which apply to a board. The
/// Outline checks section follows it (mockup row 179d).
pub(super) fn view_board_summary<'a>(
    col: Column<Element<'a, PanelMsg>>,
    fp: &'a FootprintEditorPanelContext,
    board: &'a BoardSketchProps,
    palette: PanelPalette,
    collapsed_sections: &'a CollapsedSections,
    unit: Unit,
    i18n: &I18n,
) -> Column<Element<'a, PanelMsg>> {
    let col = view_board_facts(col, fp, board, palette, collapsed_sections, unit, i18n);
    outline_checks::view_outline_checks(col, board, palette, collapsed_sections, i18n)
}

fn view_board_facts<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    fp: &'a FootprintEditorPanelContext,
    board: &'a BoardSketchProps,
    palette: PanelPalette,
    collapsed_sections: &'a CollapsedSections,
    unit: Unit,
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
    let mut rows = vec![
        (ids::BOARD_SKETCH_NAME, board.board_name.clone()),
        (ids::PCB_PROPERTIES_SIZE, size_value(board, unit, i18n)),
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

/// The Size row's value, worded as Board Properties words it: "W × H" for a
/// rectangle, "Bounding box W × H" for any other outline, in the shown unit,
/// with "· from board shape" while the sketch's closed outline sets it
/// (mockup row 179e), or that there is no outline.
fn size_value(board: &BoardSketchProps, unit: Unit, i18n: &I18n) -> String {
    let Some((w, h)) = board.board_size_mm else {
        return t!(i18n, ids::PCB_PROPERTIES_NO_OUTLINE).into_string();
    };
    let unit = crate::panels::pcb_board_properties::shown_unit(unit);
    let id = if board.outline_is_rectangle {
        ids::BOARD_SKETCH_SIZE_VALUE
    } else {
        ids::PCB_BOARD_SIZE_BOUNDING
    };
    let size = t!(
        i18n,
        id,
        "width" => length_field_text(w, unit),
        "height" => length_field_text(h, unit),
        "unit" => unit.to_string()
    )
    .into_string();
    if !board.size_from_shape {
        return size;
    }
    let source = t!(i18n, ids::PCB_BOARD_SIZE_SOURCE).into_string();
    format!("{size} {source}")
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
    col = board_targets::view_board_targets(col, id, &board.target, palette, i18n);
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

#[cfg(test)]
mod tests {
    use signex_i18n::I18n;
    use signex_types::coord::Unit;

    use super::size_value;
    use crate::panels::BoardSketchProps;

    fn size_in(board: &BoardSketchProps, unit: Unit) -> String {
        size_value(board, unit, &I18n::en_us()).replace(['\u{2068}', '\u{2069}'], "")
    }

    fn size(board: &BoardSketchProps) -> String {
        size_in(board, Unit::Mm)
    }

    /// Mockup row 179e: the sketch's closed outline sets the size, read-only
    /// "from board shape", even before Bake gives the board an outline.
    #[test]
    fn the_size_reads_from_the_board_shape_while_the_sketch_owns_the_outline() {
        let shape = BoardSketchProps {
            board_size_mm: Some((60.0, 40.0)),
            size_from_shape: true,
            outline_is_rectangle: true,
            ..BoardSketchProps::default()
        };
        assert_eq!(size(&shape), "60.000 × 40.000 mm · from board shape");
        let board = BoardSketchProps {
            size_from_shape: false,
            ..shape.clone()
        };
        assert_eq!(size(&board), "60.000 × 40.000 mm");
        assert_eq!(size(&BoardSketchProps::default()), "No outline");
    }

    /// The Size row words a shape as Board Properties does: an outline that
    /// is no rectangle reads as its bounding box, in the shown unit, with the
    /// same "· from board shape" source text.
    #[test]
    fn the_size_follows_the_board_properties_wording_and_unit() {
        let shape = BoardSketchProps {
            board_size_mm: Some((60.0, 40.0)),
            size_from_shape: true,
            outline_is_rectangle: false,
            ..BoardSketchProps::default()
        };
        assert_eq!(
            size(&shape),
            "Bounding box 60.000 × 40.000 mm · from board shape"
        );
        let rectangle = BoardSketchProps {
            board_size_mm: Some((25.4, 12.7)),
            outline_is_rectangle: true,
            ..shape
        };
        assert_eq!(
            size_in(&rectangle, Unit::Mil),
            "1000.00 × 500.00 mil · from board shape"
        );
    }
}
