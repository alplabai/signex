//! Properties panel body for the Footprint editor (HI-22 / MD-20).
//!
//! Extracted from `panels/mod.rs`. Pure view code — zero behaviour change
//! from the move. Switches between Pads / Sketch / 3D View contexts and
//! renders the v0.18.13 Library Options sections (Snap Options / Grid /
//! Guide / Other) plus the v0.16.4 role sub-forms (Pour / Keepout /
//! Cutout) and the v0.16.3 Pad placement defaults form.

use iced::widget::{
    Column, Space, button, column, container, pick_list, row, scrollable, text, text_input,
};
use iced::{Background, Border, Color, Element, Length, Theme};

use super::{
    CollapsedSections, FootprintEditorPanelContext, FootprintModeKind, KeepoutKindFlag, PanelMsg,
    SnapOptionFlag,
};

/// v0.14.2 — Properties panel body for the Footprint editor. Switches
/// between three contexts:
///
/// 1. **Pads mode + pad selected** — pad number, kind, shape, size,
///    position, layer count.
/// 2. **Sketch mode + entity selected** — entity kind, position
///    (Points only), construction flag, attached-constraint count.
/// 3. **Default** (any mode, no selection) — footprint summary
///    (name + version), counts (pads, sketch entities, constraints),
///    and the most recent solve summary when a sketch exists.
pub(super) fn view_footprint_editor_properties<'a>(
    fp: &'a FootprintEditorPanelContext,
    muted: Color,
    primary: Color,
    border_c: Color,
    input_bg: Color,
    input_bdr: Color,
    custom_filter_presets: Vec<crate::active_bar::CustomFilterPreset>,
    active_custom_filter_tab: usize,
    collapsed_sections: &'a CollapsedSections,
    accent_c: Color,
    tag_hover: Color,
    unit: signex_types::coord::Unit,
    seg_hover: Color,
) -> Element<'a, PanelMsg> {
    let mode_label = match fp.mode_kind {
        FootprintModeKind::Pads => "Pads",
        FootprintModeKind::Sketch => "Sketch",
        FootprintModeKind::View3d => "3D View",
    };

    let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);

    col = col.push(
        container(
            row![
                text(&fp.footprint_name).size(12).color(primary),
                text("·").size(12).color(muted),
                text(mode_label).size(11).color(muted),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .padding([6, 8])
        .width(Length::Fill),
    );
    col = col.push(super::thin_sep(border_c));

    // Selection-specific top section. Pads + selected pad → pad
    // summary; Sketch + selected entity → entity summary + Role
    // pick_list; otherwise → footprint summary. Sketch-mode-only
    // sections (Parameters / DOF / Warnings) follow regardless of
    // selection, so the user can monitor solve state while authoring.
    match (
        fp.mode_kind,
        fp.selected_pad.as_ref(),
        fp.selected_sketch_entity.as_ref(),
    ) {
        (FootprintModeKind::Pads, Some(pad), _) => {
            col = col.push(props_section_header("Pad", "fp_pad", collapsed_sections, primary, border_c));
            if !fp_is_collapsed("fp_pad", collapsed_sections) {
            col = props_kv_row(col, muted, input_bg, input_bdr, "Number", pad.number.clone());
            col = props_kv_row(col, muted, input_bg, input_bdr, "Kind", pad.kind_label.into());
            col = props_kv_row(col, muted, input_bg, input_bdr, "Shape", pad.shape_label.into());
            col = props_kv_row(
                col,
                muted,
                input_bg,
                input_bdr,
                "Size",
                format!("{:.3} × {:.3} mm", pad.size_mm[0], pad.size_mm[1]),
            );
            col = props_kv_row(
                col,
                muted,
                input_bg,
                input_bdr,
                "Position",
                format!("({:.3}, {:.3}) mm", pad.position_mm[0], pad.position_mm[1]),
            );
            col = props_kv_row(col, muted, input_bg, input_bdr, "Layers", pad.layer_count.to_string());

            // v0.16.6 — editable rotation row. text_input bound to the
            // selected pad's rotation_deg; routes through
            // FpEditorSetSelectedPadRotation handler which writes
            // back to state.pads[idx].rotation_deg + dirty-marks.
            let pad_idx = pad.idx;
            col = col.push(
                container(
                    row![
                        text("Rotation (°)")
                            .size(10)
                            .color(muted)
                            .width(Length::Fixed(110.0)),
                        text_input("0", &format!("{:.1}", pad.rotation_deg))
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
                            .on_input(move |v| PanelMsg::FpEditorSetSelectedPadRotation {
                                idx: pad_idx,
                                value: v,
                            }),
                    ]
                    .spacing(6)
                    .align_y(iced::Alignment::Center),
                )
                .padding([2, 8])
                .width(Length::Fill),
            );
            } // end if !fp_pad collapsed
        }
        (FootprintModeKind::Sketch, _, Some(ent)) => {
            col = col.push(props_section_header("Sketch entity", "fp_sketch_entity", collapsed_sections, primary, border_c));
            if !fp_is_collapsed("fp_sketch_entity", collapsed_sections) {
            col = props_kv_row(col, muted, input_bg, input_bdr, "Kind", ent.kind_label.into());
            if let Some([x, y]) = ent.position_mm {
                col = props_kv_row(
                    col,
                    muted,
                    input_bg,
                    input_bdr,
                    "Position",
                    format!("({x:.3}, {y:.3}) mm"),
                );
            }
            col = props_kv_row(
                col,
                muted,
                input_bg,
                input_bdr,
                "Construction",
                if ent.construction {
                    "yes".into()
                } else {
                    "no".into()
                },
            );
            col = props_kv_row(
                col,
                muted,
                input_bg,
                input_bdr,
                "Attached constraints",
                ent.attached_constraint_count.to_string(),
            );
            } // end if !fp_sketch_entity collapsed

            // v0.16.2 — Role pick_list. Visible when an entity is
            // selected; pick_list value mirrors the entity's
            // currently-attached `*Attr` slot (or `Unassigned`).
            col = col.push(props_section_header("Role", "fp_role", collapsed_sections, primary, border_c));
            if !fp_is_collapsed("fp_role", collapsed_sections) {
                if let Some(id) = fp.selected_sketch_entity_id {
                    use crate::library::messages::RoleTag;
                    let current = fp.selected_sketch_role;
                    let dropdown = pick_list(RoleTag::ALL, Some(current), move |new_role| {
                        PanelMsg::FpEditorSetRole { id, role: new_role }
                    })
                    .text_size(11)
                    .padding([3, 8])
                    .width(Length::Fill);
                    col = col.push(container(dropdown).padding([4, 8]).width(Length::Fill));
                    if !fp.selected_sketch_is_point {
                        col = col.push(
                            container(text("Pad role applies to Points only").size(9).color(muted))
                                .padding([0, 8])
                                .width(Length::Fill),
                        );
                    }

                    // v0.16.4 — role sub-forms. Render when the
                    // matching `*Attr` is set on the selected entity.
                    col = render_pour_subform(col, fp, id, muted, primary, border_c);
                    col = render_keepout_subform(col, fp, id, muted, primary, border_c);
                    col = render_cutout_subform(col, fp, id, muted, primary, border_c);
                }
            }
        }
        _ => {
            // v0.18.24 — silk-front graphic selection branch.
            // Renders BEFORE the empty-canvas Library Options when a
            // silk graphic is selected so the user can edit Text
            // content + delete the entry without leaving the
            // Properties panel.
            if let Some(silk) = fp.selected_silk_summary.as_ref() {
                col = col.push(props_section_header("Silk graphic", "fp_silk_graphic", collapsed_sections, primary, border_c));
                if !fp_is_collapsed("fp_silk_graphic", collapsed_sections) {
                col = props_kv_row(col, muted, input_bg, input_bdr, "Kind", silk.kind_label.into());
                col = props_kv_row(col, muted, input_bg, input_bdr, "Index", silk.idx.to_string());
                if let Some(content) = silk.text_content.as_ref() {
                    col = col.push(
                        container(
                            row![
                                text("Content")
                                    .size(10)
                                    .color(muted)
                                    .width(Length::Fixed(70.0)),
                                iced::widget::text_input("TEXT", content)
                                    .size(10)
                                    .padding([2, 4])
                                    .style(move |_: &Theme, _| iced::widget::text_input::Style {
                                        background: iced::Background::Color(
                                            iced::Color::from_rgba(1.0, 1.0, 1.0, 0.04),
                                        ),
                                        border: iced::Border {
                                            width: 1.0,
                                            radius: 2.0.into(),
                                            color: border_c,
                                        },
                                        icon: iced::Color::TRANSPARENT,
                                        placeholder: muted,
                                        value: primary,
                                        selection: iced::Color::from_rgba(0.4, 0.6, 1.0, 0.4,),
                                    })
                                    .on_input(PanelMsg::FpEditorSetSilkText)
                                    .width(Length::Fill),
                            ]
                            .spacing(6)
                            .align_y(iced::Alignment::Center),
                        )
                        .padding([2, 8])
                        .width(Length::Fill),
                    );
                }
                col = col.push(
                    container(
                        row![grid_manager_btn(
                            "Delete",
                            Some(PanelMsg::FpEditorDeleteSelectedSilk),
                            primary,
                            border_c,
                        )]
                        .spacing(4)
                        .align_y(iced::Alignment::Center),
                    )
                    .padding([4, 8])
                    .width(Length::Fill),
                );
                } // end if !fp_silk_graphic collapsed
            } else {
                // Selection Filter — Altium-style flat pill grid for
                // the 10 footprint kinds (3D Bodies, Keepouts, Tracks,
                // Arcs, Pads, Vias, Regions, Fills, Texts, Other) plus
                // a "Custom..." modal launcher for advanced presets.
                // Pill styling matches the schematic Properties panel's
                // `preset_chip` / `tag_btn` chrome.
                // v0.13 — flat Selection Filter pill grid removed
                // (redundant with Custom Selection Filters below + the
                // active bar's Filter dropdown). The Custom presets
                // section is the single Properties-panel surface.
                let _ = (fp, accent_c, tag_hover); // keep imports satisfied
                col = col.push(super::view_custom_selection_filters_section(
                    custom_filter_presets,
                    active_custom_filter_tab,
                    collapsed_sections,
                    muted,
                    primary,
                    border_c,
                    accent_c,
                    tag_hover,
                ));
                col = col.push(props_section_header("Footprint", "fp_footprint", collapsed_sections, primary, border_c));
                if !fp_is_collapsed("fp_footprint", collapsed_sections) {
                    col = props_kv_row(col, muted, input_bg, input_bdr, "Name", fp.footprint_name.clone());
                    col = props_kv_row(col, muted, input_bg, input_bdr, "Version", fp.version.clone());
                    col = props_kv_row(col, muted, input_bg, input_bdr, "Mode", mode_label.into());
                    col = props_kv_row(col, muted, input_bg, input_bdr, "Pads", fp.pad_count.to_string());
                    if fp.sketch_entity_count > 0 || fp.sketch_constraint_count > 0 {
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
                    }
                }
            }
        }
    }
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
    col = col.push(props_section_header("Snap Options", "fp_snap_options", collapsed_sections, primary, border_c));
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
        let snap_pill = |label: &'static str, flag: SnapOptionFlag, on: bool| -> Element<'static, PanelMsg> {
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
                    iced::widget::button::Status::Hovered => Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.06))),
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
            text("On/Off").size(10).color(muted).width(Length::Fixed(60.0)),
            text("Objects").size(10).color(muted).width(Length::Fill),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center);
        col = col.push(container(header).padding([2, 8]).width(Length::Fill));
        col = col.push(super::thin_sep(border_c));
        let snap_rows: &[(&str, SnapOptionFlag, bool)] = &[
            ("Track Vertices", SnapOptionFlag::TrackVertices, opts.snap_track_vertices),
            ("Track Lines", SnapOptionFlag::TrackLines, opts.snap_track_lines),
            ("Arc Centers", SnapOptionFlag::ArcCenters, opts.snap_arc_centers),
            ("Intersections", SnapOptionFlag::Intersections, opts.snap_intersections),
            ("Pad Centers", SnapOptionFlag::PadCenters, opts.snap_pad_centers),
            ("Pad Vertices", SnapOptionFlag::PadVertices, opts.snap_pad_vertices),
            ("Pad Edges", SnapOptionFlag::PadEdges, opts.snap_pad_edges),
            ("Via Centers", SnapOptionFlag::ViaCenters, opts.snap_via_centers),
            ("Texts", SnapOptionFlag::Texts, opts.snap_texts),
            ("Regions", SnapOptionFlag::Regions, opts.snap_regions),
            ("Footprint Origins", SnapOptionFlag::FootprintOrigins, opts.snap_footprint_origins),
            ("3D Body Snap Points", SnapOptionFlag::Body3dPoints, opts.snap_3d_body_points),
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
                text(label_owned).size(10).color(primary).width(Length::Fill),
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center);
            col = col.push(container(row_w).padding([2, 8]).width(Length::Fill));
        }

        // Snap Distance + Axis Snap Range numeric rows.
        let mk_num_row = |label: &str, value: f64, on_input: fn(String) -> PanelMsg| -> Element<'static, PanelMsg> {
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
        col = col.push(mk_num_row("Snap Distance", opts.snap_distance_mm, PanelMsg::FpEditorSetSnapDistance));
        col = col.push(mk_num_row("Axis Snap Range", opts.axis_snap_range_mm, PanelMsg::FpEditorSetAxisSnapRange));

        // Grid step retained for parity with v0.18.x flow.
        col = col.push(mk_num_row("Grid step (mm)", opts.grid_step_mm, PanelMsg::FpEditorSetSnapGridStep));
    } // end if snap_open

    // v0.18.13 — Library Options layout (Grid Manager / Guide
    // Manager / Other) below Snap Options, only on the no-selection
    // body to mirror Altium's per-state Properties surface.
    if no_selection {
        col = col.push(props_section_header("Grid Manager", "fp_grid_manager", collapsed_sections, primary, border_c));
        if !fp_is_collapsed("fp_grid_manager", collapsed_sections) {
            col = render_grid_manager(col, fp, primary, muted, border_c);
        }
        // Guide Manager removed in v0.13 — sketch-mode owns guides /
        // construction geometry; the standalone Guide Manager was
        // redundant.
        col = col.push(props_section_header("Other", "fp_other", collapsed_sections, primary, border_c));
        if !fp_is_collapsed("fp_other", collapsed_sections) {
            col = render_other_section(col, fp, primary, muted, border_c, input_bg, input_bdr, unit, seg_hover);
        }
    }

    // v0.16.2 — Sketch-mode-only sections (Parameters, DOF, Solve
    // warnings). Always visible when the editor is in Sketch mode so
    // the user can monitor solve state while authoring, regardless of
    // whether anything is selected. Migrated out of the bottom-of-canvas
    // inspector strip that shipped in v0.13.1.
    if fp.mode_kind == FootprintModeKind::Sketch {
        // Parameters
        col = col.push(props_section_header("Parameters", "fp_parameters", collapsed_sections, primary, border_c));
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
        col = col.push(props_section_header("DOF / Last solve", "fp_dof", collapsed_sections, primary, border_c));
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
            col = props_kv_row(col, muted, input_bg, input_bdr, "Iterations", s.iterations.to_string());
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
        } else {
            col = col.push(
                container(text("(no solve yet)").size(10).color(muted))
                    .padding([2, 8])
                    .width(Length::Fill),
            );
        }
        } // end if !fp_dof collapsed

        // Solve warnings
        col = col.push(props_section_header("Solve warnings", "fp_solve_warnings", collapsed_sections, primary, border_c));
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

    // v0.16.3 — "Pad placement" defaults form. Visible whenever the
    // user is in Pads mode + the PlacePad tool is active. TAB pause
    // adds a "PAUSED — TAB to resume" hint but doesn't gate the form
    // itself; the user can adjust before resuming.
    if fp.placement_active {
        col = col.push(props_section_header("Pad placement", "fp_pad_placement", collapsed_sections, primary, border_c));
        if !fp_is_collapsed("fp_pad_placement", collapsed_sections) {
        if fp.placement_paused {
            col = col.push(
                container(
                    text("PAUSED — TAB to resume placement")
                        .size(10)
                        .color(Color::from_rgba(1.0, 0.85, 0.30, 1.0)),
                )
                .padding([2, 8])
                .width(Length::Fill),
            );
        } else {
            col = col.push(
                container(
                    text("TAB to pause placement and edit defaults")
                        .size(10)
                        .color(muted),
                )
                .padding([2, 8])
                .width(Length::Fill),
            );
        }

        // Designator override — empty string = auto-increment.
        let designator_buf = fp.next_pad_designator_override.clone().unwrap_or_default();
        col = col.push(
            container(
                row![
                    text("Designator")
                        .size(10)
                        .color(muted)
                        .width(Length::Fixed(80.0)),
                    text_input("(auto)", &designator_buf)
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
                        .on_input(PanelMsg::FpEditorSetNextPadDesignator),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
            )
            .padding([2, 8])
            .width(Length::Fill),
        );

        // Size X (mm)
        col = col.push(
            container(
                row![
                    text("Size X (mm)")
                        .size(10)
                        .color(muted)
                        .width(Length::Fixed(80.0)),
                    text_input("", &format!("{:.3}", fp.next_pad_size_x_mm))
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
                        .on_input(PanelMsg::FpEditorSetNextPadSizeX),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
            )
            .padding([2, 8])
            .width(Length::Fill),
        );

        // Size Y (mm)
        col = col.push(
            container(
                row![
                    text("Size Y (mm)")
                        .size(10)
                        .color(muted)
                        .width(Length::Fixed(80.0)),
                    text_input("", &format!("{:.3}", fp.next_pad_size_y_mm))
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
                        .on_input(PanelMsg::FpEditorSetNextPadSizeY),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
            )
            .padding([2, 8])
            .width(Length::Fill),
        );

        // Side (Top / Bottom / All-THT)
        use crate::library::editor::footprint::state::PadSide;
        let side_picker = pick_list(
            PadSide::ALL_OPTIONS,
            Some(fp.next_pad_side),
            PanelMsg::FpEditorSetNextPadSide,
        )
        .text_size(10)
        .padding([3, 8]);
        col = col.push(
            container(
                row![
                    text("Side")
                        .size(10)
                        .color(muted)
                        .width(Length::Fixed(80.0)),
                    side_picker,
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
            )
            .padding([2, 8])
            .width(Length::Fill),
        );

        // v0.16.6 — Rotation (degrees, CCW positive). Persists through
        // EditorPad.rotation_deg → Pad::rotation; canvas renders
        // unrotated (rotation rendering deferred to v0.17).
        col = col.push(
            container(
                row![
                    text("Rotation (°)")
                        .size(10)
                        .color(muted)
                        .width(Length::Fixed(80.0)),
                    text_input("0", &format!("{:.1}", fp.next_pad_rotation_deg))
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
                        .on_input(PanelMsg::FpEditorSetNextPadRotation),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
            )
            .padding([2, 8])
            .width(Length::Fill),
        );
        } // end if !fp_pad_placement collapsed
    }

    // Settings + hint — always visible at the bottom of the panel
    // regardless of mode / selection so common toggles stay reachable.
    col = col.push(props_section_header("Settings", "fp_settings", collapsed_sections, primary, border_c));
    if !fp_is_collapsed("fp_settings", collapsed_sections) {
        let auto_fit_label = if fp.auto_fit_courtyard {
            "Auto-fit Courtyard \u{2713}"
        } else {
            "Auto-fit Courtyard"
        };
        let auto_fit_btn = iced::widget::button(text(auto_fit_label).size(10).color(primary))
            .padding([4, 10])
            .on_press(PanelMsg::FpEditorToggleAutoFitCourtyard)
            .style(iced::widget::button::primary);
        col = col.push(container(auto_fit_btn).padding([4, 8]).width(Length::Fill));
    }

    col = col.push(props_section_header("Hint", "fp_hint", collapsed_sections, primary, border_c));
    if !fp_is_collapsed("fp_hint", collapsed_sections) {
        let hint = match fp.mode_kind {
            FootprintModeKind::Pads => "Click a pad to edit its properties.",
            FootprintModeKind::Sketch => {
                "Click a sketch entity (Point / Line / Arc / Circle) to edit it."
            }
            FootprintModeKind::View3d => {
                "3D View — use the 3D preview pane to inspect the body."
            }
        };
        col = col.push(
            container(text(hint).size(10).color(muted))
                .padding([4, 8])
                .width(Length::Fill),
        );
    }

    scrollable(col).width(Length::Fill).into()
}

/// Section header — collapsible. Delegates to
/// `super::collapsible_section_header` so every footprint Properties
/// section gets the same clickable chevron header used by the
/// schematic's Custom Selection Filters / General sections. Each
/// call site supplies a unique `key` so collapsed state survives in
/// `PanelContext.collapsed_sections`. Callers guard their body push
/// with `if !is_section_collapsed(key, collapsed)`.
fn props_section_header<'a>(
    label: &str,
    key: &'static str,
    collapsed: &super::CollapsedSections,
    primary: Color,
    border_c: Color,
) -> iced::widget::Column<'a, PanelMsg> {
    super::collapsible_section_header(key, label, collapsed, primary, border_c)
}

/// Returns true if the section with `key` is collapsed in
/// `PanelContext.collapsed_sections`.
fn fp_is_collapsed(key: &str, collapsed: &super::CollapsedSections) -> bool {
    super::is_section_collapsed(key, collapsed)
}

/// v0.18.14.3 — Altium "Snapping" 3-segment toggle. `All Layers` is
/// the default behaviour (current pre-v0.18.14 functionality);
/// `Current Layer` is a placeholder for the v0.18.15 layer-aware
/// enforcement; `Off` short-circuits every snap priority in
/// `snap::snap_cursor` so the cursor returns the raw click.
fn render_snapping_mode_row<'a>(
    mut col: Column<'a, PanelMsg>,
    fp: &'a FootprintEditorPanelContext,
    primary: Color,
    muted: Color,
    _border_c: Color,
) -> Column<'a, PanelMsg> {
    use crate::library::editor::footprint::state::SnappingMode as M;
    let current = fp.snapping_mode;
    // v0.13 — Match the Grids/Guides/Axes pill chrome above. Mutex
    // semantics (clicking one selects only that mode), but the same
    // border / fill / padding so the row reads visually identical.
    let chip_border = Color::from_rgba8(0xE7, 0x8B, 0x2A, 1.0);
    let active_bg = Color::from_rgba8(0x2E, 0x33, 0x45, 1.0);
    let inactive_bg = Color::from_rgba8(0x1A, 0x1D, 0x28, 1.0);
    let mk_pill = move |label: &'static str, target: M, active: bool| -> Element<'static, PanelMsg> {
        iced::widget::button(
            text(label)
                .size(10)
                .color(if active { primary } else { muted })
                .align_x(iced::alignment::Horizontal::Center),
        )
        .padding([3, 12])
        .on_press(PanelMsg::FpEditorSetSnappingMode(target))
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let bg = match status {
                iced::widget::button::Status::Hovered => Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.06))),
                _ => Some(Background::Color(if active { active_bg } else { inactive_bg })),
            };
            iced::widget::button::Style {
                background: bg,
                border: Border {
                    width: 1.0,
                    radius: 2.0.into(),
                    color: chip_border,
                },
                ..iced::widget::button::Style::default()
            }
        })
        .into()
    };
    col = col.push(
        container(text("Snap layers").size(10).color(muted))
            .padding([4, 8])
            .width(Length::Fill),
    );
    col = col.push(
        container(
            row![
                mk_pill("All Layers", M::AllLayers, current == M::AllLayers),
                mk_pill("Current Layer", M::CurrentLayer, current == M::CurrentLayer),
                mk_pill("Off", M::Off, current == M::Off),
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center),
        )
        .padding([2, 8])
        .width(Length::Fill),
    );
    col
}

