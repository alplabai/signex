//! Library Browser — empty-state CTA card.
//!
//! Centred call-to-action shown when the library has no tables yet.
//! Extracted verbatim from the former single-file `browser` module.

use super::*;
use iced::widget::column;

pub(super) fn view_empty_state<'a>(
    library_path: &'a std::path::Path,
    lib: &'a OpenLibrary,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);
    // Hoisted owned path — see comment in `view_table_sidebar`.
    let lib_pb: std::path::PathBuf = library_path.to_path_buf();

    let library_for_add = lib_pb.clone();
    let add_btn = button(
        text("Add Component")
            .size(BROWSER_TEXT_SIZE)
            .color(iced::Color::WHITE),
    )
    .padding([6, 14])
    .on_press(LibraryMessage::BrowserAddComponent {
        library_path: library_for_add,
        table: None,
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

    let card = container(
        column![
            text(format!("{}.snxlib", lib.display_name))
                .size(15)
                .color(text_c),
            Space::new().height(4),
            text("No categories — Add Component to begin")
                .size(12)
                .color(muted),
            Space::new().height(14),
            add_btn,
        ]
        .spacing(0)
        .align_x(iced::Alignment::Center),
    )
    .padding(28)
    .style(move |_: &Theme| iced::widget::container::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            1.0, 1.0, 1.0, 0.02,
        ))),
        border: Border {
            width: 1.0,
            radius: 6.0.into(),
            color: border,
        },
        ..Default::default()
    });

    container(card)
        .padding(32)
        .center(Length::Fill)
        .style(crate::styles::modal_card(tokens))
        .into()
}
