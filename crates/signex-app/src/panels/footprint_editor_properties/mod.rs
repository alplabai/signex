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

/// v0.23 — Per-instance checkbox grid safety cap for Grid arrays.
/// Dense BGAs can declare hundreds of cells per axis; rendering a
/// 50×50 = 2500-checkbox grid would blow the panel viewport. Above
/// this cap the user keeps editing via `mask_expr`.
const MAX_GRID_CHECKBOX_DIM: u32 = 32;

/// v0.23 — Per-instance checkbox row safety cap for Polar arrays.
/// 64 instances covers 5° increments around a full circle; finer
/// patterns continue to author through `mask_expr`.
const MAX_POLAR_CHECKBOX_COUNT: u32 = 64;

// Submodule declarations (split from the original 4570-line file).
mod managers;
mod pad;
mod snap_options;
mod subforms;

use managers::{grid_manager_btn, render_grid_manager, render_guide_manager, render_other_section};
use pad::{
    PadEditTarget, PadFormValues, pad_check_row, pad_copper_offset_x_msg, pad_copper_offset_y_msg,
    pad_corner_radius_msg, pad_designator_msg, pad_drill_diameter_msg, pad_drill_slot_length_msg,
    pad_electrical_type_msg, pad_feature_bottom_msg, pad_feature_top_msg, pad_hole_rotation_msg,
    pad_hole_tolerance_minus_msg, pad_hole_tolerance_plus_msg, pad_input_row, pad_locked_msg,
    pad_mask_bottom_msg, pad_mask_tented_bottom_msg, pad_mask_tented_top_msg, pad_mask_top_msg,
    pad_net_msg, pad_paste_bottom_msg, pad_paste_enabled_bottom_msg, pad_paste_enabled_top_msg,
    pad_paste_top_msg, pad_pick_row, pad_plated_msg, pad_rotation_msg, pad_shape_msg, pad_side_msg,
    pad_size_x_msg, pad_size_y_msg, pad_template_library_msg, pad_template_msg,
    pad_testpoint_bottom_assembly_msg, pad_testpoint_bottom_fab_msg,
    pad_testpoint_top_assembly_msg, pad_testpoint_top_fab_msg, pad_thermal_relief_msg,
    render_pad_form_pad_features, render_pad_form_pad_stack, render_pad_form_properties,
};
use snap_options::{render_snap_subtab_row, render_snapping_mode_row};
use subforms::{
    render_cutout_subform, render_keepout_subform, render_pattern_subform, render_pour_subform,
    render_sketch_pad_subform,
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

    // v0.27 — multi-select indicator. With > 1 pad selected, show a
    // "(N pads selected)" tag next to the mode label so the user
    // knows the form below shows only the primary pad's properties
    // while highlights cover everything.
    let multi_select_tag = if fp.selected_pad_count > 1 {
        Some(format!("({} pads selected)", fp.selected_pad_count))
    } else {
        None
    };

    let mut header_row = row![
        text(&fp.footprint_name).size(12).color(primary),
        text("·").size(12).color(muted),
        text(mode_label).size(11).color(muted),
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center);
    if let Some(tag) = multi_select_tag {
        header_row = header_row.push(text("·").size(12).color(muted));
        header_row = header_row.push(text(tag).size(11).color(accent_c));
    }

    col = col.push(container(header_row).padding([6, 8]).width(Length::Fill));
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
                    col,
                    &values,
                    target,
                    false,
                    muted,
                    primary,
                    border_c,
                    collapsed_sections,
                );
                col = props_kv_row(
                    col,
                    muted,
                    input_bg,
                    input_bdr,
                    "Position",
                    format!("({:.3}, {:.3}) mm", pad.position_mm[0], pad.position_mm[1]),
                );
                col = render_pad_form_pad_stack(
                    col,
                    &values,
                    target,
                    muted,
                    primary,
                    border_c,
                    collapsed_sections,
                    &fp.selected_pad_shape_params,
                );
                col = render_pad_form_pad_features(
                    col,
                    &values,
                    target,
                    muted,
                    primary,
                    border_c,
                    collapsed_sections,
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
                        iced::widget::button(text("Edit in Sketch ▸").size(10).color(primary))
                            .padding([4, 10])
                            .on_press(PanelMsg::FpEditorEditPadInSketch { pad_idx })
                            .style(iced::widget::button::primary),
                    )
                    .padding([6, 8])
                    .width(Length::Fill),
                );
                // v0.25 polish — reserve 12 px on the right so the
                // scrollbar doesn't overlap input fields. Without
                // this, picklists and text_inputs that extend to
                // Length::Fill end exactly under the scrollbar's
                // track and the user can't reach the right edge.
                return scrollable(container(col).padding(iced::Padding {
                    top: 0.0,
                    right: 12.0,
                    bottom: 0.0,
                    left: 0.0,
                }))
                .width(Length::Fill)
                .into();
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
                col,
                &values,
                target,
                muted,
                primary,
                border_c,
                collapsed_sections,
                &[],
            );
            col = render_pad_form_pad_features(
                col,
                &values,
                target,
                muted,
                primary,
                border_c,
                collapsed_sections,
            );
            return scrollable(container(col).padding(iced::Padding {
                top: 0.0,
                right: 12.0,
                bottom: 0.0,
                left: 0.0,
            }))
            .width(Length::Fill)
            .into();
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
            col = render_pad_form_properties(
                col,
                &values,
                target,
                false,
                muted,
                primary,
                border_c,
                collapsed_sections,
            );
            // Position is the one read-only field (use drag to move).
            col = props_kv_row(
                col,
                muted,
                input_bg,
                input_bdr,
                "Position",
                format!("({:.3}, {:.3}) mm", pad.position_mm[0], pad.position_mm[1]),
            );
            col = render_pad_form_pad_stack(
                col,
                &values,
                target,
                muted,
                primary,
                border_c,
                collapsed_sections,
                &fp.selected_pad_shape_params,
            );
            col = render_pad_form_pad_features(
                col,
                &values,
                target,
                muted,
                primary,
                border_c,
                collapsed_sections,
            );
        }
        (FootprintModeKind::Sketch, _, Some(ent)) => {
            col = col.push(props_section_header(
                "Sketch entity",
                "fp_sketch_entity",
                collapsed_sections,
                primary,
                border_c,
            ));
            if !fp_is_collapsed("fp_sketch_entity", collapsed_sections) {
                col = props_kv_row(
                    col,
                    muted,
                    input_bg,
                    input_bdr,
                    "Kind",
                    ent.kind_label.into(),
                );
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
                        DofColor::Over => {
                            ("Over-constrained", Color::from_rgba(1.00, 0.20, 0.20, 1.00))
                        }
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

            // v0.24 Phase 3 (Track A3) — Unlink corner radius button.
            // Visible when the selected entity is an Arc that's part
            // of a RoundRect pad's corner outline — detected by
            // walking pads for any whose `corner_r_*_arc` sidecar
            // value matches the selected entity's id slug. The
            // button is the Properties-panel surface for the
            // right-click "Unlink radius" action; clicking it mints
            // a per-corner override for that one Arc and leaves the
            // other 3 corners on the shared corner_r parameter.
            if ent.kind_label == "Arc" {
                if let Some(id) = fp.selected_sketch_entity_id {
                    col = col.push(
                        container(
                            iced::widget::button(
                                text("Unlink corner radius").size(10).color(primary),
                            )
                            .padding([4, 10])
                            .on_press(PanelMsg::FpEditorUnlinkCornerRadius { arc_entity_id: id })
                            .style(iced::widget::button::secondary),
                        )
                        .padding([6, 8])
                        .width(Length::Fill),
                    );
                }
            }

            // v0.16.2 — Role pick_list. Visible when an entity is
            // selected; pick_list value mirrors the entity's
            // currently-attached `*Attr` slot (or `Unassigned`).
            col = col.push(props_section_header(
                "Role",
                "fp_role",
                collapsed_sections,
                primary,
                border_c,
            ));
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
                    // v0.23 — Pattern sub-form. Renders when the
                    // selected entity is the source of an Array.
                    col = render_pattern_subform(
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
                col = col.push(props_section_header(
                    "Silk graphic",
                    "fp_silk_graphic",
                    collapsed_sections,
                    primary,
                    border_c,
                ));
                if !fp_is_collapsed("fp_silk_graphic", collapsed_sections) {
                    col = props_kv_row(
                        col,
                        muted,
                        input_bg,
                        input_bdr,
                        "Kind",
                        silk.kind_label.into(),
                    );
                    col = props_kv_row(
                        col,
                        muted,
                        input_bg,
                        input_bdr,
                        "Index",
                        silk.idx.to_string(),
                    );

                    use crate::panels::SilkKindGeometry;
                    match &silk.kind {
                        SilkKindGeometry::Line { from_mm, to_mm } => {
                            col = col.push(pad_input_row(
                                "From X (mm)",
                                "0",
                                format!("{:.3}", from_mm[0]),
                                PanelMsg::FpEditorSetSilkLineFromX,
                                muted,
                                primary,
                                border_c,
                            ));
                            col = col.push(pad_input_row(
                                "From Y (mm)",
                                "0",
                                format!("{:.3}", from_mm[1]),
                                PanelMsg::FpEditorSetSilkLineFromY,
                                muted,
                                primary,
                                border_c,
                            ));
                            col = col.push(pad_input_row(
                                "To X (mm)",
                                "0",
                                format!("{:.3}", to_mm[0]),
                                PanelMsg::FpEditorSetSilkLineToX,
                                muted,
                                primary,
                                border_c,
                            ));
                            col = col.push(pad_input_row(
                                "To Y (mm)",
                                "0",
                                format!("{:.3}", to_mm[1]),
                                PanelMsg::FpEditorSetSilkLineToY,
                                muted,
                                primary,
                                border_c,
                            ));
                        }
                        SilkKindGeometry::Text {
                            position_mm,
                            content,
                            size_mm,
                        } => {
                            col = col.push(pad_input_row(
                                "Content",
                                "TEXT",
                                content.clone(),
                                PanelMsg::FpEditorSetSilkText,
                                muted,
                                primary,
                                border_c,
                            ));
                            col = col.push(pad_input_row(
                                "Position X (mm)",
                                "0",
                                format!("{:.3}", position_mm[0]),
                                PanelMsg::FpEditorSetSilkTextPositionX,
                                muted,
                                primary,
                                border_c,
                            ));
                            col = col.push(pad_input_row(
                                "Position Y (mm)",
                                "0",
                                format!("{:.3}", position_mm[1]),
                                PanelMsg::FpEditorSetSilkTextPositionY,
                                muted,
                                primary,
                                border_c,
                            ));
                            col = col.push(pad_input_row(
                                "Size (mm)",
                                "1.0",
                                format!("{:.3}", size_mm),
                                PanelMsg::FpEditorSetSilkTextSize,
                                muted,
                                primary,
                                border_c,
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
                        muted,
                        primary,
                        border_c,
                    ));
                    // Filled flag (only meaningful for closed shapes;
                    // surfacing for all so the user can flip it without
                    // hunting for the right tool).
                    col = col.push(pad_check_row(
                        "Filled",
                        silk.filled,
                        PanelMsg::FpEditorToggleSilkFilled,
                        muted,
                        primary,
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
                col = col.push(props_section_header(
                    "Footprint",
                    "fp_footprint",
                    collapsed_sections,
                    primary,
                    border_c,
                ));
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
                    col = props_kv_row(
                        col,
                        muted,
                        input_bg,
                        input_bdr,
                        "Version",
                        fp.version.clone(),
                    );
                    col = props_kv_row(col, muted, input_bg, input_bdr, "Mode", mode_label.into());
                    col = props_kv_row(
                        col,
                        muted,
                        input_bg,
                        input_bdr,
                        "Pads",
                        fp.pad_count.to_string(),
                    );
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
        col = col.push(super::thin_sep(border_c));
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
    col = render_fp_settings_and_hint(col, fp, muted, primary, border_c, collapsed_sections);

    // v0.25 polish — see early-return scrollable wrappers above for
    // why the 12 px right padding lives here.
    scrollable(container(col).padding(iced::Padding {
        top: 0.0,
        right: 12.0,
        bottom: 0.0,
        left: 0.0,
    }))
    .width(Length::Fill)
    .into()
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
        // v0.26-I — auto-courtyard toggle removed. The courtyard is
        // an authored shape (silk / sketch entity), not an auto-
        // derived bbox. Section header kept (other settings can
        // land here in v0.27+).
        let _ = fp.auto_fit_courtyard;
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
pub(super) fn props_section_header<'a>(
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
pub(super) fn fp_is_collapsed(key: &str, collapsed: &super::CollapsedSections) -> bool {
    super::is_section_collapsed(key, collapsed)
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
    col.push(super::form_input_row(
        key, &value, label_c, input_bg, input_bdr,
    ))
}
