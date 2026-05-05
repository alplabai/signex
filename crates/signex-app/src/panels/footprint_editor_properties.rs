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
            col = col.push(props_section_header("Pad", primary, border_c));
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
        }
        (FootprintModeKind::Sketch, _, Some(ent)) => {
            col = col.push(props_section_header("Sketch entity", primary, border_c));
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

            // v0.16.2 — Role pick_list. Visible when an entity is
            // selected; pick_list value mirrors the entity's
            // currently-attached `*Attr` slot (or `Unassigned`).
            col = col.push(props_section_header("Role", primary, border_c));
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
        _ => {
            // v0.18.24 — silk-front graphic selection branch.
            // Renders BEFORE the empty-canvas Library Options when a
            // silk graphic is selected so the user can edit Text
            // content + delete the entry without leaving the
            // Properties panel.
            if let Some(silk) = fp.selected_silk_summary.as_ref() {
                col = col.push(props_section_header("Silk graphic", primary, border_c));
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
            } else {
                // v0.18.13 — Altium "Library Options" no-selection
                // layout (Selection Filter at the top, full 5-section
                // body assembled after the match). The Footprint
                // summary remains as auxiliary kv-rows. Selection Filter
                // chrome reuses the schematic's
                // `view_custom_selection_filters_section` so both
                // editors render the EXACT same widget.
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
                col = col.push(props_section_header("Footprint", primary, border_c));
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
    col = col.push(props_section_header("Snap Options", primary, border_c));
    // v0.18.14.2 — Altium Snap Options sub-tab row (Grids / Guides /
    // Axes). Grids body renders the snap-to-grid checkbox stack;
    // Guides drives the Guide Manager (v0.18.20); Axes is the
    // remaining unbuilt surface (per-axis snap on SnapOptions).
    col = render_snap_subtab_row(col, fp, primary, muted, border_c);
    if fp.snap_subtab != crate::library::editor::footprint::state::SnapSubTab::Grids {
        let hint = match fp.snap_subtab {
            crate::library::editor::footprint::state::SnapSubTab::Guides => {
                "Guide Manager below — add horizontal / vertical guides; toggle individual lines via the row checkbox."
            }
            crate::library::editor::footprint::state::SnapSubTab::Axes => {
                "Per-axis snap (Snap Grid X / Y) is unbuilt — uses the Grids step uniformly for now."
            }
            _ => "",
        };
        col = col.push(
            container(text(hint).size(10).color(muted))
                .padding([4, 8])
                .width(Length::Fill),
        );
        // Sketch-mode-only sections + Library Options sections still
        // render below for the user's other Properties needs even
        // when the Grids sub-tab isn't selected.
        if no_selection {
            col = col.push(props_section_header("Grid Manager", primary, border_c));
            col = render_grid_manager(col, fp, primary, muted, border_c);
            col = col.push(props_section_header("Guide Manager", primary, border_c));
            col = render_guide_manager(col, fp, primary, muted, border_c);
            col = col.push(props_section_header("Other", primary, border_c));
            col = render_other_section(col, fp, primary, muted, border_c);
        }
        return scrollable(col).width(Length::Fill).into();
    }
    // v0.18.14.3 — Altium "Snapping" 3-segment toggle (All Layers /
    // Current Layer / Off). `Off` short-circuits all priorities in
    // `snap::snap_cursor`; `CurrentLayer` is a placeholder for the
    // v0.18.15 layer-aware enforcement.
    col = render_snapping_mode_row(col, fp, primary, muted, border_c);
    let opts = fp.snap_options;
    // v0.18.25 — snap toggles use the schematic's `form_check_row`
    // (real iced checkbox + On/Off label) so the chrome matches the
    // schematic Properties panel byte-for-byte.
    col = col.push(super::form_check_row(
        "Snap to Point",
        opts.point_hit,
        PanelMsg::FpEditorToggleSnapOption(SnapOptionFlag::PointHit),
        muted,
    ));
    col = col.push(super::form_check_row(
        "Snap horizontal/vertical (5°)",
        opts.horizontal_vertical,
        PanelMsg::FpEditorToggleSnapOption(SnapOptionFlag::HorizontalVertical),
        muted,
    ));
    col = col.push(super::form_check_row(
        "Snap angle (15° steps)",
        opts.angle,
        PanelMsg::FpEditorToggleSnapOption(SnapOptionFlag::Angle),
        muted,
    ));
    col = col.push(super::form_check_row(
        &format!("Snap to grid ({:.3} mm)", opts.grid_step_mm),
        opts.grid,
        PanelMsg::FpEditorToggleSnapOption(SnapOptionFlag::Grid),
        muted,
    ));
    // v0.18.9 — author-controlled grid step. Numeric input bound to
    // `state.snap_options.grid_step_mm`. The G key (v0.18.10)
    // populates this from the standard 1mil…2.5mm ladder; the
    // Ctrl+G modal (v0.18.11) opens a richer editor.
    col = col.push(
        container(
            row![
                text("Grid step (mm)")
                    .size(10)
                    .color(muted)
                    .width(Length::Fixed(110.0)),
                text_input("1.0", &format!("{:.3}", opts.grid_step_mm))
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
                    .on_input(PanelMsg::FpEditorSetSnapGridStep),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .padding([2, 8])
        .width(Length::Fill),
    );

    // v0.18.13 — Library Options layout (Grid Manager / Guide
    // Manager / Other) below Snap Options, only on the no-selection
    // body to mirror Altium's per-state Properties surface.
    if no_selection {
        col = col.push(props_section_header("Grid Manager", primary, border_c));
        col = render_grid_manager(col, fp, primary, muted, border_c);
        col = col.push(props_section_header("Guide Manager", primary, border_c));
        col = render_guide_manager(col, fp, primary, muted, border_c);
        col = col.push(props_section_header("Other", primary, border_c));
        col = render_other_section(col, fp, primary, muted, border_c);
    }

    // v0.16.2 — Sketch-mode-only sections (Parameters, DOF, Solve
    // warnings). Always visible when the editor is in Sketch mode so
    // the user can monitor solve state while authoring, regardless of
    // whether anything is selected. Migrated out of the bottom-of-canvas
    // inspector strip that shipped in v0.13.1.
    if fp.mode_kind == FootprintModeKind::Sketch {
        // Parameters
        col = col.push(props_section_header("Parameters", primary, border_c));
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

        // DOF / Last solve
        col = col.push(props_section_header("DOF / Last solve", primary, border_c));
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

        // Solve warnings
        col = col.push(props_section_header("Solve warnings", primary, border_c));
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
    }

    // v0.16.3 — "Pad placement" defaults form. Visible whenever the
    // user is in Pads mode + the PlacePad tool is active. TAB pause
    // adds a "PAUSED — TAB to resume" hint but doesn't gate the form
    // itself; the user can adjust before resuming.
    if fp.placement_active {
        col = col.push(props_section_header("Pad placement", primary, border_c));
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
    }

    // Settings + hint — always visible at the bottom of the panel
    // regardless of mode / selection so common toggles stay reachable.
    col = col.push(props_section_header("Settings", primary, border_c));
    let auto_fit_label = if fp.auto_fit_courtyard {
        "Auto-fit Courtyard \u{2713}"
    } else {
        "Auto-fit Courtyard"
    };
    let auto_fit_btn = iced::widget::button(text(auto_fit_label).size(10).color(primary))
        .padding([4, 10])
        .on_press(PanelMsg::FpEditorToggleAutoFitCourtyard)
        .style(iced::widget::button::secondary);
    col = col.push(container(auto_fit_btn).padding([4, 8]).width(Length::Fill));

    col = col.push(props_section_header("Hint", primary, border_c));
    let hint = match fp.mode_kind {
        FootprintModeKind::Pads => "Click a pad to edit its properties.",
        FootprintModeKind::Sketch => {
            "Click a sketch entity (Point / Line / Arc / Circle) to edit it."
        }
        FootprintModeKind::View3d => "3D View — use the 3D preview pane to inspect the body.",
    };
    col = col.push(
        container(text(hint).size(10).color(muted))
            .padding([4, 8])
            .width(Length::Fill),
    );

    scrollable(col).width(Length::Fill).into()
}

/// Section header — delegates to the schematic Properties panel's
/// `section_hdr` so the footprint editor uses the exact same widget
/// (chevron prefix + bold title + 1px rule). Wrapper exists only to
/// add the chevron without polluting every call site.
fn props_section_header<'a>(
    label: &str,
    primary: Color,
    border_c: Color,
) -> iced::widget::Column<'a, PanelMsg> {
    super::section_hdr(&format!("\u{25BC} {label}"), primary, border_c)
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
    border_c: Color,
) -> Column<'a, PanelMsg> {
    use crate::library::editor::footprint::state::SnappingMode as M;
    let current = fp.snapping_mode;
    let mk_segment =
        move |label: &'static str, target: M, active: bool| -> Element<'static, PanelMsg> {
            let bg = if active {
                iced::Color::from_rgba(0.40, 0.70, 1.00, 0.20)
            } else {
                iced::Color::from_rgba(1.0, 1.0, 1.0, 0.04)
            };
            let txt = if active { primary } else { muted };
            iced::widget::button(
                container(
                    text(label)
                        .size(10)
                        .color(txt)
                        .align_x(iced::alignment::Horizontal::Center),
                )
                .padding([3, 8])
                .width(Length::FillPortion(1))
                .center_x(Length::Fill),
            )
            .padding(0)
            .width(Length::FillPortion(1))
            .on_press(PanelMsg::FpEditorSetSnappingMode(target))
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
        container(text("Snapping").size(10).color(muted))
            .padding([4, 8])
            .width(Length::Fill),
    );
    col = col.push(
        container(
            row![
                mk_segment("All Layers", M::AllLayers, current == M::AllLayers),
                mk_segment("Current Layer", M::CurrentLayer, current == M::CurrentLayer),
                mk_segment("Off", M::Off, current == M::Off),
            ]
            .spacing(2)
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
    // Header row.
    col = col.push(
        container(
            row![
                text("Active")
                    .size(10)
                    .color(muted)
                    .width(Length::Fixed(50.0)),
                text("Name").size(10).color(muted).width(Length::Fill),
                text("Step")
                    .size(10)
                    .color(muted)
                    .width(Length::Fixed(80.0)),
                text("×").size(10).color(muted).width(Length::Fixed(28.0)),
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center),
        )
        .padding([2, 8])
        .width(Length::Fill),
    );

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
            let toggle_label = if is_active { "●" } else { "○" };
            let row_bg = if is_active {
                iced::Color::from_rgba(0.30, 0.55, 0.95, 0.16)
            } else {
                iced::Color::TRANSPARENT
            };
            col = col.push(
                container(
                    row![
                        iced::widget::button(text(toggle_label).size(10).color(primary))
                            .padding([2, 6])
                            .style(iced::widget::button::secondary)
                            .on_press(PanelMsg::FpEditorGridSetActive(idx))
                            .width(Length::Fixed(40.0)),
                        text(g.name.as_str())
                            .size(10)
                            .color(primary)
                            .width(Length::Fill),
                        text(format!("{:.3} mm", g.step_mm))
                            .size(10)
                            .color(primary)
                            .width(Length::Fixed(80.0)),
                        text(format!("×{}", g.coarse_multiplier))
                            .size(10)
                            .color(muted)
                            .width(Length::Fixed(28.0)),
                    ]
                    .spacing(4)
                    .align_y(iced::Alignment::Center),
                )
                .padding([2, 8])
                .width(Length::Fill)
                .style(move |_: &Theme| iced::widget::container::Style {
                    background: Some(iced::Background::Color(row_bg)),
                    ..Default::default()
                }),
            );
        }
    }
    // Action footer — Add / Properties / Delete.
    col = col.push(
        container(
            row![
                grid_manager_btn(
                    "Add",
                    Some(PanelMsg::FpEditorGridManagerAdd),
                    primary,
                    border_c
                ),
                grid_manager_btn(
                    "Properties",
                    Some(PanelMsg::FpEditorGridManagerProperties),
                    primary,
                    border_c,
                ),
                grid_manager_btn(
                    "Delete",
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
) -> Column<'a, PanelMsg> {
    col = col.push(
        container(
            row![
                text("Units")
                    .size(10)
                    .color(muted)
                    .width(Length::Fixed(80.0)),
                text("mm").size(10).color(primary),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        )
        .padding([2, 8])
        .width(Length::Fill),
    );
    col
}

/// Shared button factory for the Grid / Guide Manager footers.
/// Uses iced's built-in `button::secondary` so the chrome matches the
/// schematic Properties panel's action buttons (Reset to Default, etc.).
fn grid_manager_btn<'a>(
    label: &'static str,
    on_press: Option<PanelMsg>,
    primary: Color,
    _border_c: Color,
) -> Element<'a, PanelMsg> {
    let mut btn = iced::widget::button(text(label).size(10).color(primary))
        .padding([4, 10])
        .style(iced::widget::button::secondary);
    if let Some(msg) = on_press {
        btn = btn.on_press(msg);
    }
    btn.into()
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
