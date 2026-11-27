//! Library Browser — the header line (mockup row 118): library icon, the
//! library name over its "N components · N tables" caption, the search field, "Show"
//! with the lifecycle filter select, and the primary Add Component at the
//! right end.

use super::*;
use iced::widget::{column, pick_list};
use signex_widgets::dropdown_style::GraphiteDropdown as _;

/// Default type / spacing scales; equal to the old literals at the default size.
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Width of the search field (the mock's `flex: 0 1 420px`).
pub(super) const SEARCH_W: f32 = 420.0;
/// Width of the lifecycle filter select.
pub(super) const FILTER_W: f32 = 170.0;

/// A monochrome chrome icon `size` square in `ink`.
pub(super) fn tinted_icon<'a>(
    handle: iced::widget::svg::Handle,
    size: f32,
    ink: iced::Color,
) -> Element<'a, LibraryMessage> {
    iced::widget::svg(handle)
        .width(size)
        .height(size)
        .style(move |_: &Theme, _| iced::widget::svg::Style {
            color: Some(ink),
            ..Default::default()
        })
        .boxed()
}

/// The header line of the tab.
pub(super) fn view_header<'a>(
    library_path: &'a std::path::Path,
    lib: &'a OpenLibrary,
    browser: &'a LibraryBrowserState,
    tokens: &'a ThemeTokens,
    i18n: &'a I18n,
    read_only: bool,
) -> Element<'a, LibraryMessage> {
    let r = crate::styles::tokens::resolve(tokens);
    let metrics = crate::styles::ChromeMetrics::DEFAULT;
    let lib_pb: std::path::PathBuf = library_path.to_path_buf();
    let glyph = signex_types::theme::ThemeId::GraphiteDark;

    let icon = tinted_icon(crate::icons::icon_chrome_library(glyph), TS.label, r.ink3);
    let title_font = iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    };
    let title = text(UserText::new(lib.display_name.clone()))
        .size(TS.title)
        .font(title_font)
        .color(theme_ext::text_primary(tokens))
        .wrapping(iced::widget::text::Wrapping::None);
    // Real counts only: components across every table, and the table count.
    // No Git pill: the library state does not know whether it is under
    // version control.
    let total: usize = lib.tables.values().map(Vec::len).sum();
    let caption = text(t!(
        i18n,
        ids::LIBRARY_BROWSER_SUMMARY,
        "components" => total,
        "tables" => lib.tables.len()
    ))
    .size(TS.caption)
    .color(r.ink4)
    .wrapping(iced::widget::text::Wrapping::None);
    let mut title = column![title.boxed(), caption.boxed()];
    if let Some(line) = provenance_line(lib, r.ink4, i18n) {
        title = title.push(line);
    }

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
    let search_line = row![
        tinted_icon(
            crate::icons::icon_chrome_search(glyph),
            BROWSER_TEXT_SIZE,
            r.ink4
        ),
        search.boxed()
    ]
    .align_y(iced::Alignment::Center)
    .padding(iced::Padding {
        left: SP.m,
        ..iced::Padding::ZERO
    });
    let search_field = container(search_line)
        .width(Length::Fixed(SEARCH_W))
        .height(Length::Fixed(metrics.field))
        .center_y(Length::Fixed(metrics.field))
        .style(crate::styles::field(tokens, false));

    let library_for_lc = lib_pb.clone();
    let filter = pick_list(
        Some(browser.lifecycle_filter),
        LifecycleFilter::ALL.to_vec(),
        move |f: &LifecycleFilter| i18n.message_plain(f.message(), &[]),
    )
    .graphite()
    .on_select(
        move |filter: LifecycleFilter| LibraryMessage::BrowserSetLifecycleFilter {
            library_path: library_for_lc.clone(),
            filter,
        },
    )
    .width(Length::Fixed(FILTER_W));

    // The label fills the fixed-height button and centres itself; a bare
    // text sits at the top of the button's content box.
    let add_label = row![
        tinted_icon(crate::icons::icon_plus(glyph), TS.body, r.primary_ink),
        text(t!(i18n, ids::LIBRARY_BROWSER_ADD_COMPONENT))
            .size(BROWSER_TEXT_SIZE)
            .boxed(),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center)
    .height(Length::Fill);
    let add = button(add_label)
        .height(Length::Fixed(metrics.button))
        .padding([SP.xs, SP.l])
        .on_press_maybe((!read_only).then(|| LibraryMessage::BrowserAddComponent {
            library_path: lib_pb.clone(),
            table: browser.active_table.clone(),
        }))
        .style(crate::styles::primary_button(tokens));

    let line: iced::widget::Row<Element<'a, LibraryMessage>> = row![
        icon,
        title.boxed(),
        Space::new().width(SP.l).boxed(),
        search_field.boxed(),
        Space::new().width(SP.xs).boxed(),
        text(t!(i18n, ids::LIBRARY_BROWSER_SHOW))
            .size(TS.caption)
            .color(r.ink4)
            .boxed(),
        filter.boxed(),
        Space::new().width(Length::Fill).boxed(),
        add.boxed(),
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center);

    container(line)
        .padding([SP.m, SP.l])
        .width(Length::Fill)
        .style(crate::styles::tab_bar_strip(tokens))
        .boxed()
}

