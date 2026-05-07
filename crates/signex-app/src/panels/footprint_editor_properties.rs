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
    CollapsedSections, FootprintEditorPanelContext, FootprintModeKind, FootprintPadSummary,
    KeepoutKindFlag, PanelMsg, SnapOptionFlag,
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

    // v0.20 — Altium-parity context-aware Properties panel for the
    // Pads workspace. Two early-return short-circuits handle the
    // selection / placement cases; the empty-canvas (no selection,
    // no placement) state falls through to the original match block
    // below so Custom Selection Filters / Footprint / Snap Options
    // / Grid Manager / Other / Settings / Hint stay reachable when
    // the user has nothing in focus.
    //
    //   - Pad selected (and not mid-placement) → editable Pad form.
    //   - Placement tool armed or TAB-paused → next-pad-defaults form.
    //   - Otherwise → fall through to the existing empty-canvas chrome.
    if fp.mode_kind == FootprintModeKind::Pads {
        let in_placement = fp.placement_active || fp.placement_paused;
        if let Some(pad) = fp.selected_pad.as_ref() {
            if !in_placement {
                let values = PadFormValues::from_selected_pad(pad, fp);
                let target = PadEditTarget::Selected(pad.idx);
                col = render_pad_form_properties(
                    col, &values, target, false, muted, primary, border_c, collapsed_sections,
                );
                col = props_kv_row(
                    col,
                    muted,
                    input_bg,
                    input_bdr,
                    "Position",
                    format!(
                        "({:.3}, {:.3}) mm",
                        pad.position_mm[0], pad.position_mm[1]
                    ),
                );
                col = render_pad_form_pad_stack(
                    col, &values, target, muted, primary, border_c, collapsed_sections,
                );
                col = render_pad_form_pad_features(
                    col, &values, target, muted, primary, border_c, collapsed_sections,
                );
                // v0.21 — "Edit in Sketch" jump button. Visible only
                // when the pad has a backing sketch entity (auto-
                // minted on first Sketch-mode entry or placed via
                // sketch). The handler switches editor.state.mode to
                // Sketch + selects entity_id; if the pad has no
                // sketch entity yet, this is a no-op.
                let pad_idx = pad.idx;
                col = col.push(
                    container(
                        iced::widget::button(
                            text("Edit in Sketch ▸").size(10).color(primary),
                        )
                        .padding([4, 10])
                        .on_press(PanelMsg::FpEditorEditPadInSketch { pad_idx })
                        .style(iced::widget::button::primary),
                    )
                    .padding([6, 8])
                    .width(Length::Fill),
                );
                return scrollable(col).width(Length::Fill).into();
            }
        }
        if in_placement {
            let values = PadFormValues::from_next_pad(fp);
            let target = PadEditTarget::Next;
            col = render_pad_form_properties(
                col,
                &values,
                target,
                fp.placement_paused,
                muted,
                primary,
                border_c,
                collapsed_sections,
            );
            col = render_pad_form_pad_stack(
                col, &values, target, muted, primary, border_c, collapsed_sections,
            );
            col = render_pad_form_pad_features(
                col, &values, target, muted, primary, border_c, collapsed_sections,
            );
            return scrollable(col).width(Length::Fill).into();
        }
        // Empty canvas + idle → fall through to the original chrome.
    }

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
        (FootprintModeKind::Pads, Some(pad), _) if !fp.placement_active && !fp.placement_paused => {
            // v0.20 — selected-pad surface mirrors the placement form
            // (Properties / Pad Stack / Pad Features) but binds every
            // field to the selected pad via PadEditTarget::Selected.
            // Position is read-only (move-by-drag is the gesture for
            // it); everything else writes back through the dispatcher.
            let values = PadFormValues::from_selected_pad(pad, fp);
            let target = PadEditTarget::Selected(pad.idx);
            col = render_pad_form_properties(col, &values, target, false, muted, primary, border_c, collapsed_sections);
            // Position is the one read-only field (use drag to move).
            col = props_kv_row(
                col,
                muted,
                input_bg,
                input_bdr,
                "Position",
                format!("({:.3}, {:.3}) mm", pad.position_mm[0], pad.position_mm[1]),
            );
            col = render_pad_form_pad_stack(col, &values, target, muted, primary, border_c, collapsed_sections);
            col = render_pad_form_pad_features(col, &values, target, muted, primary, border_c, collapsed_sections);
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
            // v0.22 Phase A3+A4 — DOF state row. Surfaces the solver's
            // per-Point colour code (blue under / green fully / red
            // over) so the user can see the constraint state without
            // hunting for the canvas tint. Hidden when the entity has
            // no DOF colour (non-Point entities or pre-solve state).
            if let Some(dof) = ent.dof_state {
                use signex_sketch::solver::dof::DofColor;
                let (label, c) = match dof {
                    DofColor::Under => (
                        "Under-constrained",
                        Color::from_rgba(0.20, 0.40, 1.00, 1.00),
                    ),
                    DofColor::Full => (
                        "Fully constrained",
                        Color::from_rgba(0.20, 0.85, 0.30, 1.00),
                    ),
                    DofColor::Over => (
                        "Over-constrained",
                        Color::from_rgba(1.00, 0.20, 0.20, 1.00),
                    ),
                };
                col = col.push(
                    container(
                        row![
                            container(text("DOF").size(10).color(muted))
                                .width(Length::FillPortion(1)),
                            container(text(label).size(10).color(c))
                                .width(Length::FillPortion(2)),
                        ]
                        .align_y(iced::Alignment::Center),
                    )
                    .padding([3, 8])
                    .width(Length::Fill),
                );
            }
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
                    col = render_sketch_pad_subform(
                        col,
                        fp,
                        muted,
                        primary,
                        border_c,
                        collapsed_sections,
                    );
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

                use crate::panels::SilkKindGeometry;
                match &silk.kind {
                    SilkKindGeometry::Line { from_mm, to_mm } => {
                        col = col.push(pad_input_row(
                            "From X (mm)",
                            "0",
                            format!("{:.3}", from_mm[0]),
                            PanelMsg::FpEditorSetSilkLineFromX,
                            muted, primary, border_c,
                        ));
                        col = col.push(pad_input_row(
                            "From Y (mm)",
                            "0",
                            format!("{:.3}", from_mm[1]),
                            PanelMsg::FpEditorSetSilkLineFromY,
                            muted, primary, border_c,
                        ));
                        col = col.push(pad_input_row(
                            "To X (mm)",
                            "0",
                            format!("{:.3}", to_mm[0]),
                            PanelMsg::FpEditorSetSilkLineToX,
                            muted, primary, border_c,
                        ));
                        col = col.push(pad_input_row(
                            "To Y (mm)",
                            "0",
                            format!("{:.3}", to_mm[1]),
                            PanelMsg::FpEditorSetSilkLineToY,
                            muted, primary, border_c,
                        ));
                    }
                    SilkKindGeometry::Text { position_mm, content, size_mm } => {
                        col = col.push(pad_input_row(
                            "Content",
                            "TEXT",
                            content.clone(),
                            PanelMsg::FpEditorSetSilkText,
                            muted, primary, border_c,
                        ));
                        col = col.push(pad_input_row(
                            "Position X (mm)",
                            "0",
                            format!("{:.3}", position_mm[0]),
                            PanelMsg::FpEditorSetSilkTextPositionX,
                            muted, primary, border_c,
                        ));
                        col = col.push(pad_input_row(
                            "Position Y (mm)",
                            "0",
                            format!("{:.3}", position_mm[1]),
                            PanelMsg::FpEditorSetSilkTextPositionY,
                            muted, primary, border_c,
                        ));
                        col = col.push(pad_input_row(
                            "Size (mm)",
                            "1.0",
                            format!("{:.3}", size_mm),
                            PanelMsg::FpEditorSetSilkTextSize,
                            muted, primary, border_c,
                        ));
                    }
                    SilkKindGeometry::Other => {
                        col = col.push(
                            container(
                                text("Use Sketch mode for parametric editing")
                                    .size(9)
                                    .color(muted),
                            )
                            .padding([4, 8])
                            .width(Length::Fill),
                        );
                    }
                }

                // Stroke width (all kinds).
                col = col.push(pad_input_row(
                    "Stroke width (mm)",
                    "0.15",
                    format!("{:.3}", silk.stroke_width_mm),
                    PanelMsg::FpEditorSetSilkStrokeWidth,
                    muted, primary, border_c,
                ));
                // Filled flag (only meaningful for closed shapes;
                // surfacing for all so the user can flip it without
                // hunting for the right tool).
                col = col.push(pad_check_row(
                    "Filled",
                    silk.filled,
                    PanelMsg::FpEditorToggleSilkFilled,
                    muted, primary,
                ));
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
                    // Editable Name — text_input bound to the active
                    // internal footprint's `name` field via
                    // PanelMsg::FpEditorSetFootprintName.
                    let name_val = fp.footprint_name.clone();
                    col = col.push(super::form_edit_row(
                        "Name",
                        &name_val,
                        muted,
                        PanelMsg::FpEditorSetFootprintName,
                    ));
                    let _ = (input_bg, input_bdr); // form_edit_row uses iced default text_input chrome

                    // v0.21 — Altium-parity Component fields. Editable
                    // Description / Default Designator / Component
                    // Type / Height in mm. Read-only Version / Mode /
                    // Pads count below for parity with the Altium
                    // PCB Library Component Properties dialog.
                    let desc_val = fp.footprint_description.clone();
                    col = col.push(super::form_edit_row(
                        "Description",
                        &desc_val,
                        muted,
                        PanelMsg::FpEditorSetFootprintDescription,
                    ));
                    let dd_val = fp.footprint_default_designator.clone();
                    col = col.push(super::form_edit_row(
                        "Default Designator",
                        &dd_val,
                        muted,
                        PanelMsg::FpEditorSetFootprintDefaultDesignator,
                    ));
                    col = col.push(pad_pick_row(
                        "Type",
                        signex_library::primitive::footprint::ComponentType::ALL,
                        fp.footprint_component_type,
                        |t: signex_library::primitive::footprint::ComponentType| {
                            PanelMsg::FpEditorSetFootprintComponentType(t)
                        },
                        muted,
                    ));
                    let height_val = fp
                        .footprint_height_mm
                        .map(|v| format!("{v:.3}"))
                        .unwrap_or_default();
                    col = col.push(super::form_edit_row(
                        "Height (mm)",
                        &height_val,
                        muted,
                        PanelMsg::FpEditorSetFootprintHeight,
                    ));
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

    // Settings + Hint footer — always visible at the bottom of the
    // panel regardless of mode / selection so common toggles stay
    // reachable. Factored into a helper so the early-return placement
    // branch above can render the same footer.
    col = render_fp_settings_and_hint(col, fp, muted, primary, border_c, collapsed_sections);

    scrollable(col).width(Length::Fill).into()
}

