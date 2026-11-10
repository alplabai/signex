//! Properties panel body for the Footprint editor (HI-22 / MD-20).
//!
//! Extracted from `panels/mod.rs`. Pure view code — zero behaviour change
//! from the move. Switches between Pads / Sketch / 3D View contexts and
//! renders the v0.18.13 Library Options sections (Snap Options / Grid /
//! Guide / Other) plus the v0.16.4 role sub-forms (Pour / Keepout /
//! Cutout) and the v0.16.3 Pad placement defaults form.

use iced::widget::{Column, container, row, scrollable};
use iced::{Color, Element, Length, Widget};
use signex_widgets::{raw, text};

use super::{
    CollapsedSections, FootprintEditorPanelContext, FootprintModeKind, PanelMsg, PanelPalette,
    collapsible_section_header, is_section_collapsed,
};
use crate::styles::{Spacing, TypeScale};
use pad::{
    PadEditTarget, PadFormValues, render_pad_form_pad_features, render_pad_form_pad_stack,
    render_pad_form_properties,
};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

// Submodule declarations (split from the original 4570-line file).
mod conflicts;
mod managers;
mod pad;
mod pad_stack_check;
mod redundant;
mod rows;
mod sections;
mod selection;
mod silk;
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
    palette: PanelPalette,
    collapsed_sections: &'a CollapsedSections,
    unit: signex_types::coord::Unit,
    i18n: &signex_i18n::I18n,
) -> Element<'a, PanelMsg> {
    let PanelPalette {
        muted,
        primary,
        border: border_c,
        input_bg,
        accent: accent_c,
        ..
    } = palette;
    let mode_label = match fp.mode_kind {
        FootprintModeKind::Pads => "Pads",
        FootprintModeKind::Sketch => "Sketch",
        FootprintModeKind::View3d => "3D View",
    };

    let mut col: Column<Element<'a, PanelMsg>> = Column::new().spacing(0).width(Length::Fill);

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
        text(signex_i18n::UserText::new(&fp.footprint_name))
            .size(TS.label)
            .color(primary),
        text(raw("·")).size(TS.label).color(muted),
        text(raw(mode_label)).size(TS.body).color(muted),
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center);
    if let Some(tag) = multi_select_tag {
        header_row = header_row.push(text(raw("·")).size(TS.label).color(muted).boxed());
        header_row = header_row.push(text(raw(tag)).size(TS.body).color(accent_c).boxed());
    }

    col = col.push(
        container(header_row)
            .padding([6.0, SP.m])
            .width(Length::Fill)
            .boxed(),
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
        if let Some(pad) = fp.selected_pad.as_ref()
            && !in_placement
        {
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
            col = props_kv_row(
                col,
                muted,
                input_bg,
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
            // v0.21 — "Edit in Sketch" jump button. Visible only
            // when the pad has a backing sketch entity (auto-
            // minted on first Sketch-mode entry or placed via
            // sketch). The handler switches editor.state.mode to
            // Sketch + selects entity_id; if the pad has no
            // sketch entity yet, this is a no-op.
            let pad_idx = pad.idx;
            col = col.push(
                container(
                    signex_widgets::button(
                        text(raw("Edit in Sketch ▸"))
                            .size(TS.caption)
                            .color(primary),
                    )
                    .padding([SP.s, 10.0])
                    .on_press(PanelMsg::FpEditorEditPadInSketch { pad_idx })
                    .style(iced::widget::button::primary),
                )
                .padding([6.0, SP.m])
                .width(Length::Fill)
                .boxed(),
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
            .height(Length::Fill)
            .style(crate::panels::form_look::scrollbar_style)
            .boxed();
        }
        if in_placement {
            let values = PadFormValues::from_next_pad(fp);
            let target = PadEditTarget::Next;
            col = render_pad_form_properties(
                col,
                &values,
                target,
                fp.placement_paused,
                palette,
                collapsed_sections,
            );
            col = render_pad_form_pad_stack(col, &values, target, palette, collapsed_sections, &[]);
            col = render_pad_form_pad_features(col, &values, target, palette, collapsed_sections);
            return scrollable(container(col).padding(iced::Padding {
                top: 0.0,
                right: 12.0,
                bottom: 0.0,
                left: 0.0,
            }))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(crate::panels::form_look::scrollbar_style)
            .boxed();
        }
        // Empty canvas + idle → fall through to the original chrome.
    }

    // Selection-specific top section. Pads + selected pad → pad
    // summary; Sketch + selected entity → entity summary + Role
    // pick_list; otherwise → footprint summary. Sketch-mode-only
    // sections (Parameters / DOF / Warnings) follow regardless of
    // selection, so the user can monitor solve state while authoring.
    col = selection::view_selection(col, fp, mode_label, palette, collapsed_sections);
    col = pad_stack_check::view_pad_stack_check(col, fp, palette, collapsed_sections);
    col = sections::view_sections(col, fp, palette, collapsed_sections, unit, i18n);
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
    .height(Length::Fill)
    .style(crate::panels::form_look::scrollbar_style)
    .boxed()
}

/// v0.20 — common Settings + Hint footer: the Settings header and a
/// mode-specific hint string.
fn render_fp_settings_and_hint<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    fp: &'a FootprintEditorPanelContext,
    muted: Color,
    primary: Color,
    border_c: Color,
    collapsed_sections: &'a CollapsedSections,
) -> Column<Element<'a, PanelMsg>> {
    col = col.push(
        collapsible_section_header(
            "fp_settings",
            "Settings",
            collapsed_sections,
            primary,
            border_c,
        )
        .boxed(),
    );

    col = col.push(
        collapsible_section_header("fp_hint", "Hint", collapsed_sections, primary, border_c)
            .boxed(),
    );
    if !is_section_collapsed("fp_hint", collapsed_sections) {
        let hint = match fp.mode_kind {
            FootprintModeKind::Pads => "Click a pad to edit its properties.",
            FootprintModeKind::Sketch => {
                "Click a sketch entity (Point / Line / Arc / Circle) to edit it."
            }
            FootprintModeKind::View3d => "3D View — use the 3D preview pane to inspect the body.",
        };
        col = col.push(
            container(text(raw(hint)).size(TS.caption).color(muted))
                .padding([SP.s, SP.m])
                .width(Length::Fill)
                .boxed(),
        );
    }
    col
}

/// Read-only key-value row — delegates to the schematic Properties
/// panel's `form_input_row` so the footprint editor uses identical
/// chrome (orange-accent border, dark-blue selection-tinted background).
/// Returns the updated Column to keep the chained-update call style
/// the rest of this module uses.
fn props_kv_row<'a>(
    col: Column<Element<'a, PanelMsg>>,
    label_c: Color,
    input_bg: Color,
    key: &str,
    value: String,
) -> Column<Element<'a, PanelMsg>> {
    col.push(rows::value_row(key, &value, label_c, input_bg))
}
