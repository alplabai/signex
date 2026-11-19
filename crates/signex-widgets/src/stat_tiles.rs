//! Stat tiles: a row of equal-width count tiles, each a big number over a
//! caption ("38 / parts change", "3 / errors"). A tile the caller gives a
//! message is a filter toggle; the others are read-only summaries.
//!
//! Stateless: the caller owns which tiles are on and maps each press to a
//! message. Sizes derive from the body text size ([`StatTileMetrics`]);
//! colours come from the caller's tokens ([`StatTileStyle`]).

use iced::alignment::Vertical;
use iced::font::Weight;
use iced::widget::text::{Ellipsis, LineHeight, Wrapping};
use iced::widget::{Column, Row, container};
use iced::{Background, Border, Color, Element, Font, Length, Padding, Pixels, Theme, Widget};
use signex_i18n::{I18n, UserText};
use signex_types::theme::ThemeTokens;

use crate::chips::count_text;
use crate::theme_ext::{DEFAULT_BASE_SIZE, HAIRLINE, Radii, TypeScale, to_color};
use crate::{Label, button, text};

// Graphite tile sizes at a 12 px body, kept as ratios of the body size.
const PAD_V_EM: f32 = 6.0 / 12.0;
const PAD_H_EM: f32 = 10.0 / 12.0;
const COMPACT_PAD_V_EM: f32 = 4.0 / 12.0;
const COMPACT_PAD_H_EM: f32 = 8.0 / 12.0;
const GAP_EM: f32 = 6.0 / 12.0;
const RADIUS_EM: f32 = Radii::GRAPHITE.button / DEFAULT_BASE_SIZE;
/// Line height of the number, relative to its size.
const NUMBER_LINE_HEIGHT: f32 = 1.2;

/// How tall the tiles sit: the dialog summary ([`Density::Regular`]) or
/// the panel header ([`Density::Compact`], smaller padding and number).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Density {
    #[default]
    Regular,
    Compact,
}

/// Sizes of a tile row, derived from the body text size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatTileMetrics {
    /// Size of a count (mono).
    pub number_size: f32,
    /// Size of a value written as text ("IEEE 315").
    pub value_text_size: f32,
    /// Size of the caption under the value.
    pub caption_size: f32,
    pub pad_v: f32,
    pub pad_h: f32,
    /// Gap between tiles.
    pub gap: f32,
    pub radius: f32,
}

impl StatTileMetrics {
    /// Every size from `body`, the body text size of the type scale.
    pub fn from_text_size(body: f32, density: Density) -> Self {
        let scale = TypeScale::from_base(body);
        let (number_size, pad_v, pad_h) = match density {
            Density::Regular => (scale.heading, PAD_V_EM, PAD_H_EM),
            Density::Compact => (scale.title, COMPACT_PAD_V_EM, COMPACT_PAD_H_EM),
        };
        Self {
            number_size,
            value_text_size: scale.label,
            caption_size: scale.caption,
            pad_v: body * pad_v,
            pad_h: body * pad_h,
            gap: body * GAP_EM,
            radius: body * RADIUS_EM,
        }
    }

    /// Height of the value line, the same for a count and a text value so
    /// tiles in one row line up.
    pub fn value_line(&self) -> f32 {
        self.number_size * NUMBER_LINE_HEIGHT
    }
}

/// The colour of a tile's number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tone {
    #[default]
    Neutral,
    Ok,
    Info,
    Warn,
    Err,
}

/// The big value of a tile.
#[derive(Debug, Clone)]
pub enum TileValue<'a> {
    /// A count, written in the locale's digits and capped like a chip count.
    Count(u32),
    /// A word or code at the label size ("IEEE 315").
    Text(Label<'a>),
}

/// One tile. Built with [`StatTile::count`] or [`StatTile::text`].
#[derive(Debug, Clone)]
pub struct StatTile<'a, M> {
    pub value: TileValue<'a>,
    pub label: Label<'a>,
    pub tone: Tone,
    /// Selected as a filter: accent edge and accent-soft fill.
    pub is_on: bool,
    /// The message a press sends; `None` leaves the tile inert.
    pub on_press: Option<M>,
}

impl<'a, M> StatTile<'a, M> {
    pub fn count(count: u32, label: impl Into<Label<'a>>) -> Self {
        Self::new(TileValue::Count(count), label.into())
    }

    pub fn text(value: impl Into<Label<'a>>, label: impl Into<Label<'a>>) -> Self {
        Self::new(TileValue::Text(value.into()), label.into())
    }

