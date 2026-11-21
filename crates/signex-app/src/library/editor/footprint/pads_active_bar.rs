//! Pads-mode Active Bar — floating tool row for the footprint editor
//! when the editor is in [`EditorMode::Normal`].
//!
//! The Pads-mode tools: Select ([`selection_items`], at the end of the
//! bar's selection group) and Place Pad / Place Via / Place String
//! ([`placement_items`], at the start of its placement group). The mode
//! switch (Sketch / Pads / 3D) is the floating overlay
//! [`mode_switcher_overlay`].

use std::path::PathBuf;

use iced::widget::{container, row};
use iced::{Border, Length, Theme, Widget};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, raw, text};

use crate::styles::{Spacing, TypeScale};
use signex_widgets::active_bar::{ActiveBarButton, ActiveBarIcon, ActiveBarItem};
use signex_widgets::theme_ext;

use crate::app::FootprintDocument;
use crate::icons;
use crate::library::editor::footprint::state::{EditorMode, PadsTool};
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};

/// Standalone floating mode-switch widget rendered at the
/// top-left of the canvas via `Stack` overlay (separate from the
/// active bar's tools). Three connected segments in **Sketch /
/// Pads / 3D** order; the active segment paints with the accent
/// background.
pub fn mode_switcher_overlay<'a>(
    editor: &'a FootprintDocument,
    tokens: &'a ThemeTokens,
) -> iced::Element<'a, LibraryMessage> {
    let ts = TypeScale::default();
    let sp = Spacing::default();
    let mode = editor.state.mode;
    let path = editor.path.clone();
    let text_c = theme_ext::text_primary(tokens);
    let border = theme_ext::border_color(tokens);
    let on_accent = crate::styles::tokens::on_accent(tokens);
    let panel_bg = theme_ext::to_color(&tokens.panel_bg);

    let segment = move |label: &'static str,
                        target: EditorMode,
                        active: bool,
                        path: PathBuf|
          -> iced::Element<'a, LibraryMessage> {
        let label_color = if active { on_accent } else { text_c };
        button(
            container(
                text(raw(label))
                    .size(ts.body)
                    .color(label_color)
                    .align_x(iced::alignment::Horizontal::Center),
            )
            .height(Length::Fixed(22.0))
            .center_y(Length::Fixed(22.0)),
        )
        .padding([0.0, sp.l])
        .on_press(LibraryMessage::PrimitiveEditorEvent {
            path,
            msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SetMode(target)),
        })
        .style(chip_style(tokens, active))
        .boxed()
    };

    // Sketch · Pads · 3D — per user spec.
    let segments = row![
        segment(
            "Sketch",
            EditorMode::Sketch,
            matches!(mode, EditorMode::Sketch),
            path.clone()
        ),
        segment(
            "Pads",
            EditorMode::Normal,
            matches!(mode, EditorMode::Normal),
            path.clone()
        ),
        segment(
            "3D",
            EditorMode::View3d,
            matches!(mode, EditorMode::View3d),
            path.clone()
        ),
    ]
    .spacing(sp.xs)
    .align_y(iced::Alignment::Center);

    // Wrap in a panel-backed container so the chrome reads as a
    // floating chip over the canvas (matches the active bar's
    // visual rhythm).
    iced::widget::container(iced::widget::container(segments).padding(sp.s).style(
        move |_: &Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(panel_bg)),
            border: Border {
                width: 1.0,
                radius: 4.0.into(),
                color: border,
                ..Border::default()
            },
            ..iced::widget::container::Style::default()
        },
    ))
    .padding([sp.s, sp.m])
    .align_x(iced::alignment::Horizontal::Right)
    .align_y(iced::alignment::Vertical::Top)
    .width(Length::Fill)
    .height(Length::Fill)
    .boxed()
}

/// One Pads-mode tool button, selected while `tool` is the armed tool.
fn tool_button(
    editor: &FootprintDocument,
    icon: ActiveBarIcon,
    tooltip: &str,
    tool: PadsTool,
) -> ActiveBarItem<LibraryMessage> {
    ActiveBarItem::Button(ActiveBarButton {
        icon,
        tooltip: tooltip.to_string(),
        enabled: true,
        selected: editor.state.pads_tool == tool,
        on_press: Some(LibraryMessage::PrimitiveEditorEvent {
            path: editor.path.clone(),
            msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SetPadsTool(tool)),
        }),
        ..ActiveBarButton::default()
    })
}

/// The Pads-mode Select tool, the default tool.
pub fn selection_items(
    editor: &FootprintDocument,
    theme_id: signex_types::theme::ThemeId,
) -> Vec<ActiveBarItem<LibraryMessage>> {
    vec![tool_button(
        editor,
        ActiveBarIcon::Svg(icons::icon_select(theme_id)),
        "Select",
        PadsTool::Select,
    )]
}

/// The Pads-mode place tools: arm one, then click empty canvas to drop a
/// pad, a through-hole via or silk-layer text. Pure graphics live in
/// Sketch mode; a hole is a pad `kind` set in the Properties panel.
pub fn placement_items(
    editor: &FootprintDocument,
    _theme_id: signex_types::theme::ThemeId,
) -> Vec<ActiveBarItem<LibraryMessage>> {
    vec![
        tool_button(
            editor,
            ActiveBarIcon::Glyph("\u{25CF}"), // ●
            "Place Pad — click empty canvas to drop pads",
            PadsTool::PlacePad,
        ),
        tool_button(
            editor,
            ActiveBarIcon::Glyph("\u{25CE}"), // ◎ — annular ring glyph
            "Place Via — click empty canvas to drop a through-hole via",
            PadsTool::PlaceVia,
        ),
        tool_button(
            editor,
            ActiveBarIcon::Glyph("T"),
            "Place String — click empty canvas to drop silk-layer text",
            PadsTool::PlaceString,
        ),
    ]
}

/// The Pads-mode tools alone (Select, a divider, the place tools) through
/// [`signex_widgets::active_bar::view`].
pub fn view<'a>(
    editor: &'a FootprintDocument,
    theme_id: signex_types::theme::ThemeId,
    tokens: &'a ThemeTokens,
) -> iced::Element<'a, LibraryMessage> {
    let mut items = selection_items(editor, theme_id);
    items.push(ActiveBarItem::Separator);
    items.extend(placement_items(editor, theme_id));
    signex_widgets::active_bar::view(items, tokens)
}

type ChipStyle = Box<dyn Fn(&Theme, iced::widget::button::Status) -> iced::widget::button::Style>;

/// Mode / tab chip: accent fill when `active`, outline otherwise.
fn chip_style(tokens: &ThemeTokens, active: bool) -> ChipStyle {
    if active {
        Box::new(crate::styles::accent_button(tokens))
    } else {
        Box::new(crate::styles::outline_button(tokens))
    }
}