/// v0.20 — common Settings + Hint footer. Always renders the
/// Auto-fit Courtyard toggle and a mode-specific hint string.
fn render_fp_settings_and_hint<'a>(
    mut col: Column<'a, PanelMsg>,
    fp: &'a FootprintEditorPanelContext,
    muted: Color,
    primary: Color,
    border_c: Color,
    collapsed_sections: &'a CollapsedSections,
) -> Column<'a, PanelMsg> {
    col = col.push(props_section_header(
        "Settings",
        "fp_settings",
        collapsed_sections,
        primary,
        border_c,
    ));
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

    col = col.push(props_section_header(
        "Hint",
        "fp_hint",
        collapsed_sections,
        primary,
        border_c,
    ));
    if !fp_is_collapsed("fp_hint", collapsed_sections) {
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
    }
    col
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

// ───────────────────────────────────────────────────────────────────
// v0.20 — Altium-parity Pad placement properties form.
// Renders three sections under the existing placement-active branch:
// "Properties" (designator / layer / template / library / rotation),
// "Pad Stack" (shape + size + hole + per-side mask & paste + thermal +
// corner radius), and "Pad Features" (top/bottom surface + testpoint).
// All rows write back via the matching FpEditorSet*Pad* messages so
// the dispatcher can route to either `next_pad_defaults` (during
// placement) or `state.pads[idx]` (when a pad is selected) using
// the same render functions.
// ───────────────────────────────────────────────────────────────────

/// v0.20 — which pad the form's edits target. Used by every
/// `pad_*_msg` helper to pick the matching `FpEditorSetNextPad*` or
/// `FpEditorSetSelectedPad*` message variant.
#[derive(Debug, Clone, Copy)]
enum PadEditTarget {
    Next,
    Selected(usize),
}

/// v0.20 — snapshot of the values the form renders. Built from
/// `FootprintEditorPanelContext.next_pad_*` for placement, or from
/// `FootprintPadSummary` for the selected-pad branch.
#[derive(Debug, Clone)]
struct PadFormValues {
    designator: String,
    side: crate::library::editor::footprint::state::PadSide,
    rotation_deg: f64,
    template: String,
    template_library: String,
    shape: signex_library::PadShape,
    kind: signex_library::PadKind,
    size_x_mm: f64,
    size_y_mm: f64,
    drill_diameter_mm: Option<f64>,
    drill_slot_length_mm: Option<f64>,
    stack: crate::library::editor::footprint::state::PadStackUi,
    feature_top: signex_sketch::attr::PadFeature,
    feature_bottom: signex_sketch::attr::PadFeature,
    testpoint: signex_sketch::attr::TestpointFlags,
    /// Active Pad Stack tab. Drives the preview + which body the
    /// Pad Stack section renders.
    pad_stack_tab: crate::library::editor::footprint::state::PadStackTab,
    /// v0.21 — Altium-parity electrical-type / net / locked.
    electrical_type: signex_sketch::attr::ElectricalType,
    net: String,
    locked: bool,
    /// v0.21 — Pad Hole detail fields (Multi-Layer only).
    hole_tolerance_plus_mm: Option<f64>,
    hole_tolerance_minus_mm: Option<f64>,
    hole_rotation_deg: Option<f64>,
    copper_offset_x_mm: Option<f64>,
    copper_offset_y_mm: Option<f64>,
}

impl PadFormValues {
    fn from_next_pad(fp: &FootprintEditorPanelContext) -> Self {
        Self {
            designator: fp.next_pad_designator_override.clone().unwrap_or_default(),
            side: fp.next_pad_side,
            rotation_deg: fp.next_pad_rotation_deg,
            template: fp.next_pad_template.clone(),
            template_library: fp.next_pad_template_library.clone(),
            shape: fp.next_pad_shape.clone(),
            kind: fp.next_pad_kind,
            size_x_mm: fp.next_pad_size_x_mm,
            size_y_mm: fp.next_pad_size_y_mm,
            drill_diameter_mm: fp.next_pad_drill_diameter_mm,
            drill_slot_length_mm: fp.next_pad_drill_slot_length_mm,
            stack: fp.next_pad_stack.clone(),
            feature_top: fp.next_pad_feature_top,
            feature_bottom: fp.next_pad_feature_bottom,
            testpoint: fp.next_pad_testpoint,
            pad_stack_tab: fp.pad_stack_tab,
            electrical_type: fp.next_pad_electrical_type,
            net: fp.next_pad_net.clone(),
            locked: fp.next_pad_locked,
            hole_tolerance_plus_mm: fp.next_pad_hole_tolerance_plus_mm,
            hole_tolerance_minus_mm: fp.next_pad_hole_tolerance_minus_mm,
            hole_rotation_deg: fp.next_pad_hole_rotation_deg,
            copper_offset_x_mm: fp.next_pad_copper_offset_x_mm,
            copper_offset_y_mm: fp.next_pad_copper_offset_y_mm,
        }
    }
    fn from_selected_pad(pad: &FootprintPadSummary, fp: &FootprintEditorPanelContext) -> Self {
        Self {
            designator: pad.number.clone(),
            side: pad.side,
            rotation_deg: pad.rotation_deg,
            template: pad.template.clone(),
            template_library: pad.template_library.clone(),
            shape: pad.shape.clone(),
            kind: pad.kind,
            size_x_mm: pad.size_mm[0],
            // v0.21 — selected-pad hole-detail values pulled from
            // FootprintPadSummary (populated in runtime.rs).
            // (Note: the Default::default()-set fields below are
            // overridden after the struct literal — this comment
            // is intentional anchor for v0.22 cleanup.)
            size_y_mm: pad.size_mm[1],
            drill_diameter_mm: pad.drill_diameter_mm,
            drill_slot_length_mm: None, // selected-pad slot_length follow-up in v0.21
            stack: pad.stack.clone(),
            feature_top: pad.feature_top,
            feature_bottom: pad.feature_bottom,
            testpoint: pad.testpoint,
            pad_stack_tab: fp.pad_stack_tab,
            electrical_type: pad.electrical_type,
            net: pad.net.clone(),
            locked: pad.locked,
            hole_tolerance_plus_mm: pad.hole_tolerance_plus_mm,
            hole_tolerance_minus_mm: pad.hole_tolerance_minus_mm,
            hole_rotation_deg: pad.hole_rotation_deg,
            copper_offset_x_mm: pad.copper_offset_x_mm,
            copper_offset_y_mm: pad.copper_offset_y_mm,
        }
    }
}

fn pad_electrical_type_msg(t: PadEditTarget, v: signex_sketch::attr::ElectricalType) -> PanelMsg {
    match t {
        PadEditTarget::Next => PanelMsg::FpEditorSetNextPadElectricalType(v),
        PadEditTarget::Selected(idx) => {
            PanelMsg::FpEditorSetSelectedPadElectricalType { idx, value: v }
        }
    }
}
fn pad_net_msg(t: PadEditTarget, v: String) -> PanelMsg {
    match t {
        PadEditTarget::Next => PanelMsg::FpEditorSetNextPadNet(v),
        PadEditTarget::Selected(idx) => PanelMsg::FpEditorSetSelectedPadNet { idx, value: v },
    }
}
fn pad_locked_msg(t: PadEditTarget, v: bool) -> PanelMsg {
    match t {
        PadEditTarget::Next => PanelMsg::FpEditorToggleNextPadLocked(v),
        PadEditTarget::Selected(idx) => {
            PanelMsg::FpEditorToggleSelectedPadLocked { idx, value: v }
        }
    }
}
fn pad_hole_tolerance_plus_msg(t: PadEditTarget, v: String) -> PanelMsg {
    match t {
        PadEditTarget::Next => PanelMsg::FpEditorSetNextPadHoleTolerancePlus(v),
        PadEditTarget::Selected(idx) => {
            PanelMsg::FpEditorSetSelectedPadHoleTolerancePlus { idx, value: v }
        }
    }
}
fn pad_hole_tolerance_minus_msg(t: PadEditTarget, v: String) -> PanelMsg {
    match t {
        PadEditTarget::Next => PanelMsg::FpEditorSetNextPadHoleToleranceMinus(v),
        PadEditTarget::Selected(idx) => {
            PanelMsg::FpEditorSetSelectedPadHoleToleranceMinus { idx, value: v }
        }
    }
}
fn pad_hole_rotation_msg(t: PadEditTarget, v: String) -> PanelMsg {
    match t {
        PadEditTarget::Next => PanelMsg::FpEditorSetNextPadHoleRotation(v),
        PadEditTarget::Selected(idx) => {
            PanelMsg::FpEditorSetSelectedPadHoleRotation { idx, value: v }
        }
    }
}
fn pad_copper_offset_x_msg(t: PadEditTarget, v: String) -> PanelMsg {
    match t {
        PadEditTarget::Next => PanelMsg::FpEditorSetNextPadCopperOffsetX(v),
        PadEditTarget::Selected(idx) => {
            PanelMsg::FpEditorSetSelectedPadCopperOffsetX { idx, value: v }
        }
    }
}
fn pad_copper_offset_y_msg(t: PadEditTarget, v: String) -> PanelMsg {
    match t {
        PadEditTarget::Next => PanelMsg::FpEditorSetNextPadCopperOffsetY(v),
        PadEditTarget::Selected(idx) => {
            PanelMsg::FpEditorSetSelectedPadCopperOffsetY { idx, value: v }
        }
    }
}
fn pad_plated_msg(t: PadEditTarget, v: bool) -> PanelMsg {
    match t {
        PadEditTarget::Next => PanelMsg::FpEditorToggleNextPadPlated(v),
        PadEditTarget::Selected(idx) => {
            PanelMsg::FpEditorToggleSelectedPadPlated { idx, value: v }
        }
    }
}