    fn new(value: TileValue<'a>, label: Label<'a>) -> Self {
        Self {
            value,
            label,
            tone: Tone::Neutral,
            is_on: false,
            on_press: None,
        }
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    pub fn on(mut self, is_on: bool) -> Self {
        self.is_on = is_on;
        self
    }

    pub fn on_press(mut self, message: M) -> Self {
        self.on_press = Some(message);
        self
    }

    /// The tone the number is drawn in: a zero count is never coloured, so
    /// a clean run shows no red or amber.
    pub fn effective_tone(&self) -> Tone {
        match self.value {
            TileValue::Count(0) => Tone::Neutral,
            _ => self.tone,
        }
    }

    /// Whether the tile reacts to the pointer.
    pub fn is_clickable(&self) -> bool {
        self.on_press.is_some()
    }
}

/// Colours of a tile row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatTileStyle {
    pub fill: Color,
    pub edge: Color,
    /// Fill of a hovered, unselected clickable tile.
    pub hover: Color,
    pub on_fill: Color,
    pub on_edge: Color,
    pub ink: Color,
    pub caption: Color,
    pub ok: Color,
    pub info: Color,
    pub warn: Color,
    pub err: Color,
}

impl StatTileStyle {
    /// Colours from the theme; an extended token a theme leaves unset falls
    /// back to the nearest base token.
    pub fn from_tokens(tokens: &ThemeTokens) -> Self {
        let ext = &tokens.extended;
        let or = |c: Option<signex_types::theme::Color>, base| to_color(&c.unwrap_or(base));
        Self {
            fill: or(ext.field, tokens.panel_bg),
            edge: to_color(&tokens.border),
            hover: to_color(&tokens.hover),
            on_fill: or(ext.accent_soft, tokens.selection),
            on_edge: to_color(&tokens.accent),
            ink: to_color(&tokens.text),
            caption: or(ext.ink3, tokens.text_secondary),
            ok: to_color(&tokens.success),
            info: or(ext.info, tokens.accent),
            warn: to_color(&tokens.warning),
            err: to_color(&tokens.error),
        }
    }

    /// Ink of a number in `tone`.
    pub fn tone_ink(&self, tone: Tone) -> Color {
        match tone {
            Tone::Neutral => self.ink,
            Tone::Ok => self.ok,
            Tone::Info => self.info,
            Tone::Warn => self.warn,
            Tone::Err => self.err,
        }
    }

    /// Fill and edge of a tile.
    fn surface(&self, is_on: bool, is_hovered: bool) -> (Color, Color) {
        match (is_on, is_hovered) {
            (true, _) => (self.on_fill, self.on_edge),
            (false, true) => (self.hover, self.edge),
            (false, false) => (self.fill, self.edge),
        }
    }
}

fn medium() -> Font {
    Font {
        weight: Weight::Medium,
        ..Font::MONOSPACE
    }
}

fn value_text<'a, M: 'a>(
    value: TileValue<'a>,
    ink: Color,
    metrics: StatTileMetrics,
    i18n: &I18n,
) -> Element<'a, M> {
    let line = LineHeight::Absolute(Pixels(metrics.value_line()));
    let t = match value {
        TileValue::Count(n) => text(UserText::new(count_text(i18n, n)))
            .size(metrics.number_size)
            .font(medium()),
        TileValue::Text(label) => text(label).size(metrics.value_text_size),
    };
    t.color(ink)
        .line_height(line)
        .align_y(Vertical::Center)
        .wrapping(Wrapping::None)
        .ellipsis(Ellipsis::End)
        .width(Length::Fill)
        .boxed()
}

fn tile<'a, M: Clone + 'a>(
    tile: StatTile<'a, M>,
    style: StatTileStyle,
    metrics: StatTileMetrics,
    i18n: &I18n,
) -> Element<'a, M> {
    let ink = style.tone_ink(tile.effective_tone());
    let is_on = tile.is_on;
    let body: Column<Element<'a, M>> = Column::new()
        .push(value_text(tile.value, ink, metrics, i18n))
        .push(
            text(tile.label)
                .size(metrics.caption_size)
                .color(style.caption)
                .wrapping(Wrapping::None)
                .ellipsis(Ellipsis::End)
                .width(Length::Fill),
        )
        .width(Length::Fill);
    let padding = Padding::from([metrics.pad_v, metrics.pad_h]);
    let border = move |edge: Color| Border {
        color: edge,
        width: HAIRLINE,
        radius: metrics.radius.into(),
        ..Border::default()
    };

    let Some(message) = tile.on_press else {
        // Inert: a container, so no hover and no pointer cursor.
        let (fill, edge) = style.surface(is_on, false);
        return container(body)
            .padding(padding)
            .width(Length::FillPortion(1))
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(fill)),
                border: border(edge),
                ..container::Style::default()
            })
            .boxed();
    };
    button(body)
        .padding(padding)
        .width(Length::FillPortion(1))
        .on_press(message)
        .style(move |_: &Theme, status: button::Status| {
            let is_hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
            let (fill, edge) = style.surface(is_on, is_hovered);
            button::Style {
                background: Some(Background::Color(fill)),
                text_color: ink,
                border: border(edge),
                ..button::Style::default()
            }
        })
        .boxed()
}

