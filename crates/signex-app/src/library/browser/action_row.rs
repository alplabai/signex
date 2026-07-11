//! Library Browser — the grid action row (Add / Delete Selected).
//!
//! Extracted verbatim from the former single-file `browser` module.

use super::*;

pub(super) fn view_action_row<'a>(
    library_path: &'a std::path::Path,
    table: &str,
    selected: Option<RowId>,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);
    // Hoisted owned path — see comment in `view_table_sidebar`.
    let lib_pb: std::path::PathBuf = library_path.to_path_buf();

    let library_for_add = lib_pb.clone();
    let table_for_add = Some(table.to_string());
    let add_btn = button(
        text("Add Component")
            .size(BROWSER_TEXT_SIZE)
            .color(iced::Color::WHITE),
    )
    .padding([4, 12])
    .on_press(LibraryMessage::BrowserAddComponent {
        library_path: library_for_add,
        table: table_for_add,
    })
    .style(|_: &Theme, _| iced::widget::button::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgb(
            0.00, 0.47, 0.84,
        ))),
        text_color: iced::Color::WHITE,
        border: Border {
            width: 0.0,
            radius: 3.0.into(),
            color: iced::Color::TRANSPARENT,
        },
        ..iced::widget::button::Style::default()
    });

    let delete_btn: Element<'a, LibraryMessage> = if let Some(row_id) = selected {
        let library_for_del = lib_pb.clone();
        let table_for_del = table.to_string();
        button(
            text("Delete Selected")
                .size(BROWSER_TEXT_SIZE)
                .color(text_c),
        )
        .padding([4, 12])
        .on_press(LibraryMessage::BrowserDeleteRowRequest {
            library_path: library_for_del,
            table: table_for_del,
            row_id,
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
        })
        .into()
    } else {
        // Greyed-out placeholder when no row is selected.
        container(text("Delete Selected").size(BROWSER_TEXT_SIZE).color(muted))
            .padding([4, 12])
            .style(move |_: &Theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, 0.02,
                ))),
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border,
                },
                ..Default::default()
            })
            .into()
    };

    container(
        row![add_btn, Space::new().width(8), delete_btn]
            .spacing(0)
            .align_y(iced::Alignment::Center),
    )
    .padding([8, 4])
    .into()
}
