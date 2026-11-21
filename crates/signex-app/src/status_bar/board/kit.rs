//! The pieces every board popover and counter is made of: the mock's sizes,
//! the glyphs, the count badge, the card with its header, rows and footer,
//! and the small buttons (mockup row 164, `.sb-*`).

use iced::widget::{Column, Row, Space, Widget, column, container, row, svg};
use iced::{Background, Border, Color, Element, Font, Length, Padding, Theme};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::{Label, button, text};

use super::super::graphite::{SP, TS, mono_font, popup, popup_row};
use super::{BoardPopoverEvent, BoardPopoverModel};
use crate::styles::tokens::resolve;
use crate::styles::{HAIRLINE, Radii, ti};

/// One pixel of the mock at the default type scale: every size below
/// follows the scale.
pub(super) const PX: f32 = TS.body / 12.0;
/// A counter's side padding and the gap between its parts.
pub(super) const SEGMENT_PAD: f32 = 9.0 * PX;
pub(super) const SEGMENT_GAP: f32 = 5.0 * PX;
/// A glyph in the bar, and in a popover row.
pub(super) const BAR_GLYPH: f32 = 12.0 * PX;
pub(super) const ROW_GLYPH: f32 = 13.0 * PX;
/// The mono figures in the bar.
pub(super) const MONO: f32 = 11.0 * PX;
/// The count badge: its least side, side padding and figure size.
pub(super) const BADGE: f32 = 16.0 * PX;
pub(super) const BADGE_PAD: f32 = 5.0 * PX;
pub(super) const BADGE_TEXT: f32 = TS.caption;
/// A layer swatch.
pub(super) const SWATCH: f32 = 9.0 * PX;
/// The card's inner padding, a row's padding and the gap in a row.
const CARD_PAD: f32 = 6.0 * PX;
const ROW_GAP: f32 = 6.0 * PX;
/// The column a row's leading glyph or swatch sits in.
const ROW_LEAD: f32 = 16.0 * PX;
/// The tallest a list in a card grows before it scrolls.
pub(super) const LIST_MAX: f32 = 4.5 * 34.0 * PX;

/// The theme the glyphs are fetched for: they are tinted with an ink, so
/// any theme gives the same.
const GLYPH_THEME: ThemeId = ThemeId::GraphiteDark;

/// A glyph of the bar and the popovers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Glyph {
    Route,
    Ratsnest,
    Error,
    Warning,
    Selection,
    Grid,
    Layers,
    Part,
    Check,
    Chevron,
}

impl Glyph {
    fn handle(self) -> svg::Handle {
        let bytes: &'static [u8] = match self {
            Self::Route => include_bytes!("../../../assets/icons/status/route.svg"),
            Self::Ratsnest => include_bytes!("../../../assets/icons/status/ratsnest.svg"),
            Self::Error => include_bytes!("../../../assets/icons/status/error.svg"),
            Self::Warning => include_bytes!("../../../assets/icons/status/warning.svg"),
            Self::Selection => include_bytes!("../../../assets/icons/status/selection.svg"),
            Self::Grid => include_bytes!("../../../assets/icons/status/grid.svg"),
            Self::Layers => include_bytes!("../../../assets/icons/status/layers.svg"),
            Self::Part => include_bytes!("../../../assets/icons/status/part.svg"),
            Self::Check => return crate::icons::icon_chrome_check(GLYPH_THEME),
            Self::Chevron => return crate::icons::icon_chrome_chevron_down(GLYPH_THEME),
        };
        crate::icons::static_glyph(bytes)
    }
}

/// `glyph` at `side` px in `ink`.
pub(super) fn glyph<'a, M: 'a>(glyph: Glyph, side: f32, ink: Color) -> Element<'a, M> {
    svg(glyph.handle())
        .width(side)
        .height(side)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        })
        .boxed()
}

/// The UI face at `weight`.
pub(super) fn ui_font(weight: iced::font::Weight) -> Font {
    Font {
        weight,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    }
}

/// The mono face at `weight`.
pub(super) fn mono_weight(weight: iced::font::Weight) -> Font {
    Font {
        weight,
        ..mono_font()
    }
}

/// The colour a count badge is drawn in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Tone {
    Error,
    Warning,
    Accent,
    /// A zero: sunken and muted, never hidden.
    Neutral,
}

