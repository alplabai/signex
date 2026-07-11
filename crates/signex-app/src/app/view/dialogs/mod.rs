//! Shared modal-chrome constants (single source of truth) plus re-exports
//! of the shared modal primitives, kept at this path so the existing
//! `crate::app::view::dialogs::…` imports across the crate keep resolving.
//!
//! The dialog builders that used to live here were split into the child
//! modules of this `dialogs` folder (ADR-0001, issue #164) as pure code
//! motion.

use super::*;
use iced::Color;

mod annotate;
mod annotate_preview;
mod bom;
mod confirms;
mod erc;
mod project;
mod widgets;

// ── Modal chrome — single source of truth ───────────────────────────
//
// Every modal in the app (Annotate, ERC, Reset Confirm, Rename, Remove,
// Close-Tab Confirm, Print Preview) reaches for these constants so the
// header height, title font, and close-X footprint stay locked in
// step with the main-window chrome strip (`view::view_main_window_chrome`).
// Tweak here, every modal updates.

/// Modal header total height — Altium-style compact (28 px). The
/// MENU_BAR_HEIGHT (36 px) read as too chunky on small confirm /
/// rename modals; 28 keeps the header tight relative to the body.
/// Close-X follows the same height so there's no empty strip below
/// the button.
pub(crate) const MODAL_HEADER_HEIGHT: f32 = 28.0;
/// Asymmetric padding inside the modal header strip: zero on the right
/// so the close-X sits flush against the rounded corner (its own
/// top-right radius matches `MODAL_CORNER_RADIUS`); zero top/bottom so
/// the X fills the strip's full height; left inset matched to the
/// modal body padding (16 px) so the title left-aligns with the
/// body's first text column.
pub(crate) const MODAL_HEADER_PADDING: iced::Padding = iced::Padding {
    top: 0.0,
    right: 0.0,
    bottom: 0.0,
    left: 16.0,
};
/// Title text size in the modal header.
pub(crate) const MODAL_HEADER_TITLE_SIZE: f32 = 13.0;
/// Close-X hit-box width — same width the chrome close uses
/// (`view::view_main_window_chrome::chrome_btn`).
pub(crate) const MODAL_CLOSE_X_HIT_W: f32 = 46.0;
/// Close-X hit-box height — also matches the chrome close (full
/// menu-bar height) so the modal X is pixel-identical to the OS-window X.
pub(crate) const MODAL_CLOSE_X_HIT_H: f32 = MODAL_HEADER_HEIGHT;
/// SVG glyph size for the close-X. Same value the chrome close uses.
pub(crate) const MODAL_CLOSE_X_ICON: f32 = 14.0;
/// Hover background for the close-X (Windows-native destructive red).
pub(crate) const MODAL_CLOSE_X_HOVER: Color = Color::from_rgba(0.78, 0.22, 0.22, 1.0);

pub(in crate::app::view) use widgets::{draggable_header, wrap_modal};
pub(crate) use widgets::{close_x_button, detached_header};