/// v0.18.14.2 — Snap Options sub-tab strip (Grids / Guides / Axes).
/// Mirrors the schematic Properties tab-row visual rhythm. The
/// active sub-tab paints with the accent background; clicking a
/// pill sets `state.snap_subtab` via `FpEditorSetSnapSubTab`.
fn render_snap_subtab_row<'a>(
    mut col: Column<'a, PanelMsg>,
    fp: &'a FootprintEditorPanelContext,
    primary: Color,
    muted: Color,
    border_c: Color,
) -> Column<'a, PanelMsg> {
    use crate::library::editor::footprint::state::SnapSubTab as T;
    let current = fp.snap_subtab;
    let mk_pill =
        move |label: &'static str, target: T, active: bool| -> Element<'static, PanelMsg> {
            let bg = if active {
                iced::Color::from_rgba(0.40, 0.70, 1.00, 0.18)
            } else {
                iced::Color::from_rgba(1.0, 1.0, 1.0, 0.04)
            };
            let txt = if active { primary } else { muted };
            iced::widget::button(container(text(label).size(10).color(txt)).padding([2, 8]))
                .padding(0)
                .on_press(PanelMsg::FpEditorSetSnapSubTab(target))
                .style(move |_: &Theme, _| iced::widget::button::Style {
                    background: Some(iced::Background::Color(bg)),
                    border: iced::Border {
                        width: 1.0,
                        radius: 3.0.into(),
                        color: border_c,
                    },
                    ..iced::widget::button::Style::default()
                })
                .into()
        };
    col = col.push(
        container(
            row![
                mk_pill("Grids", T::Grids, current == T::Grids),
                mk_pill("Guides", T::Guides, current == T::Guides),
                mk_pill("Axes", T::Axes, current == T::Axes),
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center),
        )
        .padding([2, 8])
        .width(Length::Fill),
    );
    col
}

