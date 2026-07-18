//! Properties surface for a single hierarchical child sheet — read-only
//! identity/geometry plus the editable Border/Fill colour swatch rows
//! and stroke-width row. Moved verbatim from the former single-file
//! `element_properties` module.

use super::super::*;

/// Properties section for a single hierarchical child sheet.
/// Shows read-only info (Name / File / Position / Size) plus
/// editable Border Colour, Fill Colour and Line Width with a
/// Reset-to-default button. Colour edits open an iced_aw
/// ColorPicker overlay anchored to a swatch button.
pub(in crate::panels) fn view_child_sheet_properties<'a>(
    ctx: &'a PanelContext,
    muted: Color,
    primary: Color,
    border_c: Color,
) -> Element<'a, PanelMsg> {
    let Some(child_sheet) = ctx.selected_child_sheet.as_ref() else {
        return Column::new().width(Length::Fill).into();
    };

    let id = child_sheet.uuid;
    let name = child_sheet.name.clone();
    let filename = child_sheet.filename.clone();
    let position = format!(
        "{:.2}, {:.2}",
        child_sheet.position.x, child_sheet.position.y
    );
    let size = format!("{:.1} x {:.1} mm", child_sheet.size.0, child_sheet.size.1);

    let stroke_width = child_sheet.stroke_width;
    let stroke_color = child_sheet.stroke_color;
    let fill_color = child_sheet.fill_color;
    let border_picker_open = ctx.child_sheet_border_picker_open;
    let fill_picker_open = ctx.child_sheet_fill_picker_open;
    let border_advanced_open = ctx.child_sheet_border_advanced_open;
    let fill_advanced_open = ctx.child_sheet_fill_advanced_open;
    let stroke_width_buf = ctx.child_sheet_stroke_width_buf.clone();

    let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);

    // ── Properties (read-only identity / geometry) ──
    col = col.push(collapsible_section(
        "sel_child_sheet_props",
        "Properties",
        &ctx.collapsed_sections,
        muted,
        border_c,
        move || {
            let mut c = Column::new().spacing(0).width(Length::Fill);
            c = c.push(prop_kv_row("Name", &name, muted, primary));
            c = c.push(prop_kv_row("File", &filename, muted, primary));
            c = c.push(prop_kv_row("Position", &position, muted, primary));
            c = c.push(prop_kv_row("Size", &size, muted, primary));
            c
        },
    ));

    // ── Style (editable) ──
    col = col.push(collapsible_section(
        "sel_child_sheet_style",
        "Style",
        &ctx.collapsed_sections,
        muted,
        border_c,
        move || {
            let mut c = Column::new().spacing(0).width(Length::Fill);
            c = c.push(child_sheet_color_row(
                "Border Colour",
                id,
                stroke_color,
                border_picker_open,
                border_advanced_open,
                muted,
                border_c,
                /* is_border */ true,
            ));
            c = c.push(child_sheet_color_row(
                "Fill Colour",
                id,
                fill_color,
                fill_picker_open,
                fill_advanced_open,
                muted,
                border_c,
                /* is_border */ false,
            ));
            c = c.push(child_sheet_stroke_width_row(
                id,
                stroke_width,
                stroke_width_buf,
                muted,
                border_c,
            ));
            c = c.push(
                container(
                    iced::widget::button(
                        text("Reset to Default")
                            .size(10)
                            .color(Color::from_rgb(0.92, 0.92, 0.94)),
                    )
                    .padding([4, 10])
                    .on_press(PanelMsg::ResetChildSheetStyle(id))
                    .style(iced::widget::button::secondary),
                )
                .padding([6, 8])
                .width(Length::Fill),
            );
            c
        },
    ));

    col.into()
}

