//! Components panel view.

use super::*;
use iced::widget::column;

// ─── Components Panel (matched to Altium Designer) ───────────

pub fn view_components<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let muted = theme_ext::text_secondary(&ctx.tokens);
    let primary = theme_ext::text_primary(&ctx.tokens);
    let border_c = theme_ext::border_color(&ctx.tokens);
    let hover_c = crate::styles::ti(ctx.tokens.hover);
    let panel_bg_c = crate::styles::ti(ctx.tokens.panel_bg);
    let input_bg = crate::styles::ti(ctx.tokens.selection);
    let input_bdr = crate::styles::ti(ctx.tokens.accent);

    // ── TOP: Library selector + component list (scrollable) ──
    let mut list_col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);

    list_col = list_col.push(
        container(
            iced::widget::pick_list(
                ctx.standard_libraries.clone(),
                ctx.active_library.clone(),
                PanelMsg::SelectLibrary,
            )
            .placeholder("Select a library...")
            .text_size(11)
            .width(Length::Fill),
        )
        .padding([4, 8]),
    );

    // Search filter input
    list_col = list_col.push(
        container(
            iced::widget::text_input("Search components...", &ctx.component_filter)
                .on_input(PanelMsg::ComponentFilter)
                .size(11)
                .width(Length::Fill),
        )
        .padding([4, 8]),
    );

    list_col = list_col.push(thin_sep(border_c));
    list_col = list_col.push(
        container(
            row![
                text("Name")
                    .size(10)
                    .color(muted)
                    .width(Length::FillPortion(3)),
                text("Library")
                    .size(10)
                    .color(muted)
                    .width(Length::FillPortion(2)),
                text("Pins")
                    .size(10)
                    .color(muted)
                    .width(Length::FillPortion(1)),
            ]
            .spacing(4.0),
        )
        .padding([4, 8])
        .width(Length::Fill),
    );
    list_col = list_col.push(thin_sep(border_c));

    // Filter the symbol list
    let filter = ctx.component_filter.to_ascii_lowercase();
    let filtered_symbols: Vec<&LibrarySymbolEntry> = if filter.is_empty() {
        ctx.library_symbols.iter().collect()
    } else {
        ctx.library_symbols
            .iter()
            .filter(|entry| {
                entry.symbol_name.to_ascii_lowercase().contains(&filter)
                    || entry.library_name.to_ascii_lowercase().contains(&filter)
                    || entry.lib_id.to_ascii_lowercase().contains(&filter)
            })
            .collect()
    };

    if filtered_symbols.is_empty() {
        let msg = if ctx.active_library.is_some() {
            if filter.is_empty() {
                "Loading..."
            } else {
                "No matches"
            }
        } else {
            "Select a library above"
        };
        list_col = list_col.push(container(text(msg).size(10).color(muted)).padding([8, 8]));
    } else {
        let sel = &ctx.selected_component;
        for entry in &filtered_symbols {
            let is_sel = sel.as_deref() == Some(entry.lib_id.as_str());
            let row_bg = if is_sel {
                theme_ext::selection_color(&ctx.tokens)
            } else {
                Color::TRANSPARENT
            };
            let name_c = if is_sel { Color::WHITE } else { primary };
            let lib_id = entry.lib_id.clone();
            list_col = list_col.push(
                column![
                    iced::widget::button(
                        row![
                            text(entry.symbol_name.clone())
                                .size(10)
                                .color(name_c)
                                .width(Length::FillPortion(3))
                                .wrapping(iced::widget::text::Wrapping::None),
                            text(entry.library_name.clone())
                                .size(10)
                                .color(muted)
                                .width(Length::FillPortion(2))
                                .wrapping(iced::widget::text::Wrapping::None),
                            text(entry.pin_count.to_string())
                                .size(10)
                                .color(muted)
                                .width(Length::FillPortion(1)),
                        ]
                        .spacing(4.0),
                    )
                    .padding([3, 8])
                    .width(Length::Fill)
                    .on_press(PanelMsg::SelectComponent(lib_id))
                    .style(
                        move |_: &Theme, status: iced::widget::button::Status| {
                            let bg = match (is_sel, status) {
                                (true, _) => Some(Background::Color(row_bg)),
                                (false, iced::widget::button::Status::Hovered) => {
                                    Some(Background::Color(hover_c))
                                }
                                _ => None,
                            };
                            iced::widget::button::Style {
                                background: bg,
                                border: Border::default(),
                                ..iced::widget::button::Style::default()
                            }
                        }
                    ),
                    thin_sep(border_c),
                ]
                .spacing(0),
            );
        }
    }

    list_col = list_col.push(
        container(
            text(format!("Results: {}", filtered_symbols.len()))
                .size(10)
                .color(muted),
        )
        .padding([4, 8]),
    );

    // ── BOTTOM: Details panel (scrollable) ──
    let mut detail_col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);

    if let Some(comp_id) = &ctx.selected_component {
        let selected_entry = ctx
            .library_symbols
            .iter()
            .find(|entry| &entry.lib_id == comp_id);
        let comp_name = selected_entry
            .map(|entry| entry.symbol_name.as_str())
            .unwrap_or(comp_id.as_str());
        detail_col = detail_col.push(section_hdr(
            &format!("\u{25BC} Details  {comp_name}"),
            primary,
            border_c,
        ));
        let pin_count = ctx
            .library_symbols
            .iter()
            .find(|entry| entry.lib_id == *comp_id)
            .map(|entry| entry.pin_count)
            .unwrap_or(0);
        detail_col = detail_col.push(form_input_row(
            "Symbol", comp_name, muted, input_bg, input_bdr,
        ));
        detail_col = detail_col.push(form_input_row(
            "Pins",
            &pin_count.to_string(),
            muted,
            input_bg,
            input_bdr,
        ));
        detail_col = detail_col.push(form_input_row(
            "Library",
            selected_entry
                .map(|entry| entry.library_name.as_str())
                .or(ctx.active_library.as_deref())
                .unwrap_or(""),
            muted,
            input_bg,
            input_bdr,
        ));

        // Symbol preview canvas
        detail_col = detail_col.push(Space::new().height(4.0));
        detail_col = detail_col.push(section_hdr("\u{25BC} Models", primary, border_c));
        if let Some(lib_sym) = &ctx.selected_lib_symbol {
            // Symbol preview
            detail_col = detail_col.push(
                container(
                    container(
                        signex_widgets::symbol_preview::symbol_preview(lib_sym.clone(), 120.0)
                            .map(|_: ()| PanelMsg::ToggleGrid),
                    )
                    .width(Length::Fill)
                    .style(move |_: &Theme| container::Style {
                        background: Some(Background::Color(panel_bg_c)),
                        border: Border {
                            width: 1.0,
                            radius: 2.0.into(),
                            color: border_c,
                        },
                        ..container::Style::default()
                    }),
                )
                .padding([4, 8]),
            );

            // Footprint preview placeholder
            detail_col = detail_col.push(
                container(
                    container(
                        text("Footprint preview")
                            .size(10)
                            .color(muted)
                            .align_x(iced::alignment::Horizontal::Center),
                    )
                    .width(Length::Fill)
                    .padding([30, 8])
                    .style(move |_: &Theme| container::Style {
                        background: Some(Background::Color(panel_bg_c)),
                        border: Border {
                            width: 1.0,
                            radius: 2.0.into(),
                            color: border_c,
                        },
                        ..container::Style::default()
                    }),
                )
                .padding([4, 8]),
            );
        }

        for section in &["References", "Part Choices", "Where Used"] {
            detail_col = detail_col.push(Space::new().height(2.0));
            detail_col = detail_col.push(section_hdr(
                &format!("\u{25BC} {section}"),
                primary,
                border_c,
            ));
        }
    } else {
        detail_col = detail_col.push(
            container(text("Select a component").size(10).color(muted))
                .padding([12, 8])
                .width(Length::Fill),
        );
    }

    // Split view: list (fixed height) | handle | details (fill)
    column![
        container(scrollable(list_col).width(Length::Fill))
            .height(ctx.components_split)
            .width(Length::Fill),
        // Drag handle
        iced::widget::mouse_area(
            container(Space::new())
                .height(5.0)
                .width(Length::Fill)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(border_c)),
                    ..container::Style::default()
                }),
        )
        .interaction(iced::mouse::Interaction::ResizingVertically)
        .on_press(PanelMsg::DragComponentsSplit),
        container(scrollable(detail_col).width(Length::Fill))
            .height(Length::Fill)
            .width(Length::Fill),
    ]
    .spacing(0)
    .into()
}
