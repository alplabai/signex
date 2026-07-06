//! Git history panel — right-dock surface that follows the active
//! tab. Reuses [`signex_widgets::history_pane`] to render the
//! actual cards. State is kept minimal: an active path resolved
//! from the active tab + the last loaded vec of entries + a
//! "loading" generation counter to discard stale async results.
//!
//! Phase 0 (this module) wires the read-only view; the load path
//! is driven by `signex_app`'s dispatcher via
//! [`crate::app::HistoryLoad`] (a `Message::HistoryLoaded` variant
//! threads each result back to the panel context with a generation
//! token, so a tab switch in flight discards any pending result).

use chrono::Utc;
use iced::widget::{Column, button, column, container, scrollable, text};
use iced::{Border, Color, Element, Length, Theme};
use signex_widgets::theme_ext;

use super::{PanelContext, PanelMsg};

/// In-flight render state for the History panel. Owned by
/// [`crate::app::DocumentState`] and projected into
/// [`super::PanelContext`] each refresh.
#[derive(Debug, Clone, Default)]
pub struct HistoryPanelState {
    /// Monotonic counter — bumped every time the active tab changes
    /// (or the panel re-resolves its target path). Each in-flight
    /// async load tags itself with the current generation; results
    /// arriving after the counter has moved on are dropped.
    pub generation: u32,
    /// The path the most-recent load was issued for — `None` means
    /// no active file or the active tab has no history-trackable
    /// resource (e.g. a future ComponentEditor tab).
    pub active_path: Option<std::path::PathBuf>,
    /// Whether the most-recent load is still in flight.
    pub loading: bool,
    /// Loaded entries from the most-recent successful load.
    /// Newest-first per
    /// [`signex_library::project_file_history`]. Empty when the
    /// path has no history yet.
    pub entries: Vec<signex_widgets::HistoryEntry>,
    /// Render mode for the active load. Distinguishes "not in a git
    /// repo" (NoRepo) from "no commits yet" (entries.is_empty()).
    pub mode: HistoryRenderMode,
    /// True when the active path has uncommitted edits in the
    /// working tree (the engine reports it via `dirty_paths`).
    /// Drives the "Working tree (uncommitted changes)" pseudo-card.
    pub dirty: bool,
}

/// What the panel should render when `entries` is non-empty
/// doesn't unambiguously cover the case (e.g. the file isn't even
/// version-controlled).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HistoryRenderMode {
    /// No active file at all (no tabs, ComponentEditor tab, etc.).
    #[default]
    NoActiveFile,
    /// Active file exists but no `.git/` was found at the project
    /// root — the user hasn't enabled version control here.
    NoRepo,
    /// Load is in flight, no result yet.
    Loading,
    /// Load resolved successfully — render `entries` (possibly
    /// empty for fresh repos) plus the optional working-tree card.
    Ready,
}

/// Render the History panel. Delegates row rendering to
/// [`signex_widgets::history_pane`]; layers on the "no active
/// file" / "not in a git repo" / "loading" header + the working-
/// tree pseudo-card.
pub fn view_history<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let muted = theme_ext::text_secondary(&ctx.tokens);
    let primary = theme_ext::text_primary(&ctx.tokens);
    let border = theme_ext::border_color(&ctx.tokens);
    let panel_bg = theme_ext::panel_bg(&ctx.tokens).background;

    let state = &ctx.history;

    let inner: Element<'a, PanelMsg> = match state.mode {
        HistoryRenderMode::NoActiveFile => message_card(
            "Open a schematic, PCB, library, or primitive to see its history.",
            muted,
            border,
            panel_bg,
        ),
        HistoryRenderMode::NoRepo => message_card(
            "This file isn't under version control. \
             Enable from the project's right-click menu.",
            muted,
            border,
            panel_bg,
        ),
        HistoryRenderMode::Loading => message_card("Loading history…", muted, border, panel_bg),
        HistoryRenderMode::Ready => {
            let mut col: Column<'a, PanelMsg> = column![].spacing(6);

            if state.dirty {
                col = col.push(working_tree_card(primary, muted, border, panel_bg));
            }

            // v0.22 Phase 8.5 — render rows inline so each card can
            // carry a "Restore this version" button. Bypasses the
            // signex_widgets::history_pane (which is render-only) so
            // the panel can dispatch PanelMsg::HistoryRestoreClicked
            // on click. Empty list still falls back to the widget's
            // "No history yet." card via the helper below.
            if state.entries.is_empty() {
                col = col.push(signex_widgets::history_pane::<PanelMsg>(
                    &state.entries,
                    Utc::now(),
                    &ctx.tokens,
                ));
            } else {
                let now = Utc::now();
                for entry in &state.entries {
                    col = col.push(commit_card(entry, now, primary, muted, border, panel_bg));
                }
            }

            col.into()
        }
    };

    let body = container(inner).padding(8).width(Length::Fill);
    scrollable(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Single muted card used by the empty/no-repo/loading states.
fn message_card<'a, M: 'a>(
    msg: &'a str,
    muted: iced::Color,
    border_c: iced::Color,
    bg: Option<iced::Background>,
) -> Element<'a, M> {
    container(text(msg).size(12).color(muted))
        .padding([6, 8])
        .width(Length::Fill)
        .style(move |_theme: &iced::Theme| container::Style {
            background: bg,
            text_color: Some(muted),
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border_c,
            },
            ..container::Style::default()
        })
        .into()
}

