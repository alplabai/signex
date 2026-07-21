//! Symbol-level (nothing-selected) Properties rows + local-colour swatches.

use iced::widget::{Column, container, row, text};
use iced::{Color, Element, Length};

use super::super::{ColorFieldProps, PanelMsg, SymbolEditorPanelContext, color_field};

/// Render one Local Colors row (Fills / Lines / Pins) via the shared
/// [`color_field`] widget. `picker` carries the transient open-state;
/// the inline palette / HSV overlay only expands when it targets this
/// slot. `None` = inherit from the sheet palette.
fn local_color_field<'a>(
    label: &'a str,
    slot: crate::app::LocalColorSlot,
    current: Option<[u8; 4]>,
    muted: Color,
    border_c: Color,
    picker: Option<crate::app::LocalColorPicker>,
) -> Element<'a, PanelMsg> {
    use std::rc::Rc;

    let this = picker.filter(|p| p.slot == slot);
    let show_palette = this.is_some();
    let show_advanced = this.map(|p| p.advanced).unwrap_or(false);

    let on_pick: Rc<dyn Fn([u8; 4]) -> PanelMsg + 'static> =
        Rc::new(move |rgba| PanelMsg::SymEditorSetLocalColor { slot, color: rgba });
    let on_clear = current
        .is_some()
        .then_some(PanelMsg::SymEditorClearLocalColor(slot));

    color_field(ColorFieldProps {
        label,
        current,
        none_label: "Inherit",
        show_palette,
        show_advanced,
        muted,
        border_c,
        on_toggle: PanelMsg::SymEditorToggleLocalColorPicker(slot),
        on_advanced: PanelMsg::SymEditorOpenLocalColorAdvanced(slot),
        on_cancel: PanelMsg::SymEditorCancelLocalColorPicker,
        on_pick,
        on_clear,
    })
}

/// Symbol-level default Properties (nothing selected): identity,
/// graphical toggles, and local-colour overrides.
pub(super) fn view_symbol_selection<'a>(
    mut col: Column<'a, PanelMsg>,
    sym: &'a SymbolEditorPanelContext,
    muted: Color,
    primary: Color,
    border_c: Color,
) -> Column<'a, PanelMsg> {
    // The .snxsym editor is a SYMBOL editor — symbol-level
    // visual / geometric properties only. Component metadata
    // (Designator / Comment / Description / Type / Parameters)
    // lives on the host ComponentRow in the component library
    // and is edited from the Library Browser / Component
    // Editor, not here.

    // Helper — labelled text_input row.
    let text_field = |label: &'a str,
                      value: &'a str,
                      placeholder: &'a str,
                      on_input: fn(String) -> PanelMsg|
     -> Element<'a, PanelMsg> {
        container(
            row![
                text(label)
                    .size(10)
                    .color(muted)
                    .width(Length::FillPortion(2)),
                iced::widget::text_input(placeholder, value)
                    .padding([2, 4])
                    .size(11)
                    .on_input(on_input)
                    .width(Length::FillPortion(3)),
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center),
        )
        .padding([3, 8])
        .width(Length::Fill)
        .into()
    };

    // ── ▾ Symbol ──
    col = col.push(super::super::thin_sep(border_c));
    col = col.push(container(text("Symbol").size(11).color(primary)).padding([6, 8]));
    col = col.push(text_field(
        "Design Item ID",
        sym.symbol_name.as_str(),
        "Symbol name",
        PanelMsg::SymEditorSetSymbolName,
    ));
    col = col.push(super::prop_row_static(
        "UUID",
        sym.symbol_uuid.to_string(),
        muted,
        primary,
    ));
    col = col.push(super::prop_row_static(
        "Pins",
        sym.pins.len().to_string(),
        muted,
        primary,
    ));
    col = col.push(super::prop_row_static(
        "Graphics",
        sym.graphics.len().to_string(),
        muted,
        primary,
    ));

    // Part of Parts (Altium "Part B / of Parts: 2") — surfaces
    // the multi-part picker the user already drives via the
    // toolbar arrows or the SCH Library tree-expander.
    let part_label = if sym.active_max_part > 1 {
        format!(
            "Part {} / of Parts {}",
            sym.active_part, sym.active_max_part
        )
    } else {
        "Single-part".to_string()
    };
    col = col.push(super::prop_row_static("Part", part_label, muted, primary));

    // ── ▾ Graphical ──
    col = col.push(super::super::thin_sep(border_c));
    col = col.push(container(text("Graphical").size(11).color(primary)).padding([6, 8]));
    let mirrored_row: Element<'a, PanelMsg> = container(
        row![
            text("Mirrored")
                .size(10)
                .color(muted)
                .width(Length::FillPortion(2)),
            iced::widget::checkbox(sym.symbol_mirrored)
                .size(14)
                .on_toggle(|_| PanelMsg::SymEditorToggleSymbolMirrored),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .padding([3, 8])
    .width(Length::Fill)
    .into();
    col = col.push(mirrored_row);

    // ── Local Colors (Fills / Lines / Pins) ──
    col = col.push(super::super::thin_sep(border_c));
    col = col.push(container(text("Local Colors").size(11).color(primary)).padding([6, 8]));
    col = col.push(local_color_field(
        "Fills",
        crate::app::LocalColorSlot::Fill,
        sym.symbol_local_fill_color,
        muted,
        border_c,
        sym.local_color_picker,
    ));
    col = col.push(local_color_field(
        "Lines",
        crate::app::LocalColorSlot::Line,
        sym.symbol_local_line_color,
        muted,
        border_c,
        sym.local_color_picker,
    ));
    col = col.push(local_color_field(
        "Pins",
        crate::app::LocalColorSlot::Pin,
        sym.symbol_local_pin_color,
        muted,
        border_c,
        sym.local_color_picker,
    ));
    col
}
