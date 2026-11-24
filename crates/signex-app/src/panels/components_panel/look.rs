//! Graphite look of the Components panel: the filter field, source chips,
//! the add menu, library and component rows, empty states and separators.

use iced::widget::{container, row, svg};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Padding, Theme, Widget};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text, text_input};

use crate::library::messages::LibraryMessage;
use crate::library::state::{ComponentsMountSource, ComponentsRowStyle};
use crate::styles::tokens::{hover_overlay, resolve};
use crate::styles::{PillTone, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;

/// Height of a library or component row.
pub(super) const ROW_HEIGHT: f32 = 26.0;
/// Height of the filter field.
const FILTER_HEIGHT: f32 = 28.0;
/// Gap around the filter field and at the right edge of every row.
pub(super) const EDGE: f32 = 10.0;
/// Side of the section chevron.
const CHEVRON: f32 = signex_widgets::chevron::side(TS.body);
/// Gap between the chevron, the label and the count pill.
const CHEVRON_GAP: f32 = 6.0;
/// Left inset of library headers and empty states: under the section label.
pub(super) const LIBRARY_INSET: f32 = EDGE + CHEVRON + CHEVRON_GAP;
/// Left inset of a component row: one step in from its library.
pub(super) const ROW_INSET: f32 = LIBRARY_INSET + 12.0;

const PLUS_GLYPH: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 12 12"><path d="M6 2v8M2 6h8" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>"#;

const SEARCH_GLYPH: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 14 14"><circle cx="6" cy="6" r="4.25" fill="none" stroke="currentColor" stroke-width="1.4"/><path d="M9.25 9.25L12.5 12.5" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>"#;

/// The mono face part numbers are set in.
pub(super) fn mono() -> Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

/// A monochrome glyph tinted `ink`.
fn glyph(bytes: &'static [u8], side: f32, ink: Color) -> iced::widget::Svg<'static> {
    svg(svg::Handle::from_memory(bytes))
        .width(side)
        .height(side)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        })
}

/// The filter field: a search glyph and a borderless input in a field frame.
pub(super) fn filter_field<'a>(
    value: &'a str,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let r = resolve(tokens);
    let (ink, ink4, accent) = (ti(tokens.text), r.ink4, ti(tokens.accent));
    let input = text_input(t!(i18n, ids::PANELS_COMPONENTS_FILTER), value)
        .on_input(LibraryMessage::ComponentsPanelSetFilter)
        .padding(Padding::ZERO)
        .size(TS.body)
        .width(Length::Fill)
        .style(move |_: &Theme, _| text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            placeholder: ink4,
            value: ink,
            selection: Color { a: 0.4, ..accent },
        });
    let field = container(
        row![glyph(SEARCH_GLYPH, 14.0, ink4).boxed(), input.boxed()]
            .spacing(CHEVRON_GAP)
            .align_y(Alignment::Center),
    )
    .padding([0.0, EDGE])
    .height(FILTER_HEIGHT)
    .center_y(FILTER_HEIGHT)
    .width(Length::Fill)
    .style(crate::styles::field(tokens, false));
    // The header row holds the side insets.
    container(field)
        .padding([8.0, 0.0])
        .width(Length::Fill)
        .boxed()
}

/// Height of a source chip.
const CHIP_HEIGHT: f32 = 22.0;

