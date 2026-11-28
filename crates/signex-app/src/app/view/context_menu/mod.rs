//! Right-click / context-menu views: the reusable menu-item builders
//! (`items`), the concrete menus (`menus`), and the submenu launcher
//! (`submenu`). The folder carries the `context_menu` namespace; the
//! `impl Signex` builders here are called from `view/mod.rs` and its
//! sibling `overlays` module.

mod anchored;
// The sketch canvas menu opens through it too (mockup row 188).
pub(crate) use anchored::AnchoredMenu;
mod backdrop;
mod board_menu;
mod board_part_rows;
mod components_row;
mod dock_tab;
mod grid_picker;
mod grid_preview;
mod items;
mod layer_set;
mod layer_tab;
mod menus;
mod part_menu;
mod placement;
pub(in crate::app::view) use placement::clamp_menu_origin;
// The editors' Active Bar and sketch menus dim the window with it too.
pub(crate) use placement::{menu_scrim, menu_scrim_fill};
mod project_tree;
mod start_row;
mod submenu;
mod variant_fit;
#[cfg(test)]
mod variant_fit_tests;

#[cfg(test)]
mod catalog_agreement_tests;
#[cfg(test)]
mod tests;

use super::*;
