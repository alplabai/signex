//! Library Browser — the grid toolbar row (mockup row 118): the range
//! filters and "Add range filter…" at the left, "N of M shown · sorted by <column>" and
//! the column button at the right. The button opens the column chooser
//! (Rev and Tags start off, decided 2026-11-23).

use super::columns::{ColumnKind, GridColumn};
use super::*;

/// Default type / spacing scales; equal to the old literals at the default size.
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Side of the column button.
pub(super) const COLUMN_BUTTON: f32 = crate::styles::ChromeMetrics::DEFAULT.row;

/// Width of the chooser card: room for the longest column name.
const CHOOSER_W: f32 = 12.0 * TS.body;

const COLUMNS_GLYPH: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><rect x="2.5" y="3" width="11" height="10" rx="1" fill="none" stroke="currentColor" stroke-width="1.2"/><path d="M6.2 3v10M9.8 3v10" stroke="currentColor" stroke-width="1.2"/></svg>"#;

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

/// What the column button and its chooser act on: the tab's library, the
/// table's columns and the tab's choice.
pub(super) struct ColumnChooser<'a> {
    pub(super) library_path: &'a std::path::Path,
    pub(super) columns: &'a [GridColumn],
    pub(super) browser: &'a LibraryBrowserState,
}

/// The toolbar row above the grid, with a soft line under it; the column
/// chooser opens under its right end.
pub(super) fn view_toolbar<'a>(
    range_bar: Option<Element<'a, LibraryMessage>>,
    count: &ShownCount,
    chooser: ColumnChooser<'_>,
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
        column_button(&chooser, tokens, i18n),
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center);
    let bar = container(line)
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
        .boxed();
    if !chooser.browser.columns_menu_open {
        return bar;
    }
    let card = row![
        Space::new().width(Length::Fill).boxed(),
        column_card(&chooser, tokens, i18n),
    ];
    column![bar, container(card).padding([SP.xs, SP.l]).boxed()]
        .width(Length::Fill)
        .boxed()
}

/// The ▥ button at the right of the count line: opens or closes the
/// chooser, and stays marked while it is open.
fn column_button<'a>(
    chooser: &ColumnChooser<'_>,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    use iced::widget::button::{Status, Style};
    let r = crate::styles::tokens::resolve(tokens);
    let (edge, ink) = (theme_ext::border_color(tokens), r.ink3);
    let hover = crate::styles::tokens::overlay(tokens, crate::styles::tokens::Overlay::Hover);
    let open = chooser.browser.columns_menu_open;
    let glyph = super::header::tinted_icon(crate::icons::static_glyph(COLUMNS_GLYPH), TS.body, ink);
    let b = button(container(glyph).center(Length::Fill))
        .width(COLUMN_BUTTON)
        .height(COLUMN_BUTTON)
        .padding(0)
        .on_press(LibraryMessage::BrowserColumnsMenuToggled {
            library_path: chooser.library_path.to_path_buf(),
        })
        .style(move |_: &Theme, status: Status| Style {
            background: (open || matches!(status, Status::Hovered | Status::Pressed))
                .then_some(iced::Background::Color(hover)),
            text_color: ink,
            border: Border {
                width: 1.0,
                color: edge,
                radius: crate::styles::Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..Style::default()
        });
    super::grid::with_tooltip(b.boxed(), t!(i18n, ids::LIBRARY_BROWSER_COLUMNS), tokens)
}

