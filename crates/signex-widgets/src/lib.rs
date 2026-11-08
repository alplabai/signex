//! Custom Iced widgets for Signex EDA.
//!
//! Reusable, theme-aware widgets built on stock Iced 0.15 primitives.
//! No Iced fork — composition only.

pub mod active_bar;
pub mod chevron;
pub mod dropdown_style;
pub mod history_pane;
pub mod icon_button;
pub mod passive_calculator;
pub mod segmented;
pub mod status_bar;
pub mod tab_pill;
pub mod theme_ext;
pub mod toggle_switch;
pub mod tree_view;

// The dropdown widget now lives at `active_bar::dropdown`; keep the
// flat `active_bar_dropdown` path so external
// `signex_widgets::active_bar_dropdown::…` consumers resolve unchanged.
pub use active_bar::dropdown as active_bar_dropdown;

pub use history_pane::{HistoryEntry, history_pane};
pub use segmented::{SegmentedStyle, segmented};
pub use tab_pill::{AccentPosition, TabPill, TabPillStyle};
pub use toggle_switch::{ToggleSwitchStyle, toggle_switch};
pub use tree_view::TreeStyle;

// ─── Extension Traits (Ludusavi pattern) ─────────────────────

use iced::Element;
use iced::widget::{Column, Row};

/// Conditional push — append an element only when a condition is true.
///
/// ```rust,ignore
/// col.push_if(has_badge, || text(badge).size(10))
/// ```
pub trait PushIf<'a, M> {
    fn push_if<E: Into<Element<'a, M>>>(self, cond: bool, f: impl FnOnce() -> E) -> Self;
}

impl<'a, M: 'a> PushIf<'a, M> for Column<Element<'a, M>> {
    fn push_if<E: Into<Element<'a, M>>>(self, cond: bool, f: impl FnOnce() -> E) -> Self {
        if cond { self.push(f().into()) } else { self }
    }
}

impl<'a, M: 'a> PushIf<'a, M> for Row<Element<'a, M>> {
    fn push_if<E: Into<Element<'a, M>>>(self, cond: bool, f: impl FnOnce() -> E) -> Self {
        if cond { self.push(f().into()) } else { self }
    }
}
