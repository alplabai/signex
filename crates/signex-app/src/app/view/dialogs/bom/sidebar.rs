//! BOM preview — the Properties sidebar (General / Columns tabs) plus the
//! grouping / format / toggle / variant / column control rows it consumes.
//! Extracted from `dialogs/bom.rs` (ADR-0001, issue #164) as pure code
//! motion — the child-push order inside every row/column is preserved
//! byte-for-byte, so the rendered sidebar is pixel-identical.

use super::super::*;
use iced::widget::{Column, Space, button, column, container, row, scrollable, text};
use iced::{Background, Border, Color, Element, Length, Theme};

use super::super::widgets::section_header;

impl Signex {
    /// Build the Properties sidebar column (tab strip + active pane) for the
    /// active preview. Returns the sidebar `column` the modal drops into its
    /// main row.
    pub(super) fn bom_sidebar(&self) -> Element<'_, Message> {
        use signex_output::{BomColumn, BomFormat, BomGrouping};
        let Some(ref preview) = self.document_state.bom_preview else {
            return container(Space::new()).into();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);
        // Pill style mirrors the main-app right-side Properties
        // panel: subtle theme-border at rest, theme-accent border
        // when active (orange on Altium Dark, cyan on Alp Lab,
        // etc.). Active fill is the same accent at 15 % alpha so
        // the pill reads as "selected" without being a hard solid
        // fill that fights the theme.
        let accent_c = crate::styles::ti(tokens.accent);
        let row_pill = |label: String, on: bool, msg: Message| -> Element<'_, Message> {
            button(text(label).size(11).color(text_c))
                .padding([4, 10])
                .on_press(msg)
                .style(move |_: &Theme, status: button::Status| {
                    let bg = match (on, status) {
                        (true, _) => Color {
                            a: 0.15,
                            ..accent_c
                        },
                        (false, button::Status::Hovered | button::Status::Pressed) => {
                            Color::from_rgba(1.0, 1.0, 1.0, 0.10)
                        }
                        _ => Color::from_rgba(1.0, 1.0, 1.0, 0.04),
                    };
                    let border_color = if on { accent_c } else { border_c };
                    button::Style {
                        background: Some(Background::Color(bg)),
                        border: Border {
                            width: 1.0,
                            radius: 3.0.into(),
                            color: border_color,
                        },
                        text_color: text_c,
                        ..button::Style::default()
                    }
                })
                .into()
        };

        let grouping_row: Element<'_, Message> = row![
            text("Grouping:").size(11).color(text_muted),
            Space::new().width(8),
            row_pill(
                "Grouped".to_string(),
                preview.options.grouping == BomGrouping::Grouped,
                Message::BomPreview(BomPreviewMsg::SetGrouping(BomGrouping::Grouped)),
            ),
            Space::new().width(4),
            row_pill(
                "Ungrouped".to_string(),
                preview.options.grouping == BomGrouping::Ungrouped,
                Message::BomPreview(BomPreviewMsg::SetGrouping(BomGrouping::Ungrouped)),
            ),
            Space::new().width(4),
            row_pill(
                "Flat".to_string(),
                preview.options.grouping == BomGrouping::Flat,
                Message::BomPreview(BomPreviewMsg::SetGrouping(BomGrouping::Flat)),
            ),
        ]
        .align_y(iced::Alignment::Center)
        .into();

        let format_row: Element<'_, Message> = row![
            text("Format:").size(11).color(text_muted),
            Space::new().width(8),
            row_pill(
                "CSV".to_string(),
                preview.options.format == BomFormat::Csv,
                Message::BomPreview(BomPreviewMsg::SetFormat(BomFormat::Csv)),
            ),
            Space::new().width(4),
            row_pill(
                "XLSX".to_string(),
                preview.options.format == BomFormat::Xlsx,
                Message::BomPreview(BomPreviewMsg::SetFormat(BomFormat::Xlsx)),
            ),
            Space::new().width(4),
            row_pill(
                "HTML".to_string(),
                preview.options.format == BomFormat::Html,
                Message::BomPreview(BomPreviewMsg::SetFormat(BomFormat::Html)),
            ),
        ]
        .align_y(iced::Alignment::Center)
        .into();

        let toggle_row: Element<'_, Message> = row![
            row_pill(
                "Include DNP".to_string(),
                preview.options.include_dnp,
                Message::BomPreview(BomPreviewMsg::SetIncludeDnp(!preview.options.include_dnp)),
            ),
            Space::new().width(4),
            row_pill(
                "Include Not Fitted".to_string(),
                preview.options.include_not_fitted,
                Message::BomPreview(BomPreviewMsg::SetIncludeNotFitted(
                    !preview.options.include_not_fitted
                )),
            ),
        ]
        .align_y(iced::Alignment::Center)
        .into();

        // Variant picker — only shown when the active project
        // declares variants. "Base" is the no-override view.
        let variant_row: Element<'_, Message> = if preview.variants.is_empty() {
            Space::new().into()
        } else {
            let active_label = preview
                .options
                .active_variant
                .clone()
                .unwrap_or_else(|| "Base".to_string());
            let mut r = row![
                text("Variant:").size(11).color(text_muted),
                Space::new().width(8),
                row_pill(
                    "Base".to_string(),
                    preview.options.active_variant.is_none(),
                    Message::BomPreview(BomPreviewMsg::SetVariant(None)),
                ),
            ]
            .align_y(iced::Alignment::Center);
            for variant in &preview.variants {
                let is_active = active_label.as_str() == variant.as_str();
                r = r.push(Space::new().width(4));
                r = r.push(row_pill(
                    variant.clone(),
                    is_active,
                    Message::BomPreview(BomPreviewMsg::SetVariant(Some(variant.clone()))),
                ));
            }
            r.into()
        };
        // Column picker — toggles for the standard column set + any
        // custom-field column discovered in the rolled-up rows. The
        // pill state mirrors `preview.options.columns`; clicking
        // adds/removes from that Vec via `handle_bom_preview_toggle_column`.
        let mut custom_keys: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for r in &preview.table.rows {
            for k in r.custom.keys() {
                custom_keys.insert(k.clone());
            }
        }
        let column_options: Vec<(BomColumn, &'static str)> = vec![
            (BomColumn::Name, "Name"),
            (BomColumn::Description, "Description"),
            (BomColumn::Designator, "Designator"),
            (BomColumn::Value, "Value"),
            (BomColumn::Footprint, "Footprint"),
            (BomColumn::LibRef, "LibRef"),
            (BomColumn::Qty, "Qty"),
        ];
        let mut column_row = row![
            text("Columns:").size(11).color(text_muted),
            Space::new().width(8)
        ]
        .align_y(iced::Alignment::Center);
        for (col, label) in &column_options {
            let on = preview.options.columns.iter().any(|c| c == col);
            column_row = column_row.push(row_pill(
                label.to_string(),
                on,
                Message::BomPreview(BomPreviewMsg::ToggleColumn(col.clone())),
            ));
            column_row = column_row.push(Space::new().width(4));
        }
        for key in &custom_keys {
            let col = BomColumn::Custom(key.clone());
            let on = preview.options.columns.iter().any(|c| c == &col);
            column_row = column_row.push(row_pill(
                key.clone(),
                on,
                Message::BomPreview(BomPreviewMsg::ToggleColumn(col)),
            ));
            column_row = column_row.push(Space::new().width(4));
        }
        let column_row: Element<'_, Message> = scrollable(column_row)
            .direction(iced::widget::scrollable::Direction::Horizontal(
                iced::widget::scrollable::Scrollbar::new()
                    .width(0)
                    .scroller_width(0),
            ))
            .into();
        // Properties sidebar — General / Columns tabs, collapsible
        // sections inside.
        use crate::app::state::BomSidebarTab;
        let sidebar_tab = preview.sidebar_tab;
        let tab_pill = |label: &'static str, target: BomSidebarTab| -> Element<'_, Message> {
            let on = sidebar_tab == target;
            button(text(label.to_string()).size(11).color(text_c))
                .padding([4, 12])
                .on_press(Message::BomPreview(BomPreviewMsg::SetSidebarTab(target)))
                .style(move |_: &iced::Theme, status: button::Status| {
                    let bg = match (on, status) {
                        (true, _) => iced::Color {
                            a: 0.15,
                            ..accent_c
                        },
                        (false, button::Status::Hovered) => {
                            iced::Color::from_rgba(1.0, 1.0, 1.0, 0.06)
                        }
                        _ => iced::Color::TRANSPARENT,
                    };
                    let border_color = if on { accent_c } else { border_c };
                    button::Style {
                        background: Some(iced::Background::Color(bg)),
                        border: iced::Border {
                            width: 1.0,
                            radius: 3.0.into(),
                            color: border_color,
                        },
                        text_color: text_c,
                        ..button::Style::default()
                    }
                })
                .into()
        };

        let general_pane: Element<'_, Message> = column![
            section_header("BOM Items", text_muted),
            container(
                column![
                    grouping_row,
                    Space::new().height(6),
                    toggle_row,
                    Space::new().height(6),
                    variant_row,
                ]
                .spacing(0),
            )
            .padding([8, 12]),
            Space::new().height(8),
            section_header("Export Options", text_muted),
            container(column![format_row].spacing(0),).padding([8, 12]),
        ]
        .spacing(0)
        .into();

        // Columns tab — vertical list with a tick checkbox per row,
        // mirroring Altium's Columns table. Same standard column
        // set + any custom-field column discovered in the rolled
        // rows. Each row is clickable and toggles via
        // `Message::BomPreview(BomPreviewMsg::ToggleColumn)` — the existing handler
        // already does add/remove on the Vec.
        let _ = column_row; // pill row obsoleted by the list layout
        let mut col_list: Column<'_, Message> = Column::new().spacing(0);
        col_list = col_list.push(section_header("Columns", text_muted));
        let mut list_items: Column<'_, Message> = Column::new().spacing(0);
        let render_col_row = |col: BomColumn, label: String, on: bool| -> Element<'_, Message> {
            let pip_bg = if on {
                Color::from_rgb(0.00, 0.47, 0.84)
            } else {
                Color::from_rgba(1.0, 1.0, 1.0, 0.04)
            };
            let inner = if on {
                text("\u{2713}").size(9).color(Color::WHITE)
            } else {
                text(" ").size(9)
            };
            let pip = container(inner)
                .width(14)
                .height(14)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(pip_bg)),
                    border: Border {
                        width: 1.0,
                        radius: 2.0.into(),
                        color: border_c,
                    },
                    ..container::Style::default()
                });
            button(
                row![
                    pip,
                    Space::new().width(8),
                    text(label).size(11).color(text_c),
                    Space::new().width(Length::Fill),
                ]
                .align_y(iced::Alignment::Center),
            )
            .on_press(Message::BomPreview(BomPreviewMsg::ToggleColumn(col)))
            .padding([3, 12])
            .width(Length::Fill)
            .style(move |_: &Theme, status: button::Status| {
                let bg = match status {
                    button::Status::Hovered | button::Status::Pressed => {
                        Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.04)))
                    }
                    _ => None,
                };
                button::Style {
                    background: bg,
                    border: Border::default(),
                    text_color: text_c,
                    ..button::Style::default()
                }
            })
            .into()
        };
        for (col, label) in &column_options {
            let on = preview.options.columns.iter().any(|c| c == col);
            list_items = list_items.push(render_col_row(col.clone(), label.to_string(), on));
        }
        for key in &custom_keys {
            let col = BomColumn::Custom(key.clone());
            let on = preview.options.columns.iter().any(|c| c == &col);
            list_items = list_items.push(render_col_row(col, key.clone(), on));
        }
        col_list = col_list.push(container(list_items).padding([4, 0]));
        let columns_pane: Element<'_, Message> = col_list.into();

        let sidebar_body: Element<'_, Message> = match sidebar_tab {
            BomSidebarTab::General => general_pane,
            BomSidebarTab::Columns => columns_pane,
        };
        let sidebar = column![
            container(text("Properties").size(13).color(text_c))
                .padding([10, 12])
                .width(Length::Fill),
            container(
                row![
                    tab_pill("General", BomSidebarTab::General),
                    Space::new().width(4),
                    tab_pill("Columns", BomSidebarTab::Columns),
                ]
                .align_y(iced::Alignment::Center),
            )
            .padding([0, 12]),
            Space::new().height(8),
            scrollable(sidebar_body)
                .width(Length::Fill)
                .height(Length::Fill),
        ]
        .spacing(0);
        sidebar.into()
    }
}
