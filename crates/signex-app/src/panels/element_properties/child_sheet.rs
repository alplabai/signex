//! Properties surface for a single hierarchical child sheet — read-only
//! identity/geometry plus the editable Border/Fill colour swatch rows
//! and stroke-width row. Moved verbatim from the former single-file
//! `element_properties` module.

use super::super::*;
use iced::widget::column;

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
    let preview_color = current
        .map(|c| {
            iced::Color::from_rgba(
                c.r as f32 / 255.0,
                c.g as f32 / 255.0,
                c.b as f32 / 255.0,
                c.a as f32 / 255.0,
            )
        })
        .unwrap_or(iced::Color::from_rgba(0.5, 0.5, 0.5, 0.4));
    let label_text = if let Some(c) = current {
        format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b)
    } else {
        "Default".to_string()
    };

    let toggle_msg = if is_border {
        PanelMsg::ToggleChildSheetBorderPicker(sheet_id)
    } else {
        PanelMsg::ToggleChildSheetFillPicker(sheet_id)
    };

    // Swatch button: 18x18 colour fill + small hex / "Default" caption.
    let swatch_color = preview_color;
    let swatch: Element<'a, PanelMsg> = container(Space::new())
        .width(18)
        .height(18)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(swatch_color)),
            border: Border {
                width: 1.0,
                color: border_c,
                radius: 2.0.into(),
            },
            ..container::Style::default()
        })
        .into();

    let swatch_button = iced::widget::button(
        row![
            swatch,
            text(label_text)
                .size(10)
                .color(Color::from_rgb(0.90, 0.90, 0.92)),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, 6])
    .on_press(toggle_msg)
    .style(iced::widget::button::secondary);

    // Build the per-channel "submit colour" message.
    let make_submit = move |c: iced::Color| -> PanelMsg {
        if is_border {
            PanelMsg::EditChildSheetBorderColor(sheet_id, c)
        } else {
            PanelMsg::EditChildSheetFillColor(sheet_id, c)
        }
    };

    // ── Advanced (HSV / RGB) overlay ──
    if show_advanced {
        let picker = iced_aw::ColorPicker::new(
            true,
            preview_color,
            swatch_button,
            PanelMsg::CancelChildSheetColorPicker,
            move |c| make_submit(c),
        );
        return container(
            row![
                text(label.to_string()).size(10).color(muted).width(96),
                picker,
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center),
        )
        .padding([4, 8])
        .width(Length::Fill)
        .into();
    }

    // The header row (label + swatch button) is always shown.
    let header = container(
        row![
            text(label.to_string()).size(10).color(muted).width(96),
            swatch_button,
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .padding([4, 8])
    .width(Length::Fill);

    if !show_picker {
        return header.into();
    }

    // ── Inline preset palette (rendered below the row, full width) ──
    let presets: [(&str, [u8; 3]); 12] = [
        ("Black", [0x00, 0x00, 0x00]),
        ("Dark Gray", [0x40, 0x40, 0x40]),
        ("Gray", [0x80, 0x80, 0x80]),
        ("White", [0xFF, 0xFF, 0xFF]),
        ("Red", [0xC0, 0x39, 0x2B]),
        ("Orange", [0xE6, 0x7E, 0x22]),
        ("Yellow", [0xF1, 0xC4, 0x0F]),
        ("Olive", [0xB4, 0xA5, 0x58]),
        ("Green", [0x27, 0xAE, 0x60]),
        ("Teal", [0x16, 0xA0, 0x85]),
        ("Blue", [0x29, 0x80, 0xB9]),
        ("Purple", [0x8E, 0x44, 0xAD]),
    ];

    // 6 columns × 2 rows of preset swatches; each cell stretches
    // proportionally so the grid always fills the available panel
    // width (no clipping in narrow docks).
    let mut palette_grid: Column<'a, PanelMsg> = Column::new().spacing(4);
    for chunk in presets.chunks(6) {
        let mut r: iced::widget::Row<'a, PanelMsg> = iced::widget::Row::new().spacing(4);
        for (_name, rgb) in chunk {
            let c = iced::Color::from_rgb(
                rgb[0] as f32 / 255.0,
                rgb[1] as f32 / 255.0,
                rgb[2] as f32 / 255.0,
            );
            let swatch_btn = iced::widget::button(Space::new())
                .width(Length::Fill)
                .height(22)
                .padding(0)
                .on_press(make_submit(c))
                .style(move |_t: &Theme, _s| iced::widget::button::Style {
                    background: Some(Background::Color(c)),
                    border: Border {
                        width: 1.0,
                        color: border_c,
                        radius: 2.0.into(),
                    },
                    ..iced::widget::button::Style::default()
                });
            r = r.push(swatch_btn);
        }
        palette_grid = palette_grid.push(r);
    }

    let mut palette_col: Column<'a, PanelMsg> =
        Column::new().spacing(6).padding([6, 8]).width(Length::Fill);
    palette_col = palette_col.push(text("Preset Colours").size(10).color(muted));
    palette_col = palette_col.push(palette_grid);

    let mut action_row: iced::widget::Row<'a, PanelMsg> =
        iced::widget::Row::new().spacing(4).width(Length::Fill);
    action_row = action_row.push(
        iced::widget::button(
            text("Custom…")
                .size(10)
                .color(Color::from_rgb(0.92, 0.92, 0.94)),
        )
        .padding([4, 10])
        .width(Length::Fill)
        .on_press(PanelMsg::OpenChildSheetAdvancedPicker(sheet_id, is_border))
        .style(iced::widget::button::secondary),
    );
    if current.is_some() {
        action_row = action_row.push(
            iced::widget::button(
                text("Reset to Default")
                    .size(10)
                    .color(Color::from_rgb(0.92, 0.92, 0.94)),
            )
            .padding([4, 10])
            .width(Length::Fill)
            .on_press(PanelMsg::ResetChildSheetStyle(sheet_id))
            .style(iced::widget::button::secondary),
        );
    }
    palette_col = palette_col.push(action_row);

    let palette_panel = container(palette_col)
        .width(Length::Fill)
        .style(move |_t: &Theme| container::Style {
            background: Some(Background::Color(Color::from_rgb(0.16, 0.16, 0.18))),
            border: Border {
                width: 1.0,
                color: border_c,
                radius: 4.0.into(),
            },
            ..container::Style::default()
        });

    column![header, container(palette_panel).padding([0, 8])]
        .spacing(4)
        .width(Length::Fill)
        .into()
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
