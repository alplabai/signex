//! The board segments of the bar (m164): Routed with its progress bar,
//! Unrouted, DRC with the age of the last run, and Selected, each a button
//! that opens its popover; the board's Grid read-out; and the layer pill's
//! swatch and chevron. Each segment's width is computed from the same texts
//! it draws, so a popover can be anchored above it (`anchor.rs`).

use iced::widget::{Row, Space, Widget, container, mouse_area, row};
use iced::{Background, Border, Color, Element, Length, Theme};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text};

use super::super::graphite::{SP, TS};
use super::kit::{
    BAR_GLYPH, Glyph, MONO, PX, SEGMENT_GAP, SEGMENT_PAD, SWATCH, Tone, badge, badge_width,
    count_text, glyph, ui_font,
};
use super::{BoardCounter, BoardReadout, ConnectionTally, DrcAge, DrcTally};
use crate::app::StatusBarRequest;
use crate::dock::measure::text_width_in;
use crate::styles::tokens::resolve;
use crate::styles::{HAIRLINE, ti};

/// The progress bar of the Routed counter.
const PROGRESS_W: f32 = 56.0 * PX;
const PROGRESS_H: f32 = 5.0 * PX;

/// The request a counter's click sends; the window is filled in by
/// `Signex::status_bar_message`.
fn clicked(counter: BoardCounter) -> StatusBarRequest {
    StatusBarRequest::BoardCounterClicked {
        window: None,
        counter,
    }
}

/// A layer's colour swatch: the mock's 9 px square with a 1 px line-strong
/// border, so a near-white layer shows on a light surface.
pub(in crate::status_bar) fn layer_swatch<'a, M: 'a>(
    colour: Color,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let edge = resolve(tokens).line_strong;
    container(Space::new())
        .width(SWATCH)
        .height(SWATCH)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(colour)),
            border: Border {
                width: HAIRLINE,
                color: edge,
                radius: (2.0 * PX).into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The chevron after the layer pill's name: it opens a popover.
pub(in crate::status_bar) fn layer_chevron<'a, M: 'a>(tokens: &ThemeTokens) -> Element<'a, M> {
    glyph(Glyph::Chevron, BAR_GLYPH, resolve(tokens).ink4)
}

/// The label of a counter, in the bar's caption size.
fn label<'a>(s: Localized, tokens: &ThemeTokens) -> Element<'a, StatusBarRequest> {
    text(s)
        .size(TS.caption)
        .color(resolve(tokens).ink3)
        .wrapping(iced::widget::text::Wrapping::None)
        .boxed()
}

/// A mono figure in the bar.
fn figure<'a>(s: Localized, tokens: &ThemeTokens) -> Element<'a, StatusBarRequest> {
    text(s)
        .size(MONO)
        .font(super::super::graphite::mono_font())
        .color(ti(tokens.text))
        .wrapping(iced::widget::text::Wrapping::None)
        .boxed()
}

fn label_width(s: &Localized) -> f32 {
    text_width_in(
        &s.to_string(),
        TS.caption,
        ui_font(iced::font::Weight::Normal),
    )
}

fn figure_width(s: &Localized) -> f32 {
    text_width_in(&s.to_string(), MONO, super::super::graphite::mono_font())
}

/// The value of the board's Grid read-out: the pitch in the bar's unit, or
/// OFF while the grid is hidden.
pub(crate) fn grid_value(
    i18n: &I18n,
    visible: bool,
    size_mm: f32,
    unit: signex_types::coord::Unit,
) -> Localized {
    if visible {
        let value = super::super::graphite::grid_value(size_mm, unit);
        t!(i18n, ids::BOARD_STATUS_VALUE, "value" => value)
    } else {
        t!(i18n, ids::BOARD_STATUS_GRID_OFF)
    }
}

/// The board's Grid read-out: the grid glyph, "Grid" and the mono value. A
/// click shows or hides the grid, as on a sheet; a right-click opens the
/// Grid popover.
pub(in crate::status_bar) fn board_grid_readout<'a>(
    value: Localized,
    open: bool,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, StatusBarRequest> {
    let muted = resolve(tokens).ink3;
    let face = row![
        glyph(Glyph::Grid, BAR_GLYPH, muted),
        label(t!(i18n, ids::BOARD_STATUS_GRID), tokens),
        figure(value, tokens),
    ]
    .spacing(SEGMENT_GAP)
    .align_y(iced::Alignment::Center);
    let readout = button(face)
        .padding([1.0, SP.s])
        .style(segment_style(tokens, open))
        .on_press(StatusBarRequest::ToggleGrid);
    mouse_area(readout)
        .on_right_press(clicked(BoardCounter::Grid))
        .boxed()
}

