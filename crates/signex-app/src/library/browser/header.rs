//! Library Browser — header toolbar (add / search / lifecycle filter).
//!
//! The tab-bar strip above the grid: the `+ Component` button, the
//! lifecycle-filter pick-list and the search box. Extracted verbatim
//! from the former single-file `browser` module.

use super::*;

pub(super) fn view_header<'a>(
    library_path: &'a std::path::Path,
    _lib: &'a OpenLibrary,
    browser: &'a LibraryBrowserState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    // Hoisted owned path — see comment in `view_table_sidebar`.
    let lib_pb: std::path::PathBuf = library_path.to_path_buf();

    // The "+" button — opens the New Component modal pre-selected to
    // this library + active table.
    let library_for_plus = lib_pb.clone();
    let table_for_plus = browser.active_table.clone();
    let plus_btn = button(text("+ Component").size(BROWSER_TEXT_SIZE).color(text_c))
        .padding([4, 10])
        .on_press(LibraryMessage::BrowserAddComponent {
            library_path: library_for_plus,
            table: table_for_plus,
        })
        .style(|_: &Theme, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.04,
            ))),
            text_color: iced::Color::WHITE,
            border: Border {
                width: 0.0,
                radius: 3.0.into(),
                color: iced::Color::TRANSPARENT,
            },
            ..iced::widget::button::Style::default()
        });

    // Add-table control. Either the inline name form (while
    let library_for_search = lib_pb.clone();
    let search = text_input("Search…", &browser.search)
        .on_input(move |s| LibraryMessage::BrowserSearchChanged {
            library_path: library_for_search.clone(),
            value: s,
        })
        .padding(4)
        .size(BROWSER_TEXT_SIZE)
        .width(Length::Fixed(220.0));

    // Lifecycle filter pill — Stage 18 surfaces `ComponentRow.state`
    // as a first-class browser filter so users can pivot between
    // "preferred only", "include deprecated", etc. without touching
    // every row's lifecycle field.
    let library_for_lc = lib_pb.clone();
    let lifecycle_picker = pick_list(
        LifecycleFilter::ALL.to_vec(),
        Some(browser.lifecycle_filter),
        move |f| LibraryMessage::BrowserSetLifecycleFilter {
            library_path: library_for_lc.clone(),
            filter: f,
        },
    )
    .placeholder("Lifecycle")
    .padding(4)
    .text_size(BROWSER_TEXT_SIZE);

    container(
        row![
            plus_btn,
            Space::new().width(Length::Fill),
            lifecycle_picker,
            Space::new().width(8),
            search,
        ]
        .spacing(0)
        .align_y(iced::Alignment::Center),
    )
    .padding([6, 10])
    .style(crate::styles::tab_bar_strip(tokens))
    .into()
}
