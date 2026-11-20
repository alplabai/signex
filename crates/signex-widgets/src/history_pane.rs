//! Reusable per-primitive git history pane.
//!
//! Per `v0.9-snxlib-as-file-plan.md` §3 ("History panel inside the
//! per-primitive editor"), every primitive editor (SCH Library,
//! Footprint, Sim) and the Library Browser tab binds the same
//! widget to its current selection. Stage 17 of that plan asks for
//! a *scaffold*: a simple text-only column of cards listing
//! commits, no graph lane / diff tabs / filter UI / right-click
//! menu yet — all of those are follow-up polish stages.
//!
//! The widget is a **builder function** rather than a custom
//! `iced::Widget` impl: it returns an `Element` composed from stock
//! `column!` / `text` / `container` primitives so the shape is easy
//! to extend incrementally as later stages add affordances.
//!
//! The struct definition lives in `signex_library::HistoryEntry` so
//! the trait method that produces it (`LibraryAdapter::history`)
//! and the widget that consumes it agree on the data shape without
//! crossing a circular dep — `signex-widgets` depends on
//! `signex-types` only, so we re-declare the *shape* of an entry
//! locally as [`HistoryEntry`] and let callers convert at the
//! boundary. Same reason `tab_pill::TabPillStyle` exists separate
//! from any "real" tab type.

use crate::{raw, text};
use chrono::{DateTime, Duration, Utc};
use iced::widget::{Column, Space, Widget, column, container};
use iced::{Border, Element, Length};
use signex_types::theme::ThemeTokens;

use crate::theme_ext;

/// Plain-data view of one commit row, decoupled from
/// `signex_library::HistoryEntry` so the widget crate doesn't pull
/// the library crate as a dep.
///
/// The library crate's [`signex_library::HistoryEntry`] is the
/// canonical source — callers convert via `From<&_>` at the
/// boundary (see the `From` impl wired up by callers when both
/// crates are in scope).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryEntry {
    /// Full or short commit SHA.
    pub sha: String,
    /// `Author Name` from `git log`.
    pub author_name: String,
    /// `author@example.com` — used by the (later) avatar disc to
    /// derive its hashed colour.
    pub author_email: String,
    /// Author timestamp.
    pub time: DateTime<Utc>,
    /// First line of the commit message.
    pub subject: String,
}

/// Build a column-of-cards rendering of `entries` for the SCH
/// Library / Footprint / Sim editor's right-side history pane.
///
/// Layout per row (Stage 17 — text only):
///
/// ```text
/// ┌───────────────────────────────┐
/// │ alpCaner            12m ago   │  ← author + relative time
/// │ fix pin 4 typo                │  ← subject
/// │ a3bbcc6                       │  ← short SHA (muted)
/// └───────────────────────────────┘
/// ```
///
/// Empty `entries` renders a single muted "No history yet." card so
/// fresh libraries don't show an empty void. `now` is parameterised
/// to keep the relative-time math testable; production callers pass
/// `Utc::now()`.
pub fn history_pane<'a, M>(
    entries: &[HistoryEntry],
    now: DateTime<Utc>,
    tokens: &ThemeTokens,
) -> Element<'a, M>
where
    M: 'a,
{
    if entries.is_empty() {
        return empty_pane(tokens);
    }

    let muted = theme_ext::text_secondary(tokens);
    let primary = theme_ext::text_primary(tokens);
    let border = theme_ext::border_color(tokens);
    let panel = theme_ext::panel_bg(tokens);

    let mut col: Column<Element<'a, M>> = column![].spacing(6);

    for entry in entries {
        let header = iced::widget::row![
            text(signex_i18n::UserText::new(entry.author_name.clone()))
                .size(12)
                .color(primary),
            Space::new().width(Length::Fill),
            text(raw(format_relative(entry.time, now)))
                .size(11)
                .color(muted),
        ]
        .spacing(8);

        let card_inner = column![
            header,
            text(signex_i18n::UserText::new(entry.subject.clone()))
                .size(13)
                .color(primary),
            text(signex_i18n::UserText::new(short_sha(&entry.sha)))
                .size(11)
                .color(muted),
        ]
        .spacing(2);

        let bg = panel.background;
        let card = container(card_inner)
            .padding([6, 8])
            .width(Length::Fill)
            .style(move |_theme: &iced::Theme| container::Style {
                background: bg,
                text_color: Some(primary),
                border: Border::default().width(1.0).rounded(3.0).color(border),
                ..container::Style::default()
            });
        col = col.push(card.boxed());
    }

    col.boxed()
}