/// Width of [`board_grid_readout`] holding `value`.
pub(super) fn grid_readout_width(value: &Localized, i18n: &I18n) -> f32 {
    2.0 * SP.s
        + BAR_GLYPH
        + SEGMENT_GAP
        + label_width(&t!(i18n, ids::BOARD_STATUS_GRID))
        + SEGMENT_GAP
        + figure_width(value)
}

/// A counter's look: flat, the hover fill under the pointer, the selection
/// fill while its popover is open.
fn segment_style(
    tokens: &ThemeTokens,
    open: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let (hover, selected, ink) = (ti(tokens.hover), ti(tokens.selection), ti(tokens.text));
    move |_, status| button::Style {
        background: match (open, status) {
            (true, _) => Some(Background::Color(selected)),
            (false, button::Status::Hovered | button::Status::Pressed) => {
                Some(Background::Color(hover))
            }
            _ => None,
        },
        text_color: ink,
        ..button::Style::default()
    }
}

/// One counter of the bar: `parts` in a full-height button, then the faint
/// rule at its right unless it is the last.
fn segment<'a>(
    parts: Vec<Element<'a, StatusBarRequest>>,
    counter: BoardCounter,
    open: bool,
    last: bool,
    tokens: &ThemeTokens,
) -> Element<'a, StatusBarRequest> {
    let face = Row::with_children(parts)
        .spacing(SEGMENT_GAP)
        .align_y(iced::Alignment::Center);
    let b = button(container(face).center_y(Length::Fill))
        .padding([0.0, SEGMENT_PAD])
        .height(Length::Fill)
        .style(segment_style(tokens, open))
        .on_press(clicked(counter));
    if last {
        return b.boxed();
    }
    let line = resolve(tokens).line_soft;
    let rule = container(Space::new())
        .width(HAIRLINE)
        .height(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(line)),
            ..container::Style::default()
        });
    row![b, rule].height(Length::Fill).boxed()
}

/// Width of a counter holding parts of `inner` px with `gaps` gaps.
fn segment_width(inner: f32, gaps: usize, last: bool) -> f32 {
    let rule = if last { 0.0 } else { HAIRLINE };
    2.0 * SEGMENT_PAD + inner + gaps as f32 * SEGMENT_GAP + rule
}

/// The texts of the Routed counter: "Routed", "38/46" and "83%".
fn routed_texts(c: ConnectionTally, i18n: &I18n) -> [Localized; 3] {
    [
        t!(i18n, ids::BOARD_STATUS_ROUTED),
        t!(i18n, ids::BOARD_STATUS_ROUTED_COUNT, "routed" => c.routed, "total" => c.total),
        t!(i18n, ids::BOARD_STATUS_ROUTED_PERCENT, "percent" => c.percent()),
    ]
}

