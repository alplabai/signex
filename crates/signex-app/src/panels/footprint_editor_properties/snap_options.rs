//! The Snap Options section: the Snapping scope (All Layers / Current
//! Layer / Off) across the panel, then the snap targets, the objects for
//! snapping and the distances, greyed and inert while snapping is Off.
//! Off short-circuits every snap priority in `snap::snap_cursor`.

use iced::widget::Column;
use iced::{Element, Length, Widget as _};
use signex_i18n::{MessageId, ids, t};
use signex_widgets::Label;

use super::super::{PanelMsg, SnapOptionFlag};
use super::body::{
    Ctx, check_cell, check_grid, fields, full_width_segment, gated, is_open, mini_head, section,
};
use super::illustration::linked_field;
use super::rows::{edit_row, edit_row_note};
use crate::library::editor::footprint::SnappingMode;

const KEY: &str = "fp_snap_options";

/// The snap targets: grids, guides and axes, each on or off on its own.
fn targets(cx: Ctx<'_, '_>) -> [(MessageId, SnapOptionFlag, bool); 3] {
    let opts = &cx.fp.snap_options;
    [
        (
            ids::EDITORS_FP_PROPS_SNAP_GRIDS,
            SnapOptionFlag::SnapToGrids,
            opts.snap_to_grids,
        ),
        (
            ids::EDITORS_FP_PROPS_SNAP_GUIDES,
            SnapOptionFlag::SnapToGuides,
            opts.snap_to_guides,
        ),
        (
            ids::EDITORS_FP_PROPS_SNAP_AXES,
            SnapOptionFlag::SnapToAxes,
            opts.snap_to_axes,
        ),
    ]
}

/// The objects the cursor snaps to, in the mockup's order, then the two it
/// does not show.
fn objects(cx: Ctx<'_, '_>) -> [(MessageId, SnapOptionFlag, bool); 12] {
    use SnapOptionFlag as F;
    let o = &cx.fp.snap_options;
    [
        (
            ids::EDITORS_FP_PROPS_OBJECT_PAD_CENTERS,
            F::PadCenters,
            o.snap_pad_centers,
        ),
        (
            ids::EDITORS_FP_PROPS_OBJECT_PAD_EDGES,
            F::PadEdges,
            o.snap_pad_edges,
        ),
        (
            ids::EDITORS_FP_PROPS_OBJECT_PAD_VERTICES,
            F::PadVertices,
            o.snap_pad_vertices,
        ),
        (
            ids::EDITORS_FP_PROPS_OBJECT_FOOTPRINT_ORIGINS,
            F::FootprintOrigins,
            o.snap_footprint_origins,
        ),
        (
            ids::EDITORS_FP_PROPS_OBJECT_TRACK_LINES,
            F::TrackLines,
            o.snap_track_lines,
        ),
        (
            ids::EDITORS_FP_PROPS_OBJECT_TRACK_VERTICES,
            F::TrackVertices,
            o.snap_track_vertices,
        ),
        (
            ids::EDITORS_FP_PROPS_OBJECT_ARC_CENTERS,
            F::ArcCenters,
            o.snap_arc_centers,
        ),
        (
            ids::EDITORS_FP_PROPS_OBJECT_INTERSECTIONS,
            F::Intersections,
            o.snap_intersections,
        ),
        (
            ids::EDITORS_FP_PROPS_OBJECT_REGIONS,
            F::Regions,
            o.snap_regions,
        ),
        (ids::EDITORS_FP_PROPS_OBJECT_TEXTS, F::Texts, o.snap_texts),
        (
            ids::EDITORS_FP_PROPS_OBJECT_VIA_CENTERS,
            F::ViaCenters,
            o.snap_via_centers,
        ),
        (
            ids::EDITORS_FP_PROPS_OBJECT_BODY_POINTS,
            F::Body3dPoints,
            o.snap_3d_body_points,
        ),
    ]
}