/// v0.22 Phase 8.5 — Commit-row card with a "Restore this version"
/// button. Same visual shape as `signex_widgets::history_pane`'s
/// cards (author + relative time + subject + short SHA) plus a
/// muted button on the bottom that fires
/// `PanelMsg::HistoryRestoreClicked { sha }` on press. The handler
/// runs `LocalGitProjectAdapter::restore_at` against the active
/// tab's owning project.
fn commit_card<'a>(
    entry: &'a signex_widgets::HistoryEntry,
    now: chrono::DateTime<Utc>,
    primary: Color,
    muted: Color,
    border_c: Color,
    bg: Option<iced::Background>,
) -> Element<'a, PanelMsg> {
    let header = iced::widget::row![
        text(entry.author_name.clone()).size(12).color(primary),
        iced::widget::Space::new().width(Length::Fill),
        text(format_relative_simple(entry.time, now))
            .size(11)
            .color(muted),
    ]
    .spacing(8);

    let restore_btn = button(text("Restore this version").size(10).color(muted))
        .padding([2, 6])
        .on_press(PanelMsg::HistoryRestoreClicked {
            sha: entry.sha.clone(),
        })
        .style(move |_t: &Theme, _| iced::widget::button::Style {
            background: None,
            border: Border {
                width: 1.0,
                radius: 2.0.into(),
                color: border_c,
            },
            text_color: muted,
            ..iced::widget::button::Style::default()
        });

    let footer = iced::widget::row![
        text(short_sha(&entry.sha)).size(11).color(muted),
        iced::widget::Space::new().width(Length::Fill),
        restore_btn,
    ]
    .align_y(iced::Alignment::Center)
    .spacing(8);

    let card_inner = column![
        header,
        text(entry.subject.clone()).size(13).color(primary),
        footer,
    ]
    .spacing(2);

    container(card_inner)
        .padding([6, 8])
        .width(Length::Fill)
        .style(move |_theme: &Theme| container::Style {
            background: bg,
            text_color: Some(primary),
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border_c,
            },
            ..container::Style::default()
        })
        .into()
}

fn short_sha(full: &str) -> String {
    full.chars().take(7).collect()
}

/// Coarse relative-time helper. Mirrors what
/// `signex_widgets::history_pane::format_relative` does but is
/// inlined here so the panel doesn't need to expose the widget's
/// internal helper.
fn format_relative_simple(time: chrono::DateTime<Utc>, now: chrono::DateTime<Utc>) -> String {
    let delta = now.signed_duration_since(time);
    let secs = delta.num_seconds().max(0);
    if secs < 60 {
        return format!("{secs}s ago");
    }
    let mins = secs / 60;
    if mins < 60 {
        return format!("{mins}m ago");
    }
    let hours = mins / 60;
    if hours < 24 {
        return format!("{hours}h ago");
    }
    let days = hours / 24;
    if days < 30 {
        return format!("{days}d ago");
    }
    let months = days / 30;
    if months < 12 {
        return format!("{months}mo ago");
    }
    format!("{}y ago", months / 12)
}

/// "Working tree (uncommitted changes)" pseudo-card pinned above the
/// committed history when the active file is dirty. Same visual
/// shape as a `signex_widgets::history_pane` card so it reads as a
/// peer of the actual commits.
fn working_tree_card<'a, M: 'a>(
    primary: iced::Color,
    muted: iced::Color,
    border_c: iced::Color,
    bg: Option<iced::Background>,
) -> Element<'a, M> {
    let inner = column![
        text("Working tree").size(12).color(primary),
        text("Uncommitted changes").size(11).color(muted),
    ]
    .spacing(2);
    container(inner)
        .padding([6, 8])
        .width(Length::Fill)
        .style(move |_theme: &iced::Theme| container::Style {
            background: bg,
            text_color: Some(primary),
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border_c,
            },
            ..container::Style::default()
        })
        .into()
}
