use iced::widget::{Column, container, pick_list, row, text};
use iced::{Color, Element, Length, Widget as _};

use super::super::{
    CollapsedSections, FootprintEditorPanelContext, FootprintModeKind, PanelMsg, PanelPalette,
};
use super::managers::grid_manager_btn;
use super::pad::{
    PadEditTarget, PadFormValues, pad_check_row, pad_input_row, pad_pick_row,
    render_pad_form_pad_features, render_pad_form_pad_stack, render_pad_form_properties,
};
use super::props_kv_row;
use super::silk::silk_geometry_rows;
use super::subforms::{
    render_cutout_subform, render_keepout_subform, render_pattern_subform, render_pour_subform,
    render_sketch_pad_subform,
};
use crate::panels::{collapsible_section_header, is_section_collapsed};
use crate::styles::{Spacing, TypeScale};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

pub(super) fn view_selection<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    fp: &'a FootprintEditorPanelContext,
    mode_label: &str,
    palette: PanelPalette,
    custom_filter_presets: Vec<crate::active_bar::CustomFilterPreset>,
    active_custom_filter_tab: usize,
    collapsed_sections: &'a CollapsedSections,
) -> Column<Element<'a, PanelMsg>> {
    let PanelPalette {
        muted,
        primary,
        border: border_c,
        input_bg,
        input_bdr,
        panel_bg,
        ..
    } = palette;
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
                palette,
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
                palette,
                collapsed_sections,
                &fp.selected_pad_shape_params,
            );
            col = render_pad_form_pad_features(col, &values, target, palette, collapsed_sections);
        }
        (FootprintModeKind::Sketch, _, Some(ent)) => {
            col = col.push(
                collapsible_section_header(
                    "fp_sketch_entity",
                    "Sketch entity",
                    collapsed_sections,
                    primary,
                    border_c,
                )
                .boxed(),
            );
            if !is_section_collapsed("fp_sketch_entity", collapsed_sections) {
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
                                container(text("DOF").size(TS.caption).color(muted))
                                    .width(Length::FillPortion(1)),
                                container(text(label).size(TS.caption).color(c))
                                    .width(Length::FillPortion(2)),
                            ]
                            .align_y(iced::Alignment::Center),
                        )
                        .padding([3.0, SP.m])
                        .width(Length::Fill)
                        .boxed(),
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
            if ent.kind_label == "Arc"
                && let Some(id) = fp.selected_sketch_entity_id
            {
                col = col.push(
                    container(
                        iced::widget::button(
                            text("Unlink corner radius").size(TS.caption).color(primary),
                        )
                        .padding([SP.s, 10.0])
                        .on_press(PanelMsg::FpEditorUnlinkCornerRadius { arc_entity_id: id })
                        .style(iced::widget::button::secondary),
                    )
                    .padding([6.0, SP.m])
                    .width(Length::Fill)
                    .boxed(),
                );
            }

            // v0.16.2 — Role pick_list. Visible when an entity is
            // selected; pick_list value mirrors the entity's
            // currently-attached `*Attr` slot (or `Unassigned`).
            col = col.push(
                collapsible_section_header(
                    "fp_role",
                    "Role",
                    collapsed_sections,
                    primary,
                    border_c,
                )
                .boxed(),
            );
            if !is_section_collapsed("fp_role", collapsed_sections)
                && let Some(id) = fp.selected_sketch_entity_id
            {
                use crate::library::messages::RoleTag;
                let current = fp.selected_sketch_role;
                let dropdown = pick_list(Some(current), RoleTag::ALL, |r: &RoleTag| r.to_string())
                    .on_select(move |new_role| PanelMsg::FpEditorSetRole { id, role: new_role })
                    .text_size(11)
                    .padding([3.0, SP.m])
                    .width(Length::Fill);
                col = col.push(
                    container(dropdown)
                        .padding([SP.s, SP.m])
                        .width(Length::Fill)
                        .boxed(),
                );
                if !fp.selected_sketch_is_point {
                    col = col.push(
                        container(
                            text("Pad role applies to Points only")
                                .size(TS.caption)
                                .color(muted),
                        )
                        .padding([0.0, SP.m])
                        .width(Length::Fill)
                        .boxed(),
                    );
                }

                // v0.16.4 — role sub-forms. Render when the
                // matching `*Attr` is set on the selected entity.
                col = render_pour_subform(col, fp, id, muted, primary, border_c);
                col = render_keepout_subform(col, fp, id, muted, primary, border_c, panel_bg);
                col = render_cutout_subform(col, fp, id, muted, primary, border_c, panel_bg);
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
                col = render_pattern_subform(col, fp, muted, primary, border_c, collapsed_sections);
            }
        }
        _ => {
            // v0.18.24 — silk-front graphic selection branch.
            // Renders BEFORE the empty-canvas Library Options when a
            // silk graphic is selected so the user can edit Text
            // content + delete the entry without leaving the
            // Properties panel.
            if let Some(silk) = fp.selected_silk_summary.as_ref() {
                col = col.push(
                    collapsible_section_header(
                        "fp_silk_graphic",
                        "Silk graphic",
                        collapsed_sections,
                        primary,
                        border_c,
                    )
                    .boxed(),
                );
                if !is_section_collapsed("fp_silk_graphic", collapsed_sections) {
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

                    if silk.from_sketch {
                        // #1718: a baked entry is the sketch's; Pads mode
                        // shows it but does not edit it.
                        col = col.push(
                            container(
                                text("From sketch: edit in Sketch mode")
                                    .size(TS.caption)
                                    .color(muted),
                            )
                            .padding([SP.s, SP.m])
                            .width(Length::Fill)
                            .boxed(),
                        );
                    } else {
                        col = silk_geometry_rows(col, &silk.kind, muted, primary, border_c);

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
                                .spacing(SP.s)
                                .align_y(iced::Alignment::Center),
                            )
                            .padding([SP.s, SP.m])
                            .width(Length::Fill)
                            .boxed(),
                        );
                    }
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
                col = col.push(
                    super::super::view_custom_selection_filters_section(
                        custom_filter_presets,
                        active_custom_filter_tab,
                        collapsed_sections,
                        palette,
                    )
                    .boxed(),
                );
                col = col.push(
                    collapsible_section_header(
                        "fp_footprint",
                        "Footprint",
                        collapsed_sections,
                        primary,
                        border_c,
                    )
                    .boxed(),
                );
                if !is_section_collapsed("fp_footprint", collapsed_sections) {
                    // Editable Name — text_input bound to the active
                    // internal footprint's `name` field via
                    // PanelMsg::FpEditorSetFootprintName.
                    let name_val = fp.footprint_name.clone();
                    col = col.push(super::super::form_edit_row(
                        "Name",
                        &name_val,
                        muted,
                        PanelMsg::FpEditorSetFootprintName,
                    ));

                    // v0.21 — Altium-parity Component fields. Editable
                    // Description / Default Designator / Component
                    // Type / Height in mm. Read-only Version / Mode /
                    // Pads count below for parity with the Altium
                    // PCB Library Component Properties dialog.
                    let desc_val = fp.footprint_description.clone();
                    col = col.push(super::super::form_edit_row(
                        "Description",
                        &desc_val,
                        muted,
                        PanelMsg::FpEditorSetFootprintDescription,
                    ));
                    let dd_val = fp.footprint_default_designator.clone();
                    col = col.push(super::super::form_edit_row(
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
                    col = col.push(super::super::form_edit_row(
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
    col
}
