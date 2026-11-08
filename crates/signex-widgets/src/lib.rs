//! Custom Iced widgets for Signex EDA.
//!
//! Reusable, theme-aware widgets built on stock Iced 0.15 primitives.
//! No Iced fork — composition only.

pub mod active_bar;
pub mod history_pane;
mod house;
pub mod icon_button;
mod label;
pub mod passive_calculator;
pub mod status_bar;
pub mod tab_pill;
pub mod theme_ext;
pub mod translate;
pub mod tree_view;

// The dropdown widget now lives at `active_bar::dropdown`; keep the
// flat `active_bar_dropdown` path so external
// `signex_widgets::active_bar_dropdown::…` consumers resolve unchanged.
pub use active_bar::dropdown as active_bar_dropdown;

pub use history_pane::{HistoryEntry, history_pane};
pub use house::{CheckboxLabel, Content, button, checkbox, text, text_input, tooltip};
pub use label::{Label, raw};

// The iced modules behind the house functions of the same name, so
// `use signex_widgets::button;` brings `button(..)` and `button::Style`.
pub mod button {
    pub use iced::widget::button::*;
}
pub mod checkbox {
    pub use iced::widget::checkbox::*;
}
pub mod text {
    pub use iced::widget::text::*;
}
pub mod text_input {
    pub use iced::widget::text_input::*;
}
pub mod tooltip {
    pub use iced::widget::tooltip::*;
}
pub use tab_pill::{AccentPosition, TabPill, TabPillStyle};
pub use translate::Translate;

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