/// The panel header: "+ Add" and Previews on the first line, the search
/// field across the whole dock width under them, so its placeholder stays
/// readable at any dock width (mockup row 52).
pub(super) fn header_row<'a>(
    filter: Element<'a, LibraryMessage>,
    row_style: ComponentsRowStyle,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let ink = ti(tokens.text);
    let add = button(
        row![
            glyph(PLUS_GLYPH, 12.0, ink),
            text(t!(i18n, ids::PANELS_COMPONENTS_ADD))
                .size(TS.caption)
                .color(ink)
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .padding([3.0, 8.0])
    .on_press(LibraryMessage::ComponentsPanelAddMenuToggled)
    .style(crate::panels::form_look::ghost_button);
    let controls = row![add, previews_switch(row_style, tokens, i18n)]
        .spacing(CHEVRON_GAP)
        .align_y(Alignment::Center);
    iced::widget::column![controls, filter]
        .padding(Padding {
            top: super::SP.s,
            right: EDGE,
            bottom: 0.0,
            left: EDGE,
        })
        .width(Length::Fill)
        .boxed()
}

/// The info line under the header that says what each bare number of the
/// search matched, with part counts: `100  100 pF 8 · 100 nF 8 · name or
/// text 2` (components-cx decisions 6 and 9). `None` without one.
pub(super) fn match_summary<'a>(
    lines: &[super::query::MatchLine],
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Option<Element<'a, LibraryMessage>> {
    /// Values listed before "+N more".
    const SHOWN: usize = 5;
    let r = resolve(tokens);
    let ink = r.info;
    let item = |label: Localized| -> Element<'a, LibraryMessage> {
        text(label).size(TS.caption).color(ink).boxed()
    };
    let sep = || -> Element<'a, LibraryMessage> {
        text(t!(i18n, ids::PANELS_COMPONENTS_MATCH_SEPARATOR))
            .size(TS.caption)
            .color(Color { a: 0.55, ..ink })
            .boxed()
    };
    let mut col = iced::widget::Column::new().spacing(super::SP.xs);
    for line in lines {
        if line.values.is_empty() && line.by_name == 0 {
            continue;
        }
        let mut items: Vec<Element<'a, LibraryMessage>> = line
            .values
            .iter()
            .take(SHOWN)
            .map(|(value, count)| {
                item(t!(
                    i18n,
                    ids::PANELS_COMPONENTS_MATCH_VALUE,
                    "value" => value.as_str(),
                    "count" => *count
                ))
            })
            .collect();
        if line.values.len() > SHOWN {
            items.push(item(t!(
                i18n,
                ids::PANELS_COMPONENTS_MATCH_MORE,
                "count" => line.values.len() - SHOWN
            )));
        }
        if line.by_name > 0 {
            items.push(item(t!(
                i18n,
                ids::PANELS_COMPONENTS_MATCH_BY_NAME,
                "count" => line.by_name
            )));
        }
        let mut words = iced::widget::Row::new()
            .spacing(super::SP.xs)
            .align_y(Alignment::Center)
            .push(
                text(signex_i18n::UserText::new(line.word.clone()))
                    .size(TS.caption)
                    .color(ink)
                    .font(Font {
                        weight: iced::font::Weight::Semibold,
                        ..Font::DEFAULT
                    })
                    .boxed(),
            );
        for (i, entry) in items.into_iter().enumerate() {
            if i > 0 {
                words = words.push(sep());
            }
            words = words.push(entry);
        }
        col = col.push(words.wrap().boxed());
    }
    if lines.iter().all(|l| l.values.is_empty() && l.by_name == 0) {
        return None;
    }
    let fill = r.info_soft;
    Some(
        container(col)
            .padding([super::SP.xs, EDGE])
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(fill)),
                ..container::Style::default()
            })
            .boxed(),
    )
}

/// The "Previews" switch next to Add: rows with a symbol thumbnail and a
/// description, remembered as a preference (#2535).
fn previews_switch<'a>(
    row_style: ComponentsRowStyle,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let r = resolve(tokens);
    let on = row_style.shows_previews();
    let style = signex_widgets::toggle_switch::ToggleSwitchStyle::new(
        ti(tokens.accent),
        r.line_strong,
        crate::styles::tokens::on_accent(tokens),
    );
    let flipped = ComponentsRowStyle::from_previews(!on);
    let switch = signex_widgets::toggle_switch::toggle_switch(
        on,
        style,
        Some(LibraryMessage::ComponentsPanelRowStylePicked(flipped)),
    );
    // The label flips the switch too.
    let label = button(
        text(t!(i18n, ids::PANELS_COMPONENTS_PREVIEWS))
            .size(TS.caption)
            .color(r.ink3),
    )
    .padding(0.0)
    .on_press(LibraryMessage::ComponentsPanelRowStylePicked(flipped))
    .style(|_: &Theme, _| button::Style::default());
    crate::panels::graphite::hint_tooltip(
        row![switch, label]
            .spacing(super::SP.s)
            .align_y(Alignment::Center)
            .boxed(),
        t!(i18n, ids::PANELS_COMPONENTS_PREVIEWS_TOOLTIP),
        tokens,
        iced::widget::tooltip::Position::Bottom,
    )
}