// v0.20 — message constructor helpers. Each switches on PadEditTarget
// to emit the matching FpEditorSetNextPad*/FpEditorSetSelectedPad*
// variant. The form's row builders consume these via a closure
// captured by `move |v| pad_*_msg(target, v)` so the same render
// functions service both targets.
fn pad_designator_msg(t: PadEditTarget, v: String) -> PanelMsg {
    match t {
        PadEditTarget::Next => PanelMsg::FpEditorSetNextPadDesignator(v),
        PadEditTarget::Selected(idx) => PanelMsg::FpEditorSetSelectedPadDesignator { idx, value: v },
    }
}
fn pad_side_msg(t: PadEditTarget, s: crate::library::editor::footprint::state::PadSide) -> PanelMsg {
    match t {
        PadEditTarget::Next => PanelMsg::FpEditorSetNextPadSide(s),
        PadEditTarget::Selected(idx) => PanelMsg::FpEditorSetSelectedPadSide { idx, side: s },
    }
}
fn pad_rotation_msg(t: PadEditTarget, v: String) -> PanelMsg {
    match t {
        PadEditTarget::Next => PanelMsg::FpEditorSetNextPadRotation(v),
        PadEditTarget::Selected(idx) => PanelMsg::FpEditorSetSelectedPadRotation { idx, value: v },
    }
}
fn pad_template_msg(t: PadEditTarget, v: String) -> PanelMsg {
    match t {
        PadEditTarget::Next => PanelMsg::FpEditorSetNextPadTemplate(v),
        PadEditTarget::Selected(idx) => PanelMsg::FpEditorSetSelectedPadTemplate { idx, value: v },
    }
}
fn pad_template_library_msg(t: PadEditTarget, v: String) -> PanelMsg {
    match t {
        PadEditTarget::Next => PanelMsg::FpEditorSetNextPadTemplateLibrary(v),
        PadEditTarget::Selected(idx) => PanelMsg::FpEditorSetSelectedPadTemplateLibrary { idx, value: v },
    }
}
fn pad_shape_msg(t: PadEditTarget, s: signex_library::PadShape) -> PanelMsg {
    match t {
        PadEditTarget::Next => PanelMsg::FpEditorSetNextPadShape(s),
        PadEditTarget::Selected(idx) => PanelMsg::FpEditorSetSelectedPadShape { idx, shape: s },
    }
}
fn pad_size_x_msg(t: PadEditTarget, v: String) -> PanelMsg {
    match t {
        PadEditTarget::Next => PanelMsg::FpEditorSetNextPadSizeX(v),
        PadEditTarget::Selected(idx) => PanelMsg::FpEditorSetSelectedPadSizeX { idx, value: v },
    }
}
fn pad_size_y_msg(t: PadEditTarget, v: String) -> PanelMsg {
    match t {
        PadEditTarget::Next => PanelMsg::FpEditorSetNextPadSizeY(v),
        PadEditTarget::Selected(idx) => PanelMsg::FpEditorSetSelectedPadSizeY { idx, value: v },
    }
}
fn pad_drill_diameter_msg(t: PadEditTarget, v: String) -> PanelMsg {
    match t {
        PadEditTarget::Next => PanelMsg::FpEditorSetNextPadDrillDiameter(v),
        PadEditTarget::Selected(idx) => PanelMsg::FpEditorSetSelectedPadDrillDiameter { idx, value: v },
    }
}
fn pad_drill_slot_length_msg(t: PadEditTarget, v: String) -> PanelMsg {
    match t {
        PadEditTarget::Next => PanelMsg::FpEditorSetNextPadDrillSlotLength(v),
        PadEditTarget::Selected(idx) => PanelMsg::FpEditorSetSelectedPadDrillSlotLength { idx, value: v },
    }
}
fn pad_corner_radius_msg(t: PadEditTarget, v: String) -> PanelMsg {
    match t {
        PadEditTarget::Next => PanelMsg::FpEditorSetNextPadCornerRadiusPct(v),
        PadEditTarget::Selected(idx) => PanelMsg::FpEditorSetSelectedPadCornerRadiusPct { idx, value: v },
    }
}
fn pad_paste_top_msg(t: PadEditTarget, v: String) -> PanelMsg {
    match t {
        PadEditTarget::Next => PanelMsg::FpEditorSetNextPadPasteMarginTop(v),
        PadEditTarget::Selected(idx) => PanelMsg::FpEditorSetSelectedPadPasteMarginTop { idx, value: v },
    }
}
fn pad_paste_bottom_msg(t: PadEditTarget, v: String) -> PanelMsg {
    match t {
        PadEditTarget::Next => PanelMsg::FpEditorSetNextPadPasteMarginBottom(v),
        PadEditTarget::Selected(idx) => PanelMsg::FpEditorSetSelectedPadPasteMarginBottom { idx, value: v },
    }
}
fn pad_paste_enabled_top_msg(t: PadEditTarget, v: bool) -> PanelMsg {
    match t {
        PadEditTarget::Next => PanelMsg::FpEditorToggleNextPadPasteEnabledTop(v),
        PadEditTarget::Selected(idx) => PanelMsg::FpEditorToggleSelectedPadPasteEnabledTop { idx, value: v },
    }
}
fn pad_paste_enabled_bottom_msg(t: PadEditTarget, v: bool) -> PanelMsg {
    match t {
        PadEditTarget::Next => PanelMsg::FpEditorToggleNextPadPasteEnabledBottom(v),
        PadEditTarget::Selected(idx) => PanelMsg::FpEditorToggleSelectedPadPasteEnabledBottom { idx, value: v },
    }
}
fn pad_mask_top_msg(t: PadEditTarget, v: String) -> PanelMsg {
    match t {
        PadEditTarget::Next => PanelMsg::FpEditorSetNextPadMaskMarginTop(v),
        PadEditTarget::Selected(idx) => PanelMsg::FpEditorSetSelectedPadMaskMarginTop { idx, value: v },
    }
}
fn pad_mask_bottom_msg(t: PadEditTarget, v: String) -> PanelMsg {
    match t {
        PadEditTarget::Next => PanelMsg::FpEditorSetNextPadMaskMarginBottom(v),
        PadEditTarget::Selected(idx) => PanelMsg::FpEditorSetSelectedPadMaskMarginBottom { idx, value: v },
    }
}
fn pad_mask_tented_top_msg(t: PadEditTarget, v: bool) -> PanelMsg {
    match t {
        PadEditTarget::Next => PanelMsg::FpEditorToggleNextPadMaskTentedTop(v),
        PadEditTarget::Selected(idx) => PanelMsg::FpEditorToggleSelectedPadMaskTentedTop { idx, value: v },
    }
}
fn pad_mask_tented_bottom_msg(t: PadEditTarget, v: bool) -> PanelMsg {
    match t {
        PadEditTarget::Next => PanelMsg::FpEditorToggleNextPadMaskTentedBottom(v),
        PadEditTarget::Selected(idx) => PanelMsg::FpEditorToggleSelectedPadMaskTentedBottom { idx, value: v },
    }
}
fn pad_thermal_relief_msg(t: PadEditTarget, v: bool) -> PanelMsg {
    match t {
        PadEditTarget::Next => PanelMsg::FpEditorToggleNextPadThermalRelief(v),
        PadEditTarget::Selected(idx) => PanelMsg::FpEditorToggleSelectedPadThermalRelief { idx, value: v },
    }
}
fn pad_feature_top_msg(t: PadEditTarget, v: signex_sketch::attr::PadFeature) -> PanelMsg {
    match t {
        PadEditTarget::Next => PanelMsg::FpEditorSetNextPadFeatureTop(v),
        PadEditTarget::Selected(idx) => PanelMsg::FpEditorSetSelectedPadFeatureTop { idx, value: v },
    }
}
fn pad_feature_bottom_msg(t: PadEditTarget, v: signex_sketch::attr::PadFeature) -> PanelMsg {
    match t {
        PadEditTarget::Next => PanelMsg::FpEditorSetNextPadFeatureBottom(v),
        PadEditTarget::Selected(idx) => PanelMsg::FpEditorSetSelectedPadFeatureBottom { idx, value: v },
    }
}
fn pad_testpoint_top_assembly_msg(t: PadEditTarget, v: bool) -> PanelMsg {
    match t {
        PadEditTarget::Next => PanelMsg::FpEditorToggleNextPadTestpointTopAssembly(v),
        PadEditTarget::Selected(idx) => PanelMsg::FpEditorToggleSelectedPadTestpointTopAssembly { idx, value: v },
    }
}
fn pad_testpoint_top_fab_msg(t: PadEditTarget, v: bool) -> PanelMsg {
    match t {
        PadEditTarget::Next => PanelMsg::FpEditorToggleNextPadTestpointTopFab(v),
        PadEditTarget::Selected(idx) => PanelMsg::FpEditorToggleSelectedPadTestpointTopFab { idx, value: v },
    }
}
fn pad_testpoint_bottom_assembly_msg(t: PadEditTarget, v: bool) -> PanelMsg {
    match t {
        PadEditTarget::Next => PanelMsg::FpEditorToggleNextPadTestpointBottomAssembly(v),
        PadEditTarget::Selected(idx) => PanelMsg::FpEditorToggleSelectedPadTestpointBottomAssembly { idx, value: v },
    }
}
fn pad_testpoint_bottom_fab_msg(t: PadEditTarget, v: bool) -> PanelMsg {
    match t {
        PadEditTarget::Next => PanelMsg::FpEditorToggleNextPadTestpointBottomFab(v),
        PadEditTarget::Selected(idx) => PanelMsg::FpEditorToggleSelectedPadTestpointBottomFab { idx, value: v },
    }
}

/// v0.20 — single-line label + text-input row used by every Pad
/// Properties field. Mirrors the existing rotation/size_x rows'
/// chrome (40 px label, padded input, dim border).
fn pad_input_row<'a>(
    label: &'a str,
    placeholder: &'a str,
    value: String,
    on_input: impl Fn(String) -> PanelMsg + 'a,
    muted: Color,
    primary: Color,
    border_c: Color,
) -> iced::Element<'a, PanelMsg> {
    container(
        row![
            text(label).size(10).color(muted).width(Length::Fixed(110.0)),
            text_input(placeholder, &value)
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
}

/// v0.20 — pick_list row for a Pad Properties field.
fn pad_pick_row<'a, T>(
    label: &'a str,
    options: &'a [T],
    selected: T,
    on_change: impl Fn(T) -> PanelMsg + 'a + Clone,
    muted: Color,
) -> iced::Element<'a, PanelMsg>
where
    T: Clone + Eq + std::fmt::Display + 'static,
{
    container(
        row![
            text(label).size(10).color(muted).width(Length::Fixed(110.0)),
            pick_list(options, Some(selected), on_change)
                .text_size(10)
                .padding([3, 8])
                .width(Length::Fill),
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, 8])
    .width(Length::Fill)
    .into()
}

/// v0.20 — checkbox row for a Pad Properties field. Label on left,
/// flat checkbox on right.
fn pad_check_row<'a>(
    label: &'a str,
    on: bool,
    on_toggle: impl Fn(bool) -> PanelMsg + 'a,
    muted: Color,
    primary: Color,
) -> iced::Element<'a, PanelMsg> {
    container(
        row![
            text(label).size(10).color(if on { primary } else { muted }).width(Length::Fixed(110.0)),
            iced::widget::checkbox(on)
                .on_toggle(on_toggle)
                .size(12)
                .spacing(0),
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, 8])
    .width(Length::Fill)
    .into()
}

