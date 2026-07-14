use iced::widget::{Column, button, container, row, text, text_input};
use iced::{Background, Border, Color, Element, Length, Theme};

use super::super::{
    CollapsedSections, FootprintEditorPanelContext, FootprintModeKind, PanelMsg, SnapOptionFlag,
};
use super::managers::{render_grid_manager, render_other_section};
use super::snap_options::render_snapping_mode_row;
use super::{fp_is_collapsed, props_kv_row, props_section_header};

#[allow(clippy::too_many_arguments)]
pub(super) fn view_sections<'a>(
    mut col: Column<'a, PanelMsg>,
    fp: &'a FootprintEditorPanelContext,
    muted: Color,
    primary: Color,
    border_c: Color,
    input_bg: Color,
    input_bdr: Color,
    collapsed_sections: &'a CollapsedSections,
    unit: signex_types::coord::Unit,
    seg_hover: Color,
) -> Column<'a, PanelMsg> {
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
    col = col.push(props_section_header(
        "Snap Options",
        "fp_snap_options",
        collapsed_sections,
        primary,
        border_c,
    ));
    let snap_open = !fp_is_collapsed("fp_snap_options", collapsed_sections);
    let opts = fp.snap_options;
    if snap_open {
        // v0.13 — Snap target categories as INDEPENDENT toggle pills
        // (Altium PCB Library editor parity): each can be on
        // simultaneously. Replaces the previous mutex sub-tab.
        col = col.push(
            container(text("Snap targets").size(10).color(muted))
                .padding([4, 8])
                .width(Length::Fill),
        );
        let snap_pill =
            |label: &'static str, flag: SnapOptionFlag, on: bool| -> Element<'static, PanelMsg> {
                iced::widget::button(
                    text(label)
                        .size(10)
                        .color(if on { primary } else { muted })
                        .align_x(iced::alignment::Horizontal::Center),
                )
                .padding([3, 12])
                .on_press(PanelMsg::FpEditorToggleSnapOption(flag))
                .style(move |_: &Theme, status: iced::widget::button::Status| {
                    let bg = match status {
                        iced::widget::button::Status::Hovered => {
                            Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.06)))
                        }
                        _ => Some(Background::Color(if on {
                            Color::from_rgba8(0x2E, 0x33, 0x45, 1.0)
                        } else {
                            Color::from_rgba8(0x1A, 0x1D, 0x28, 1.0)
                        })),
                    };
                    iced::widget::button::Style {
                        background: bg,
                        border: Border {
                            width: 1.0,
                            radius: 2.0.into(),
                            color: input_bdr,
                        },
                        ..iced::widget::button::Style::default()
                    }
                })
                .into()
            };
        col = col.push(
            container(
                row![
                    snap_pill("Grids", SnapOptionFlag::SnapToGrids, opts.snap_to_grids),
                    snap_pill("Guides", SnapOptionFlag::SnapToGuides, opts.snap_to_guides),
                    snap_pill("Axes", SnapOptionFlag::SnapToAxes, opts.snap_to_axes),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
            )
            .padding([2, 8])
            .width(Length::Fill),
        );

        // v0.18.14.3 — "Snapping" mode (All Layers / Current Layer /
        // Off). Mutually exclusive — selecting one deselects the
        // others — but rendered as a 3-segment toggle row so it reads
        // visually like the Altium scope picker.
        col = render_snapping_mode_row(col, fp, primary, muted, border_c);

        // v0.13 — Altium-style "Objects for snapping" table. 12-row
        // checkbox list mapped to the snap_* fields on SnapOptions.
        col = col.push(
            container(text("Objects for snapping").size(10).color(muted))
                .padding([6, 8])
                .width(Length::Fill),
        );
        let header = row![
            text("On/Off")
                .size(10)
                .color(muted)
                .width(Length::Fixed(60.0)),
            text("Objects").size(10).color(muted).width(Length::Fill),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center);
        col = col.push(container(header).padding([2, 8]).width(Length::Fill));
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
                        .size(12)
                        .spacing(0),
                )
                .width(Length::Fixed(60.0))
                .padding([0, 0]),
                text(label_owned)
                    .size(10)
                    .color(primary)
                    .width(Length::Fill),
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center);
            col = col.push(container(row_w).padding([2, 8]).width(Length::Fill));
        }

        // Snap Distance + Axis Snap Range numeric rows.
        let mk_num_row = |label: &str,
                          value: f64,
                          on_input: fn(String) -> PanelMsg|
         -> Element<'static, PanelMsg> {
            container(
                row![
                    text(label.to_string())
                        .size(10)
                        .color(muted)
                        .width(Length::Fixed(110.0)),
                    text_input("", &format!("{value:.3}"))
                        .size(10)
                        .padding(2)
                        .style(move |_: &Theme, _| iced::widget::text_input::Style {
                            background: iced::Background::Color(iced::Color::from_rgba(
                                1.0, 1.0, 1.0, 0.04,
                            )),
                            border: iced::Border {
                                width: 1.0,
                                radius: 2.0.into(),
                                color: border_c,
                            },
                            icon: iced::Color::TRANSPARENT,
                            placeholder: muted,
                            value: primary,
                            selection: iced::Color::from_rgba(0.4, 0.6, 1.0, 0.4),
                        })
                        .on_input(on_input),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
            )
            .padding([2, 8])
            .width(Length::Fill)
            .into()
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
        col = col.push(props_section_header(
            "Grid Manager",
            "fp_grid_manager",
            collapsed_sections,
            primary,
            border_c,
        ));
        if !fp_is_collapsed("fp_grid_manager", collapsed_sections) {
            col = render_grid_manager(col, fp, primary, muted, border_c);
        }
        // Guide Manager removed in v0.13 — sketch-mode owns guides /
        // construction geometry; the standalone Guide Manager was
        // redundant.
        col = col.push(props_section_header(
            "Other",
            "fp_other",
            collapsed_sections,
            primary,
            border_c,
        ));
        if !fp_is_collapsed("fp_other", collapsed_sections) {
            col = render_other_section(
                col, fp, primary, muted, border_c, input_bg, input_bdr, unit, seg_hover,
            );
        }
    }

    // v0.16.2 — Sketch-mode-only sections (Parameters, DOF, Solve
    // warnings). Always visible when the editor is in Sketch mode so
    // the user can monitor solve state while authoring, regardless of
    // whether anything is selected. Migrated out of the bottom-of-canvas
    // inspector strip that shipped in v0.13.1.
    if fp.mode_kind == FootprintModeKind::Sketch {
        // Parameters
        col = col.push(props_section_header(
            "Parameters",
            "fp_parameters",
            collapsed_sections,
            primary,
            border_c,
        ));
        if !fp_is_collapsed("fp_parameters", collapsed_sections) {
            if fp.sketch_parameters.is_empty() {
                col = col.push(
                    container(text("(none — add via expression)").size(10).color(muted))
                        .padding([2, 8])
                        .width(Length::Fill),
                );
            } else {
                for (name, expr) in &fp.sketch_parameters {
                    let name_clone = name.clone();
                    let row = row![
                        text(name)
                            .size(10)
                            .color(primary)
                            .width(Length::Fixed(110.0)),
                        text_input("expression…", expr)
                            .size(10)
                            .padding(2)
                            .style(move |_: &Theme, _| iced::widget::text_input::Style {
                                background: iced::Background::Color(iced::Color::from_rgba(
                                    1.0, 1.0, 1.0, 0.04,
                                )),
                                border: iced::Border {
                                    width: 1.0,
                                    radius: 2.0.into(),
                                    color: border_c,
                                },
                                icon: iced::Color::TRANSPARENT,
                                placeholder: muted,
                                value: primary,
                                selection: iced::Color::from_rgba(0.4, 0.6, 1.0, 0.4),
                            })
                            .on_input(move |new_expr| PanelMsg::FpEditorEditParameter {
                                name: name_clone.clone(),
                                expr: new_expr,
                            }),
                    ]
                    .spacing(6)
                    .align_y(iced::Alignment::Center);
                    col = col.push(container(row).padding([2, 8]).width(Length::Fill));
                }
            }
        } // end if !fp_parameters collapsed

        // DOF / Last solve
        col = col.push(props_section_header(
            "DOF / Last solve",
            "fp_dof",
            collapsed_sections,
            primary,
            border_c,
        ));
        if !fp_is_collapsed("fp_dof", collapsed_sections) {
            col = props_kv_row(
                col,
                muted,
                input_bg,
                input_bdr,
                "Sketch entities",
                fp.sketch_entity_count.to_string(),
            );
            col = props_kv_row(
                col,
                muted,
                input_bg,
                input_bdr,
                "Constraints",
                fp.sketch_constraint_count.to_string(),
            );
            if let Some(s) = fp.last_solve.as_ref() {
                col = props_kv_row(
                    col,
                    muted,
                    input_bg,
                    input_bdr,
                    "Iterations",
                    s.iterations.to_string(),
                );
                col = props_kv_row(
                    col,
                    muted,
                    input_bg,
                    input_bdr,
                    "Elapsed",
                    format!("{} ms", s.elapsed_ms),
                );
                col = props_kv_row(
                    col,
                    muted,
                    input_bg,
                    input_bdr,
                    "Residual norm",
                    format!("{:.3e}", s.final_residual_norm),
                );
                col = props_kv_row(
                    col,
                    muted,
                    input_bg,
                    input_bdr,
                    "Over-constrained",
                    s.over_constraint_count.to_string(),
                );

                // v0.22 Phase E3+E4 — list of over-constrained
                // constraints. Each row shows the kind label + residual
                // magnitude, sorted descending so the worst offender is
                // first. Click → select the focus entity (first Point /
                // Line referenced by the constraint) so the canvas pans
                // and the red constraint icon sits in view. Hidden when
                // count == 0; the user reads "Over-constrained: 0" and
                // moves on.
                if !s.over_constraints.is_empty() {
                    col = col.push(
                        container(text("Conflicts (worst first)").size(10).color(muted))
                            .padding([4, 8])
                            .width(Length::Fill),
                    );
                    for oc in s.over_constraints.iter().take(8) {
                        let label =
                            format!("{} — residual {:.3e}", oc.kind_label, oc.residual_magnitude);
                        let inner: Element<'_, _> = if let Some(focus_id) = oc.focus_entity_id {
                            button(
                                text(label)
                                    .size(10)
                                    .color(Color::from_rgba(1.00, 0.55, 0.55, 1.00)),
                            )
                            .padding([2, 8])
                            .width(Length::Fill)
                            .on_press(PanelMsg::FpEditorSelectSketchEntity { id: focus_id })
                            .style(move |_t: &Theme, _| iced::widget::button::Style {
                                background: Some(iced::Background::Color(iced::Color::from_rgba(
                                    1.0, 0.30, 0.30, 0.06,
                                ))),
                                ..iced::widget::button::Style::default()
                            })
                            .into()
                        } else {
                            container(
                                text(label)
                                    .size(10)
                                    .color(Color::from_rgba(1.00, 0.55, 0.55, 1.00)),
                            )
                            .padding([2, 8])
                            .width(Length::Fill)
                            .into()
                        };
                        // v0.23 — per-row precision. Each row passes its
                        // own ConstraintId so the canvas isolates that
                        // single constraint at full red and dims the
                        // rest (including other over-constraints).
                        let cid = oc.constraint_id;
                        let oc_row: Element<'_, _> = iced::widget::mouse_area(inner)
                            .on_enter(PanelMsg::FpEditorHoverOverConstraint {
                                constraint: Some(cid),
                            })
                            .on_exit(PanelMsg::FpEditorHoverOverConstraint { constraint: None })
                            .into();
                        col = col.push(oc_row);
                    }
                    if s.over_constraints.len() > 8 {
                        col = col.push(
                            container(
                                text(format!("… +{} more", s.over_constraints.len() - 8))
                                    .size(9)
                                    .color(muted),
                            )
                            .padding([2, 8])
                            .width(Length::Fill),
                        );
                    }
                }
            } else {
                col = col.push(
                    container(text("(no solve yet)").size(10).color(muted))
                        .padding([2, 8])
                        .width(Length::Fill),
                );
            }
        } // end if !fp_dof collapsed

        // Solve warnings
        col = col.push(props_section_header(
            "Solve warnings",
            "fp_solve_warnings",
            collapsed_sections,
            primary,
            border_c,
        ));
        if !fp_is_collapsed("fp_solve_warnings", collapsed_sections) {
            if fp.solve_warnings.is_empty() {
                col = col.push(
                    container(text("(none)").size(10).color(muted))
                        .padding([2, 8])
                        .width(Length::Fill),
                );
            } else {
                for w in fp.solve_warnings.iter().take(8) {
                    col = col.push(
                        container(text(w).size(9).color(muted))
                            .padding([2, 8])
                            .width(Length::Fill),
                    );
                }
                if fp.solve_warnings.len() > 8 {
                    col = col.push(
                        container(
                            text(format!("… +{} more", fp.solve_warnings.len() - 8))
                                .size(9)
                                .color(muted),
                        )
                        .padding([2, 8])
                        .width(Length::Fill),
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