/// The licence and source of a converted library (#3112), when its manifest
/// names either: one caption line under the summary. Read-only.
fn provenance_line<'a>(
    lib: &'a OpenLibrary,
    ink: iced::Color,
    i18n: &I18n,
) -> Option<Element<'a, LibraryMessage>> {
    let p = &lib.provenance;
    let caption = |line| {
        text(line)
            .size(TS.caption)
            .color(ink)
            .wrapping(iced::widget::text::Wrapping::None)
            .boxed()
    };
    let mut parts: Vec<Element<'a, LibraryMessage>> = Vec::new();
    if let Some(license) = p.license.as_deref().filter(|s| !s.trim().is_empty()) {
        parts.push(caption(
            t!(i18n, ids::LIBRARY_BROWSER_LICENSE, "license" => license),
        ));
    }
    if let Some(source) = p.source.as_deref().filter(|s| !s.trim().is_empty()) {
        parts.push(caption(
            t!(i18n, ids::LIBRARY_BROWSER_SOURCE, "source" => source),
        ));
    }
    (!parts.is_empty()).then(|| row(parts).spacing(SP.m).boxed())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn library(provenance: signex_library::LibraryProvenance) -> OpenLibrary {
        OpenLibrary {
            root: std::path::PathBuf::from("/tmp/Converted.snxlib"),
            display_name: "Converted".into(),
            library_id: uuid::Uuid::from_u128(1),
            tables: std::collections::HashMap::new(),
            cached_components: Vec::new(),
            cached_symbols: Vec::new(),
            cached_footprints: Vec::new(),
            cached_sims: Vec::new(),
            cached_classes: Vec::new(),
            observed_stamp: None,
            dangling: Default::default(),
            pricing: Default::default(),
            own_edit_stamp: None,
            display: Default::default(),
            provenance,
        }
    }

    /// #3112: the licence / source line shows only when the manifest names one.
    #[test]
    fn the_provenance_line_shows_only_when_a_licence_or_source_is_set() {
        let i18n = I18n::en_us();
        let ink = iced::Color::BLACK;
        assert!(provenance_line(&library(Default::default()), ink, &i18n).is_none());
        let blank = signex_library::LibraryProvenance {
            license: Some("  ".into()),
            ..Default::default()
        };
        assert!(provenance_line(&library(blank), ink, &i18n).is_none());
        let licensed = signex_library::LibraryProvenance {
            license: Some("CC-BY-SA-4.0".into()),
            ..Default::default()
        };
        assert!(provenance_line(&library(licensed), ink, &i18n).is_some());
        let sourced = signex_library::LibraryProvenance {
            source: Some("https://example.com/parts v2".into()),
            ..Default::default()
        };
        assert!(provenance_line(&library(sourced), ink, &i18n).is_some());
    }
}
