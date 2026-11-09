use iced::widget::{Column, container, row, text, text_input};
use iced::{Element, Length, Theme, Widget};

use super::super::{
    CollapsedSections, FootprintEditorPanelContext, FootprintModeKind, PanelMsg, PanelPalette,
    SnapOptionFlag,
};
use super::conflicts;
use super::managers::{render_grid_manager, render_other_section};
use super::props_kv_row;
use super::snap_options::render_snapping_mode_row;
use crate::panels::{collapsible_section_header, is_section_collapsed};
use crate::styles::{Spacing, TypeScale};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

pub(super) fn view_sections<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    fp: &'a FootprintEditorPanelContext,
    palette: PanelPalette,
    collapsed_sections: &'a CollapsedSections,
    unit: signex_types::coord::Unit,
) -> Column<Element<'a, PanelMsg>> {
    let PanelPalette {
        muted,
        primary,
        border: border_c,
        input_bg,
        ..
    } = palette;
    // Track no-selection state so the v0.18.13 Library Options
    // sections (Grid Manager / Guide Manager / Other) only render
    // in the empty-canvas Properties body.
    // v0.18.24 — silk graphic selection counts as a selection too:
    // hide Manager sections so the silk panel branch isn't shadowed.
    let no_selection = fp.selected_pad.is_none()
        && fp.selected_sketch_entity.is_none()
        && fp.selected_silk_summary.is_none();

    // v0.18.11.2 — Snap Options promoted out of the no-selection
    // branch so it stays reachable while a pad/entity is selected.
    // Earlier (v0.17.0) the section was tucked inside the empty-
    // canvas summary which made the controls disappear the moment
    // the user clicked a pad.
    col = col.push(
        collapsible_section_header(
            "fp_snap_options",
            "Snap Options",
            collapsed_sections,
            primary,
            border_c,
        )
        .boxed(),
    );
    let snap_open = !is_section_collapsed("fp_snap_options", collapsed_sections);
    let opts = fp.snap_options;
    if snap_open {
        // v0.13 — Snap target categories as INDEPENDENT toggle pills
        // (Altium PCB Library editor parity): each can be on
        // simultaneously. Replaces the previous mutex sub-tab.
        col = col.push(
            container(text("Snap targets").size(TS.caption).color(muted))
                .padding([SP.s, SP.m])
                .width(Length::Fill)
                .boxed(),
        );
        let snap_pill =
            |label: &'static str, flag: SnapOptionFlag, on: bool| -> Element<'static, PanelMsg> {
                iced::widget::button(
                    text(label)
                        .size(TS.caption)
                        .color(if on { primary } else { muted })
                        .align_x(iced::alignment::Horizontal::Center),
                )
                .padding([3.0, SP.l])
                .on_press(PanelMsg::FpEditorToggleSnapOption(flag))
                .style(move |theme: &Theme, status: iced::widget::button::Status| {
                    let hovered = matches!(status, iced::widget::button::Status::Hovered);
                    crate::panels::form_look::segment_cell(theme, on, hovered)
                })
                .boxed()
            };
        col = col.push(
            container(
                row![
                    snap_pill("Grids", SnapOptionFlag::SnapToGrids, opts.snap_to_grids),
                    snap_pill("Guides", SnapOptionFlag::SnapToGuides, opts.snap_to_guides),
                    snap_pill("Axes", SnapOptionFlag::SnapToAxes, opts.snap_to_axes),
                ]
                .spacing(SP.s)
                .align_y(iced::Alignment::Center),
            )
            .padding([SP.xs, SP.m])
            .width(Length::Fill)
            .style(crate::panels::form_look::segmented_frame)
            .boxed(),
        );

        // v0.18.14.3 — "Snapping" mode (All Layers / Current Layer /
        // Off). Mutually exclusive — selecting one deselects the
        // others — but rendered as a 3-segment toggle row so it reads
        // visually like the Altium scope picker.
        col = render_snapping_mode_row(col, fp, primary, muted, border_c);

        // v0.13 — Altium-style "Objects for snapping" table. 12-row
        // checkbox list mapped to the snap_* fields on SnapOptions.
        col = col.push(
            container(text("Objects for snapping").size(TS.caption).color(muted))
                .padding([6.0, SP.m])
                .width(Length::Fill)
                .boxed(),
        );
        let header = row![
            text("On/Off")
                .size(TS.caption)
                .color(muted)
                .width(Length::Fixed(60.0)),
            text("Objects")
                .size(TS.caption)
                .color(muted)
                .width(Length::Fill),
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center);
        col = col.push(
            container(header)
                .padding([SP.xs, SP.m])
                .width(Length::Fill)
                .boxed(),
        );
        col = col.push(super::super::thin_sep(border_c));
        let snap_rows: &[(&str, SnapOptionFlag, bool)] = &[
            (
                "Track Vertices",
                SnapOptionFlag::TrackVertices,
                opts.snap_track_vertices,
            ),
            (
                "Track Lines",
                SnapOptionFlag::TrackLines,
                opts.snap_track_lines,
            ),
            (
                "Arc Centers",
                SnapOptionFlag::ArcCenters,
                opts.snap_arc_centers,
            ),
            (
                "Intersections",
                SnapOptionFlag::Intersections,
                opts.snap_intersections,
            ),
            (
                "Pad Centers",
                SnapOptionFlag::PadCenters,
                opts.snap_pad_centers,
            ),
            (
                "Pad Vertices",
                SnapOptionFlag::PadVertices,
                opts.snap_pad_vertices,
            ),
            ("Pad Edges", SnapOptionFlag::PadEdges, opts.snap_pad_edges),
            (
                "Via Centers",
                SnapOptionFlag::ViaCenters,
                opts.snap_via_centers,
            ),
            ("Texts", SnapOptionFlag::Texts, opts.snap_texts),
            ("Regions", SnapOptionFlag::Regions, opts.snap_regions),
            (
                "Footprint Origins",
                SnapOptionFlag::FootprintOrigins,
                opts.snap_footprint_origins,
            ),
            (
                "3D Body Snap Points",
                SnapOptionFlag::Body3dPoints,
                opts.snap_3d_body_points,
            ),
        ];
        for &(label, flag, on) in snap_rows {
            let label_owned: String = label.to_string();
            let row_w = row![
                container(
                    iced::widget::checkbox(on)
                        .on_toggle(move |_| PanelMsg::FpEditorToggleSnapOption(flag))
                        .size(TS.label)
                        .spacing(0),
                )
                .width(Length::Fixed(60.0))
                .padding([0, 0]),
                text(label_owned)
                    .size(TS.caption)
                    .color(primary)
                    .width(Length::Fill),
            ]
            .spacing(SP.s)
            .align_y(iced::Alignment::Center);
            col = col.push(
                container(row_w)
                    .padding([SP.xs, SP.m])
                    .width(Length::Fill)
                    .boxed(),
            );
        }

        // Snap Distance + Axis Snap Range numeric rows.
        let mk_num_row = |label: &str,
                          value: f64,
                          on_input: fn(String) -> PanelMsg|
         -> Element<'static, PanelMsg> {
            container(
                row![
                    text(label.to_string())
                        .size(TS.caption)
                        .color(muted)
                        .width(Length::Fixed(110.0)),
                    text_input("", format!("{value:.3}"))
                        .size(TS.caption)
                        .padding(SP.xs)
                        .style(crate::panels::form_look::text_input_style)
                        .on_input(on_input),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
            )
            .padding([SP.xs, SP.m])
            .width(Length::Fill)
            .boxed()
        };
        col = col.push(mk_num_row(
            "Snap Distance",
            opts.snap_distance_mm,
            PanelMsg::FpEditorSetSnapDistance,
        ));
        col = col.push(mk_num_row(
            "Axis Snap Range",
            opts.axis_snap_range_mm,
            PanelMsg::FpEditorSetAxisSnapRange,
        ));

        // Grid step retained for parity with v0.18.x flow.
        col = col.push(mk_num_row(
            "Grid step (mm)",
            opts.grid_step_mm,
            PanelMsg::FpEditorSetSnapGridStep,
        ));
    } // end if snap_open

    // v0.18.13 — Library Options layout (Grid Manager / Guide
    // Manager / Other) below Snap Options, only on the no-selection
    // body to mirror Altium's per-state Properties surface.
    if no_selection {
        col = col.push(
            collapsible_section_header(
                "fp_grid_manager",
                "Grid Manager",
                collapsed_sections,
                primary,
                border_c,
            )
            .boxed(),
        );
        if !is_section_collapsed("fp_grid_manager", collapsed_sections) {
            col = render_grid_manager(col, fp, primary, muted, border_c);
        }
        // Guide Manager removed in v0.13 — sketch-mode owns guides /
        // construction geometry; the standalone Guide Manager was
        // redundant.
        col = col.push(
            collapsible_section_header("fp_other", "Other", collapsed_sections, primary, border_c)
                .boxed(),
        );
        if !is_section_collapsed("fp_other", collapsed_sections) {
            col = render_other_section(col, fp, palette, unit);
        }
    }

    // v0.16.2 — Sketch-mode-only sections (Parameters, DOF, Solve
    // warnings). Always visible when the editor is in Sketch mode so
    // the user can monitor solve state while authoring, regardless of
    // whether anything is selected. Migrated out of the bottom-of-canvas
    // inspector strip that shipped in v0.13.1.
    if fp.mode_kind == FootprintModeKind::Sketch {
        // Parameters
        col = col.push(
            collapsible_section_header(
                "fp_parameters",
                "Parameters",
                collapsed_sections,
                primary,
                border_c,
            )
            .boxed(),
        );
        if !is_section_collapsed("fp_parameters", collapsed_sections) {
            if fp.sketch_parameters.is_empty() {
                col = col.push(
                    container(
                        text("(none — add via expression)")
                            .size(TS.caption)
                            .color(muted),
                    )
                    .padding([SP.xs, SP.m])
                    .width(Length::Fill)
                    .boxed(),
                );
            } else {
                for (name, expr) in &fp.sketch_parameters {
                    let name_clone = name.clone();
                    let row = row![
                        text(name)
                            .size(TS.caption)
                            .color(primary)
                            .width(Length::Fixed(110.0)),
                        text_input("expression…", expr)
                            .size(TS.caption)
                            .padding(SP.xs)
                            .style(crate::panels::form_look::text_input_style)
                            .on_input(move |new_expr| PanelMsg::FpEditorEditParameter {
                                name: name_clone.clone(),
                                expr: new_expr,
                            }),
                    ]
                    .spacing(6)
                    .align_y(iced::Alignment::Center);
                    col = col.push(
                        container(row)
                            .padding([SP.xs, SP.m])
                            .width(Length::Fill)
                            .boxed(),
                    );
                }
            }
        } // end if !fp_parameters collapsed

        // DOF / Last solve
        col = col.push(
            collapsible_section_header(
                "fp_dof",
                "DOF / Last solve",
                collapsed_sections,
                primary,
                border_c,
            )
            .boxed(),
        );
        if !is_section_collapsed("fp_dof", collapsed_sections) {
            col = props_kv_row(
                col,
                muted,
                input_bg,
                "Sketch entities",
                fp.sketch_entity_count.to_string(),
            );
            col = props_kv_row(
                col,
                muted,
                input_bg,
                "Constraints",
                fp.sketch_constraint_count.to_string(),
            );
            if let Some(s) = fp.last_solve.as_ref() {
                col = props_kv_row(col, muted, input_bg, "Iterations", s.iterations.to_string());
                col = props_kv_row(
                    col,
                    muted,
                    input_bg,
                    "Elapsed",
                    format!("{} ms", s.elapsed_ms),
                );
                col = props_kv_row(
                    col,
                    muted,
                    input_bg,
                    "Residual norm",
                    format!("{:.3e}", s.final_residual_norm),
                );
                col = props_kv_row(
                    col,
                    muted,
                    input_bg,
                    "Over-constrained",
                    s.over_constraint_count.to_string(),
                );
                col = super::redundant::push_redundant_rows(col, s, muted, input_bg);
            } else {
                col = col.push(
                    container(text("(no solve yet)").size(TS.caption).color(muted))
                        .padding([SP.xs, SP.m])
                        .width(Length::Fill)
                        .boxed(),
                );
            }

            // Conflicting constraints, worst first: the last solve's, then
            // the last rejected edit's. Click selects the row's entity.
            let conflicts = conflicts::conflict_rows(
                fp.last_solve
                    .as_ref()
                    .map_or(&[], |s| s.over_constraints.as_slice()),
                &fp.rejected_conflicts,
            );
            if !conflicts.is_empty() {
                col = col.push(
                    container(
                        text("Conflicts (worst first)")
                            .size(TS.caption)
                            .color(muted),
                    )
                    .padding([SP.s, SP.m])
                    .width(Length::Fill)
                    .boxed(),
                );
                for row in conflicts.iter().take(conflicts::MAX_ROWS) {
                    col = col.push(conflicts::conflict_row(row));
                }
                if conflicts.len() > conflicts::MAX_ROWS {
                    col = col.push(
                        container(
                            text(format!("… +{} more", conflicts.len() - conflicts::MAX_ROWS))
                                .size(TS.caption)
                                .color(muted),
                        )
                        .padding([SP.xs, SP.m])
                        .width(Length::Fill)
                        .boxed(),
                    );
                }
            }
        } // end if !fp_dof collapsed

        // Solve warnings
        col = col.push(
            collapsible_section_header(
                "fp_solve_warnings",
                "Solve warnings",
                collapsed_sections,
                primary,
                border_c,
            )
            .boxed(),
        );
        if !is_section_collapsed("fp_solve_warnings", collapsed_sections) {
            if fp.solve_warnings.is_empty() {
                col = col.push(
                    container(text("(none)").size(TS.caption).color(muted))
                        .padding([SP.xs, SP.m])
                        .width(Length::Fill)
                        .boxed(),
                );
            } else {
                for w in fp.solve_warnings.iter().take(8) {
                    col = col.push(
                        container(text(w).size(TS.caption).color(muted))
                            .padding([SP.xs, SP.m])
                            .width(Length::Fill)
                            .boxed(),
                    );
                }
                if fp.solve_warnings.len() > 8 {
                    col = col.push(
                        container(
                            text(format!("… +{} more", fp.solve_warnings.len() - 8))
                                .size(TS.caption)
                                .color(muted),
                        )
                        .padding([SP.xs, SP.m])
                        .width(Length::Fill)
                        .boxed(),
                    );
                }
            }
        } // end if !fp_solve_warnings collapsed
    }

    // Settings + Hint footer — always visible at the bottom of the
    // panel regardless of mode / selection so common toggles stay
    // reachable. Factored into a helper so the early-return placement
    // branch above can render the same footer.
    col
}
