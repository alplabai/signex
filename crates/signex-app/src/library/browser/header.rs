//! Library Browser — header toolbar (search / lifecycle filter).
//!
//! The tab-bar strip above the grid: the lifecycle-filter pick-list and the
//! search box.

use super::*;
use iced::widget::column;

/// Default type / spacing scales; equal to the old literals at the default size.
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// What the count line right of the filter pills reads: the rows the search
/// and filters leave, of all rows in the active table, and the sort column.
pub(super) struct ShownCount {
    pub(super) shown: usize,
    pub(super) total: usize,
    pub(super) sorted_by: Option<String>,
}

impl ShownCount {
    /// "14 of 87 shown", with " · sorted by Internal PN" when sorted.
    pub(super) fn label(&self, i18n: &I18n) -> signex_i18n::Localized {
        match &self.sorted_by {
            Some(column) => t!(
                i18n,
                ids::LIBRARY_BROWSER_SHOWN_SORTED,
                "shown" => self.shown,
                "total" => self.total,
                "column" => column.as_str()
            ),
            None => t!(
                i18n,
                ids::LIBRARY_BROWSER_SHOWN,
                "shown" => self.shown,
                "total" => self.total
            ),
        }
    }
}

/// Graphite header: library title, a flexible search field and the primary
/// Add Component button, then the lifecycle filter as pills and the count.
pub(super) fn view_header<'a>(
    library_path: &'a std::path::Path,
    lib: &'a OpenLibrary,
    browser: &'a LibraryBrowserState,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    read_only: bool,
    count: &ShownCount,
) -> Element<'a, LibraryMessage> {
    let r = crate::styles::tokens::resolve(tokens);
    let radii = crate::styles::Radii::GRAPHITE;
    let metrics = crate::styles::ChromeMetrics::DEFAULT;
    let lib_pb: std::path::PathBuf = library_path.to_path_buf();

    let library_for_search = lib_pb.clone();
    let search = text_input(t!(i18n, ids::LIBRARY_BROWSER_SEARCH), &browser.search)
        .on_input(move |s| LibraryMessage::BrowserSearchChanged {
            library_path: library_for_search.clone(),
            value: s,
        })
        .padding(SP.s)
        .size(BROWSER_TEXT_SIZE)
        .style(super::look::bare_input(tokens))
        .width(Length::Fill);
    let search_ink = r.ink4;
    let search_icon = iced::widget::svg(crate::icons::icon_chrome_search(
        signex_types::theme::ThemeId::GraphiteDark,
    ))
    .width(BROWSER_TEXT_SIZE)
    .height(BROWSER_TEXT_SIZE)
    .style(move |_: &Theme, _| iced::widget::svg::Style {
        color: Some(search_ink),
        ..Default::default()
    });
    let search_line = row![search_icon, search]
        .align_y(iced::Alignment::Center)
        .padding(iced::Padding {
            left: SP.m,
            ..iced::Padding::ZERO
        });
    let search_field = container(search_line)
        .width(Length::Fill)
        .height(Length::Fixed(metrics.button))
        .center_y(Length::Fixed(metrics.button))
        .style(crate::styles::field(tokens, false));

    // The label fills the fixed-height button and centres itself; a bare
    // text sits at the top of the button's content box.
    let add_label = text(t!(i18n, ids::LIBRARY_BROWSER_ADD_COMPONENT))
        .size(BROWSER_TEXT_SIZE)
        .height(Length::Fill)
        .align_y(iced::alignment::Vertical::Center);
    let add = button(add_label)
        .height(Length::Fixed(metrics.button))
        .padding([SP.xs, SP.l])
        .on_press_maybe((!read_only).then(|| LibraryMessage::BrowserAddComponent {
            library_path: lib_pb.clone(),
            table: browser.active_table.clone(),
        }))
        .style(crate::styles::primary_button(tokens));

    let title = text(UserText::new(format!("{}.snxlib", lib.display_name)))
        .size(TS.heading)
        .color(theme_ext::text_primary(tokens));

    // Real counts only: components across every table, and the table count.
    let total: usize = lib.tables.values().map(Vec::len).sum();
    let caption = text(t!(
        i18n,
        ids::LIBRARY_BROWSER_SUMMARY,
        "components" => total,
        "tables" => lib.tables.len()
    ))
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
    let mut pills: iced::widget::Row<Element<'a, LibraryMessage>> = row![
        text(t!(i18n, ids::LIBRARY_BROWSER_SHOW))
            .size(TS.caption)
            .color(ink3)
            .boxed()
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center)
    .width(Length::Fill);
    for &filter in LifecycleFilter::ALL {
        let active = browser.lifecycle_filter == filter;
        let library_for_lc = lib_pb.clone();
        pills = pills.push(
            button(text(t!(i18n, filter.message())).size(TS.body))
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

    pills = pills.push(Space::new().width(Length::Fill).boxed());
    pills = pills.push(
        text(count.label(i18n))
            .size(TS.caption)
            .color(ink3)
            .wrapping(iced::widget::text::Wrapping::None)
            .boxed(),
    );

    container(column![top.boxed(), pills.boxed()].spacing(SP.s))
        .padding([SP.m, SP.l])
        .width(Length::Fill)
        .style(crate::styles::tab_bar_strip(tokens))
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shown(count: &ShownCount) -> String {
        let i18n = signex_i18n::I18n::en_us_ref();
        count
            .label(i18n)
            .into_string()
            .replace(['\u{2068}', '\u{2069}'], "")
    }

    /// #2262: the count line names the rows left after the search and
    /// filters, of the table's rows, and the sort column when there is one.
    #[test]
    fn the_count_line_reads_shown_of_total_and_the_sort_column() {
        let mut count = ShownCount {
            shown: 14,
            total: 87,
            sorted_by: None,
        };
        assert_eq!(shown(&count), "14 of 87 shown");
        count.sorted_by = Some("Internal PN".into());
        assert_eq!(shown(&count), "14 of 87 shown · sorted by Internal PN");
    }
}