/// v0.18.21 — Grid Manager table. One row per `GridDef`. The active
/// row is highlighted; clicking another row activates it (mirrors its
/// step / display style onto `snap_options`). The footer's Add /
/// Properties / Delete operate on the active row.
fn render_grid_manager<'a>(
    mut col: Column<'a, PanelMsg>,
    fp: &'a FootprintEditorPanelContext,
    primary: Color,
    muted: Color,
    border_c: Color,
) -> Column<'a, PanelMsg> {
    // Header row — Altium PCB Library editor columns: Prior / Name /
    // Color / Origin / Enabled. "Step" stays as a sub-row info line
    // since Altium puts it in the Properties dialog, not the grid row.
    col = col.push(
        container(
            row![
                text("Prior")
                    .size(10)
                    .color(muted)
                    .width(Length::Fixed(40.0)),
                text("Name").size(10).color(muted).width(Length::Fill),
                text("Color")
                    .size(10)
                    .color(muted)
                    .width(Length::Fixed(40.0)),
                text("Origin")
                    .size(10)
                    .color(muted)
                    .width(Length::Fixed(60.0)),
                text("Enabled")
                    .size(10)
                    .color(muted)
                    .width(Length::Fixed(50.0)),
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center),
        )
        .padding([2, 8])
        .width(Length::Fill),
    );
    col = col.push(super::thin_sep(border_c));

    let active_idx = fp.active_grid_idx;
    if fp.grids.is_empty() {
        col = col.push(
            container(text("(no grids)").size(10).color(muted))
                .padding([4, 8])
                .width(Length::Fill),
        );
    } else {
        for (idx, g) in fp.grids.iter().enumerate() {
            let is_active = idx == active_idx;
            let row_bg = if is_active {
                iced::Color::from_rgba(0.30, 0.55, 0.95, 0.16)
            } else {
                iced::Color::TRANSPARENT
            };
            // Color swatch — placeholder using the theme accent until
            // GridDef.color lands. Click does nothing yet.
            let swatch = container(Space::new())
                .width(Length::Fixed(20.0))
                .height(Length::Fixed(14.0))
                .style(move |_: &Theme| iced::widget::container::Style {
                    background: Some(Background::Color(Color::from_rgba8(
                        0xff, 0xff, 0xff, 1.0,
                    ))),
                    border: Border {
                        width: 1.0,
                        radius: 2.0.into(),
                        color: border_c,
                    },
                    ..Default::default()
                });
            // Enabled column — checkbox toggling active grid.
            let enabled_check = iced::widget::checkbox(is_active)
                .on_toggle(move |_| PanelMsg::FpEditorGridSetActive(idx))
                .size(12)
                .spacing(0);
            col = col.push(
                container(
                    row![
                        text(format!("{}", (idx + 1) * 10))
                            .size(10)
                            .color(if is_active { primary } else { muted })
                            .width(Length::Fixed(40.0)),
                        text(g.name.as_str())
                            .size(10)
                            .color(if is_active { primary } else { muted })
                            .width(Length::Fill),
                        container(swatch)
                            .width(Length::Fixed(40.0))
                            .padding([0, 0]),
                        text("0,0")
                            .size(10)
                            .color(muted)
                            .width(Length::Fixed(60.0)),
                        container(enabled_check)
                            .width(Length::Fixed(50.0))
                            .center_x(Length::Shrink),
                    ]
                    .spacing(4)
                    .align_y(iced::Alignment::Center),
                )
                .padding([3, 8])
                .width(Length::Fill)
                .style(move |_: &Theme| iced::widget::container::Style {
                    background: Some(iced::Background::Color(row_bg)),
                    ..Default::default()
                }),
            );
        }
    }
    // Action footer — Add / Properties / Delete using primary
    // (orange-accent) buttons + a unicode trash glyph for Delete to
    // mirror Altium's icon-style footer.
    col = col.push(super::thin_sep(border_c));
    col = col.push(
        container(
            row![
                Space::new().width(Length::Fill),
                grid_manager_btn(
                    "Add",
                    Some(PanelMsg::FpEditorGridManagerAdd),
                    primary,
                    border_c,
                ),
                grid_manager_btn(
                    "Properties",
                    Some(PanelMsg::FpEditorGridManagerProperties),
                    primary,
                    border_c,
                ),
                grid_manager_btn(
                    "\u{1F5D1}",
                    if fp.grids.len() > 1 {
                        Some(PanelMsg::FpEditorGridManagerDelete)
                    } else {
                        None
                    },
                    primary,
                    border_c,
                ),
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center),
        )
        .padding([4, 8])
        .width(Length::Fill),
    );
    col
}

