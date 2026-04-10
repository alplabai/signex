//! Custom Iced widgets for Signex EDA.
//!
//! Reusable, theme-aware widgets built on stock Iced 0.14 primitives.
//! No Iced fork — composition only.

pub mod icon_button;
pub mod status_bar;
pub mod symbol_preview;
pub mod theme_ext;
pub mod tree_view;

// ─── Extension Traits (Ludusavi pattern) ─────────────────────

use iced::widget::{Column, Row};
use iced::Element;

/// Conditional push — append an element only when a condition is true.
///
/// ```rust,ignore
/// col.push_if(has_badge, || text(badge).size(10))
/// ```
pub trait PushIf<'a, M> {
    fn push_if<E: Into<Element<'a, M>>>(self, cond: bool, f: impl FnOnce() -> E) -> Self;
}

impl<'a, M: 'a> PushIf<'a, M> for Column<'a, M> {
    fn push_if<E: Into<Element<'a, M>>>(self, cond: bool, f: impl FnOnce() -> E) -> Self {
        if cond {
            self.push(f())
        } else {
            self
        }
    }
}

impl<'a, M: 'a> PushIf<'a, M> for Row<'a, M> {
    fn push_if<E: Into<Element<'a, M>>>(self, cond: bool, f: impl FnOnce() -> E) -> Self {
        if cond {
            self.push(f())
        } else {
            self
        }
    }
}