/// The flags as a two-column checkbox grid.
fn cells<'a>(
    cx: Ctx<'a, '_>,
    flags: impl IntoIterator<Item = (MessageId, SnapOptionFlag, bool)>,
) -> Element<'a, PanelMsg> {
    let cells = flags
        .into_iter()
        .map(|(label, flag, is_on)| {
            check_cell(
                t!(cx.i18n, label),
                is_on,
                PanelMsg::FpEditorToggleSnapOption(flag),
                cx.tokens,
            )
        })
        .collect();
    check_grid(cells)
}

/// The Snap Options section. `with_grid_step` adds the active grid's step
/// where the Grid Manager is not shown (a selection, the board summary).
pub(super) fn view_snap_options<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
    with_grid_step: bool,
) -> Column<Element<'a, PanelMsg>> {
    let Ctx {
        fp, tokens, i18n, ..
    } = cx;
    let title = t!(i18n, ids::EDITORS_FP_PROPS_SECTION_SNAP);
    col = col.push(section(KEY, &title, None, cx.collapsed, tokens, i18n));
    if !is_open(KEY, cx.collapsed) {
        return col;
    }
    let hover = &fp.illustration_hover;
    let opts = &fp.snap_options;
    let muted = cx.palette.muted;

    col = col.push(mini_head(
        &t!(i18n, ids::EDITORS_FP_PROPS_SNAPPING),
        tokens,
        i18n,
    ));
    let scopes = [
        (
            SnappingMode::AllLayers,
            ids::EDITORS_FP_PROPS_SNAP_ALL_LAYERS,
        ),
        (
            SnappingMode::CurrentLayer,
            ids::EDITORS_FP_PROPS_SNAP_CURRENT_LAYER,
        ),
        (SnappingMode::Off, ids::EDITORS_FP_PROPS_SNAP_OFF),
    ]
    .map(|(mode, label)| (mode, Label::from(t!(i18n, label))));
    col = col.push(full_width_segment(
        scopes,
        fp.snapping_mode,
        tokens,
        PanelMsg::FpEditorSetSnappingMode,
    ));

    let distance = edit_row(
        &t!(i18n, ids::EDITORS_FP_PROPS_SNAP_DISTANCE).into_string(),
        &format!("{:.3}", opts.snap_distance_mm),
        muted,
        PanelMsg::FpEditorSetSnapDistance,
    );
    let axis_label = t!(i18n, ids::EDITORS_FP_PROPS_AXIS_SNAP_RANGE).into_string();
    let axis_value = format!("{:.3}", opts.axis_snap_range_mm);
    let axis_range = if opts.snap_to_axes {
        edit_row(
            &axis_label,
            &axis_value,
            muted,
            PanelMsg::FpEditorSetAxisSnapRange,
        )
    } else {
        edit_row_note(
            &axis_label,
            &axis_value,
            t!(i18n, ids::EDITORS_FP_PROPS_AXES_OFF),
            muted,
            PanelMsg::FpEditorSetAxisSnapRange,
        )
    };
    let dependents = Column::new()
        .width(Length::Fill)
        .push(cells(cx, targets(cx)))
        .push(mini_head(
            &t!(i18n, ids::EDITORS_FP_PROPS_SNAP_OBJECTS),
            tokens,
            i18n,
        ))
        .push(cells(cx, objects(cx)))
        .push(linked_field(distance, fields::SNAP_DISTANCE, hover, tokens))
        .push(gated(
            linked_field(axis_range, fields::AXIS_SNAP_RANGE, hover, tokens),
            opts.snap_to_axes,
            tokens,
        ));
    col = col.push(gated(
        dependents.boxed(),
        fp.snapping_mode != SnappingMode::Off,
        tokens,
    ));

    if with_grid_step {
        col = col.push(linked_field(
            edit_row(
                &t!(i18n, ids::EDITORS_FP_PROPS_GRID_STEP_ROW).into_string(),
                &format!("{:.3}", opts.grid_step_mm),
                muted,
                PanelMsg::FpEditorSetSnapGridStep,
            ),
            fields::GRID_STEP,
            hover,
            tokens,
        ));
    }
    col
}
