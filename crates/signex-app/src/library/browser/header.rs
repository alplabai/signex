//! Library Browser — header toolbar (search / lifecycle filter).
//!
//! The tab-bar strip above the grid: the lifecycle-filter pick-list and the
//! search box.

use super::*;

/// Default type / spacing scales; equal to the old literals at the default size.
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

pub(super) fn view_header<'a>(
    library_path: &'a std::path::Path,
    _lib: &'a OpenLibrary,
    browser: &'a LibraryBrowserState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    // Hoisted owned path — see comment in `view_table_sidebar`.
    let lib_pb: std::path::PathBuf = library_path.to_path_buf();

    let library_for_search = lib_pb.clone();
    let search = text_input("Search…", &browser.search)
        .on_input(move |s| LibraryMessage::BrowserSearchChanged {
            library_path: library_for_search.clone(),
            value: s,
        })
        .padding(SP.s)
        .size(BROWSER_TEXT_SIZE)
        .width(Length::Fixed(220.0));

    // Lifecycle filter pill — Stage 18 surfaces `ComponentRow.state`
    // as a first-class browser filter so users can pivot between
    // "preferred only", "include deprecated", etc. without touching
    // every row's lifecycle field.
    let library_for_lc = lib_pb.clone();
    let lifecycle_picker = pick_list(
        Some(browser.lifecycle_filter),
        LifecycleFilter::ALL,
        |f: &LifecycleFilter| f.to_string(),
    )
    .on_select(
        move |f: LifecycleFilter| LibraryMessage::BrowserSetLifecycleFilter {
            library_path: library_for_lc.clone(),
            filter: f,
        },
    )
    .placeholder("Lifecycle")
    .padding(SP.s)
    .text_size(BROWSER_TEXT_SIZE);

    container(
        row![
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
    .boxed()
}
