//! Generic Active Bar — a floating row of icon buttons used by every
//! Signex editor surface (schematic, schematic library, PCB, PCB
//! library) to surface the primary place / select tools.
//!
//! Altium-parity affordance: the bar floats over the canvas at the
//! top, the active tool reads with the accent background, disabled
//! tools (e.g. v0.9.x stubs) read greyed-out, and tooltips spell
//! out the tool name. Each editor builds its own
//! `Vec<ActiveBarItem<M>>` with the editor-specific message type;
//! this widget just renders.
//!
//! The schematic editor's existing in-tree `active_bar.rs` (with
//! dropdowns + advanced state) stays as-is for now — too risky to
//! migrate in one pass — but new editors and the planned PCB
//! / PCB library surfaces use this widget.
//!
//! # Example
//!
//! ```ignore
//! use signex_widgets::active_bar::{view, ActiveBarItem, ActiveBarIcon};
//!
//! let items = vec![
//!     ActiveBarItem {
//!         icon: ActiveBarIcon::Svg(SELECT_SVG.clone()),
//!         tooltip: "Select".into(),
//!         enabled: true,
//!         selected: matches!(tool, Tool::Select),
//!         on_press: Some(Msg::SetTool(Tool::Select)),
//!     },
//!     // …
//! ];
//! view(items, &tokens)
//! ```
//!
//! Render the resulting `Element<M>` into a `Stack` overlay so it
//! floats over the canvas content.

use iced::widget::{button, container, image, row, svg, text, tooltip};
use iced::{Border, Color, Element, Length, Theme};
use signex_types::theme::ThemeTokens;

use crate::theme_ext;

/// One button in the Active Bar.
///
/// `M` is the editor's message type; the bar is generic over it so
/// every editor can publish its own messages without sharing a
/// common message enum.
pub struct ActiveBarItem<M: 'static + Clone> {
    /// Glyph or SVG to render in the button.
    pub icon: ActiveBarIcon,
    /// Tooltip text shown on hover.
    pub tooltip: String,
    /// `false` → button greys out and ignores clicks. Used for v0.9.x
    /// stub tools that haven't been wired yet so the user still sees
    /// the icon (Altium parity) without the ability to fire it.
    pub enabled: bool,
    /// `true` → button paints with the accent background (Altium's
    /// "armed tool" highlight).
    pub selected: bool,
    /// Message published on press. `None` keeps the button
    /// non-interactive even when `enabled` is true (passive item).
    pub on_press: Option<M>,
}

/// What to render inside a button — SVG (preferred), bitmap image
/// (PNG/JPEG via raster Handle), or a Unicode glyph fallback for
/// items that have no icon yet.
pub enum ActiveBarIcon {
    /// `iced::widget::svg::Handle`. Use for any SVG asset shipped in
    /// `assets/icons/*.svg` — `signex-app::assets::svg_handle("…")`
    /// gives one.
    Svg(svg::Handle),
    /// Raster image handle (PNG/JPEG). Reserved for icons that don't
    /// have an SVG version.
    Raster(image::Handle),
    /// Unicode glyph fallback. Use for tools whose SVG icon hasn't
    /// been authored yet so the bar still shows something
    /// recognisable. Picked the geometry / roman letter that most
    /// closely matches the tool.
    Glyph(&'static str),
}

/// Pixel sizes — tightened to match the compact dark pill in
/// Altium's reference SchLib screenshot. Buttons are 22 px square
/// with 14 px icons; 3 px outer pad keeps the bar a single
/// horizontal row.
const BTN_SIZE: f32 = 22.0;
const ICON_SIZE: f32 = 14.0;
const BAR_PADDING: f32 = 3.0;
const BAR_RADIUS: f32 = 4.0;

/// Render the bar.
///
/// Returns an `Element<M>` ready to push into a `Stack` overlay
/// layer. The bar is `Length::Shrink` width-wise so the caller
/// controls horizontal positioning (centred via a parent
/// `container.align_x(Center)`, etc.).
pub fn view<'a, M>(items: Vec<ActiveBarItem<M>>, tokens: &'a ThemeTokens) -> Element<'a, M>
where
    M: 'static + Clone,
{
    let bar_bg = theme_ext::to_color(&tokens.panel_bg);
    let border = theme_ext::border_color(tokens);

    let mut row_widget = row![].spacing(2).align_y(iced::Alignment::Center);
    for item in items {
        row_widget = row_widget.push(view_item(item, tokens));
    }

    container(row_widget)
        .padding(BAR_PADDING)
        .style(move |_: &Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(bar_bg)),
            border: Border {
                width: 1.0,
                radius: BAR_RADIUS.into(),
                color: border,
            },
            ..iced::widget::container::Style::default()
        })
        .into()
}

fn view_item<'a, M>(item: ActiveBarItem<M>, tokens: &'a ThemeTokens) -> Element<'a, M>
where
    M: 'static + Clone,
{
    let text_c = theme_ext::text_primary(tokens);
    let muted_c = theme_ext::text_secondary(tokens);
    let accent_c = theme_ext::accent_color(tokens);
    let border = theme_ext::border_color(tokens);

    let icon_color = if !item.enabled {
        Color { a: 0.4, ..muted_c }
    } else if item.selected {
        Color::WHITE
    } else {
        text_c
    };

    let icon_widget: Element<'a, M> = match item.icon {
        ActiveBarIcon::Svg(handle) => svg(handle)
            .width(Length::Fixed(ICON_SIZE))
            .height(Length::Fixed(ICON_SIZE))
            .style(move |_: &Theme, _| iced::widget::svg::Style {
                color: Some(icon_color),
            })
            .into(),
        ActiveBarIcon::Raster(handle) => image(handle)
            .width(Length::Fixed(ICON_SIZE))
            .height(Length::Fixed(ICON_SIZE))
            .into(),
        ActiveBarIcon::Glyph(s) => text(s.to_string()).size(13).color(icon_color).into(),
    };

    let selected = item.selected;
    let enabled = item.enabled;

    let mut btn = button(
        container(icon_widget)
            .width(Length::Fixed(BTN_SIZE))
            .height(Length::Fixed(BTN_SIZE))
            .center_x(Length::Fixed(BTN_SIZE))
            .center_y(Length::Fixed(BTN_SIZE)),
    )
    .padding(0)
    .style(move |_: &Theme, status: iced::widget::button::Status| {
        let bg = if !enabled {
            None
        } else if selected {
            Some(iced::Background::Color(accent_c))
        } else {
            match status {
                iced::widget::button::Status::Hovered => Some(iced::Background::Color(
                    Color::from_rgba(1.0, 1.0, 1.0, 0.08),
                )),
                _ => Some(iced::Background::Color(Color::from_rgba(
                    1.0, 1.0, 1.0, 0.02,
                ))),
            }
        };
        iced::widget::button::Style {
            background: bg,
            border: Border {
                width: if selected { 1.0 } else { 0.0 },
                radius: 3.0.into(),
                color: border,
            },
            ..iced::widget::button::Style::default()
        }
    });
    if enabled && let Some(msg) = item.on_press {
        btn = btn.on_press(msg);
    }

    let tooltip_text = item.tooltip.clone();
    tooltip(
        btn,
        container(text(tooltip_text).size(11).color(text_c))
            .padding([2, 6])
            .style(move |_: &Theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(theme_ext::to_color(
                    &tokens.panel_bg,
                ))),
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border,
                },
                ..iced::widget::container::Style::default()
            }),
        tooltip::Position::Bottom,
    )
    .into()
}
