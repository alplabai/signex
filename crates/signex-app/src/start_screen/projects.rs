//! Project rows and Home's project lists: Pinned Projects (#2305), shown
//! only when something is pinned, then Recent Projects (at most
//! [`super::MAX_START_RECENT`], with "Show all" when there are more). A row
//! is the project's thumbnail tile, its name and path, when it was last
//! opened and, on Home, Pin / Unpin on hover. Each row opens its project
//! and opens a menu on right-click; a pinned project missing on disk says
//! so and keeps Remove in view. Recent Projects is hidden when every recent
//! project is pinned.

use std::path::Path;

use chrono::{DateTime, TimeZone, Utc};
use iced::widget::{column, container, hover, mouse_area, row};
use iced::{Alignment, Background, Border, Element, Length, Widget as _};
use signex_i18n::{DateLength, I18n, Localized, MessageId, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text};

use super::kit::{
    GLYPH_THEME, Look, Size, action_button, empty_state, icon, list_card, mono, pill,
    reserved_button, section_header_with, weighted,
};
use super::{ProjectSections, StartPage, StartScreenMsg, message, recent_label, recent_message};
use crate::app::Message;
use crate::pinned_projects::PinnedProjects;
use crate::project_thumbnail::ProjectThumbnails;
use crate::recent_projects::RecentProjects;
use crate::styles::{
    PillTone, Radii, Spacing, TypeScale, ti,
    tokens::{hover_overlay, resolve},
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
/// Row padding (9 x 14 at the default size).
const ROW_PAD: [f32; 2] = [SP.m, SP.l];
/// Width of the last-opened column (84 px at the default size).
pub(super) const WHEN_W: f32 = TS.body * 7.0;

/// Which list a row is in, and for a pinned row whether it is missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RowKind {
    Recent,
    Pinned { missing: bool },
}

impl RowKind {
    pub(super) fn of(path: &Path, pinned: &PinnedProjects) -> Self {
        if pinned.contains(path) {
            Self::Pinned {
                missing: pinned.is_missing(path),
            }
        } else {
            Self::Recent
        }
    }

    fn is_missing(self) -> bool {
        self == Self::Pinned { missing: true }
    }
}

/// What every project row reads.
#[derive(Clone, Copy)]
pub(super) struct Rows<'a> {
    pub tokens: &'a ThemeTokens,
    pub i18n: &'a I18n,
    pub recent: &'a RecentProjects,
    pub thumbnails: &'a ProjectThumbnails,
    pub now: DateTime<Utc>,
}

/// When a project was last opened, as its row shows it: "today 09:12",
/// "yesterday", an earlier day in the locale's medium date, or "—" when
/// it is not known. Read on the operating system's local clock.
pub fn opened_label(i18n: &I18n, opened_at: Option<i64>, now: DateTime<Utc>) -> Localized {
    opened_label_in(i18n, opened_at, now, &chrono::Local)
}

pub(super) fn opened_label_in<Z: TimeZone>(
    i18n: &I18n,
    opened_at: Option<i64>,
    now: DateTime<Utc>,
    zone: &Z,
) -> Localized
where
    Z::Offset: std::fmt::Display,
{
    let Some(at) = opened_at.and_then(|secs| DateTime::<Utc>::from_timestamp(secs, 0)) else {
        return t!(i18n, ids::START_OPENED_UNKNOWN);
    };
    let local = at.with_timezone(zone);
    let today = now.with_timezone(zone).date_naive();
    let day = local.date_naive();
    if day == today {
        // The wall clock as the Messages panel shows it, to the minute.
        t!(i18n, ids::START_OPENED_TODAY, "time" => local.format("%H:%M").to_string())
    } else if today.pred_opt() == Some(day) {
        t!(i18n, ids::START_OPENED_YESTERDAY)
    } else {
        t!(i18n, ids::START_OPENED_DATE, "date" => i18n.date(at, DateLength::Medium))
    }
}

