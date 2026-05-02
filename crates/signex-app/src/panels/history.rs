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
use iced::widget::{Column, column, container, scrollable, text};
use iced::{Border, Element, Length};
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

            // Delegate the actual list rendering to
            // `signex_widgets::history_pane`. It already renders a
            // muted "No history yet." card when the slice is empty,
            // so we don't special-case zero-entry repos here — the
            // working-tree pseudo-card above takes care of the
            // dirty-only state.
            col = col.push(signex_widgets::history_pane::<PanelMsg>(
                &state.entries,
                Utc::now(),
                &ctx.tokens,
            ));

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