/// `(fill, ink)` of a badge of `tone`.
pub(super) fn tone_colours(tone: Tone, tokens: &ThemeTokens) -> (Color, Color) {
    let r = resolve(tokens);
    match tone {
        Tone::Error => (r.err_soft, ti(tokens.error)),
        Tone::Warning => (r.warn_soft, ti(tokens.warning)),
        Tone::Accent => (r.accent_soft, r.accent_soft_ink),
        Tone::Neutral => (r.sunken, r.ink3),
    }
}

/// The badge's text: the count, through the catalog.
pub(super) fn count_text(i18n: &I18n, count: usize) -> Localized {
    t!(i18n, ids::BOARD_STATUS_COUNT, "count" => count)
}

/// Width of a badge holding `label`: the figures and its padding, at least
/// [`BADGE`].
pub(super) fn badge_width(label: &str) -> f32 {
    let figures = crate::dock::measure::text_width_in(
        label,
        BADGE_TEXT,
        mono_weight(iced::font::Weight::Semibold),
    );
    (figures + 2.0 * BADGE_PAD).max(BADGE)
}

/// The tone a badge of `count` is drawn in: a zero is neutral, so it reads
/// as nothing to do without leaving the bar.
pub(super) fn badge_tone(count: usize, tone: Tone) -> Tone {
    if count == 0 { Tone::Neutral } else { tone }
}

/// A pill holding `count` in `tone`; a zero is always [`Tone::Neutral`].
pub(super) fn badge<'a, M: 'a>(
    count: usize,
    tone: Tone,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, M> {
    let (fill, ink) = tone_colours(badge_tone(count, tone), tokens);
    let label = count_text(i18n, count);
    let width = badge_width(&label.to_string());
    container(
        text(label)
            .size(BADGE_TEXT)
            .font(mono_weight(iced::font::Weight::Semibold))
            .color(ink),
    )
    .width(width)
    .height(BADGE)
    .center_x(width)
    .center_y(BADGE)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            radius: Radii::GRAPHITE.pill.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

/// The card around a popover's parts, `width` wide.
pub(super) fn card<'a>(
    width: f32,
    parts: Vec<Element<'a, BoardPopoverEvent>>,
    tokens: &ThemeTokens,
) -> Element<'a, BoardPopoverEvent> {
    container(Column::with_children(parts).width(Length::Fill))
        .width(width)
        .padding(CARD_PAD)
        .style(popup(tokens))
        .boxed()
}

/// The header: the bold title, then `trailing` (a caption or a pill).
pub(super) fn header<'a>(
    title: Localized,
    trailing: Option<Element<'a, BoardPopoverEvent>>,
    tokens: &ThemeTokens,
) -> Element<'a, BoardPopoverEvent> {
    let mut line = row![
        text(title)
            .size(TS.body)
            .font(ui_font(iced::font::Weight::Semibold))
            .color(ti(tokens.text))
            .width(Length::Fill),
    ]
    .spacing(ROW_GAP)
    .align_y(iced::Alignment::Center);
    if let Some(trailing) = trailing {
        line = line.push(trailing);
    }
    container(line)
        .padding(Padding {
            top: 4.0 * PX,
            right: CARD_PAD,
            bottom: CARD_PAD,
            left: CARD_PAD,
        })
        .boxed()
}

/// A muted caption in the header or after a name.
pub(super) fn caption<'a, M: 'a>(
    label: impl Into<Label<'a>>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    text(label)
        .size(TS.caption)
        .color(resolve(tokens).ink3)
        .wrapping(iced::widget::text::Wrapping::None)
        .boxed()
}

/// A detail line under a row's name: faint, one line, ellipsised.
pub(super) fn detail<'a, M: 'a>(
    label: impl Into<Label<'a>>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    text(label)
        .size(TS.caption)
        .color(resolve(tokens).ink4)
        .wrapping(iced::widget::text::Wrapping::None)
        .ellipsis(iced::widget::text::Ellipsis::End)
        .width(Length::Fill)
        .boxed()
}