/// v0.20 — render the "Properties" section. `target` selects whether
/// edits write to `next_pad_defaults` (placement form) or to a
/// specific selected pad. The pause/resume hint banner only shows
/// for the placement form (`PadEditTarget::Next`).
fn render_pad_form_properties<'a>(
    mut col: Column<'a, PanelMsg>,
    values: &PadFormValues,
    target: PadEditTarget,
    placement_paused: bool,
    muted: Color,
    primary: Color,
    border_c: Color,
    collapsed_sections: &'a CollapsedSections,
) -> Column<'a, PanelMsg> {
    col = col.push(props_section_header(
        "Properties",
        "fp_pad_properties",
        collapsed_sections,
        primary,
        border_c,
    ));
    if fp_is_collapsed("fp_pad_properties", collapsed_sections) {
        return col;
    }

    // TAB pause hint banner — only for the placement form.
    if matches!(target, PadEditTarget::Next) {
        if placement_paused {
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
    }

    col = col.push(pad_input_row(
        "Designator",
        "(auto)",
        values.designator.clone(),
        move |v| pad_designator_msg(target, v),
        muted,
        primary,
        border_c,
    ));

    use crate::library::editor::footprint::state::PadSide;
    col = col.push(pad_pick_row(
        "Layer",
        PadSide::ALL_OPTIONS,
        values.side,
        move |s: PadSide| pad_side_msg(target, s),
        muted,
    ));

    col = col.push(pad_pick_row(
        "Electrical Type",
        signex_sketch::attr::ElectricalType::ALL,
        values.electrical_type,
        move |v: signex_sketch::attr::ElectricalType| pad_electrical_type_msg(target, v),
        muted,
    ));

    col = col.push(pad_input_row(
        "Net",
        "(unassigned)",
        values.net.clone(),
        move |v| pad_net_msg(target, v),
        muted,
        primary,
        border_c,
    ));

    col = col.push(pad_input_row(
        "Rotation (°)",
        "0",
        format!("{:.1}", values.rotation_deg),
        move |v| pad_rotation_msg(target, v),
        muted,
        primary,
        border_c,
    ));

    col = col.push(pad_check_row(
        "Locked",
        values.locked,
        move |v| pad_locked_msg(target, v),
        muted,
        primary,
    ));

    col = col.push(pad_input_row(
        "Template",
        "",
        values.template.clone(),
        move |v| pad_template_msg(target, v),
        muted,
        primary,
        border_c,
    ));
    col = col.push(pad_input_row(
        "Library",
        "",
        values.template_library.clone(),
        move |v| pad_template_library_msg(target, v),
        muted,
        primary,
        border_c,
    ));

    col
}

/// v0.20 — render the "Pad Stack" section: copper shape + size,
/// hole, per-side paste / mask expansions, tented flags, thermal
/// relief, corner radius. Mirrors the Altium PCB Library Pad Stack
/// section: a stylized 2D preview at the top, a Simple/Top-Middle-
/// Bottom/Full Stack tab strip, then the field rows. The tabs are
/// UI-only structure today; per-layer overrides require a v0.21
/// schema follow-up so all three tabs render the same content.
fn render_pad_form_pad_stack<'a>(
    mut col: Column<'a, PanelMsg>,
    values: &PadFormValues,
    target: PadEditTarget,
    muted: Color,
    primary: Color,
    border_c: Color,
    collapsed_sections: &'a CollapsedSections,
) -> Column<'a, PanelMsg> {
    col = col.push(props_section_header(
        "Pad Stack",
        "fp_pad_stack",
        collapsed_sections,
        primary,
        border_c,
    ));
    if fp_is_collapsed("fp_pad_stack", collapsed_sections) {
        return col;
    }

    // ── Preview ──
    col = col.push(pad_stack_preview(values));

    // ── Tab strip (Simple / Top-Middle-Bottom / Full Stack) ──
    col = col.push(pad_stack_tab_strip(values, primary, muted, border_c));

    // ── Altium-parity table layout ── COPPER / HOLE / PASTE / SOLDER
    // each render as a header row + N data rows where every cell sits
    // side-by-side. Column widths use FillPortion so the table
    // adapts to any panel width without truncation.
    use crate::library::editor::footprint::state::{PadSide, PadStackTab};
    let is_multilayer = matches!(values.side, PadSide::All);
    let show_top_side = matches!(values.side, PadSide::Top | PadSide::All);
    let show_bottom_side = matches!(values.side, PadSide::Bottom | PadSide::All);

    let current_shape = PadShapeChoice::from_lib(&values.shape);

    // ── COPPER table ──
    col = col.push(pad_table_header(
        &["COPPER", "X-Size", "Y-Size", "Shape", "Relief"],
        &[3, 2, 2, 3, 1],
        muted,
        border_c,
    ));
    let copper_rows: &[&str] = match values.pad_stack_tab {
        PadStackTab::Simple => &["All Layers"],
        PadStackTab::TopMiddleBottom => &["Top Layer", "Mid Layer", "Bottom Layer"],
        PadStackTab::FullStack => match values.side {
            PadSide::Top => &["F.Cu", "F.Mask", "F.Paste"],
            PadSide::Bottom => &["B.Cu", "B.Mask", "B.Paste"],
            PadSide::All => &["Top Layer", "Inner", "Bottom Layer"],
        },
    };
    for (idx, row_label) in copper_rows.iter().enumerate() {
        let is_first = idx == 0;
        col = col.push(pad_copper_row(
            row_label,
            values,
            current_shape,
            target,
            muted,
            primary,
            border_c,
            // Only the first (top) row drives shape / size; mid / bottom
            // mirror until the v0.21 schema lands.
            is_first,
        ));
    }
    // Corner radius stays as its own labelled row beneath the table —
    // it's a shape-modifier, not a per-layer override.
    if matches!(current_shape, PadShapeChoice::RoundedRectangle) {
        let corner_buf = values
            .stack
            .corner_radius_pct
            .map(|v| format!("{v:.0}"))
            .unwrap_or_default();
        col = col.push(pad_input_row(
            "Corner radius %",
            "25",
            corner_buf,
            move |v| pad_corner_radius_msg(target, v),
            muted,
            primary,
            border_c,
        ));
    }

    // ── HOLE table ── Multi-Layer pads only.
    if is_multilayer {
        col = col.push(pad_table_header(
            &["HOLE", "Size", "Length", "Shape", "Plated"],
            &[3, 2, 2, 3, 1],
            muted,
            border_c,
        ));
        let hole_shape_current = if values.drill_slot_length_mm.is_some() {
            HoleShapeChoice::Slot
        } else {
            HoleShapeChoice::Round
        };
        let slot_default = values
            .drill_diameter_mm
            .map(|d| d * 1.5)
            .unwrap_or(1.0);
        let drill_buf = values
            .drill_diameter_mm
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default();
        let slot_buf = values
            .drill_slot_length_mm
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default();
        let slot_enabled = matches!(hole_shape_current, HoleShapeChoice::Slot);
        col = col.push(pad_table_row(
            "Pad Hole",
            vec![
                pad_table_input_cell(
                    drill_buf,
                    "0",
                    move |v| pad_drill_diameter_msg(target, v),
                    muted,
                    primary,
                    border_c,
                ),
                if slot_enabled {
                    pad_table_input_cell(
                        slot_buf,
                        "0",
                        move |v| pad_drill_slot_length_msg(target, v),
                        muted,
                        primary,
                        border_c,
                    )
                } else {
                    pad_table_disabled_cell("0mm", muted, border_c)
                },
                pad_table_picklist_cell(
                    HoleShapeChoice::ALL,
                    hole_shape_current,
                    move |c: HoleShapeChoice| match c {
                        HoleShapeChoice::Round => {
                            pad_drill_slot_length_msg(target, String::new())
                        }
                        HoleShapeChoice::Slot => {
                            pad_drill_slot_length_msg(target, format!("{slot_default:.3}"))
                        }
                    },
                ),
                pad_table_check_cell(
                    !matches!(values.kind, signex_library::PadKind::NptHole),
                    move |v| pad_plated_msg(target, v),
                ),
            ],
            &[2, 2, 3, 1],
            muted,
            primary,
            border_c,
        ));
        // v0.21 — Hole detail fields. Tolerance is reporting-only
        // (drives IPC-356 / drill table), Rotation orients Slot/Rect
        // holes, Copper Offset shifts the copper outline relative to
        // the hole centre. Only shown for Multi-Layer pads.
        col = col.push(
            container(text("Hole Details").size(9).color(muted))
                .padding([6, 8])
                .width(Length::Fill),
        );
        let tol_p_buf = values
            .hole_tolerance_plus_mm
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default();
        col = col.push(pad_input_row(
            "Tolerance + (mm)",
            "0",
            tol_p_buf,
            move |v| pad_hole_tolerance_plus_msg(target, v),
            muted, primary, border_c,
        ));
        let tol_m_buf = values
            .hole_tolerance_minus_mm
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default();
        col = col.push(pad_input_row(
            "Tolerance − (mm)",
            "0",
            tol_m_buf,
            move |v| pad_hole_tolerance_minus_msg(target, v),
            muted, primary, border_c,
        ));
        let rot_buf = values
            .hole_rotation_deg
            .map(|v| format!("{v:.1}"))
            .unwrap_or_default();
        col = col.push(pad_input_row(
            "Hole rotation (°)",
            "0",
            rot_buf,
            move |v| pad_hole_rotation_msg(target, v),
            muted, primary, border_c,
        ));
        let cox_buf = values
            .copper_offset_x_mm
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default();
        col = col.push(pad_input_row(
            "Copper offset X (mm)",
            "0",
            cox_buf,
            move |v| pad_copper_offset_x_msg(target, v),
            muted, primary, border_c,
        ));
        let coy_buf = values
            .copper_offset_y_mm
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default();
        col = col.push(pad_input_row(
            "Copper offset Y (mm)",
            "0",
            coy_buf,
            move |v| pad_copper_offset_y_msg(target, v),
            muted, primary, border_c,
        ));
    }

    // ── PASTE table ──
    if show_top_side || show_bottom_side {
        col = col.push(pad_table_header(
            &["PASTE", "Expansion", "%", "Shape", "Enabled"],
            &[3, 2, 2, 3, 1],
            muted,
            border_c,
        ));
    }
    if show_top_side {
        let paste_top_buf = values
            .stack
            .paste_margin_top
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default();
        let paste_top_mode = if values.stack.paste_margin_top.is_some() {
            ExpansionMode::Manual
        } else {
            ExpansionMode::Rule
        };
        col = col.push(pad_table_row(
            "Top Paste",
            vec![
                pad_table_input_cell(
                    paste_top_buf,
                    "(rule)",
                    move |v| pad_paste_top_msg(target, v),
                    muted,
                    primary,
                    border_c,
                ),
                pad_table_disabled_cell("0%", muted, border_c),
                pad_table_picklist_cell(
                    ExpansionMode::ALL,
                    paste_top_mode,
                    move |m: ExpansionMode| match m {
                        ExpansionMode::Rule => pad_paste_top_msg(target, String::new()),
                        ExpansionMode::Manual => pad_paste_top_msg(target, "0".into()),
                    },
                ),
                pad_table_check_cell(values.stack.paste_enabled_top, move |v| {
                    pad_paste_enabled_top_msg(target, v)
                }),
            ],
            &[2, 2, 3, 1],
            muted,
            primary,
            border_c,
        ));
    }
    if show_bottom_side {
        let paste_bot_buf = values
            .stack
            .paste_margin_bottom
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default();
        let paste_bot_mode = if values.stack.paste_margin_bottom.is_some() {
            ExpansionMode::Manual
        } else {
            ExpansionMode::Rule
        };
        col = col.push(pad_table_row(
            "Bottom Paste",
            vec![
                pad_table_input_cell(
                    paste_bot_buf,
                    "(rule)",
                    move |v| pad_paste_bottom_msg(target, v),
                    muted,
                    primary,
                    border_c,
                ),
                pad_table_disabled_cell("0%", muted, border_c),
                pad_table_picklist_cell(
                    ExpansionMode::ALL,
                    paste_bot_mode,
                    move |m: ExpansionMode| match m {
                        ExpansionMode::Rule => pad_paste_bottom_msg(target, String::new()),
                        ExpansionMode::Manual => pad_paste_bottom_msg(target, "0".into()),
                    },
                ),
                pad_table_check_cell(values.stack.paste_enabled_bottom, move |v| {
                    pad_paste_enabled_bottom_msg(target, v)
                }),
            ],
            &[2, 2, 3, 1],
            muted,
            primary,
            border_c,
        ));
    }

    // ── SOLDER table ──
    if show_top_side || show_bottom_side {
        col = col.push(pad_table_header(
            &["SOLDER", "Expansion", "Shape", "Tented"],
            &[3, 2, 3, 1],
            muted,
            border_c,
        ));
    }
    if show_top_side {
        let mask_top_buf = values
            .stack
            .mask_margin_top
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default();
        let mask_top_mode = if values.stack.mask_margin_top.is_some() {
            ExpansionMode::Manual
        } else {
            ExpansionMode::Rule
        };
        col = col.push(pad_table_row(
            "Top Solder Mask",
            vec![
                pad_table_input_cell(
                    mask_top_buf,
                    "(rule)",
                    move |v| pad_mask_top_msg(target, v),
                    muted,
                    primary,
                    border_c,
                ),
                pad_table_picklist_cell(
                    ExpansionMode::ALL,
                    mask_top_mode,
                    move |m: ExpansionMode| match m {
                        ExpansionMode::Rule => pad_mask_top_msg(target, String::new()),
                        ExpansionMode::Manual => pad_mask_top_msg(target, "0".into()),
                    },
                ),
                pad_table_check_cell(values.stack.mask_tented_top, move |v| {
                    pad_mask_tented_top_msg(target, v)
                }),
            ],
            &[2, 3, 1],
            muted,
            primary,
            border_c,
        ));
    }
    if show_bottom_side {
        let mask_bot_buf = values
            .stack
            .mask_margin_bottom
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default();
        let mask_bot_mode = if values.stack.mask_margin_bottom.is_some() {
            ExpansionMode::Manual
        } else {
            ExpansionMode::Rule
        };
        col = col.push(pad_table_row(
            "Bottom Solder Mask",
            vec![
                pad_table_input_cell(
                    mask_bot_buf,
                    "(rule)",
                    move |v| pad_mask_bottom_msg(target, v),
                    muted,
                    primary,
                    border_c,
                ),
                pad_table_picklist_cell(
                    ExpansionMode::ALL,
                    mask_bot_mode,
                    move |m: ExpansionMode| match m {
                        ExpansionMode::Rule => pad_mask_bottom_msg(target, String::new()),
                        ExpansionMode::Manual => pad_mask_bottom_msg(target, "0".into()),
                    },
                ),
                pad_table_check_cell(values.stack.mask_tented_bottom, move |v| {
                    pad_mask_tented_bottom_msg(target, v)
                }),
            ],
            &[2, 3, 1],
            muted,
            primary,
            border_c,
        ));
    }

    col
}