fn empty_pane<'a, M>(tokens: &ThemeTokens) -> Element<'a, M>
where
    M: 'a,
{
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);
    let bg = theme_ext::panel_bg(tokens).background;
    container(text(raw("No history yet.")).size(12).color(muted))
        .padding([6, 8])
        .width(Length::Fill)
        .style(move |_theme: &iced::Theme| container::Style {
            background: bg,
            text_color: Some(muted),
            border: Border::default().width(1.0).rounded(3.0).color(border),
            ..container::Style::default()
        })
        .boxed()
}

/// Trim a SHA to the seven-char shorthand `git log --oneline` uses.
/// Anything shorter than 7 chars (e.g. a test stub) is returned
/// untouched. LO-4 considered: returning `&str` here would force the
/// iced `text()` widget to capture the entry's lifetime, which doesn't
/// compose with the row-builder iterator's per-item ownership. The
/// allocation is paid anyway when the widget materialises the string.
pub(crate) fn short_sha(sha: &str) -> String {
    if sha.len() >= 7 {
        sha[..7].to_string()
    } else {
        sha.to_string()
    }
}

/// How long ago something happened, in the largest whole unit: the one
/// bucketing rule of the widget crate (this pane and the history graph).
/// Future times (clock skew, a badly-set author date) count as zero seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Age {
    Seconds(i64),
    Minutes(i64),
    Hours(i64),
    Days(i64),
    Months(i64),
    Years(i64),
}

pub(crate) fn relative_age(time: DateTime<Utc>, now: DateTime<Utc>) -> Age {
    let diff = now.signed_duration_since(time);
    if diff < Duration::seconds(60) {
        return Age::Seconds(diff.num_seconds().max(0));
    }
    if diff < Duration::minutes(60) {
        return Age::Minutes(diff.num_minutes());
    }
    if diff < Duration::hours(24) {
        return Age::Hours(diff.num_hours());
    }
    if diff < Duration::days(30) {
        return Age::Days(diff.num_days());
    }
    if diff < Duration::days(365) {
        return Age::Months(diff.num_days() / 30);
    }
    Age::Years(diff.num_days() / 365)
}

/// Render `time` as a Slack/GitHub-style relative timestamp ("12m
/// ago", "3h ago", "5d ago", "2mo ago", "3y ago"). Under a minute, and
/// future times, read "just now" rather than a misleading figure.
fn format_relative(time: DateTime<Utc>, now: DateTime<Utc>) -> String {
    match relative_age(time, now) {
        Age::Seconds(_) => "just now".into(),
        Age::Minutes(n) => format!("{n}m ago"),
        Age::Hours(n) => format!("{n}h ago"),
        Age::Days(n) => format!("{n}d ago"),
        Age::Months(n) => format!("{n}mo ago"),
        Age::Years(n) => format!("{n}y ago"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_time_buckets() {
        let now = Utc::now();
        assert_eq!(format_relative(now, now), "just now");
        assert_eq!(
            format_relative(now - Duration::seconds(45), now),
            "just now"
        );
        assert_eq!(format_relative(now - Duration::minutes(12), now), "12m ago");
        assert_eq!(format_relative(now - Duration::hours(3), now), "3h ago");
        assert_eq!(format_relative(now - Duration::days(5), now), "5d ago");
        assert_eq!(format_relative(now - Duration::days(60), now), "2mo ago");
        assert_eq!(format_relative(now - Duration::days(800), now), "2y ago");
    }

    #[test]
    fn short_sha_trims_to_seven() {
        assert_eq!(short_sha("a3bbcc6abc1234"), "a3bbcc6");
        assert_eq!(short_sha("abc"), "abc");
    }

    #[test]
    fn future_time_is_just_now() {
        // Clock skew: an author timestamp slightly in the future
        // shouldn't render "-2m ago".
        let now = Utc::now();
        assert_eq!(format_relative(now + Duration::seconds(5), now), "just now");
    }
}