/// Home's section headings and lists, in screen order. `listed` is how
/// many projects the Projects page lists ("Show all N").
pub(super) fn home_sections<'a>(
    rows: Rows<'a>,
    sections: &ProjectSections<'_>,
    pinned: &PinnedProjects,
    listed: usize,
) -> Vec<Element<'a, Message>> {
    let (tokens, i18n) = (rows.tokens, rows.i18n);
    let mut out = Vec::with_capacity(4);
    if !sections.pinned.is_empty() {
        out.push(section_header_with(
            tokens,
            i18n,
            t!(i18n, ids::START_PINNED_PROJECTS),
            sections.pinned.len(),
            None,
        ));
        let list = sections
            .pinned
            .iter()
            .map(|path| project_row(rows, path, RowKind::of(path, pinned), true))
            .collect();
        out.push(list_card(tokens, list));
    }
    // Every recent project is pinned: an empty Recent section would claim
    // there are none.
    if !sections.show_recent {
        return out;
    }
    let show_all = sections.has_more_recent().then(|| {
        action_button(
            tokens,
            (Look::Ghost, Size::Small),
            None,
            t!(i18n, ids::START_SHOW_ALL, "count" => listed),
            Some(message(StartScreenMsg::PageSelected(StartPage::Projects))),
        )
    });
    out.push(section_header_with(
        tokens,
        i18n,
        t!(i18n, ids::START_RECENT_PROJECTS),
        sections.recent_total,
        show_all,
    ));
    if sections.recent.is_empty() {
        out.push(crate::panels::empty_state_card(
            tokens,
            t!(i18n, ids::START_NO_RECENT_TITLE),
            t!(i18n, ids::START_NO_RECENT_BODY),
            None,
        ));
    } else {
        let list = sections
            .recent
            .iter()
            .map(|path| project_row(rows, path, RowKind::Recent, true))
            .collect();
        out.push(list_card(tokens, list));
    }
    out
}

/// A list card of `rows`, or the empty state when there are none.
pub(super) fn list_or_empty<'a>(
    tokens: &ThemeTokens,
    rows: Vec<Element<'a, Message>>,
    empty: impl FnOnce() -> Element<'a, Message>,
) -> Element<'a, Message> {
    if rows.is_empty() {
        list_card(tokens, vec![empty()])
    } else {
        list_card(tokens, rows)
    }
}

/// The empty state of a filtered list.
pub(super) fn no_match<'a>(
    tokens: &ThemeTokens,
    i18n: &I18n,
    title: MessageId,
    query: &str,
) -> Element<'a, Message> {
    empty_state(
        tokens,
        i18n,
        (
            t!(i18n, title, "query" => query.to_owned()),
            t!(i18n, ids::START_FILTER_NO_MATCH_BODY),
        ),
        true,
    )
}