/// v0.18.20 — Guide Manager. One row per guide carrying an enabled
/// checkbox, axis label, position field, and a per-row delete button.
/// Footer surfaces `Add Vertical` / `Add Horizontal` buttons that
/// append a new entry at world (0, 0) on the chosen axis.
fn render_guide_manager<'a>(
    mut col: Column<'a, PanelMsg>,
    fp: &'a FootprintEditorPanelContext,
    primary: Color,
    muted: Color,
    border_c: Color,
) -> Column<'a, PanelMsg> {
    use crate::library::editor::footprint::state::GuideAxis;

    col = col.push(
        container(
            row![
                text("On").size(10).color(muted).width(Length::Fixed(28.0)),
                text("Axis")
                    .size(10)
                    .color(muted)
                    .width(Length::Fixed(60.0)),
                text("Position (mm)")
                    .size(10)
                    .color(muted)
                    .width(Length::Fill),
                text("").size(10).color(muted).width(Length::Fixed(50.0)),
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center),
        )
        .padding([2, 8])
        .width(Length::Fill),
    );

    if fp.guides.is_empty() {
        col = col.push(
            container(text("(no guides)").size(10).color(muted))
                .padding([4, 8])
                .width(Length::Fill),
        );
    } else {
        for (idx, g) in fp.guides.iter().enumerate() {
            let axis_label = match g.axis {
                GuideAxis::Vertical => "Vert",
                GuideAxis::Horizontal => "Horiz",
            };
            let pos_str = format!("{:.3}", g.position_mm);
            let toggle_label = if g.enabled { "X" } else { " " };
            col = col.push(
                container(
                    row![
                        iced::widget::button(text(toggle_label).size(10).color(primary))
                            .padding([2, 6])
                            .style(iced::widget::button::secondary)
                            .on_press(PanelMsg::FpEditorGuideToggle(idx))
                            .width(Length::Fixed(24.0)),
                        text(axis_label)
                            .size(10)
                            .color(primary)
                            .width(Length::Fixed(60.0)),
                        iced::widget::text_input("0.000", &pos_str)
                            .size(10)
                            .padding([2, 4])
                            .on_input(move |raw| { PanelMsg::FpEditorGuideSetPosition(idx, raw) })
                            .width(Length::Fill),
                        grid_manager_btn(
                            "Del",
                            Some(PanelMsg::FpEditorGuideDelete(idx)),
                            primary,
                            border_c,
                        ),
                    ]
                    .spacing(4)
                    .align_y(iced::Alignment::Center),
                )
                .padding([2, 8])
                .width(Length::Fill),
            );
        }
    }

    col = col.push(
        container(
            row![
                grid_manager_btn(
                    "Add Vertical",
                    Some(PanelMsg::FpEditorGuideAddVertical),
                    primary,
                    border_c,
                ),
                grid_manager_btn(
                    "Add Horizontal",
                    Some(PanelMsg::FpEditorGuideAddHorizontal),
                    primary,
                    border_c,
                ),
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center),
        )
        .padding([4, 8])
        .width(Length::Fill),
    );
    col
}

