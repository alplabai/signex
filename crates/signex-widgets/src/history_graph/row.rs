//! One row beside the graph column: subject and pills over a caption line
//! of author disc, author, age and short SHA. The working-tree row has the
//! same shape with a Commit… button in place of the SHA.

use chrono::{DateTime, Utc};
use iced::border::Radius;
use iced::widget::text::{Ellipsis, Wrapping};
use iced::widget::{Row, Space, column, container, row};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Padding, Theme, Widget};
use signex_i18n::{I18n, Localized, UserText, ids, t};

use super::model::{GraphCommit, RefPill, initials};
use super::style::{
    AVATAR_EM, AVATAR_INK, AVATAR_TEXT_EM, CAPTION_GAP_EM, COMMIT_BUTTON_EM, HistoryGraphStyle,
    PILL_HEIGHT_EM, PILL_PAD_EM, PILL_RADIUS_EM, PILL_TEXT_EM, ROW_EM, ROW_LINE_GAP_EM, ROW_PAD_EM,
    SELECTED_BAR_EM, SMALL_BUTTON_PAD_EM, TITLE_GAP_EM,
};
use crate::history_pane::{Age, relative_age, short_sha};
use crate::theme_ext::{HAIRLINE, dimmed};
use crate::{Label, button, text};

/// What every row of one graph shares.
pub(super) struct RowContext<'r> {
    pub style: HistoryGraphStyle,
    pub i18n: &'r I18n,
    pub now: DateTime<Utc>,
}

/// "12m ago" for `time`, in the locale of `i18n`.
pub fn relative_time(i18n: &I18n, time: DateTime<Utc>, now: DateTime<Utc>) -> Localized {
    let (id, count) = match relative_age(time, now) {
        Age::Seconds(n) => (ids::PANELS_HISTORY_AGO_SECONDS, n),
        Age::Minutes(n) => (ids::PANELS_HISTORY_AGO_MINUTES, n),
        Age::Hours(n) => (ids::PANELS_HISTORY_AGO_HOURS, n),
        Age::Days(n) => (ids::PANELS_HISTORY_AGO_DAYS, n),
        Age::Months(n) => (ids::PANELS_HISTORY_AGO_MONTHS, n),
        Age::Years(n) => (ids::PANELS_HISTORY_AGO_YEARS, n),
    };
    // The number goes in as written, as the History panel's cards do.
    t!(i18n, id, "count" => count.to_string())
}

fn semibold() -> Font {
    Font {
        weight: iced::font::Weight::Semibold,
        ..crate::ui_font::regular()
    }
}

/// A one-line text cut with an ellipsis.
fn line<'a>(label: impl Into<Label<'a>>, size: f32, ink: Color) -> iced::widget::Text<'a> {
    text(label)
        .size(size)
        .color(ink)
        .wrapping(Wrapping::None)
        .ellipsis(Ellipsis::End)
}

/// The shell every row shares: fixed height, the selection bar slot at the
/// left and the two lines centred.
fn shell<'a, M: 'a>(
    s: &HistoryGraphStyle,
    first: Element<'a, M>,
    second: Element<'a, M>,
    is_selected: bool,
) -> Element<'a, M> {
    let bar_width = s.em(SELECTED_BAR_EM);
    let radius = s.radii.button;
    let accent = s.accent;
    let bar =
        container(Space::new().width(bar_width).height(Length::Fill)).style(move |_: &Theme| {
            container::Style {
                background: is_selected.then_some(Background::Color(accent)),
                border: Border {
                    radius: Radius {
                        top_left: radius,
                        bottom_left: radius,
                        ..Radius::default()
                    },
                    ..Border::default()
                },
                ..container::Style::default()
            }
        });
    let (pad_v, pad_h) = (s.em(ROW_PAD_EM.0), s.em(ROW_PAD_EM.1));
    let lines = container(column![first, second].spacing(s.em(ROW_LINE_GAP_EM)))
        .width(Length::Fill)
        .height(Length::Fill)
        .align_y(Alignment::Center)
        .padding(Padding {
            top: pad_v,
            bottom: pad_v,
            left: pad_h - bar_width,
            right: pad_h,
        });
    row![bar, lines]
        .width(Length::Fill)
        .height(s.em(ROW_EM))
        .boxed()
}

/// A commit row. `on_press` is the caller's select message for it.
pub(super) fn commit_row<'a, M: Clone + 'a>(
    ctx: &RowContext<'_>,
    commit: &GraphCommit,
    lane: usize,
    is_selected: bool,
    on_press: Option<M>,
) -> Element<'a, M> {
    let s = ctx.style;
    let subject_ink = if commit.is_merge() { s.ink3 } else { s.ink };
    let mut first: Row<Element<'a, M>> = Row::new()
        .spacing(s.em(TITLE_GAP_EM))
        .align_y(Alignment::Center)
        .push(
            line(
                UserText::new(commit.subject.clone()),
                s.text_size,
                subject_ink,
            )
            .width(Length::Fill)
            .boxed(),
        );
    for pill in &commit.refs {
        first = first.push(ref_pill(ctx, pill, lane));
    }

    let age = relative_time(ctx.i18n, commit.time, ctx.now).into_string();
    let byline = t!(ctx.i18n, ids::PANELS_HISTORY_BYLINE,
        "author" => commit.author_name.clone(), "age" => age);
    let second = row![
        avatar(&s, &commit.author_name, &commit.author_email),
        line(byline, s.caption_size, s.ink4),
        Space::new().width(Length::Fill),
        text(UserText::new(short_sha(&commit.sha)))
            .size(s.caption_size)
            .font(Font::MONOSPACE)
            .color(s.ink4)
            .wrapping(Wrapping::None),
    ]
    .spacing(s.em(CAPTION_GAP_EM))
    .align_y(Alignment::Center);

    let clickable = on_press.is_some();
    let hover = s.hover;
    button(shell(&s, first.boxed(), second.boxed(), is_selected))
        .width(Length::Fill)
        .padding(Padding::ZERO)
        .on_press_maybe(on_press)
        .style(move |_: &Theme, status: button::Status| {
            let hot =
                clickable && matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: (is_selected || hot).then_some(Background::Color(hover)),
                text_color: subject_ink,
                border: Border {
                    radius: s.radii.button.into(),
                    ..Border::default()
                },
                ..button::Style::default()
            }
        })
        .boxed()
}