/// A row of equal-width stat tiles, counts written in the locale of
/// `i18n`. Sizes derive from `body`, the body text size.
pub fn stat_tiles<'a, M: Clone + 'a>(
    tiles: Vec<StatTile<'a, M>>,
    density: Density,
    tokens: &ThemeTokens,
    body: f32,
    i18n: &I18n,
) -> Element<'a, M> {
    let style = StatTileStyle::from_tokens(tokens);
    let metrics = StatTileMetrics::from_text_size(body, density);
    let mut row: Row<Element<'a, M>> = Row::new().spacing(metrics.gap).width(Length::Fill);
    for t in tiles {
        row = row.push(tile(t, style, metrics, i18n));
    }
    row.boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::theme::{ThemeId, theme_tokens};

    /// Fixture text for the tests.
    fn sample(text: &str) -> Label<'static> {
        crate::raw(text.to_owned())
    }

    #[test]
    fn metrics_at_the_12_px_body_match_the_mockup() {
        let m = StatTileMetrics::from_text_size(12.0, Density::Regular);
        assert!((m.pad_v - 6.0).abs() < 1e-4);
        assert!((m.pad_h - 10.0).abs() < 1e-4);
        assert!((m.gap - 6.0).abs() < 1e-4);
        assert!((m.radius - 8.0).abs() < 1e-4);
        assert_eq!(m.number_size, TypeScale::from_base(12.0).heading);
        assert_eq!(m.caption_size, TypeScale::from_base(12.0).caption);
        let c = StatTileMetrics::from_text_size(12.0, Density::Compact);
        assert!((c.pad_v - 4.0).abs() < 1e-4);
        assert!((c.pad_h - 8.0).abs() < 1e-4);
        assert_eq!(c.number_size, TypeScale::from_base(12.0).title);
    }

    #[test]
    fn metrics_scale_with_the_body_size() {
        for density in [Density::Regular, Density::Compact] {
            let a = StatTileMetrics::from_text_size(12.0, density);
            let b = StatTileMetrics::from_text_size(24.0, density);
            assert!((b.number_size - 2.0 * a.number_size).abs() < 1e-4);
            assert!((b.pad_h - 2.0 * a.pad_h).abs() < 1e-4);
            assert!((b.radius - 2.0 * a.radius).abs() < 1e-4);
            assert!((b.value_line() - 2.0 * a.value_line()).abs() < 1e-4);
        }
    }

    #[test]
    fn each_tone_maps_to_its_token() {
        let tokens = theme_tokens(ThemeId::GraphiteLight);
        let s = StatTileStyle::from_tokens(&tokens);
        assert_eq!(s.tone_ink(Tone::Neutral), to_color(&tokens.text));
        assert_eq!(s.tone_ink(Tone::Ok), to_color(&tokens.success));
        assert_eq!(s.tone_ink(Tone::Warn), to_color(&tokens.warning));
        assert_eq!(s.tone_ink(Tone::Err), to_color(&tokens.error));
        let info = tokens.extended.info.unwrap_or(tokens.accent);
        assert_eq!(s.tone_ink(Tone::Info), to_color(&info));
    }

    #[test]
    fn zero_count_is_neutral_whatever_the_tone() {
        for tone in [Tone::Ok, Tone::Info, Tone::Warn, Tone::Err] {
            let zero = StatTile::<()>::count(0, sample("errors")).tone(tone);
            assert_eq!(zero.effective_tone(), Tone::Neutral);
            let some = StatTile::<()>::count(3, sample("errors")).tone(tone);
            assert_eq!(some.effective_tone(), tone);
        }
        let text = StatTile::<()>::text(sample("IEEE 315"), sample("standard")).tone(Tone::Info);
        assert_eq!(text.effective_tone(), Tone::Info);
    }

    #[test]
    fn tile_without_a_message_is_inert() {
        let inert = StatTile::<u8>::count(38, sample("parts change"));
        assert!(!inert.is_clickable());
        let filter = StatTile::count(14, sample("Add")).on_press(1u8).on(true);
        assert!(filter.is_clickable());
        assert!(filter.is_on);
    }

    #[test]
    fn selected_tile_takes_the_accent_edge_and_soft_fill() {
        let tokens = theme_tokens(ThemeId::GraphiteDark);
        let s = StatTileStyle::from_tokens(&tokens);
        assert_eq!(s.surface(true, false), (s.on_fill, s.on_edge));
        assert_eq!(s.surface(true, true), (s.on_fill, s.on_edge));
        assert_eq!(s.surface(false, false), (s.fill, s.edge));
        assert_eq!(s.on_edge, to_color(&tokens.accent));
    }

    #[test]
    fn every_tile_kind_builds_an_element() {
        let tokens = theme_tokens(ThemeId::GraphiteLight);
        let i18n = I18n::en_us_ref();
        let _: Element<'_, u8> = stat_tiles(
            vec![
                StatTile::count(38, sample("parts change")),
                StatTile::text(sample("IEEE 315"), sample("standard")),
                StatTile::count(3, sample("Problems"))
                    .tone(Tone::Err)
                    .on(true)
                    .on_press(3),
                StatTile::count(0, sample("keepout")).tone(Tone::Err),
            ],
            Density::Compact,
            &tokens,
            12.0,
            i18n,
        );
    }
}