/// v0.18.13 — Other section. Today carries only a Units toggle;
/// future home for additional document-level options.
fn render_other_section<'a>(
    mut col: Column<'a, PanelMsg>,
    _fp: &'a FootprintEditorPanelContext,
    primary: Color,
    muted: Color,
    _border_c: Color,
    input_bg: Color,
    input_bdr: Color,
    unit: signex_types::coord::Unit,
    seg_hover: Color,
) -> Column<'a, PanelMsg> {
    use signex_types::coord::Unit;
    // Units row — mm/mils segmented selector (Altium parity). Reuses
    // the schematic Properties panel's `seg_btn` widget so the chrome
    // matches byte-for-byte.
    col = col.push(super::form_label("Units", muted));
    col = col.push(
        container(
            row![
                super::seg_btn(
                    "mm",
                    unit == Unit::Mm,
                    PanelMsg::SetUnit(Unit::Mm),
                    input_bg,
                    primary,
                    muted,
                    seg_hover,
                    input_bdr,
                ),
                super::seg_btn(
                    "mils",
                    unit == Unit::Mil,
                    PanelMsg::SetUnit(Unit::Mil),
                    input_bg,
                    primary,
                    muted,
                    seg_hover,
                    input_bdr,
                ),
            ]
            .spacing(0.0)
            .width(Length::Fill),
        )
        .padding([2, 8]),
    );
    col
}