/// One project row. `hover_action` adds Pin / Unpin on hover (Home); a
/// missing pinned project always shows Remove.
pub(super) fn project_row<'a>(
    rows: Rows<'a>,
    path: &Path,
    kind: RowKind,
    hover_action: bool,
) -> Element<'a, Message> {
    let (tokens, i18n) = (rows.tokens, rows.i18n);
    let (ink, ink4) = (ti(tokens.text), resolve(tokens).ink4);
    let missing = kind.is_missing();
    let tile = crate::project_thumbnail::tile(tokens, i18n, rows.thumbnails.slot(path, missing));
    let details = column![
        name_line(tokens, i18n, path, kind),
        // A long path ends in an ellipsis before the time column.
        text(UserText::new(path.display().to_string()))
            .size(TS.caption)
            .font(mono())
            .color(ink4)
            .wrapping(iced::widget::text::Wrapping::None)
            .ellipsis(iced::widget::text::Ellipsis::End)
            .width(Length::Fill),
    ]
    .spacing(SP.xs)
    .width(Length::Fill);
    let opened = if missing {
        t!(i18n, ids::START_OPENED_UNKNOWN)
    } else {
        opened_label(i18n, rows.recent.opened_at(path), rows.now)
    };
    let when = text(opened)
        .size(TS.caption)
        .color(ink4)
        .wrapping(iced::widget::text::Wrapping::None)
        .width(WHEN_W)
        .align_x(iced::alignment::Horizontal::Right);
    let mut entry = row![tile, details.boxed(), when.boxed()]
        .spacing(SP.l)
        .align_y(Alignment::Center);
    let (label, glyph, event) = row_action(kind, path);
    let look = (Look::Outline, Size::Small);
    // A missing project keeps Remove in view; the inner button takes the
    // press before the row.
    let hover_button = if missing {
        entry = entry.push(action_button(
            tokens,
            look,
            glyph,
            t!(i18n, label),
            Some(message(event)),
        ));
        None
    } else if hover_action {
        // The room is kept while the button is hidden, so the date never
        // shifts. A recent row keeps its pin there, in the date's ink.
        entry = entry.push(reserved_button(
            tokens,
            look,
            glyph.clone(),
            t!(i18n, label),
            (kind == RowKind::Recent).then_some(ink4),
        ));
        Some(action_button(
            tokens,
            look,
            glyph,
            t!(i18n, label),
            Some(message(event)),
        ))
    } else {
        None
    };
    let open = match kind {
        RowKind::Recent => recent_message(path),
        RowKind::Pinned { .. } => message(StartScreenMsg::PinnedProjectClicked(path.to_path_buf())),
    };
    let hover_fill = hover_overlay(tokens);
    let base = button(entry)
        .width(Length::Fill)
        .padding(ROW_PAD)
        .on_press(open)
        .style(move |_: &iced::Theme, status| button::Style {
            background: matches!(status, button::Status::Hovered | button::Status::Pressed)
                .then_some(Background::Color(hover_fill)),
            text_color: ink,
            border: Border {
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..button::Style::default()
        });
    let base = mouse_area(base)
        .on_right_press(message(StartScreenMsg::RowRightClicked(path.to_path_buf())))
        .boxed();
    let Some(action) = hover_button else {
        return base;
    };
    let action = container(action)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding([0.0, ROW_PAD[1]])
        .align_x(iced::alignment::Horizontal::Right)
        .align_y(iced::alignment::Vertical::Center);
    hover(base, action).boxed()
}

/// The name on one line (an ellipsis when long), the pin mark of a pinned
/// project, or the "Missing on disk" pill, which drops below the name when
/// the row is too narrow for both.
fn name_line<'a>(
    tokens: &ThemeTokens,
    i18n: &I18n,
    path: &Path,
    kind: RowKind,
) -> Element<'a, Message> {
    let missing = kind.is_missing();
    let ink = if missing {
        resolve(tokens).ink3
    } else {
        ti(tokens.text)
    };
    let mut line = row![
        text(UserText::new(recent_label(path)))
            .size(TS.label)
            .font(weighted(iced::font::Weight::Medium))
            .color(ink)
            .wrapping(iced::widget::text::Wrapping::None)
            .ellipsis(iced::widget::text::Ellipsis::End)
            .boxed()
    ]
    .spacing(SP.s + SP.xs / 2.0)
    .align_y(Alignment::Center);
    if missing {
        line = line.push(pill(
            tokens,
            t!(i18n, ids::START_PROJECT_MISSING),
            PillTone::Err,
        ));
    } else if kind != RowKind::Recent {
        line = line.push(icon(
            crate::icons::icon_start_pin(GLYPH_THEME),
            TS.body,
            resolve(tokens).accent_text,
        ));
    }
    line.wrap().vertical_spacing(SP.xs).boxed()
}

/// The row's own action: its label, glyph and the event it sends.
fn row_action(
    kind: RowKind,
    path: &Path,
) -> (MessageId, Option<iced::widget::svg::Handle>, StartScreenMsg) {
    let path = path.to_path_buf();
    let pin = Some(crate::icons::icon_start_pin(GLYPH_THEME));
    match kind {
        RowKind::Recent => (ids::START_PIN, pin, StartScreenMsg::ProjectPinned(path)),
        RowKind::Pinned { missing: false } => {
            (ids::START_UNPIN, pin, StartScreenMsg::ProjectUnpinned(path))
        }
        RowKind::Pinned { missing: true } => (
            ids::START_REMOVE,
            None,
            StartScreenMsg::ProjectUnpinned(path),
        ),
    }
}
