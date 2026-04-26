//! Supply tab — `SupplierLink` rows + paste-URL field +
//! "Refresh from API" stub.

use iced::widget::{Space, button, column, container, row, scrollable, text, text_input};
use iced::{Border, Element, Length, Theme};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::super::messages::{EditorMsg, LibraryMessage};
use super::super::state::ComponentEditorState;

pub fn view<'a>(
    editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
    window_id: iced::window::Id,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    // Top paste-URL field + Refresh button.
    let paste_url = text_input(
        "Paste a DigiKey / Mouser / LCSC / JLCPCB URL…",
        // Phase 1 doesn't store the paste buffer separately — Phase
        // 2 wires it into a dedicated SupplyTabState. The inline
        // box above just demonstrates the field.
        "",
    )
    .padding([4, 8])
    .size(12)
    .on_input(move |s| LibraryMessage::EditorEvent {
        window_id,
        msg: EditorMsg::SupplyPasteUrlChanged(s),
    });
    let refresh_btn =
        button(container(text("Refresh from API").size(11).color(text_c)).padding([4, 12]))
            .on_press(LibraryMessage::EditorEvent {
                window_id,
                msg: EditorMsg::SupplyRefreshFromApi,
            })
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, 0.04,
                ))),
                text_color: text_c,
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border,
                },
                ..iced::widget::button::Style::default()
            });

    let top_row = row![
        container(paste_url).width(Length::Fill),
        Space::new().width(8),
        refresh_btn,
    ]
    .align_y(iced::Alignment::Center);

    // Header row above the supplier list.
    let list_header = row![
        text("Distributor")
            .size(10)
            .color(muted)
            .width(Length::FillPortion(2)),
        Space::new().width(8),
        text("SKU")
            .size(10)
            .color(muted)
            .width(Length::FillPortion(3)),
        Space::new().width(8),
        text("URL")
            .size(10)
            .color(muted)
            .width(Length::FillPortion(4)),
        Space::new().width(8),
        text("").size(10).width(Length::Fixed(80.0)),
    ]
    .padding([4, 4]);

    let mut rows = column![list_header].spacing(2);
    for (idx, link) in editor.draft.shared.suppliers.iter().enumerate() {
        let row_idx = idx;
        let dist_input = text_input("DigiKey", &link.distributor)
            .padding([4, 8])
            .size(11)
            .on_input(move |s| LibraryMessage::EditorEvent {
                window_id,
                msg: EditorMsg::SupplySetDistributor {
                    idx: row_idx,
                    value: s,
                },
            });
        let sku_input = text_input("311-10.0KCRCT-ND", &link.sku)
            .padding([4, 8])
            .size(11)
            .on_input(move |s| LibraryMessage::EditorEvent {
                window_id,
                msg: EditorMsg::SupplySetSku {
                    idx: row_idx,
                    value: s,
                },
            });
        let url_text = link.url.clone().unwrap_or_default();
        let url_input = text_input("https://", &url_text)
            .padding([4, 8])
            .size(11)
            .on_input(move |s| LibraryMessage::EditorEvent {
                window_id,
                msg: EditorMsg::SupplySetUrl {
                    idx: row_idx,
                    value: s,
                },
            });
        let remove_btn =
            button(container(text("\u{2212} Remove").size(10).color(text_c)).padding([3, 8]))
                .on_press(LibraryMessage::EditorEvent {
                    window_id,
                    msg: EditorMsg::SupplyRemoveRow(row_idx),
                })
                .style(move |_: &Theme, _| iced::widget::button::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgba(
                        1.0, 1.0, 1.0, 0.04,
                    ))),
                    text_color: text_c,
                    border: Border {
                        width: 1.0,
                        radius: 3.0.into(),
                        color: border,
                    },
                    ..iced::widget::button::Style::default()
                });
        let row_widget = row![
            container(dist_input).width(Length::FillPortion(2)),
            Space::new().width(8),
            container(sku_input).width(Length::FillPortion(3)),
            Space::new().width(8),
            container(url_input).width(Length::FillPortion(4)),
            Space::new().width(8),
            container(remove_btn).width(Length::Fixed(80.0)),
        ]
        .padding([0, 4])
        .align_y(iced::Alignment::Center);
        rows = rows.push(row_widget);
    }
    if editor.draft.shared.suppliers.is_empty() {
        rows = rows.push(
            container(
                text("No supplier links yet — paste a URL or Add Supplier below.")
                    .size(11)
                    .color(muted),
            )
            .padding([8, 4]),
        );
    }

    let add_btn = button(container(text("+ Add Supplier").size(11).color(text_c)).padding([4, 12]))
        .on_press(LibraryMessage::EditorEvent {
            window_id,
            msg: EditorMsg::SupplyAddRow,
        })
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.04,
            ))),
            text_color: text_c,
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
            },
            ..iced::widget::button::Style::default()
        });

    container(
        column![
            top_row,
            Space::new().height(10),
            scrollable(rows).height(Length::Fill).width(Length::Fill),
            Space::new().height(8),
            row![add_btn],
        ]
        .spacing(0)
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .style(crate::styles::modal_card(tokens))
    .padding(14)
    .into()
}