/// Shared button factory for the Grid / Guide Manager footers.
/// Uses iced's built-in `button::primary` (accent-filled) so the
/// chrome matches the "+ Add Filter" call-to-action button in the
/// Custom Selection Filters section above.
fn grid_manager_btn<'a>(
    label: &'static str,
    on_press: Option<PanelMsg>,
    primary: Color,
    _border_c: Color,
) -> Element<'a, PanelMsg> {
    let mut btn = iced::widget::button(text(label).size(10).color(primary))
        .padding([4, 10])
        .style(iced::widget::button::primary);
    if let Some(msg) = on_press {
        btn = btn.on_press(msg);
    }
    btn.into()
}

/// Footprint selection-filter pill — Altium-style toggle button using
/// the schematic Properties panel's chip styling for visual parity.
/// Active = solid accent border + white text; inactive = muted bg.
fn fp_filter_pill(
    label: &'static str,
    kind: crate::library::editor::footprint::state::SelectionFilterKind,
    enabled: bool,
    hover_bg: Color,
    border_c: Color,
) -> Element<'static, PanelMsg> {
    let active_bg = Color::from_rgba8(0x2E, 0x33, 0x45, 1.0);
    let inactive_bg = Color::from_rgba8(0x1A, 0x1D, 0x28, 1.0);
    let text_on = Color::WHITE;
    let text_off = Color::from_rgba8(0x66, 0x6A, 0x7E, 1.0);
    iced::widget::button(
        text(label.to_string())
            .size(10)
            .color(if enabled { text_on } else { text_off })
            .align_x(iced::alignment::Horizontal::Center),
    )
    .padding([3, 8])
    .on_press(PanelMsg::FpEditorToggleSelectionFilter(kind))
    .style(move |_: &Theme, status: iced::widget::button::Status| {
        let bg = match status {
            iced::widget::button::Status::Hovered => Background::Color(hover_bg),
            _ => Background::Color(if enabled { active_bg } else { inactive_bg }),
        };
        iced::widget::button::Style {
            background: Some(bg),
            border: Border {
                width: 1.0,
                radius: 2.0.into(),
                color: border_c,
            },
            text_color: if enabled { text_on } else { text_off },
            ..iced::widget::button::Style::default()
        }
    })
    .into()
}