/// v0.20 — Altium-style table header row. Renders the column titles
/// in muted small text with the same FillPortion layout the data
/// rows use, so columns line up vertically. First cell is the
/// section family name (COPPER / HOLE / PASTE / SOLDER).
fn pad_table_header<'a>(
    cols: &[&'static str],
    portions: &[u16],
    muted: Color,
    border_c: Color,
) -> iced::Element<'a, PanelMsg> {
    let mut row = iced::widget::Row::new().spacing(4).align_y(iced::Alignment::Center);
    for (i, label) in cols.iter().enumerate() {
        let portion = portions.get(i).copied().unwrap_or(1);
        row = row.push(
            text(label.to_string())
                .size(9)
                .color(muted)
                .width(Length::FillPortion(portion)),
        );
    }
    container(row)
        .padding([4, 8])
        .width(Length::Fill)
        .style(move |_: &Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.03,
            ))),
            border: iced::Border {
                width: 0.0,
                radius: 0.0.into(),
                color: border_c,
            },
            ..iced::widget::container::Style::default()
        })
        .into()
}

/// v0.20 — Altium-style table data row. First cell is the row label
/// (e.g. "All Layers", "Pad Hole", "Top Paste"); remaining cells are
/// caller-provided Elements. Width portions match the header row.
fn pad_table_row<'a>(
    label: &'a str,
    cells: Vec<iced::Element<'a, PanelMsg>>,
    portions: &[u16],
    muted: Color,
    primary: Color,
    _border_c: Color,
) -> iced::Element<'a, PanelMsg> {
    let _ = muted;
    let label_portion = 3_u16;
    let mut row = iced::widget::Row::new().spacing(4).align_y(iced::Alignment::Center);
    row = row.push(
        text(label.to_string())
            .size(10)
            .color(primary)
            .width(Length::FillPortion(label_portion)),
    );
    for (i, cell) in cells.into_iter().enumerate() {
        let portion = portions.get(i).copied().unwrap_or(1);
        row = row.push(container(cell).width(Length::FillPortion(portion)));
    }
    container(row).padding([3, 8]).width(Length::Fill).into()
}

/// v0.20 — text_input cell with the same chrome as `pad_input_row`'s
/// input but no leading label — meant for table data rows.
fn pad_table_input_cell<'a>(
    value: String,
    placeholder: &'a str,
    on_input: impl Fn(String) -> PanelMsg + 'a,
    muted: Color,
    primary: Color,
    border_c: Color,
) -> iced::Element<'a, PanelMsg> {
    text_input(placeholder, &value)
        .size(10)
        .padding(2)
        .style(move |_: &Theme, _| iced::widget::text_input::Style {
            background: iced::Background::Color(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.04)),
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
        .on_input(on_input)
        .into()
}

/// v0.20 — pick_list cell for table data rows.
fn pad_table_picklist_cell<'a, T>(
    options: &'a [T],
    selected: T,
    on_change: impl Fn(T) -> PanelMsg + 'a + Clone,
) -> iced::Element<'a, PanelMsg>
where
    T: Clone + Eq + std::fmt::Display + 'static,
{
    pick_list(options, Some(selected), on_change)
        .text_size(10)
        .padding([2, 6])
        .width(Length::Fill)
        .into()
}

/// v0.20 — checkbox cell for table data rows.
fn pad_table_check_cell<'a>(
    on: bool,
    on_toggle: impl Fn(bool) -> PanelMsg + 'a,
) -> iced::Element<'a, PanelMsg> {
    container(
        iced::widget::checkbox(on)
            .on_toggle(on_toggle)
            .size(12)
            .spacing(0),
    )
    .padding([2, 4])
    .into()
}

/// v0.20 — disabled / read-only cell. Shows a value with greyed
/// chrome but no input handler. Used for column placeholders that
/// don't yet have a backing field (e.g. "0%" in the PASTE table
/// where percentage overrides aren't wired yet).
fn pad_table_disabled_cell<'a>(
    value: impl Into<String>,
    muted: Color,
    border_c: Color,
) -> iced::Element<'a, PanelMsg> {
    container(text(value.into()).size(10).color(muted))
        .padding([3, 6])
        .style(move |_: &Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.02,
            ))),
            border: iced::Border {
                width: 1.0,
                radius: 2.0.into(),
                color: border_c,
            },
            ..iced::widget::container::Style::default()
        })
        .width(Length::Fill)
        .into()
}

/// v0.20 — static text cell. No chrome, just dim text — used for
/// columns like "Rule Expansion" that aren't yet user-editable
/// (a v0.21 follow-up adds the per-rule override picker).
fn pad_table_static_cell<'a>(
    value: impl Into<String>,
    muted: Color,
) -> iced::Element<'a, PanelMsg> {
    container(text(value.into()).size(10).color(muted))
        .padding([3, 6])
        .width(Length::Fill)
        .into()
}

/// v0.20 — single COPPER table row. Built inline because all four
/// data cells (X-Size, Y-Size, Shape, Relief) reference different
/// fields on PadFormValues + different message constructors.
fn pad_copper_row<'a>(
    label: &'a str,
    values: &PadFormValues,
    current_shape: PadShapeChoice,
    target: PadEditTarget,
    muted: Color,
    primary: Color,
    border_c: Color,
    is_authoritative: bool,
) -> iced::Element<'a, PanelMsg> {
    let x_buf = format!("{:.3}", values.size_x_mm);
    let y_buf = format!("{:.3}", values.size_y_mm);
    let cells = if is_authoritative {
        vec![
            pad_table_input_cell(
                x_buf,
                "",
                move |v| pad_size_x_msg(target, v),
                muted,
                primary,
                border_c,
            ),
            pad_table_input_cell(
                y_buf,
                "",
                move |v| pad_size_y_msg(target, v),
                muted,
                primary,
                border_c,
            ),
            pad_table_picklist_cell(PadShapeChoice::ALL, current_shape, move |c| {
                pad_shape_msg(target, c.to_lib())
            }),
            pad_table_check_cell(values.stack.thermal_relief, move |v| {
                pad_thermal_relief_msg(target, v)
            }),
        ]
    } else {
        // Mid / Bottom rows mirror Top — no per-layer overrides yet.
        vec![
            pad_table_disabled_cell(&format!("{:.3}", values.size_x_mm), muted, border_c),
            pad_table_disabled_cell(&format!("{:.3}", values.size_y_mm), muted, border_c),
            pad_table_static_cell(&format!("{current_shape}"), muted),
            pad_table_disabled_cell("", muted, border_c),
        ]
    };
    pad_table_row(label, cells, &[2, 2, 3, 1], muted, primary, border_c)
}