/// One swatch row in the child-sheet Style section.
///
/// Click flow:
///   1. Click swatch → expands an inline preset palette panel below
///      the row (full panel width, like the canvas-font popup) with
///      a 12-colour grid plus "Custom…" and (when an override is
///      active) "Reset to Default".
///   2. Click the swatch again to collapse, or click "Custom…" to
///      switch to the iced_aw HSV / RGB ColorPicker overlay.
///
/// Both the palette pick and the advanced-picker submit reuse the
/// same `EditChildSheet*Color` message so engine command + undo/redo
/// round-trip is identical for both paths.
fn child_sheet_color_row<'a>(
    label: &'a str,
    sheet_id: uuid::Uuid,
    current: Option<signex_types::schematic::StrokeColor>,
    show_picker: bool,
    show_advanced: bool,
    muted: Color,
    border_c: Color,
    is_border: bool,
) -> Element<'a, PanelMsg> {
    use std::rc::Rc;

    // StrokeColor → [u8; 4] RGBA so the generic widget stays agnostic
    // of the schematic colour type.
    let current_rgba = current.map(|c| [c.r, c.g, c.b, c.a]);

    let on_toggle = if is_border {
        PanelMsg::ToggleChildSheetBorderPicker(sheet_id)
    } else {
        PanelMsg::ToggleChildSheetFillPicker(sheet_id)
    };

    // Both the preset pick and the HSV submit reuse the same
    // `EditChildSheet*Color` message so the engine command + undo/redo
    // round-trip is identical for both paths. `from_rgba8` reproduces
    // the exact `iced::Color` the old preset path emitted; the handler
    // re-quantises to `StrokeColor` regardless.
    let on_pick: Rc<dyn Fn([u8; 4]) -> PanelMsg + 'static> = Rc::new(move |rgba: [u8; 4]| {
        let color = iced::Color::from_rgba8(rgba[0], rgba[1], rgba[2], rgba[3] as f32 / 255.0);
        if is_border {
            PanelMsg::EditChildSheetBorderColor(sheet_id, color)
        } else {
            PanelMsg::EditChildSheetFillColor(sheet_id, color)
        }
    });

    // Reset-to-default only shows when an override is active — matches
    // the former `current.is_some()` gate.
    let on_clear = current
        .is_some()
        .then_some(PanelMsg::ResetChildSheetStyle(sheet_id));

    color_field(ColorFieldProps {
        label,
        current: current_rgba,
        none_label: "Default",
        show_palette: show_picker,
        show_advanced,
        muted,
        border_c,
        on_toggle,
        on_advanced: PanelMsg::OpenChildSheetAdvancedPicker(sheet_id, is_border),
        on_cancel: PanelMsg::CancelChildSheetColorPicker,
        on_pick,
        on_clear,
    })
}

/// Numeric stroke-width row for the child-sheet Style section.
fn child_sheet_stroke_width_row<'a>(
    sheet_id: uuid::Uuid,
    stored_value: f64,
    buffered: Option<String>,
    muted: Color,
    border_c: Color,
) -> Element<'a, PanelMsg> {
    let display = match buffered {
        Some(s) => s,
        None => format!("{:.4}", stored_value)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string(),
    };
    let display_for_input = display.clone();
    let input = iced::widget::text_input("0.1524", &display_for_input)
        .size(11)
        .padding(4)
        .width(120)
        .on_input(move |s| PanelMsg::ChildSheetStrokeWidthTyping(sheet_id, s))
        .on_submit(PanelMsg::CommitChildSheetStrokeWidth(sheet_id))
        .style(
            move |_theme: &Theme, _status| iced::widget::text_input::Style {
                background: Background::Color(Color::from_rgba(0.07, 0.07, 0.08, 1.0)),
                border: Border {
                    width: 1.0,
                    color: border_c,
                    radius: 2.0.into(),
                },
                icon: Color::from_rgba(0.7, 0.7, 0.7, 1.0),
                placeholder: Color::from_rgba(0.5, 0.5, 0.5, 1.0),
                value: Color::from_rgb(0.95, 0.95, 0.96),
                selection: Color::from_rgba(0.24, 0.62, 0.97, 0.4),
            },
        );

    container(
        row![
            text("Line Width (mm)").size(10).color(muted).width(96),
            input,
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .padding([4, 8])
    .width(Length::Fill)
    .into()
}
