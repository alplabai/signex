//! Library Browser — header toolbar (search / lifecycle filter).
//!
//! The tab-bar strip above the grid: the lifecycle-filter pick-list and the
//! search box.

use super::*;
use iced::widget::column;

/// Default type / spacing scales; equal to the old literals at the default size.
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

pub(super) fn view_header<'a>(
    library_path: &'a std::path::Path,
    lib: &'a OpenLibrary,
    browser: &'a LibraryBrowserState,
    tokens: &'a ThemeTokens,
    read_only: bool,
) -> Element<'a, LibraryMessage> {
    graphite_header(library_path, lib, browser, tokens, read_only)
}

/// Graphite header: library title, a flexible search field and the primary
/// Add Component button, then the lifecycle filter as pills.
fn graphite_header<'a>(
    library_path: &'a std::path::Path,
    lib: &'a OpenLibrary,
    browser: &'a LibraryBrowserState,
    tokens: &'a ThemeTokens,
    read_only: bool,
) -> Element<'a, LibraryMessage> {
    let r = crate::styles::tokens::resolve(tokens);
    let radii = crate::styles::Radii::GRAPHITE;
    let metrics = crate::styles::ChromeMetrics::DEFAULT;
    let lib_pb: std::path::PathBuf = library_path.to_path_buf();

    let library_for_search = lib_pb.clone();
    let search = text_input(
        raw("Search Internal PN, MPN, value, parameter"),
        &browser.search,
    )
    .on_input(move |s| LibraryMessage::BrowserSearchChanged {
        library_path: library_for_search.clone(),
        value: s,
    })
    .padding(SP.s)
    .size(BROWSER_TEXT_SIZE)
    .style(super::look::bare_input(tokens))
    .width(Length::Fill);
    let search_field = container(search)
        .width(Length::Fill)
        .height(Length::Fixed(metrics.button))
        .center_y(Length::Fixed(metrics.button))
        .style(crate::styles::field(tokens, false));

    let add = button(text(raw("Add Component")).size(BROWSER_TEXT_SIZE))
        .height(Length::Fixed(metrics.button))
        .padding([SP.xs, SP.l])
        .on_press_maybe((!read_only).then(|| LibraryMessage::BrowserAddComponent {
            library_path: lib_pb.clone(),
            table: browser.active_table.clone(),
        }))
        .style(crate::styles::primary_button(tokens));

    let title = text(raw(format!("{}.snxlib", lib.display_name)))
        .size(TS.heading)
        .color(theme_ext::text_primary(tokens));

    // Real counts only: components across every table, and the table count.
    let total: usize = lib.tables.values().map(Vec::len).sum();
    let caption = text(raw(format!(
        "{total} components · {} tables",
        lib.tables.len()
    )))
    .size(TS.caption)
    .color(r.ink3);
    let title = column![title.boxed(), caption.boxed()].spacing(SP.xs);

    let top: iced::widget::Row<Element<'a, LibraryMessage>> =
        row![title.boxed(), search_field.boxed(), add.boxed()]
            .spacing(SP.l)
            .align_y(iced::Alignment::Center);

    let hover = crate::styles::tokens::overlay(tokens, crate::styles::tokens::Overlay::Hover);
    let (line, ink3) = (r.line_soft, r.ink3);
    let (accent_fill, accent_ink) = (r.accent_soft, r.accent_soft_ink);
    let text_c = theme_ext::text_primary(tokens);
    let mut pills: iced::widget::Row<Element<'a, LibraryMessage>> =
        row![text(raw("Show")).size(TS.caption).color(ink3).boxed()]
            .spacing(SP.s)
            .align_y(iced::Alignment::Center);
    for &filter in LifecycleFilter::ALL {
        let active = browser.lifecycle_filter == filter;
        let library_for_lc = lib_pb.clone();
        pills = pills.push(
            button(text(raw(filter.label())).size(TS.body))
                .padding([SP.xs, SP.m])
                .on_press(LibraryMessage::BrowserSetLifecycleFilter {
                    library_path: library_for_lc,
                    filter,
                })
                .style(move |_: &Theme, status: iced::widget::button::Status| {
                    let hovered = matches!(status, iced::widget::button::Status::Hovered);
                    iced::widget::button::Style {
                        background: (active || hovered).then_some(iced::Background::Color(
                            if active { accent_fill } else { hover },
                        )),
                        text_color: if active { accent_ink } else { text_c },
                        border: Border {
                            width: 1.0,
                            color: line,
                            radius: radii.pill.into(),
                            ..Border::default()
                        },
                        ..iced::widget::button::Style::default()
                    }
                })
                .boxed(),
        );
    }

    container(column![top.boxed(), pills.boxed()].spacing(SP.s))
        .padding([SP.m, SP.l])
        .width(Length::Fill)
        .style(crate::styles::tab_bar_strip(tokens))
        .boxed()
}