/// v0.20 — render the "Pad Features" section: top/bottom surface
/// treatment + testpoint flags.
fn render_pad_form_pad_features<'a>(
    mut col: Column<'a, PanelMsg>,
    values: &PadFormValues,
    target: PadEditTarget,
    muted: Color,
    primary: Color,
    border_c: Color,
    collapsed_sections: &'a CollapsedSections,
) -> Column<'a, PanelMsg> {
    col = col.push(props_section_header(
        "Pad Features",
        "fp_pad_features",
        collapsed_sections,
        primary,
        border_c,
    ));
    if fp_is_collapsed("fp_pad_features", collapsed_sections) {
        return col;
    }

    use signex_sketch::attr::PadFeature;
    col = col.push(pad_pick_row(
        "Top Side",
        PadFeature::ALL,
        values.feature_top,
        move |v: PadFeature| pad_feature_top_msg(target, v),
        muted,
    ));
    col = col.push(pad_pick_row(
        "Bottom Side",
        PadFeature::ALL,
        values.feature_bottom,
        move |v: PadFeature| pad_feature_bottom_msg(target, v),
        muted,
    ));

    col = col.push(
        container(text("Testpoint").size(9).color(muted))
            .padding([8, 8])
            .width(Length::Fill),
    );
    col = col.push(pad_check_row(
        "Top Assembly",
        values.testpoint.top_assembly,
        move |v| pad_testpoint_top_assembly_msg(target, v),
        muted,
        primary,
    ));
    col = col.push(pad_check_row(
        "Top Fab",
        values.testpoint.top_fab,
        move |v| pad_testpoint_top_fab_msg(target, v),
        muted,
        primary,
    ));
    col = col.push(pad_check_row(
        "Bottom Assembly",
        values.testpoint.bottom_assembly,
        move |v| pad_testpoint_bottom_assembly_msg(target, v),
        muted,
        primary,
    ));
    col = col.push(pad_check_row(
        "Bottom Fab",
        values.testpoint.bottom_fab,
        move |v| pad_testpoint_bottom_fab_msg(target, v),
        muted,
        primary,
    ));
    let _ = border_c;
    col
}

/// v0.20 — Pad Stack preview. CPU-side iso-projected 3D rendering of
/// the pad: copper top face (red), solder mask outset (blue) at the
/// board surface, and a hole punched through both for THT pads.
/// Uses a 60° camera tilt so the viewer sees the top face plus the
/// stack thickness as in Altium's PCB Library preview.
///
/// Mirrors the projection helper in `preview3d.rs` but for a single
/// centred pad — no body / courtyard / bbox math.
fn pad_stack_preview<'a>(values: &PadFormValues) -> iced::Element<'a, PanelMsg> {
    use iced::widget::canvas;

    #[derive(Debug)]
    struct Preview {
        size_x_mm: f64,
        size_y_mm: f64,
        shape: signex_library::PadShape,
        drill_diameter_mm: Option<f64>,
    }

    impl<Message> canvas::Program<Message> for Preview {
        type State = ();
        fn draw(
            &self,
            _state: &Self::State,
            renderer: &iced::Renderer,
            _theme: &iced::Theme,
            bounds: iced::Rectangle,
            _cursor: iced::mouse::Cursor,
        ) -> Vec<canvas::Geometry> {
            let mut frame = canvas::Frame::new(renderer, bounds.size());
            let bg = iced::Color::from_rgba8(0x18, 0x1B, 0x21, 1.0);
            frame.fill_rectangle(
                iced::Point::ORIGIN,
                bounds.size(),
                canvas::Fill::from(bg),
            );

            // Geometry constants.
            let pad_w = self.size_x_mm.max(0.001);
            let pad_h = self.size_y_mm.max(0.001);
            let mask_outset_mm = pad_w.max(pad_h) * 0.10; // 10% of pad
            let mask_w = pad_w + 2.0 * mask_outset_mm;
            let mask_h = pad_h + 2.0 * mask_outset_mm;
            // Visual thickness — exaggerated vs. real (~0.05mm copper)
            // so the side wall reads at panel scale.
            let copper_thickness_mm = pad_w.max(pad_h) * 0.10;
            let mask_thickness_mm = copper_thickness_mm;

            // 30° isometric projection — matches `preview3d.rs`. Both
            // X+ and Y+ rotate to screen-up directions, Z+ is screen-up.
            //   sx = (x - y) * cos30
            //   sy = -((x + y) * sin30 + z)
            // This makes the XY plane look tilted as a diamond, with
            // the pad's thickness extruded upward — same as Altium's
            // PCB Library preview tilt.
            let cos30 = 0.866_025_4_f32;
            let sin30 = 0.500_f32;

            // Fit projected bbox into 75% of frame. The iso bbox in
            // screen units: width = (mask_w + mask_h) * cos30, height
            // = (mask_w + mask_h) * sin30 + total_thickness.
            let proj_w = ((mask_w + mask_h) as f32) * cos30;
            let proj_h = ((mask_w + mask_h) as f32) * sin30
                + (copper_thickness_mm + mask_thickness_mm) as f32;
            let scale = ((bounds.width * 0.75 / proj_w).min(bounds.height * 0.75 / proj_h))
                .max(2.0);
            let cx = bounds.width / 2.0;
            let cy = bounds.height / 2.0 + bounds.height * 0.10; // shift down slightly

            // Project (x, y, z) world → screen.
            let project = move |x: f32, y: f32, z: f32| -> iced::Point {
                let sx = (x - y) * cos30 * scale;
                let sy = -((x + y) * sin30 + z) * scale;
                iced::Point::new(cx + sx, cy + sy)
            };

            let mask_color = iced::Color::from_rgba8(0x2E, 0x6B, 0xD9, 1.0);
            let mask_dark = iced::Color::from_rgba8(0x1F, 0x49, 0x97, 1.0);
            let copper_color = iced::Color::from_rgba8(0xD9, 0x3D, 0x3D, 1.0);
            let copper_dark = iced::Color::from_rgba8(0x99, 0x2A, 0x2A, 1.0);
            let hole_color = iced::Color::from_rgba8(0x70, 0x70, 0x70, 1.0);
            let hole_dark = iced::Color::from_rgba8(0x40, 0x40, 0x40, 1.0);

            let is_round = matches!(
                self.shape,
                signex_library::PadShape::Round | signex_library::PadShape::Oval
            );

            // v0.20 — generate the pad / mask outline in world-space
            // CCW order, then project each at the requested Z. The
            // shape determines how the perimeter is sampled:
            //   Round/Oval     → N evenly-spaced ellipse points
            //   Rect           → 4 corners
            //   RoundRect      → 4 straight edges + 4 quarter arcs
            //   Chamfered      → 4 corners with optional 45° cuts
            //   Custom / etc.  → fallback to rect corners
            let shape_for_outline = self.shape.clone();
            let perimeter_world =
                move |hw: f32, hh: f32, segments: usize| -> Vec<(f32, f32)> {
                    use signex_library::PadShape as PS;
                    use std::f32::consts::{FRAC_PI_2, PI, TAU};
                    match &shape_for_outline {
                        PS::Round | PS::Oval => (0..segments)
                            .map(|i| {
                                let t = i as f32 / segments as f32 * TAU;
                                (hw * t.cos(), hh * t.sin())
                            })
                            .collect(),
                        PS::RoundRect { radius_ratio } => {
                            let r = (hw.min(hh) * (*radius_ratio as f32 * 2.0))
                                .max(0.1)
                                .min(hw.min(hh));
                            let inner_w = hw - r;
                            let inner_h = hh - r;
                            // Distribute samples roughly equally across
                            // 4 arcs + 4 sides. A power-of-2-ish split
                            // (8 + 2) keeps the curvature smooth without
                            // exploding the vertex count.
                            let arc_n = 8;
                            let mut pts: Vec<(f32, f32)> = Vec::new();
                            // Walk CCW starting at south-edge midpoint.
                            // South edge: (-inner_w, -hh) → (inner_w, -hh)
                            pts.push((-inner_w, -hh));
                            pts.push((inner_w, -hh));
                            // SE arc: center (inner_w, -inner_h), -π/2 → 0
                            for i in 1..=arc_n {
                                let t = -FRAC_PI_2 + (i as f32 / arc_n as f32) * FRAC_PI_2;
                                pts.push((inner_w + r * t.cos(), -inner_h + r * t.sin()));
                            }
                            // East edge: (hw, -inner_h) → (hw, inner_h)
                            pts.push((hw, inner_h));
                            // NE arc: center (inner_w, inner_h), 0 → π/2
                            for i in 1..=arc_n {
                                let t = (i as f32 / arc_n as f32) * FRAC_PI_2;
                                pts.push((inner_w + r * t.cos(), inner_h + r * t.sin()));
                            }
                            // North edge: (inner_w, hh) → (-inner_w, hh)
                            pts.push((-inner_w, hh));
                            // NW arc: center (-inner_w, inner_h), π/2 → π
                            for i in 1..=arc_n {
                                let t = FRAC_PI_2 + (i as f32 / arc_n as f32) * FRAC_PI_2;
                                pts.push((-inner_w + r * t.cos(), inner_h + r * t.sin()));
                            }
                            // West edge: (-hw, inner_h) → (-hw, -inner_h)
                            pts.push((-hw, -inner_h));
                            // SW arc: center (-inner_w, -inner_h), π → 3π/2
                            for i in 1..=arc_n {
                                let t = PI + (i as f32 / arc_n as f32) * FRAC_PI_2;
                                pts.push((-inner_w + r * t.cos(), -inner_h + r * t.sin()));
                            }
                            let _ = segments;
                            pts
                        }
                        PS::Chamfered { chamfer_ratio, corners } => {
                            let c = (hw.min(hh) * (*chamfer_ratio as f32 * 2.0))
                                .max(0.1)
                                .min(hw.min(hh));
                            let mut pts: Vec<(f32, f32)> = Vec::new();
                            // CCW from south edge.
                            // SE corner.
                            if corners.bottom_right {
                                pts.push((hw - c, -hh));
                                pts.push((hw, -hh + c));
                            } else {
                                pts.push((hw, -hh));
                            }
                            // NE corner.
                            if corners.top_right {
                                pts.push((hw, hh - c));
                                pts.push((hw - c, hh));
                            } else {
                                pts.push((hw, hh));
                            }
                            // NW corner.
                            if corners.top_left {
                                pts.push((-hw + c, hh));
                                pts.push((-hw, hh - c));
                            } else {
                                pts.push((-hw, hh));
                            }
                            // SW corner.
                            if corners.bottom_left {
                                pts.push((-hw, -hh + c));
                                pts.push((-hw + c, -hh));
                            } else {
                                pts.push((-hw, -hh));
                            }
                            let _ = segments;
                            pts
                        }
                        _ => vec![(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)],
                    }
                };

            let perimeter_pts =
                |hw: f32, hh: f32, z: f32, segments: usize| -> Vec<iced::Point> {
                    perimeter_world(hw, hh, segments)
                        .into_iter()
                        .map(|(x, y)| project(x, y, z))
                        .collect()
                };

            let segments = 40;
            let copper_z_top = (copper_thickness_mm + mask_thickness_mm) as f32;
            let copper_z_bot = mask_thickness_mm as f32;
            let mask_z_top = mask_thickness_mm as f32;
            let mask_z_bot = 0.0_f32;

            let pad_hw = (pad_w / 2.0) as f32;
            let pad_hh = (pad_h / 2.0) as f32;
            let mask_hw = (mask_w / 2.0) as f32;
            let mask_hh = (mask_h / 2.0) as f32;

            // Helper: fill a polygon path from points.
            let fill_poly = |frame: &mut canvas::Frame, pts: &[iced::Point], color: iced::Color| {
                if pts.len() < 3 {
                    return;
                }
                let path = canvas::Path::new(|b| {
                    b.move_to(pts[0]);
                    for p in &pts[1..] {
                        b.line_to(*p);
                    }
                    b.close();
                });
                frame.fill(&path, canvas::Fill::from(color));
            };

            // v0.20 — generic visibility test for any CCW perimeter:
            // edge i→j has world delta (dx, dy); outward normal
            // = (dy, -dx). For 30° iso looking from NE+up, the
            // edge is visible iff outward_x + outward_y < 0
            // → dy - dx < 0 → dy < dx. Works uniformly for round,
            // rect, round-rect, and chamfered perimeters.
            let strip_visible_world = |world_pts: &[(f32, f32)], i: usize| -> bool {
                let j = (i + 1) % world_pts.len();
                let (xi, yi) = world_pts[i];
                let (xj, yj) = world_pts[j];
                let dx = xj - xi;
                let dy = yj - yi;
                dy < dx
            };

            // ── Mask: bottom face is hidden by board; draw the side
            //    walls + top face.
            let mask_world = perimeter_world(mask_hw, mask_hh, segments);
            let mask_top_pts: Vec<iced::Point> = mask_world
                .iter()
                .map(|(x, y)| project(*x, *y, mask_z_top))
                .collect();
            let mask_bot_pts: Vec<iced::Point> = mask_world
                .iter()
                .map(|(x, y)| project(*x, *y, mask_z_bot))
                .collect();
            for i in 0..mask_top_pts.len() {
                if !strip_visible_world(&mask_world, i) {
                    continue;
                }
                let j = (i + 1) % mask_top_pts.len();
                let quad = [
                    mask_bot_pts[i],
                    mask_bot_pts[j],
                    mask_top_pts[j],
                    mask_top_pts[i],
                ];
                fill_poly(&mut frame, &quad, mask_dark);
            }
            fill_poly(&mut frame, &mask_top_pts, mask_color);

            // ── Copper: side walls + top face.
            let cu_world = perimeter_world(pad_hw, pad_hh, segments);
            let cu_top_pts: Vec<iced::Point> = cu_world
                .iter()
                .map(|(x, y)| project(*x, *y, copper_z_top))
                .collect();
            let cu_bot_pts: Vec<iced::Point> = cu_world
                .iter()
                .map(|(x, y)| project(*x, *y, copper_z_bot))
                .collect();
            for i in 0..cu_top_pts.len() {
                if !strip_visible_world(&cu_world, i) {
                    continue;
                }
                let j = (i + 1) % cu_top_pts.len();
                let quad = [
                    cu_bot_pts[i],
                    cu_bot_pts[j],
                    cu_top_pts[j],
                    cu_top_pts[i],
                ];
                fill_poly(&mut frame, &quad, copper_dark);
            }
            fill_poly(&mut frame, &cu_top_pts, copper_color);

            // ── Hole (THT only): cylinder through the stack. The
            //    visible inner wall is the FAR side from the camera,
            //    which by the same predicate is the SW arc — i.e.,
            //    `cos t + sin t < 0`. Draw it then the hole disc.
            if let Some(d) = self.drill_diameter_mm {
                let hr = (d / 2.0) as f32;
                // Hole is always round — sample as round even for
                // rectangular pads. Build CCW world points then use
                // the same generic strip_visible_world predicate.
                let hole_world: Vec<(f32, f32)> = (0..segments)
                    .map(|i| {
                        let t = i as f32 / segments as f32 * std::f32::consts::TAU;
                        (hr * t.cos(), hr * t.sin())
                    })
                    .collect();
                let hole_top_pts: Vec<iced::Point> = hole_world
                    .iter()
                    .map(|(x, y)| project(*x, *y, copper_z_top + 0.01))
                    .collect();
                let hole_bot_pts: Vec<iced::Point> = hole_world
                    .iter()
                    .map(|(x, y)| project(*x, *y, mask_z_bot - 0.01))
                    .collect();
                for i in 0..hole_top_pts.len() {
                    if !strip_visible_world(&hole_world, i) {
                        continue;
                    }
                    let j = (i + 1) % hole_top_pts.len();
                    let quad = [
                        hole_bot_pts[i],
                        hole_bot_pts[j],
                        hole_top_pts[j],
                        hole_top_pts[i],
                    ];
                    fill_poly(&mut frame, &quad, hole_dark);
                }
                fill_poly(&mut frame, &hole_top_pts, hole_color);
            }
            let _ = (is_round, perimeter_pts); // tidied below; suppress unused warnings

            vec![frame.into_geometry()]
        }
    }

    let preview = Preview {
        size_x_mm: values.size_x_mm,
        size_y_mm: values.size_y_mm,
        shape: values.shape.clone(),
        drill_diameter_mm: values.drill_diameter_mm,
    };
    container(
        canvas(preview)
            .width(Length::Fill)
            .height(Length::Fixed(160.0)),
    )
    .padding([4, 8])
    .width(Length::Fill)
    .into()
}