/// v0.13 — Altium-style Selection Filter pill grid. Replaces the
/// schematic's tab/preset selection-filter widget with a flat row of
/// 10 pills (3D Bodies / Keepouts / Tracks / Arcs / Pads / Vias /
/// Regions / Fills / Texts / Other) + a Custom modal launcher.
fn render_fp_selection_filter<'a>(
    mut col: iced::widget::Column<'a, PanelMsg>,
    fp: &'a FootprintEditorPanelContext,
    accent_c: Color,
    tag_hover: Color,
) -> iced::widget::Column<'a, PanelMsg> {
    use crate::library::editor::footprint::state::SelectionFilterKind as K;

    // Custom button — opens the existing FpEditorOpenSelectionFilterCustom
    // modal where users can tweak the per-kind flags and save presets.
    col = col.push(
        container(
            iced::widget::button(text("Custom").size(10))
                .padding([3, 10])
                .style(iced::widget::button::secondary)
                .on_press(PanelMsg::FpEditorOpenSelectionFilterCustom),
        )
        .padding([4, 8])
        .width(Length::Fill),
    );

    // Pill row — 10 kinds in Altium's display order. Wrap so the
    // grid reflows on narrow panels.
    let f = fp.selection_filter;
    let mut wrap = iced_aw::Wrap::new().spacing(4.0).line_spacing(4.0);
    for &(label, kind) in &[
        ("3D Bodies", K::Bodies3d),
        ("Keepouts", K::Keepouts),
        ("Tracks", K::Tracks),
        ("Arcs", K::Arcs),
        ("Pads", K::Pads),
        ("Vias", K::Vias),
        ("Regions", K::Regions),
        ("Fills", K::Fills),
        ("Texts", K::Texts),
        ("Other", K::Other),
    ] {
        wrap = wrap.push(fp_filter_pill(label, kind, f.get(kind), tag_hover, accent_c));
    }
    col = col.push(container(wrap).padding([2, 8]).width(Length::Fill));
    col
}

