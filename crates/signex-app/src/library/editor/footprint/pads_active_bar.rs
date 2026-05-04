//! Pads-mode Active Bar — floating tool row for the footprint editor
//! when the editor is in [`EditorMode::Normal`].
//!
//! Mirrors Altium's PCB Library editor active-bar layout: Select +
//! Place Pad / Track / Arc / String / Polygon / Hole, an Auto-fit
//! Courtyard toggle, and a Sketch-mode entry button. Tools that
//! aren't wired in v0.14.x ship as stubs (greyed icons, no
//! `on_press`) so the bar reads as the eventual finished surface,
//! not a half-built one.
//!
//! The full set is intentional Altium parity — every pad/track/poly
//! placement lands as wiring goes in. The minimal v0.14.2 wiring is:
//! - Select: cursor (no specific message; the canvas's empty-space
//!   click already adds a pad in this mode, so "Place Pad" stays
//!   visually present in the bar but acts as a discoverability
//!   reminder until a proper place-pad gesture lands).
//! - Auto-fit Courtyard: existing `FootprintToggleAutoFit` toggle.
//! - Edit Sketch: mode-switch to `EditorMode::Sketch`.

use std::path::PathBuf;

use iced::Element;
use signex_types::theme::ThemeTokens;
use signex_widgets::active_bar::{ActiveBarButton, ActiveBarIcon, ActiveBarItem};

use crate::app::FootprintEditorState;
use crate::icons;
use crate::library::editor::footprint::state::EditorMode;
use crate::library::messages::{LibraryMessage, PrimitiveEditorMsg};

/// Build the Pads-mode Active Bar items.
pub fn items(
    editor: &FootprintEditorState,
    theme_id: signex_types::theme::ThemeId,
) -> Vec<ActiveBarItem<LibraryMessage>> {
    let path: PathBuf = editor.path.clone();
    let auto_fit_on = editor.state.auto_fit_courtyard;

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

    let stub_svg = |label: &str, icon: ActiveBarIcon| -> ActiveBarItem<LibraryMessage> {
        ActiveBarItem::Button(ActiveBarButton {
            icon,
            tooltip: format!("{label} (coming soon)"),
            enabled: false,
            selected: false,
            on_press: None,
            ..ActiveBarButton::default()
        })
    };

    // Select cursor — no dedicated message in pads mode (no tool
    // state machine yet); leaves selection behaviour unchanged.
    let select = ActiveBarItem::Button(ActiveBarButton {
        icon: ActiveBarIcon::Svg(icons::icon_select(theme_id)),
        tooltip: "Select".into(),
        enabled: true,
        selected: true, // the de-facto active tool in pads mode
        on_press: None,
        ..ActiveBarButton::default()
    });

    // Auto-fit Courtyard toggle.
    let auto_fit_path = path.clone();
    let auto_fit = ActiveBarItem::Button(ActiveBarButton {
        icon: ActiveBarIcon::Glyph("\u{25A1}"), // ▢ rectangle (courtyard)
        tooltip: if auto_fit_on {
            "Auto-fit Courtyard (on — click to disable)".into()
        } else {
            "Auto-fit Courtyard (off — click to enable)".into()
        },
        enabled: true,
        selected: auto_fit_on,
        on_press: Some(LibraryMessage::PrimitiveEditorEvent {
            path: auto_fit_path,
            msg: PrimitiveEditorMsg::FootprintToggleAutoFit,
        }),
        ..ActiveBarButton::default()
    });

    // Delete selected pad — emits the existing message; greyed when
    // no pad is selected.
    let delete_path = path.clone();
    let has_selection = editor.state.selected_pad.is_some();
    let delete = ActiveBarItem::Button(ActiveBarButton {
        icon: ActiveBarIcon::Glyph("\u{2421}"), // ␡ DELETE symbol
        tooltip: if has_selection {
            "Delete selected pad".into()
        } else {
            "Delete (select a pad first)".into()
        },
        enabled: has_selection,
        selected: false,
        on_press: if has_selection {
            Some(LibraryMessage::PrimitiveEditorEvent {
                path: delete_path,
                msg: PrimitiveEditorMsg::FootprintDeleteSelected,
            })
        } else {
            None
        },
        ..ActiveBarButton::default()
    });

    // Edit Sketch — mode switch into the parametric sketcher.
    let sketch_path = path.clone();
    let edit_sketch = ActiveBarItem::Button(ActiveBarButton {
        icon: ActiveBarIcon::Svg(icons::icon_shape_arc(theme_id)),
        tooltip: "Edit Sketch — open the parametric sketcher".into(),
        enabled: true,
        selected: false,
        on_press: Some(LibraryMessage::PrimitiveEditorEvent {
            path: sketch_path,
            msg: PrimitiveEditorMsg::FootprintSetMode(EditorMode::Sketch),
        }),
        ..ActiveBarButton::default()
    });

    vec![
        select,
        ActiveBarItem::Separator,
        // Altium-parity Place tools — most are stubs in v0.14.x.
        // Place Pad has no dedicated tool state yet (clicking empty
        // canvas adds a pad), so it ships as a stub here too — wiring
        // a tool-state machine + ghost preview lands in v0.15+.
        stub("Place Pad", "\u{25CF}"), // ●
        stub_svg(
            "Place Track",
            ActiveBarIcon::Svg(icons::icon_shape_line(theme_id)),
        ),
        stub_svg(
            "Place Arc",
            ActiveBarIcon::Svg(icons::icon_shape_arc(theme_id)),
        ),
        stub_svg(
            "Place Region (Polygon)",
            ActiveBarIcon::Svg(icons::icon_shape_polygon(theme_id)),
        ),
        stub("Place Fill", "\u{25A0}"),  // ■
        stub("Place String", "T"),
        stub("Place Hole", "\u{25CE}"), // ◎
        ActiveBarItem::Separator,
        delete,
        auto_fit,
        ActiveBarItem::Separator,
        edit_sketch,
    ]
}

/// Convenience wrapper — build items + render via
/// [`signex_widgets::active_bar::view`].
pub fn view<'a>(
    editor: &'a FootprintEditorState,
    theme_id: signex_types::theme::ThemeId,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    signex_widgets::active_bar::view(items(editor, theme_id), tokens)
}