/// v0.20 — Pad Stack tab strip (Simple / Top-Middle-Bottom / Full
/// Stack). UI-only structure today; per-layer overrides require a
/// v0.21 schema follow-up so the body stays the same across tabs.
fn pad_stack_tab_strip<'a>(
    values: &PadFormValues,
    primary: Color,
    muted: Color,
    border_c: Color,
) -> iced::Element<'a, PanelMsg> {
    use crate::library::editor::footprint::state::PadStackTab;
    let current = values.pad_stack_tab;
    let chip_border = border_c;
    let active_bg = Color::from_rgba8(0x2E, 0x33, 0x45, 1.0);
    let inactive_bg = Color::from_rgba8(0x1A, 0x1D, 0x28, 1.0);
    let mk = move |label: &'static str, target: PadStackTab| -> iced::Element<'a, PanelMsg> {
        let active = current == target;
        iced::widget::button(
            text(label)
                .size(10)
                .color(if active { primary } else { muted })
                .align_x(iced::alignment::Horizontal::Center),
        )
        .padding([4, 10])
        .width(Length::FillPortion(1))
        .on_press(PanelMsg::FpEditorSetPadStackTab(target))
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let bg = match status {
                iced::widget::button::Status::Hovered => Some(iced::Background::Color(
                    Color::from_rgba(1.0, 1.0, 1.0, 0.06),
                )),
                _ => Some(iced::Background::Color(if active {
                    active_bg
                } else {
                    inactive_bg
                })),
            };
            iced::widget::button::Style {
                background: bg,
                border: iced::Border {
                    width: 1.0,
                    radius: 2.0.into(),
                    color: chip_border,
                },
                ..iced::widget::button::Style::default()
            }
        })
        .into()
    };
    container(
        row![
            mk("Simple", PadStackTab::Simple),
            mk("Top-Middle-Bottom", PadStackTab::TopMiddleBottom),
            mk("Full Stack", PadStackTab::FullStack),
        ]
        .spacing(2)
        .align_y(iced::Alignment::Center),
    )
    .padding([4, 8])
    .width(Length::Fill)
    .into()
}

/// v0.20 — pick_list-friendly proxy for `signex_library::PadShape`.
/// Mirrors Altium's COPPER → Shape dropdown verbatim minus
/// "Custom Shape" (sketch mode owns freeform geometry):
///   Round / Rectangular / Octagonal / Rounded Rectangle /
///   Chamfered Rectangle / Donut.
/// Schema-mapping notes:
///   - Octagonal / Donut have no native variant on
///     `signex_library::PadShape` yet; both fall back to Round at
///     bake. Round trip preserves the picker selection across
///     sessions once we add schema variants in v0.21.
///   - Chamfered Rectangle uses the existing `Chamfered` variant
///     with sensible defaults (25% chamfer, all corners).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PadShapeChoice {
    Round,
    Rectangular,
    Octagonal,
    RoundedRectangle,
    ChamferedRectangle,
    Donut,
}

impl PadShapeChoice {
    const ALL: &'static [PadShapeChoice] = &[
        PadShapeChoice::Round,
        PadShapeChoice::Rectangular,
        PadShapeChoice::Octagonal,
        PadShapeChoice::RoundedRectangle,
        PadShapeChoice::ChamferedRectangle,
        PadShapeChoice::Donut,
    ];

    fn from_lib(s: &signex_library::PadShape) -> Self {
        match s {
            signex_library::PadShape::Round => PadShapeChoice::Round,
            signex_library::PadShape::Rect => PadShapeChoice::Rectangular,
            signex_library::PadShape::RoundRect { .. } => PadShapeChoice::RoundedRectangle,
            signex_library::PadShape::Chamfered { .. } => PadShapeChoice::ChamferedRectangle,
            // Oval / Custom / Octagonal / Donut have no 1:1 schema
            // home today; collapse to Round so the picker stays
            // consistent. Custom Shape is intentionally absent — use
            // sketch mode for freeform geometry.
            _ => PadShapeChoice::Round,
        }
    }
    fn to_lib(self) -> signex_library::PadShape {
        use signex_library::primitive::footprint::ChamferedCorners;
        match self {
            PadShapeChoice::Round => signex_library::PadShape::Round,
            PadShapeChoice::Rectangular => signex_library::PadShape::Rect,
            PadShapeChoice::RoundedRectangle => {
                signex_library::PadShape::RoundRect { radius_ratio: 0.25 }
            }
            PadShapeChoice::ChamferedRectangle => signex_library::PadShape::Chamfered {
                chamfer_ratio: 0.25,
                corners: ChamferedCorners::all(),
            },
            // v0.21 schema follow-up: native Octagonal + Donut. Until
            // then Round is the closest mappable shape (Donut's
            // hole comes from the drill anyway).
            PadShapeChoice::Octagonal => signex_library::PadShape::Round,
            PadShapeChoice::Donut => signex_library::PadShape::Round,
        }
    }
}