/// A row: the leading glyph or swatch, the body, and what trails it. A row
/// with `on_press` is a button with the hover fill.
pub(super) fn list_row<'a>(
    lead: Option<Element<'a, BoardPopoverEvent>>,
    body: Element<'a, BoardPopoverEvent>,
    trail: Option<Element<'a, BoardPopoverEvent>>,
    on_press: Option<BoardPopoverEvent>,
    tokens: &ThemeTokens,
) -> Element<'a, BoardPopoverEvent> {
    let lead = container(lead.unwrap_or_else(|| Space::new().boxed()))
        .width(ROW_LEAD)
        .center_x(ROW_LEAD);
    let mut line = Row::new()
        .push(lead.boxed())
        .push(container(body).width(Length::Fill).boxed())
        .spacing(ROW_GAP)
        .align_y(iced::Alignment::Center);
    if let Some(trail) = trail {
        line = line.push(trail);
    }
    // The mock's 26 px row, its 2 px padding inside.
    let face = container(line)
        .width(Length::Fill)
        .center_y(crate::status_bar::popup_row_height() - 4.0 * PX);
    let face = container(face)
        .width(Length::Fill)
        .padding([2.0 * PX, CARD_PAD]);
    match on_press {
        Some(event) => button(face)
            .padding(0)
            .width(Length::Fill)
            .style(popup_row(tokens))
            .on_press(event)
            .boxed(),
        None => face.boxed(),
    }
}

/// The caption that says which keys step through a popover's choices:
/// both, one, or none when the profile binds neither.
pub(super) fn cycles(next: &str, prev: &str, i18n: &I18n) -> Option<Localized> {
    match (next.is_empty(), prev.is_empty()) {
        (false, false) => Some(t!(
            i18n,
            ids::BOARD_STATUS_CYCLES_BOTH,
            "next" => next.to_string(),
            "prev" => prev.to_string()
        )),
        (false, true) => Some(t!(i18n, ids::BOARD_STATUS_CYCLES, "key" => next.to_string())),
        (true, false) => Some(t!(i18n, ids::BOARD_STATUS_CYCLES, "key" => prev.to_string())),
        (true, true) => None,
    }
}

/// A row with a detail line under its name; its Zoom link is the button.
pub(super) fn two_line_row<'a>(
    lead: Element<'a, BoardPopoverEvent>,
    first: Element<'a, BoardPopoverEvent>,
    second: Element<'a, BoardPopoverEvent>,
    trail: Element<'a, BoardPopoverEvent>,
) -> Element<'a, BoardPopoverEvent> {
    let body = column![first, second].spacing(1.0 * PX).width(Length::Fill);
    let line = row![
        container(lead).width(ROW_LEAD).center_x(ROW_LEAD),
        body,
        trail,
    ]
    .spacing(ROW_GAP)
    .align_y(iced::Alignment::Center);
    container(line)
        .width(Length::Fill)
        .padding([2.0 * PX, CARD_PAD])
        .boxed()
}

/// The Zoom link at a row's end.
pub(super) fn zoom_link<'a>(
    event: BoardPopoverEvent,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, BoardPopoverEvent> {
    let ink = resolve(tokens).accent_text;
    button(
        text(t!(i18n, ids::BOARD_STATUS_ZOOM))
            .size(TS.caption)
            .color(ink),
    )
    .padding([0.0, SP.xs])
    .style(button::text)
    .on_press(event)
    .boxed()
}

/// The check mark of the picked row.
pub(super) fn check<'a>(on: bool, tokens: &ThemeTokens) -> Element<'a, BoardPopoverEvent> {
    if on {
        glyph(
            Glyph::Check,
            ROW_GLYPH,
            crate::status_bar::check_ink(tokens),
        )
    } else {
        Space::new().width(ROW_GLYPH).boxed()
    }
}

/// A small footer button; `primary` takes the accent fill. `None` draws it
/// greyed and inert.
pub(super) fn small_button<'a>(
    label: Localized,
    lead: Option<Glyph>,
    primary: bool,
    on_press: Option<BoardPopoverEvent>,
    tokens: &ThemeTokens,
) -> Element<'a, BoardPopoverEvent> {
    let enabled = on_press.is_some();
    let ink = if primary {
        resolve(tokens).primary_ink
    } else if enabled {
        ti(tokens.text)
    } else {
        resolve(tokens).ink4
    };
    let mut face = Row::new()
        .spacing(4.0 * PX)
        .align_y(iced::Alignment::Center);
    if let Some(lead) = lead {
        face = face.push(glyph(lead, BAR_GLYPH, ink));
    }
    face = face.push(text(label).size(TS.caption).color(ink).boxed());
    let b = button(face)
        .padding([3.0 * PX, 8.0 * PX])
        .on_press_maybe(on_press);
    if primary {
        b.style(crate::styles::primary_button(tokens)).boxed()
    } else {
        b.style(crate::styles::outline_button(tokens)).boxed()
    }
}

