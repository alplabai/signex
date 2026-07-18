//! Properties panel body for the Footprint editor (HI-22 / MD-20).
//!
//! Extracted from `panels/mod.rs`. Pure view code — zero behaviour change
//! from the move. Switches between Pads / Sketch / 3D View contexts and
//! renders the v0.18.13 Library Options sections (Snap Options / Grid /
//! Guide / Other) plus the v0.16.4 role sub-forms (Pour / Keepout /
//! Cutout) and the v0.16.3 Pad placement defaults form.

use iced::widget::{Column, container, row, scrollable, text};
use iced::{Color, Element, Length};

use super::{CollapsedSections, FootprintEditorPanelContext, FootprintModeKind, PanelMsg};
use pad::{
    PadEditTarget, PadFormValues, render_pad_form_pad_features, render_pad_form_pad_stack,
    render_pad_form_properties,
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
mod sections;
mod selection;
mod snap_options;
mod subforms;

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
    col = selection::view_selection(
        col,
        fp,
        mode_label,
        muted,
        primary,
        border_c,
        input_bg,
        input_bdr,
        custom_filter_presets,
        active_custom_filter_tab,
        collapsed_sections,
        accent_c,
        tag_hover,
    );
    col = sections::view_sections(
        col,
        fp,
        muted,
        primary,
        border_c,
        input_bg,
        input_bdr,
        collapsed_sections,
        unit,
        seg_hover,
    );
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
