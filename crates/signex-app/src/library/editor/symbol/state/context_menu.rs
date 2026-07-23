//! Right-click canvas context menu state for the symbol editor.
//! Mirrors `library::editor::footprint::state::context_menu` 1:1 in
//! structure — see that module for the wider design rationale.

/// `(x, y)` are **window-absolute** screen coords (already include
/// menu-bar + tab-bar offsets — computed in `canvas::input::pointer`'s
/// `on_secondary_release` from `bounds.x + cursor.x`). `target` records
/// what the cursor was over at right-click time so the update layer can
/// select-first (Altium parity) before opening the menu; `open_submenu`
/// tracks which submenu row (if any) is accordion-expanded in place.
#[derive(Debug, Clone)]
pub struct SymbolContextMenuState {
    pub x: f32,
    pub y: f32,
    pub target: SymbolContextTarget,
    pub open_submenu: Option<SymbolContextSubmenu>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolContextTarget {
    /// Right-clicked on bare canvas (no pin / graphic hit).
    Empty,
    /// Right-clicked on pin index `idx` in `Symbol::pins`.
    Pin(usize),
    /// Right-clicked on graphic index `idx` in `Symbol::graphics`.
    Graphic(usize),
}

/// The one submenu the symbol context menu currently has (Place ▸).
/// Kept as an enum (not a bare `bool`) so a second submenu can slot in
/// later without reshaping `SymbolContextMenuState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolContextSubmenu {
    Place,
}
