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

use iced::widget::{button, row, text};
use iced::{Border, Color, Element, Length, Theme};
use signex_types::theme::ThemeTokens;
use signex_widgets::active_bar::{ActiveBarButton, ActiveBarIcon, ActiveBarItem};
use signex_widgets::theme_ext;

use crate::app::FootprintEditorState;
use crate::icons;
use crate::library::editor::footprint::state::{EditorMode, PadsTool};
use crate::library::messages::{LibraryMessage, PrimitiveEditorMsg};

/// v0.14.2 — standalone floating mode-switch widget rendered at the
/// top-left of the canvas via `Stack` overlay (separate from the
/// active bar's tools). Three connected segments in **Sketch /
/// Pads / 3D** order; the active segment paints with the accent
/// background.
pub fn mode_switcher_overlay<'a>(
    editor: &'a FootprintEditorState,
    tokens: &'a ThemeTokens,
) -> iced::Element<'a, LibraryMessage> {
    let mode = editor.state.mode;
    let path = editor.path.clone();
    let text_c = theme_ext::text_primary(tokens);
    let border = theme_ext::border_color(tokens);
    let accent = theme_ext::to_color(&tokens.accent);
    let panel_bg = theme_ext::to_color(&tokens.panel_bg);

    let segment = move |label: &'static str,
                        target: EditorMode,
                        active: bool,
                        path: PathBuf|
     -> iced::Element<'a, LibraryMessage> {
        let label_color = if active { iced::Color::WHITE } else { text_c };
        button(
            text(label)
                .size(11)
                .color(label_color)
                .align_x(iced::alignment::Horizontal::Center),
        )
        .padding([5, 12])
        .on_press(LibraryMessage::PrimitiveEditorEvent {
            path,
            msg: PrimitiveEditorMsg::FootprintSetMode(target),
        })
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: if active {
                Some(iced::Background::Color(accent))
            } else {
                Some(iced::Background::Color(Color::from_rgba(
                    1.0, 1.0, 1.0, 0.04,
                )))
            },
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: if active { accent } else { border },
            },
            ..iced::widget::button::Style::default()
        })
        .into()
    };

    // Sketch · Pads · 3D — per user spec.
    let segments = row![
        segment("Sketch", EditorMode::Sketch, matches!(mode, EditorMode::Sketch), path.clone()),
        segment("Pads", EditorMode::Normal, matches!(mode, EditorMode::Normal), path.clone()),
        segment("3D", EditorMode::View3d, matches!(mode, EditorMode::View3d), path.clone()),
    ]
    .spacing(2)
    .align_y(iced::Alignment::Center);

    // Wrap in a panel-backed container so the chrome reads as a
    // floating chip over the canvas (matches the active bar's
    // visual rhythm).
    iced::widget::container(
        iced::widget::container(segments)
            .padding(4)
            .style(move |_: &Theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(panel_bg)),
                border: Border {
                    width: 1.0,
                    radius: 4.0.into(),
                    color: border,
                },
                ..iced::widget::container::Style::default()
            }),
    )
    .padding([6, 10])
    .align_x(iced::alignment::Horizontal::Right)
    .align_y(iced::alignment::Vertical::Top)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// Build the Pads-mode Active Bar items.
pub fn items(
    editor: &FootprintEditorState,
    theme_id: signex_types::theme::ThemeId,
    tokens: &ThemeTokens,
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

    // v0.15 — Pads-mode tool state machine. Select is the default;
    // PlacePad makes empty-canvas clicks drop a pad at the cursor.
    let pads_tool = editor.state.pads_tool;
    let select_path = path.clone();
    let select = ActiveBarItem::Button(ActiveBarButton {
        icon: ActiveBarIcon::Svg(icons::icon_select(theme_id)),
        tooltip: "Select".into(),
        enabled: true,
        selected: pads_tool == PadsTool::Select,
        on_press: Some(LibraryMessage::PrimitiveEditorEvent {
            path: select_path,
            msg: PrimitiveEditorMsg::FootprintSetPadsTool(PadsTool::Select),
        }),
        ..ActiveBarButton::default()
    });

    // Place Pad — wired in v0.15. Activate the tool, then click an
    // empty area of the canvas to drop a pad there.
    let place_pad_path = path.clone();
    let place_pad = ActiveBarItem::Button(ActiveBarButton {
        icon: ActiveBarIcon::Glyph("\u{25CF}"), // ●
        tooltip: "Place Pad — click empty canvas to drop pads".into(),
        enabled: true,
        selected: pads_tool == PadsTool::PlacePad,
        on_press: Some(LibraryMessage::PrimitiveEditorEvent {
            path: place_pad_path,
            msg: PrimitiveEditorMsg::FootprintSetPadsTool(PadsTool::PlacePad),
        }),
        ..ActiveBarButton::default()
    });

    // v0.14.2: Auto-fit Courtyard moved to the Properties panel
    // default body (Settings section). The active bar stays focused
    // on placement/edit tools.
    let _ = auto_fit_on;

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

    // v0.14.2: dedicated "Edit Sketch" button removed — mode
    // segments at the left of the bar drive mode switching.

    let _ = tokens;
    vec![
        select,
        ActiveBarItem::Separator,
        // v0.15 — Place Pad now has a real tool-state machine.
        place_pad,
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
    ]
}

/// Convenience wrapper — build items + render via
/// [`signex_widgets::active_bar::view`].
pub fn view<'a>(
    editor: &'a FootprintEditorState,
    theme_id: signex_types::theme::ThemeId,
    tokens: &'a ThemeTokens,
) -> iced::Element<'a, LibraryMessage> {
    signex_widgets::active_bar::view(items(editor, theme_id, tokens), tokens)
}
