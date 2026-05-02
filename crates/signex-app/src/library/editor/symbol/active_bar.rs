//! SchLib editor's Active Bar — the floating tool bar over the
//! `.snxsym` canvas, mirroring the schematic editor's Altium-style
//! Active Bar pattern but with SchLib-specific tools.
//!
//! Built on top of the reusable
//! [`signex_widgets::active_bar`] widget so the same component
//! powers (eventually) the schematic, schematic library, PCB, and
//! PCB library editors. Each editor builds its own
//! `Vec<ActiveBarItem<M>>` describing icon + tooltip + selection
//! state + on_press; the widget renders.
//!
//! # SchLib tool set (Altium parity)
//!
//! | Slot | Tool | Wired? |
//! |------|------|--------|
//! | 1    | Select          | ✓ |
//! | 2    | Place Pin       | ✓ |
//! | 3    | Place Line      | ✓ |
//! | 4    | Place Rectangle | ✓ |
//! | 5    | Place Round Rectangle | stub |
//! | 6    | Place Polygon   | stub |
//! | 7    | Place Ellipse (Circle) | ✓ |
//! | 8    | Place Pie Chart | stub |
//! | 9    | Place Elliptical Arc | stub |
//! | 10   | Place Arc       | ✓ |
//! | 11   | Place Bezier    | stub |
//! | 12   | Place Text String | ✓ |
//! | 13   | Place Text Frame | stub |
//! | 14   | Place Image     | stub |
//!
//! Stub tools render with a greyed icon and no `on_press` so the
//! Active Bar visually matches Altium's full toolbox; the actual
//! wiring lands in v0.9.x.

use std::path::PathBuf;

use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::active_bar::{ActiveBarButton, ActiveBarIcon, ActiveBarItem};

use crate::icons;
use crate::library::editor::symbol::canvas::SymbolTool;
use crate::library::messages::{LibraryMessage, PrimitiveEditorMsg, SymbolToolMsg};

/// Build the SchLib Active Bar items for the given editor state +
/// active theme. `path` is the editor's `.snxsym` path so the
/// emitted `LibraryMessage::PrimitiveEditorEvent` is keyed correctly.
pub fn items(
    path: &PathBuf,
    active_tool: SymbolTool,
    theme_id: ThemeId,
) -> Vec<ActiveBarItem<LibraryMessage>> {
    let tool = |label: &str,
                tool: SymbolTool,
                msg: SymbolToolMsg,
                icon: ActiveBarIcon|
     -> ActiveBarItem<LibraryMessage> {
        ActiveBarItem::Button(ActiveBarButton {
            icon,
            tooltip: label.to_string(),
            enabled: true,
            selected: active_tool == tool,
            on_press: Some(LibraryMessage::PrimitiveEditorEvent {
                path: path.clone(),
                msg: PrimitiveEditorMsg::SymbolSetTool(msg),
            }),
            ..ActiveBarButton::default()
        })
    };
    let stub = |label: &str, glyph: &'static str| -> ActiveBarItem<LibraryMessage> {
        ActiveBarItem::Button(ActiveBarButton {
            icon: ActiveBarIcon::Glyph(glyph),
            tooltip: format!("{label} (coming soon)"),
            enabled: false,
            selected: false,
            on_press: None,
            ..ActiveBarButton::default()
        })
    };

    vec![
        tool(
            "Select",
            SymbolTool::Select,
            SymbolToolMsg::Select,
            ActiveBarIcon::Svg(icons::icon_select(theme_id)),
        ),
        tool(
            "Place Pin",
            SymbolTool::AddPin,
            SymbolToolMsg::AddPin,
            // No dedicated pin svg yet — use the move glyph as a
            // placeholder shape until a pin svg lands.
            ActiveBarIcon::Glyph("\u{2192}"), // → arrow (pin tip)
        ),
        tool(
            "Place Line",
            SymbolTool::PlaceLine,
            SymbolToolMsg::PlaceLine,
            ActiveBarIcon::Svg(icons::icon_shape_line(theme_id)),
        ),
        tool(
            "Place Rectangle",
            SymbolTool::PlaceRectangle,
            SymbolToolMsg::PlaceRectangle,
            ActiveBarIcon::Svg(icons::icon_shape_rect(theme_id)),
        ),
        stub("Place Round Rectangle", "\u{25A2}"), // ▢
        stub("Place Polygon", "\u{2B20}"),         // ⬠
        tool(
            "Place Ellipse",
            SymbolTool::PlaceCircle,
            SymbolToolMsg::PlaceCircle,
            ActiveBarIcon::Svg(icons::icon_shape_circle(theme_id)),
        ),
        stub("Place Pie Chart", "\u{25D4}"),      // ◔
        stub("Place Elliptical Arc", "\u{27D3}"), // ⟓
        tool(
            "Place Arc",
            SymbolTool::PlaceArc,
            SymbolToolMsg::PlaceArc,
            ActiveBarIcon::Svg(icons::icon_shape_arc(theme_id)),
        ),
        stub("Place Bezier", "\u{223F}"), // ∿
        tool(
            "Place Text String",
            SymbolTool::PlaceText,
            SymbolToolMsg::PlaceText,
            // No dedicated text svg in the icon registry — use
            // the canonical "T" glyph.
            ActiveBarIcon::Glyph("T"),
        ),
        stub("Place Text Frame", "\u{25AD}"), // ▭
        stub("Place Image", "\u{25A3}"),      // ▣
    ]
}

/// Convenience wrapper — build the items + render via
/// [`signex_widgets::active_bar::view`]. Most callers use this; the
/// raw `items` builder is exposed for tests / per-editor variants
/// that need to splice their own buttons in.
pub fn view<'a>(
    path: &PathBuf,
    active_tool: SymbolTool,
    theme_id: ThemeId,
    tokens: &'a ThemeTokens,
) -> iced::Element<'a, LibraryMessage> {
    signex_widgets::active_bar::view(items(path, active_tool, theme_id), tokens)
}