/// The chooser: "Columns", then one checkbox row per column in grid order.
/// The Internal PN names the row and always shows, so it is not listed.
fn column_card<'a>(
    chooser: &ColumnChooser<'_>,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    use iced::widget::button::{Status, Style};
    let r = crate::styles::tokens::resolve(tokens);
    let ink = theme_ext::text_primary(tokens);
    let hover = crate::styles::tokens::overlay(tokens, crate::styles::tokens::Overlay::Hover);
    let mut list: Column<Element<'a, LibraryMessage>> = column![
        container(
            text(t!(i18n, ids::LIBRARY_BROWSER_COLUMNS))
                .size(TS.caption)
                .color(r.ink4)
                .wrapping(iced::widget::text::Wrapping::None),
        )
        .padding([SP.xs, SP.m])
        .boxed()
    ];
    for slot in super::layout::slots(chooser.columns, true) {
        let super::layout::Slot::Column(c) = slot else {
            continue;
        };
        if matches!(c.kind, ColumnKind::InternalPn) {
            continue;
        }
        let key = c.kind.sort_key();
        let name = match c.kind {
            ColumnKind::Symbol => t!(i18n, ids::LIBRARY_BROWSER_COLUMN_SYM_TIP).into_string(),
            ColumnKind::Footprint => t!(i18n, ids::LIBRARY_BROWSER_COLUMN_FPT_TIP).into_string(),
            _ => c.caption(i18n),
        };
        // No toggle of its own: the row's button takes the click.
        let tick = signex_widgets::checkbox(chooser.browser.column_shown(&key))
            .size(TS.body)
            .style(crate::panels::form_look::checkbox_style);
        let cells = row![
            tick.boxed(),
            text(raw(name))
                .size(BROWSER_TEXT_SIZE)
                .color(ink)
                .wrapping(iced::widget::text::Wrapping::None)
                .boxed(),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center);
        list = list.push(
            button(cells)
                .padding([SP.xs, SP.m])
                .width(Length::Fill)
                .on_press(LibraryMessage::BrowserColumnToggled {
                    library_path: chooser.library_path.to_path_buf(),
                    column_key: key,
                })
                .style(move |_: &Theme, status: Status| Style {
                    background: matches!(status, Status::Hovered | Status::Pressed)
                        .then_some(iced::Background::Color(hover)),
                    text_color: ink,
                    ..Style::default()
                })
                .boxed(),
        );
    }
    container(list.width(Length::Fixed(CHOOSER_W)))
        .padding(SP.xs)
        .style(crate::styles::context_menu(tokens))
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout_probe::{lay_out_texts, messages_on_click, messages_on_click_at};

    const WINDOW: iced::Size = iced::Size::new(900.0, 400.0);

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

    fn column(label: &str, kind: ColumnKind) -> GridColumn {
        GridColumn {
            label: label.into(),
            kind,
            width: 100.0,
        }
    }

    fn columns() -> Vec<GridColumn> {
        vec![
            column("Internal PN", ColumnKind::InternalPn),
            column("MPN", ColumnKind::Mpn),
            column("Rev", ColumnKind::Rev),
            column("Symbol", ColumnKind::Symbol),
            column("Tags", ColumnKind::Tags),
        ]
    }

    const COUNT: ShownCount = ShownCount {
        shown: 3,
        total: 9,
        sorted_by: None,
    };

    fn toolbar<'a>(
        columns: &'a [GridColumn],
        browser: &'a LibraryBrowserState,
        tokens: &'a ThemeTokens,
    ) -> Element<'a, LibraryMessage> {
        view_toolbar(
            None,
            &COUNT,
            ColumnChooser {
                library_path: std::path::Path::new("lib.snxlib"),
                columns,
                browser,
            },
            tokens,
            signex_i18n::I18n::en_us_ref(),
        )
    }

    fn texts(element: Element<'_, LibraryMessage>) -> Vec<String> {
        lay_out_texts(element, WINDOW)
            .into_iter()
            .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
            .collect()
    }

    /// Mockup row 118: the column button sits at the right of the count
    /// line and opens the chooser; closed, no column list shows.
    #[test]
    fn the_column_button_at_the_right_of_the_count_line_opens_the_chooser() {
        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteDark);
        let cols = columns();
        let browser = LibraryBrowserState::new("lib.snxlib".into());
        let laid = lay_out_texts(toolbar(&cols, &browser, &tokens), WINDOW);
        let count = laid
            .iter()
            .find(|t| t.text.contains("3 of 9 shown"))
            .expect("count line");
        assert!(
            !laid
                .iter()
                .any(|t| t.text == "MPN" || t.text == "Symbol bound"),
            "closed, the chooser lists nothing: {laid:#?}"
        );
        // The button is the last thing on the line, right of the count.
        let x = WINDOW.width - SP.l - COLUMN_BUTTON / 2.0;
        assert!(count.bounds.x + count.bounds.width < x - COLUMN_BUTTON / 2.0);
        let at = iced::Point::new(x, count.bounds.center_y());
        let sent = messages_on_click_at(toolbar(&cols, &browser, &tokens), WINDOW, None, at);
        assert!(
            sent.iter()
                .any(|m| matches!(m, LibraryMessage::BrowserColumnsMenuToggled { .. })),
            "{sent:?}"
        );
    }

    /// Mockup row 118 (decided 2026-11-23): the open chooser lists every
    /// column but the Internal PN, with Rev and Tags off, and a row press
    /// turns its column on or off.
    #[test]
    fn the_open_chooser_lists_the_columns_and_toggles_one_per_press() {
        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight);
        let cols = columns();
        let mut browser = LibraryBrowserState::new("lib.snxlib".into());
        browser.columns_menu_open = true;
        let shown = texts(toolbar(&cols, &browser, &tokens));
        for name in ["Columns", "MPN", "Rev", "Symbol bound", "Tags"] {
            assert!(shown.iter().any(|t| t == name), "{name}: {shown:#?}");
        }
        assert!(!shown.iter().any(|t| t == "Internal PN"), "{shown:#?}");
        assert!(!browser.column_shown("version"));
        assert!(!browser.column_shown("parameters.tags"));
        assert!(browser.column_shown("mpn"));

        let sent = messages_on_click(toolbar(&cols, &browser, &tokens), WINDOW, "Rev");
        assert!(
            sent.iter().any(|m| matches!(
                m,
                LibraryMessage::BrowserColumnToggled { column_key, .. } if column_key == "version"
            )),
            "{sent:?}"
        );
    }
}