/// v0.16.4 — Pour role sub-form. Renders when the entity's `pour`
/// attr is set; otherwise the column passes through unchanged.
fn render_pour_subform<'a>(
    mut col: Column<'a, PanelMsg>,
    fp: &'a FootprintEditorPanelContext,
    id: signex_sketch::id::SketchEntityId,
    muted: Color,
    primary: Color,
    border_c: Color,
) -> Column<'a, PanelMsg> {
    let Some(pour) = fp.selected_pour.as_ref() else {
        return col;
    };
    col = col.push(
        container(text("Pour properties").size(10).color(primary))
            .padding([4, 8])
            .width(Length::Fill),
    );

    // Net (text input — empty = unassigned)
    let net_buf = pour.net.clone().unwrap_or_default();
    col = col.push(
        container(
            row![
                text("Net").size(10).color(muted).width(Length::Fixed(80.0)),
                text_input("(none)", &net_buf)
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
                    .on_input(move |v| PanelMsg::FpEditorSetPourNet { id, value: v }),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .padding([2, 8])
        .width(Length::Fill),
    );

    // Fill type (Solid / Hatched / Outline)
    let fill_picker = pick_list(
        signex_sketch::attr::PourFillType::ALL,
        Some(pour.fill_type),
        move |v| PanelMsg::FpEditorSetPourFillType { id, value: v },
    )
    .text_size(10)
    .padding([3, 8]);
    col = col.push(
        container(
            row![
                text("Fill")
                    .size(10)
                    .color(muted)
                    .width(Length::Fixed(80.0)),
                fill_picker,
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .padding([2, 8])
        .width(Length::Fill),
    );

    // Priority (u32 text input)
    col = col.push(
        container(
            row![
                text("Priority")
                    .size(10)
                    .color(muted)
                    .width(Length::Fixed(80.0)),
                text_input("0", &pour.priority.to_string())
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
                    .on_input(move |v| PanelMsg::FpEditorSetPourPriority { id, value: v }),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .padding([2, 8])
        .width(Length::Fill),
    );

    col
}

/// v0.16.4 — Keepout role sub-form. Renders the 6 kind flags as a
/// vertical checklist when the entity's `keepout` attr is set.
fn render_keepout_subform<'a>(
    mut col: Column<'a, PanelMsg>,
    fp: &'a FootprintEditorPanelContext,
    id: signex_sketch::id::SketchEntityId,
    muted: Color,
    primary: Color,
    _border_c: Color,
) -> Column<'a, PanelMsg> {
    let Some(k) = fp.selected_keepout.as_ref() else {
        return col;
    };
    col = col.push(
        container(text("Keepout kinds").size(10).color(primary))
            .padding([4, 8])
            .width(Length::Fill),
    );

    // v0.18.25 — keepout kinds use the schematic's `form_check_row`
    // (real iced checkbox + On/Off) so the chrome matches the
    // schematic Properties panel byte-for-byte.
    col = col.push(super::form_check_row(
        "No routing",
        k.no_routing,
        PanelMsg::FpEditorSetKeepoutKind {
            id,
            kind: KeepoutKindFlag::NoRouting,
            value: !k.no_routing,
        },
        muted,
    ));
    col = col.push(super::form_check_row(
        "No components",
        k.no_components,
        PanelMsg::FpEditorSetKeepoutKind {
            id,
            kind: KeepoutKindFlag::NoComponents,
            value: !k.no_components,
        },
        muted,
    ));
    col = col.push(super::form_check_row(
        "No copper",
        k.no_copper,
        PanelMsg::FpEditorSetKeepoutKind {
            id,
            kind: KeepoutKindFlag::NoCopper,
            value: !k.no_copper,
        },
        muted,
    ));
    col = col.push(super::form_check_row(
        "No vias",
        k.no_vias,
        PanelMsg::FpEditorSetKeepoutKind {
            id,
            kind: KeepoutKindFlag::NoVias,
            value: !k.no_vias,
        },
        muted,
    ));
    col = col.push(super::form_check_row(
        "No drilling",
        k.no_drilling,
        PanelMsg::FpEditorSetKeepoutKind {
            id,
            kind: KeepoutKindFlag::NoDrilling,
            value: !k.no_drilling,
        },
        muted,
    ));
    col = col.push(super::form_check_row(
        "No pours",
        k.no_pours,
        PanelMsg::FpEditorSetKeepoutKind {
            id,
            kind: KeepoutKindFlag::NoPours,
            value: !k.no_pours,
        },
        muted,
    ));

    col
}

/// v0.16.4 — BoardCutout role sub-form. Edge-radius expression input
/// + through-vs-partial-depth toggle.
fn render_cutout_subform<'a>(
    mut col: Column<'a, PanelMsg>,
    fp: &'a FootprintEditorPanelContext,
    id: signex_sketch::id::SketchEntityId,
    muted: Color,
    primary: Color,
    border_c: Color,
) -> Column<'a, PanelMsg> {
    let Some(c) = fp.selected_cutout.as_ref() else {
        return col;
    };
    col = col.push(
        container(text("Cutout properties").size(10).color(primary))
            .padding([4, 8])
            .width(Length::Fill),
    );

    let radius_buf = c.edge_radius_expr.clone().unwrap_or_default();
    col = col.push(
        container(
            row![
                text("Edge radius")
                    .size(10)
                    .color(muted)
                    .width(Length::Fixed(80.0)),
                text_input("(sharp)", &radius_buf)
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
                    .on_input(move |v| PanelMsg::FpEditorSetCutoutEdgeRadius { id, value: v }),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .padding([2, 8])
        .width(Length::Fill),
    );

    col = col.push(super::form_check_row(
        "Through (full board depth)",
        c.through,
        PanelMsg::FpEditorSetCutoutThrough {
            id,
            value: !c.through,
        },
        muted,
    ));

    col
}

/// Read-only key-value row — delegates to the schematic Properties
/// panel's `form_input_row` so the footprint editor uses identical
/// chrome (orange-accent border, dark-blue selection-tinted background).
/// Returns the updated Column to keep the chained-update call style
/// the rest of this module uses.
fn props_kv_row<'a>(
    col: Column<'a, PanelMsg>,
    label_c: Color,
    input_bg: Color,
    input_bdr: Color,
    key: &str,
    value: String,
) -> Column<'a, PanelMsg> {
    col.push(super::form_input_row(key, &value, label_c, input_bg, input_bdr))
}