/// The progress bar: the line-strong track, the ok fill at `percent`.
fn progress<'a>(percent: usize, tokens: &ThemeTokens) -> Element<'a, StatusBarRequest> {
    let (track, fill) = (resolve(tokens).line_strong, ti(tokens.success));
    let done = PROGRESS_W * percent.min(100) as f32 / 100.0;
    let radius = iced::border::Radius::from(PROGRESS_H / 2.0 + 0.5);
    let bar = container(Space::new())
        .width(done)
        .height(PROGRESS_H)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                radius,
                ..Border::default()
            },
            ..container::Style::default()
        });
    container(bar)
        .width(PROGRESS_W)
        .height(PROGRESS_H)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(track)),
            border: Border {
                radius,
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The DRC counter's caption: how long ago the last run landed.
fn drc_age_text(drc: DrcTally, i18n: &I18n) -> Localized {
    match drc {
        DrcTally::NotRun => t!(i18n, ids::BOARD_STATUS_DRC_NOT_RUN),
        DrcTally::Ran {
            age: DrcAge::JustNow,
            ..
        } => t!(i18n, ids::BOARD_STATUS_DRC_JUST_NOW),
        DrcTally::Ran {
            age: DrcAge::Minutes(minutes),
            ..
        } => t!(i18n, ids::BOARD_STATUS_DRC_MINUTES_AGO, "minutes" => minutes),
    }
}

fn drc_violations(drc: DrcTally) -> usize {
    match drc {
        DrcTally::NotRun => 0,
        DrcTally::Ran { violations, .. } => violations,
    }
}

/// The counters of `b`, left to right, with the counter each one is.
pub(super) fn counter_list(b: &BoardReadout) -> Vec<BoardCounter> {
    let mut out = Vec::new();
    if b.connections.is_some() {
        out.extend([BoardCounter::Routed, BoardCounter::Unrouted]);
    }
    out.extend([BoardCounter::Drc, BoardCounter::Selected]);
    out
}

/// The board counters of the bar, left to right.
pub(in crate::status_bar) fn board_counters<'a>(
    b: &BoardReadout,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, StatusBarRequest> {
    let list = counter_list(b);
    let segments = list.iter().enumerate().map(|(i, &counter)| {
        let open = b.open == Some(counter.popover());
        let last = i + 1 == list.len();
        segment(
            counter_parts(counter, b, tokens, i18n),
            counter,
            open,
            last,
            tokens,
        )
    });
    Row::with_children(segments)
        .height(Length::Fill)
        .align_y(iced::Alignment::Center)
        .boxed()
}

/// What one counter holds.
fn counter_parts<'a>(
    counter: BoardCounter,
    b: &BoardReadout,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Vec<Element<'a, StatusBarRequest>> {
    let muted = resolve(tokens).ink3;
    let connections = b.connections.unwrap_or_default();
    match counter {
        BoardCounter::Routed => {
            let [name, count, percent] = routed_texts(connections, i18n);
            vec![
                glyph(Glyph::Route, BAR_GLYPH, ti(tokens.success)),
                label(name, tokens),
                figure(count, tokens),
                progress(connections.percent(), tokens),
                figure(percent, tokens),
            ]
        }
        BoardCounter::Unrouted => vec![
            glyph(Glyph::Ratsnest, BAR_GLYPH, muted),
            label(t!(i18n, ids::BOARD_STATUS_UNROUTED), tokens),
            badge(connections.unrouted, Tone::Warning, tokens, i18n),
        ],
        BoardCounter::Drc => {
            let violations = drc_violations(b.drc);
            let ink = if violations > 0 {
                ti(tokens.error)
            } else {
                muted
            };
            vec![
                glyph(Glyph::Error, BAR_GLYPH, ink),
                label(t!(i18n, ids::BOARD_STATUS_DRC), tokens),
                badge(violations, Tone::Error, tokens, i18n),
                label(drc_age_text(b.drc, i18n), tokens),
            ]
        }
        BoardCounter::Selected => vec![
            glyph(Glyph::Selection, BAR_GLYPH, muted),
            label(t!(i18n, ids::BOARD_STATUS_SELECTED), tokens),
            badge(b.selected, Tone::Accent, tokens, i18n),
        ],
        // The pill and the Grid read-out are drawn by the w3a clusters.
        BoardCounter::Layer | BoardCounter::Grid => Vec::new(),
    }
}

/// Width of each counter of `b`, in [`counter_list`] order.
pub(super) fn counter_widths(b: &BoardReadout, i18n: &I18n) -> Vec<(BoardCounter, f32)> {
    let list = counter_list(b);
    let connections = b.connections.unwrap_or_default();
    let badge_of = |n: usize| badge_width(&count_text(i18n, n).to_string());
    list.iter()
        .enumerate()
        .map(|(i, &counter)| {
            let last = i + 1 == list.len();
            let width = match counter {
                BoardCounter::Routed => {
                    let [name, count, percent] = routed_texts(connections, i18n);
                    let inner = BAR_GLYPH
                        + label_width(&name)
                        + figure_width(&count)
                        + PROGRESS_W
                        + figure_width(&percent);
                    segment_width(inner, 4, last)
                }
                BoardCounter::Unrouted => {
                    let name = t!(i18n, ids::BOARD_STATUS_UNROUTED);
                    let inner = BAR_GLYPH + label_width(&name) + badge_of(connections.unrouted);
                    segment_width(inner, 2, last)
                }
                BoardCounter::Drc => {
                    let name = t!(i18n, ids::BOARD_STATUS_DRC);
                    let age = drc_age_text(b.drc, i18n);
                    let inner = BAR_GLYPH
                        + label_width(&name)
                        + badge_of(drc_violations(b.drc))
                        + label_width(&age);
                    segment_width(inner, 3, last)
                }
                BoardCounter::Selected => {
                    let name = t!(i18n, ids::BOARD_STATUS_SELECTED);
                    let inner = BAR_GLYPH + label_width(&name) + badge_of(b.selected);
                    segment_width(inner, 2, last)
                }
                BoardCounter::Layer | BoardCounter::Grid => 0.0,
            };
            (counter, width)
        })
        .collect()
}

#[cfg(test)]
pub(super) fn drc_caption(drc: DrcTally, i18n: &I18n) -> Localized {
    drc_age_text(drc, i18n)
}
