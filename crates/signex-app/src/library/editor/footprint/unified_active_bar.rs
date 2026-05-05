//! v0.18.14 — Altium-style unified active bar for the footprint
//! editor. Replaces the per-mode `pads_active_bar::view` /
//! `sketch_mode::active_bar::view` mounting in `standalone.rs`.
//!
//! The bar's tool slot (left half) follows the active editor mode
//! (`EditorMode::Sketch` → sketch tools; `EditorMode::Normal` →
//! pads tools; `EditorMode::View3d` → no tools). The right half
//! carries the eight Selection Filter pills regardless of mode so
//! the user can dial in what's selectable without leaving the
//! canvas.
//!
//! Per the schematic-Properties convention: kind pills live HERE,
//! not in the right-dock Properties panel. The Properties panel
//! shows a `Custom…` button that opens a richer modal (lands in
//! v0.18.14 phase 2).

use iced::Element;
use signex_types::theme::ThemeTokens;
use signex_widgets::active_bar::{ActiveBarButton, ActiveBarIcon, ActiveBarItem};

use crate::app::FootprintEditorState;
use crate::library::editor::footprint::state::{
    EditorMode, SelectionFilter, SelectionFilterKind,
};
use crate::library::messages::{LibraryMessage, PrimitiveEditorMsg};

/// Build the unified bar items + render via
/// [`signex_widgets::active_bar::view`]. Mode-specific tools come
/// from the existing per-mode `items()` functions; the Selection
/// Filter pill row is appended after a separator so the visual
/// rhythm stays grouped.
pub fn view<'a>(
    editor: &'a FootprintEditorState,
    theme_id: signex_types::theme::ThemeId,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let mut items: Vec<ActiveBarItem<LibraryMessage>> = match editor.state.mode {
        EditorMode::Sketch => {
            crate::library::editor::footprint::sketch_mode::active_bar::items(
                editor, theme_id, tokens,
            )
        }
        EditorMode::Normal => {
            crate::library::editor::footprint::pads_active_bar::items(
                editor, theme_id, tokens,
            )
        }
        EditorMode::View3d => Vec::new(),
    };
    if !items.is_empty() {
        items.push(ActiveBarItem::Separator);
    }
    items.extend(selection_filter_items(
        editor.path.clone(),
        editor.state.selection_filter,
    ));
    signex_widgets::active_bar::view(items, tokens)
}

/// Build the eight Selection Filter pill items
/// (Pads / Tracks / Arcs / Pours / 3D Bodies / Keepouts / Cutouts /
/// Texts). Each pill is a glyph-icon button whose `selected` flag
/// reflects whether that kind is currently selectable.
fn selection_filter_items(
    path: std::path::PathBuf,
    filter: SelectionFilter,
) -> Vec<ActiveBarItem<LibraryMessage>> {
    use SelectionFilterKind as K;
    let cases: [(&'static str, &'static str, K, bool); 8] = [
        ("Pad", "P", K::Pads, filter.pads),
        ("Track", "T", K::Tracks, filter.tracks),
        ("Arc", "A", K::Arcs, filter.arcs),
        ("Pour", "■", K::Pours, filter.pours),
        ("3D Body", "3", K::Bodies3d, filter.bodies_3d),
        ("Keepout", "K", K::Keepouts, filter.keepouts),
        ("Cutout", "C", K::Cutouts, filter.cutouts),
        ("Text", "Tx", K::Texts, filter.texts),
    ];
    cases
        .iter()
        .map(|&(label, glyph, kind, on)| {
            ActiveBarItem::Button(ActiveBarButton {
                icon: ActiveBarIcon::Glyph(glyph),
                tooltip: format!("Selectable: {label} ({})", if on { "yes" } else { "no" }),
                enabled: true,
                selected: on,
                on_press: Some(LibraryMessage::PrimitiveEditorEvent {
                    path: path.clone(),
                    msg: PrimitiveEditorMsg::FootprintToggleSelectionFilter(kind),
                }),
                ..ActiveBarButton::default()
            })
        })
        .collect()
}