/// The footer: a hairline above, then `parts` in a row that wraps.
pub(super) fn footer<'a>(
    parts: Vec<Element<'a, BoardPopoverEvent>>,
    tokens: &ThemeTokens,
) -> Element<'a, BoardPopoverEvent> {
    let mut wrap = iced_aw::Wrap::new()
        .spacing(CARD_PAD)
        .line_spacing(CARD_PAD)
        .align_items(iced::Alignment::Center);
    for part in parts {
        wrap = wrap.push(part);
    }
    footer_with(wrap.boxed(), tokens)
}

/// A footer laid out by its caller: one row whose first part takes the
/// free width (the Grid popover's Snap to grid).
pub(super) fn footer_row<'a>(
    parts: Vec<Element<'a, BoardPopoverEvent>>,
    tokens: &ThemeTokens,
) -> Element<'a, BoardPopoverEvent> {
    let line = Row::with_children(parts)
        .spacing(CARD_PAD)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill);
    footer_with(line.boxed(), tokens)
}

fn footer_with<'a>(
    content: Element<'a, BoardPopoverEvent>,
    tokens: &ThemeTokens,
) -> Element<'a, BoardPopoverEvent> {
    let line = resolve(tokens).line_soft;
    let rule =
        container(Space::new().width(Length::Fill).height(HAIRLINE)).style(move |_: &Theme| {
            container::Style {
                background: Some(Background::Color(line)),
                ..container::Style::default()
            }
        });
    column![
        Space::new().height(4.0 * PX),
        rule,
        container(content).padding(Padding {
            top: CARD_PAD,
            right: CARD_PAD,
            bottom: 2.0 * PX,
            left: CARD_PAD,
        }),
    ]
    .width(Length::Fill)
    .boxed()
}

/// The card of `model`, drawn for its kind.
pub(crate) fn popover_view<'a>(
    model: &BoardPopoverModel,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, BoardPopoverEvent> {
    match model {
        BoardPopoverModel::Layer(list) => super::layer_popover::view(list, tokens, i18n),
        BoardPopoverModel::LayerDropdown(groups) => {
            super::layer_dropdown::view(groups, tokens, i18n)
        }
        BoardPopoverModel::Grid(grid) => super::grid_popover::view(grid, tokens, i18n),
        BoardPopoverModel::Unrouted(list) => super::unrouted_popover::view(list, tokens, i18n),
        BoardPopoverModel::Drc(list) => super::drc_popover::view(list, tokens, i18n),
        BoardPopoverModel::Selection(list) => super::selection_popover::view(list, tokens, i18n),
    }
}

/// Width of each card, in the mock's px.
pub(crate) fn card_width(kind: super::BoardPopoverKind) -> f32 {
    use super::BoardPopoverKind as K;
    PX * match kind {
        K::Layer | K::ActiveLayer => 230.0,
        K::Grid | K::Selection => 300.0,
        K::Unrouted => 320.0,
        K::Drc => 330.0,
    }
}

/// Rows a list shows before it scrolls.
const LIST_ROWS: usize = 4;

/// A list of two-line rows: up to [`LIST_ROWS`] it takes their height, past
/// that it is [`LIST_MAX`] tall and scrolls.
pub(super) fn scrolling<'a>(
    rows: Vec<Element<'a, BoardPopoverEvent>>,
) -> Element<'a, BoardPopoverEvent> {
    let height = if rows.len() > LIST_ROWS {
        Length::Fixed(LIST_MAX)
    } else {
        Length::Shrink
    };
    iced::widget::scrollable(Column::with_children(rows).width(Length::Fill))
        .height(height)
        .direction(crate::styles::thin_vertical())
        .boxed()
}