/// Width of a row symbol thumbnail.
pub(super) const THUMB_WIDTH: f32 = TS.body * 3.0;
/// Height of a row symbol thumbnail.
pub(super) const THUMB_HEIGHT: f32 = TS.body * 2.5;
/// Height of a Previews row: thumbnail, name and one line of description.
pub(super) const PREVIEW_ROW_HEIGHT: f32 = TS.body * 10.0 / 3.0;

/// The empty box a thumbnail not read yet takes, so rows keep their layout.
pub(super) fn thumb_placeholder<'a>() -> Element<'a, LibraryMessage> {
    iced::widget::Space::new()
        .width(THUMB_WIDTH)
        .height(THUMB_HEIGHT)
        .boxed()
}

/// The small "Place" button of a hovered row or a recent part.
pub(super) fn place_button<'a>(
    on_press: LibraryMessage,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    button(
        text(t!(i18n, ids::PANELS_COMPONENTS_PLACE))
            .size(TS.caption)
            .color(ti(tokens.text)),
    )
    .padding([super::SP.xs, super::SP.m])
    .on_press(on_press)
    .style(crate::styles::outline_button(tokens))
    .boxed()
}

/// The menu "+ Add" opens: where to add the library.
pub(super) fn add_menu<'a>(tokens: &ThemeTokens, i18n: &I18n) -> Element<'a, LibraryMessage> {
    let ink = ti(tokens.text);
    let hover = hover_overlay(tokens);
    let mut items = iced::widget::Column::new().spacing(0);
    for source in ComponentsMountSource::ADDABLE {
        items = items.push(
            button(text(t!(i18n, source.message())).size(TS.body).color(ink))
                .padding([5.0, 10.0])
                .width(Length::Fill)
                .on_press(LibraryMessage::ComponentsPanelAddLibrary(*source))
                .style(move |_: &Theme, status: button::Status| button::Style {
                    background: matches!(status, button::Status::Hovered | button::Status::Pressed)
                        .then_some(Background::Color(hover)),
                    text_color: ink,
                    border: Border {
                        radius: 6.0.into(),
                        ..Border::default()
                    },
                    ..button::Style::default()
                }),
        );
    }
    container(
        container(items)
            .padding(4.0)
            .width(Length::Fill)
            .style(crate::styles::card(tokens)),
    )
    .padding([0.0, EDGE])
    .width(Length::Fill)
    .boxed()
}

/// One source chip: its name and library count; the chosen one is lit.
pub(super) fn source_chip<'a>(
    label: Localized,
    count: usize,
    selected: bool,
    on_press: LibraryMessage,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let r = resolve(tokens);
    let ink = if selected { ti(tokens.text) } else { r.ink3 };
    let (fill, line) = if selected {
        (r.accent_soft, ti(tokens.accent))
    } else {
        (Color::TRANSPARENT, ti(tokens.border))
    };
    let hover = r.accent_soft;
    button(
        row![
            text(label).size(TS.caption).color(ink),
            text(t!(i18n, ids::PANELS_COMPONENTS_COUNT, "count" => count))
                .size(TS.caption)
                .font(mono())
                .color(r.ink4),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .padding([0.0, 8.0])
    .height(CHIP_HEIGHT)
    .on_press(on_press)
    .style(move |_: &Theme, status: button::Status| button::Style {
        background: if selected {
            Some(Background::Color(fill))
        } else {
            matches!(status, button::Status::Hovered | button::Status::Pressed)
                .then_some(Background::Color(hover))
        },
        text_color: ink,
        border: Border {
            width: 1.0,
            color: line,
            radius: 11.0.into(),
            ..Border::default()
        },
        ..button::Style::default()
    })
    .boxed()
}

/// The source chips under the search box, wrapped to the dock width, with
/// the library-count caption at the right.
pub(super) fn chip_row<'a>(
    chips: Vec<Element<'a, LibraryMessage>>,
    caption: Localized,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let mut wrap = iced_aw::Wrap::new().spacing(6.0).line_spacing(6.0);
    for chip in chips {
        wrap = wrap.push(chip);
    }
    container(
        row![
            container(wrap).width(Length::Fill),
            text(caption).size(TS.caption).color(resolve(tokens).ink4),
        ]
        .spacing(CHEVRON_GAP)
        .align_y(Alignment::Center),
    )
    .padding([0.0, EDGE])
    .width(Length::Fill)
    .boxed()
}

