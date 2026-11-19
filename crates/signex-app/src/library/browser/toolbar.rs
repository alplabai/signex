//! Library Browser — the grid toolbar row (#118): the range filters and
//! "Add range filter…" at the left, "N of M shown · sorted by <column>" at
//! the right.

use super::*;

/// Default type / spacing scales; equal to the old literals at the default size.
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// What the count line reads: the rows the search and filters leave, of
/// all rows in the active table, and the sort column.
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

/// The toolbar row above the grid, with a soft line under it.
pub(super) fn view_toolbar<'a>(
    range_bar: Option<Element<'a, LibraryMessage>>,
    count: &ShownCount,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let r = crate::styles::tokens::resolve(tokens);
    let line_soft = r.line_soft;
    let filters = range_bar.unwrap_or_else(|| Space::new().boxed());
    let line: iced::widget::Row<Element<'a, LibraryMessage>> = row![
        filters,
        Space::new().width(Length::Fill).boxed(),
        text(count.label(i18n))
            .size(TS.caption)
            .color(r.ink4)
            .wrapping(iced::widget::text::Wrapping::None)
            .boxed(),
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center);
    container(line)
        .padding([SP.m, SP.l])
        .width(Length::Fill)
        .style(move |_: &Theme| iced::widget::container::Style {
            border: Border {
                color: line_soft,
                bottom: iced::border::Side::default().width(1.0),
                ..Border::default()
            },
            ..iced::widget::container::Style::default()
        })
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