/// The uncommitted-changes row above the commits, with the caller's
/// Commit… message (none draws the button disabled).
pub(super) fn working_tree_row<'a, M: Clone + 'a>(
    ctx: &RowContext<'_>,
    on_commit: Option<M>,
) -> Element<'a, M> {
    let s = ctx.style;
    let first = line(
        t!(ctx.i18n, ids::PANELS_HISTORY_UNCOMMITTED),
        s.text_size,
        s.ink3,
    )
    .width(Length::Fill)
    .boxed();
    let second = row![
        line(
            t!(ctx.i18n, ids::PANELS_HISTORY_WORKING_TREE_CAPTION),
            s.caption_size,
            s.ink4
        ),
        Space::new().width(Length::Fill),
        small_button(&s, t!(ctx.i18n, ids::PANELS_HISTORY_COMMIT), on_commit),
    ]
    .spacing(s.em(CAPTION_GAP_EM))
    .align_y(Alignment::Center);
    shell(&s, first, second.boxed(), false)
}

/// The small outlined button of the History panel: the working tree's
/// Commit… and the detail strip's actions. `on_press` none draws it
/// disabled.
pub fn small_button<'a, M: Clone + 'a>(
    s: &HistoryGraphStyle,
    label: impl Into<Label<'a>>,
    on_press: Option<M>,
) -> Element<'a, M> {
    let s = *s;
    let enabled = on_press.is_some();
    button(
        container(text(label).size(s.caption_size).wrapping(Wrapping::None))
            .height(Length::Fill)
            .align_y(Alignment::Center),
    )
    .height(s.em(COMMIT_BUTTON_EM))
    .padding(Padding::from([0.0, s.em(SMALL_BUTTON_PAD_EM)]))
    .on_press_maybe(on_press)
    .style(move |_: &Theme, status: button::Status| {
        let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
        let fill = if enabled && hot { s.hover } else { s.field };
        let ink = if enabled { s.ink } else { dimmed(s.ink) };
        button::Style {
            background: Some(Background::Color(fill)),
            text_color: ink,
            border: Border {
                color: s.border,
                width: HAIRLINE,
                radius: s.radii.small.into(),
                ..Border::default()
            },
            ..button::Style::default()
        }
    })
    .boxed()
}

/// The author's initials on a disc hashed from the e-mail.
fn avatar<'a, M: 'a>(s: &HistoryGraphStyle, name: &str, email: &str) -> Element<'a, M> {
    let side = s.em(AVATAR_EM);
    let fill = HistoryGraphStyle::avatar_fill(email);
    container(
        text(UserText::new(initials(name, email)))
            .size(s.em(AVATAR_TEXT_EM))
            .font(semibold())
            .color(AVATAR_INK)
            .wrapping(Wrapping::None),
    )
    .center(side)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            radius: (side / 2.0).into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

/// Ink, fill and edge of a pill on a commit in `lane`.
pub(super) fn pill_colors(
    s: &HistoryGraphStyle,
    pill: &RefPill,
    lane: usize,
) -> (Color, Option<Color>, Color) {
    match pill {
        RefPill::Head => (s.ink3, Some(s.sunken), Color::TRANSPARENT),
        RefPill::Branch { is_current, .. } => {
            let c = if *is_current {
                s.accent
            } else {
                s.lane_color(lane)
            };
            (c, None, c)
        }
        RefPill::Tag { .. } => (s.ink3, Some(s.sunken), s.border),
    }
}

/// A branch, tag or HEAD pill.
fn ref_pill<'a, M: 'a>(ctx: &RowContext<'_>, pill: &RefPill, lane: usize) -> Element<'a, M> {
    let s = ctx.style;
    let (label, size, radius): (Label<'a>, f32, f32) = match pill {
        // HEAD keeps the panel's small pill look: caption text, round ends.
        RefPill::Head => (
            t!(ctx.i18n, ids::PANELS_HISTORY_HEAD).into(),
            s.caption_size,
            s.radii.pill,
        ),
        RefPill::Branch { name, .. } | RefPill::Tag { name } => (
            UserText::new(name.clone()).into(),
            s.em(PILL_TEXT_EM),
            s.em(PILL_RADIUS_EM),
        ),
    };
    let (ink, fill, edge) = pill_colors(&s, pill, lane);
    container(text(label).size(size).color(ink).wrapping(Wrapping::None))
        .height(s.em(PILL_HEIGHT_EM))
        .align_y(Alignment::Center)
        .padding(Padding::from([0.0, s.em(PILL_PAD_EM)]))
        .style(move |_: &Theme| container::Style {
            background: fill.map(Background::Color),
            border: Border {
                color: edge,
                width: HAIRLINE,
                radius: radius.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}