/// A secondary empty-state line.
pub(super) fn empty_line<'a>(
    message: Localized,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    container(text(message).size(TS.caption).color(resolve(tokens).ink4))
        .padding(Padding {
            top: 0.0,
            right: EDGE,
            bottom: 8.0,
            left: LIBRARY_INSET,
        })
        .width(Length::Fill)
        .boxed()
}

/// A library row: expand chevron, name, source badge and component count.
pub(super) fn library_header<'a>(
    root: std::path::PathBuf,
    name: String,
    source: ComponentsMountSource,
    count: Localized,
    collapsed: bool,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let r = resolve(tokens);
    let hover = hover_overlay(tokens);
    let ink = ti(tokens.text);
    let content = row![
        signex_widgets::chevron::chevron(!collapsed, TS.body, r.ink3),
        text(signex_i18n::UserText::new(name))
            .size(TS.body)
            .color(ink)
            .width(Length::Fill)
            .wrapping(iced::widget::text::Wrapping::None),
        crate::library::editor::kit::toned_pill(
            t!(i18n, source.badge()).into_string(),
            PillTone::Neutral,
            tokens
        ),
        text(count).size(TS.caption).font(mono()).color(r.ink4),
    ]
    .spacing(CHEVRON_GAP)
    .align_y(Alignment::Center);
    button(content)
        .padding(Padding {
            top: 0.0,
            right: EDGE,
            bottom: 0.0,
            left: EDGE,
        })
        .height(ROW_HEIGHT)
        .width(Length::Fill)
        .on_press(LibraryMessage::ComponentsPanelToggleLibrary(root))
        .style(move |_: &Theme, status: button::Status| button::Style {
            background: matches!(status, button::Status::Hovered | button::Status::Pressed)
                .then_some(Background::Color(hover)),
            text_color: ink,
            ..button::Style::default()
        })
        .boxed()
}

/// Colour of the separators between sections.
pub(super) fn separator(tokens: &ThemeTokens) -> Color {
    resolve(tokens).line_soft
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mockup row 52: in a narrow dock the search field sits under Add and
    /// Previews and spans the dock, so its placeholder stays readable.
    #[test]
    fn the_filter_spans_a_narrow_dock_under_add() {
        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight);
        let i18n = I18n::en_us_ref();
        let header = || {
            header_row(
                filter_field("", &tokens, i18n),
                ComponentsRowStyle::default(),
                &tokens,
                i18n,
            )
        };
        let dock = iced::Size::new(240.0, 120.0);
        let texts = crate::layout_probe::lay_out_texts(header(), dock);
        let add = crate::layout_probe::find(&texts, "Add");
        let inputs = crate::layout_probe::lay_out_inputs(header(), dock);
        let filter = inputs.first().expect("the search field").bounds;
        assert!(
            filter.y >= add.bounds.y + add.bounds.height,
            "filter {filter:?} not under Add {:?}",
            add.bounds
        );
        // The probe gives the text input inside the field box, which spans
        // the dock between the header insets (EDGE each side) and pads its
        // content by EDGE: the input ends at the box's inner right edge and
        // starts right after the 14 px search glyph and its gap.
        let right = filter.x + filter.width;
        assert!(
            (right - (dock.width - 2.0 * EDGE)).abs() <= 1.0,
            "filter {filter:?} does not reach the dock's right inset"
        );
        assert!(
            filter.x <= 2.0 * EDGE + 14.0 + CHEVRON_GAP + 1.0,
            "filter {filter:?} does not start at the dock's left inset"
        );
    }

    #[test]
    fn section_chevron_box_is_the_shared_one_for_the_body_size() {
        assert_eq!(CHEVRON, signex_widgets::chevron::side(TS.body));
        assert_eq!(LIBRARY_INSET, EDGE + CHEVRON + CHEVRON_GAP);
    }
}