impl std::fmt::Display for PadShapeChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            PadShapeChoice::Round => "Round",
            PadShapeChoice::Rectangular => "Rectangular",
            PadShapeChoice::Octagonal => "Octagonal",
            PadShapeChoice::RoundedRectangle => "Rounded Rectangle",
            PadShapeChoice::ChamferedRectangle => "Chamfered Rectangle",
            PadShapeChoice::Donut => "Donut",
        })
    }
}

/// v0.20 — Altium-parity HOLE → Shape dropdown. Round / Slot today;
/// Rectangular hole deferred until the schema gains it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HoleShapeChoice {
    Round,
    Slot,
}

impl HoleShapeChoice {
    const ALL: &'static [HoleShapeChoice] = &[HoleShapeChoice::Round, HoleShapeChoice::Slot];
}

impl std::fmt::Display for HoleShapeChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            HoleShapeChoice::Round => "Round",
            HoleShapeChoice::Slot => "Slot",
        })
    }
}

/// v0.21 — Altium-parity PASTE / SOLDER expansion mode picker.
/// `Rule` defers to the per-board design rule (Solder Mask Expansion
/// / Paste Mask Expansion). `Manual` overrides with an explicit
/// per-pad value (consumed by the matching expansion column).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpansionMode {
    Rule,
    Manual,
}

impl ExpansionMode {
    const ALL: &'static [ExpansionMode] = &[ExpansionMode::Rule, ExpansionMode::Manual];
}

impl std::fmt::Display for ExpansionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ExpansionMode::Rule => "Rule Expansion",
            ExpansionMode::Manual => "Manual",
        })
    }
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

/// v0.21 — Sketch-mode Pad Attributes sub-form. Renders when the
/// selected sketch entity carries a `PadAttr`. Mirrors the
/// Altium-parity Pad Properties / Pad Stack / Pad Features fields
/// surfaced for Pads-mode placement, but bound to the sketch
/// entity's PadAttr rather than the flat-pad list. Geometry-shaping
/// fields (size_x_expr / size_y_expr / mask_margin_expr / etc.)
/// stay sketch-parameterised — those are authored via the Sketch
/// parameter editor, not this form.
fn render_sketch_pad_subform<'a>(
    mut col: Column<'a, PanelMsg>,
    fp: &'a FootprintEditorPanelContext,
    muted: Color,
    primary: Color,
    border_c: Color,
    collapsed_sections: &'a CollapsedSections,
) -> Column<'a, PanelMsg> {
    let Some(p) = fp.selected_sketch_pad.as_ref() else {
        return col;
    };
    let id = p.id;

    col = col.push(props_section_header(
        "Pad Attributes",
        "fp_sketch_pad_attrs",
        collapsed_sections,
        primary,
        border_c,
    ));
    if fp_is_collapsed("fp_sketch_pad_attrs", collapsed_sections) {
        return col;
    }

    // Properties group.
    col = col.push(pad_pick_row(
        "Electrical Type",
        signex_sketch::attr::ElectricalType::ALL,
        p.electrical_type,
        move |v: signex_sketch::attr::ElectricalType| {
            PanelMsg::FpEditorSetSketchPadElectricalType { id, value: v }
        },
        muted,
    ));
    col = col.push(pad_input_row(
        "Net",
        "(unassigned)",
        p.net.clone(),
        move |v| PanelMsg::FpEditorSetSketchPadNet { id, value: v },
        muted,
        primary,
        border_c,
    ));
    col = col.push(pad_check_row(
        "Locked",
        p.locked,
        move |v| PanelMsg::FpEditorToggleSketchPadLocked { id, value: v },
        muted,
        primary,
    ));
    col = col.push(pad_input_row(
        "Template",
        "",
        p.template.clone(),
        move |v| PanelMsg::FpEditorSetSketchPadTemplate { id, value: v },
        muted,
        primary,
        border_c,
    ));
    col = col.push(pad_input_row(
        "Library",
        "",
        p.template_library.clone(),
        move |v| PanelMsg::FpEditorSetSketchPadTemplateLibrary { id, value: v },
        muted,
        primary,
        border_c,
    ));

    // Pad Features.
    col = col.push(
        container(text("Pad Features").size(9).color(muted))
            .padding([6, 8])
            .width(Length::Fill),
    );
    col = col.push(pad_pick_row(
        "Top Side",
        signex_sketch::attr::PadFeature::ALL,
        p.feature_top,
        move |v: signex_sketch::attr::PadFeature| {
            PanelMsg::FpEditorSetSketchPadFeatureTop { id, value: v }
        },
        muted,
    ));
    col = col.push(pad_pick_row(
        "Bottom Side",
        signex_sketch::attr::PadFeature::ALL,
        p.feature_bottom,
        move |v: signex_sketch::attr::PadFeature| {
            PanelMsg::FpEditorSetSketchPadFeatureBottom { id, value: v }
        },
        muted,
    ));

    // Testpoint flags.
    col = col.push(
        container(text("Testpoint").size(9).color(muted))
            .padding([6, 8])
            .width(Length::Fill),
    );
    col = col.push(pad_check_row(
        "Top Assembly",
        p.testpoint.top_assembly,
        move |v| PanelMsg::FpEditorToggleSketchPadTestpointTopAssembly { id, value: v },
        muted,
        primary,
    ));
    col = col.push(pad_check_row(
        "Top Fab",
        p.testpoint.top_fab,
        move |v| PanelMsg::FpEditorToggleSketchPadTestpointTopFab { id, value: v },
        muted,
        primary,
    ));
    col = col.push(pad_check_row(
        "Bottom Assembly",
        p.testpoint.bottom_assembly,
        move |v| PanelMsg::FpEditorToggleSketchPadTestpointBottomAssembly { id, value: v },
        muted,
        primary,
    ));
    col = col.push(pad_check_row(
        "Bottom Fab",
        p.testpoint.bottom_fab,
        move |v| PanelMsg::FpEditorToggleSketchPadTestpointBottomFab { id, value: v },
        muted,
        primary,
    ));

    // Pad Stack overrides (the bool-typed subset; expression-typed
    // mask/paste margins stay sketch-parameterised).
    col = col.push(
        container(text("Pad Stack").size(9).color(muted))
            .padding([6, 8])
            .width(Length::Fill),
    );
    col = col.push(pad_check_row(
        "Top paste enabled",
        p.paste_top_enabled,
        move |v| PanelMsg::FpEditorToggleSketchPadPasteEnabledTop { id, value: v },
        muted,
        primary,
    ));
    col = col.push(pad_check_row(
        "Bottom paste enabled",
        p.paste_bottom_enabled,
        move |v| PanelMsg::FpEditorToggleSketchPadPasteEnabledBottom { id, value: v },
        muted,
        primary,
    ));
    col = col.push(pad_check_row(
        "Top mask tented",
        p.mask_top_tented,
        move |v| PanelMsg::FpEditorToggleSketchPadMaskTentedTop { id, value: v },
        muted,
        primary,
    ));
    col = col.push(pad_check_row(
        "Bottom mask tented",
        p.mask_bottom_tented,
        move |v| PanelMsg::FpEditorToggleSketchPadMaskTentedBottom { id, value: v },
        muted,
        primary,
    ));
    col = col.push(pad_check_row(
        "Thermal relief",
        p.thermal_relief,
        move |v| PanelMsg::FpEditorToggleSketchPadThermalRelief { id, value: v },
        muted,
        primary,
    ));
    let corner_buf = p
        .corner_radius_pct
        .map(|v| format!("{v:.0}"))
        .unwrap_or_default();
    col = col.push(pad_input_row(
        "Corner radius %",
        "25",
        corner_buf,
        move |v| PanelMsg::FpEditorSetSketchPadCornerRadiusPct { id, value: v },
        muted,
        primary,
        border_c,
    ));

    // Hole Details (THT pads only).
    if p.has_drill {
        col = col.push(
            container(text("Hole Details").size(9).color(muted))
                .padding([6, 8])
                .width(Length::Fill),
        );
        let tol_p_buf = p
            .hole_tolerance_plus_mm
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default();
        col = col.push(pad_input_row(
            "Tolerance + (mm)",
            "0",
            tol_p_buf,
            move |v| PanelMsg::FpEditorSetSketchPadHoleTolerancePlus { id, value: v },
            muted,
            primary,
            border_c,
        ));
        let tol_m_buf = p
            .hole_tolerance_minus_mm
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default();
        col = col.push(pad_input_row(
            "Tolerance − (mm)",
            "0",
            tol_m_buf,
            move |v| PanelMsg::FpEditorSetSketchPadHoleToleranceMinus { id, value: v },
            muted,
            primary,
            border_c,
        ));
        let rot_buf = p
            .hole_rotation_deg
            .map(|v| format!("{v:.1}"))
            .unwrap_or_default();
        col = col.push(pad_input_row(
            "Hole rotation (°)",
            "0",
            rot_buf,
            move |v| PanelMsg::FpEditorSetSketchPadHoleRotation { id, value: v },
            muted,
            primary,
            border_c,
        ));
        let cox_buf = p
            .copper_offset_x_mm
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default();
        col = col.push(pad_input_row(
            "Copper offset X (mm)",
            "0",
            cox_buf,
            move |v| PanelMsg::FpEditorSetSketchPadCopperOffsetX { id, value: v },
            muted,
            primary,
            border_c,
        ));
        let coy_buf = p
            .copper_offset_y_mm
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default();
        col = col.push(pad_input_row(
            "Copper offset Y (mm)",
            "0",
            coy_buf,
            move |v| PanelMsg::FpEditorSetSketchPadCopperOffsetY { id, value: v },
            muted,
            primary,
            border_c,
        ));
    }

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
