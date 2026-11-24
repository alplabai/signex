//! The Library panel's row style (mockup row 119): the two-button compact /
//! thumbnail switch before Delete, and the thumbnail tile a thumbnail row
//! starts with. One style serves all three kinds.

use iced::widget::tooltip::Position;
use iced::widget::{Row, container};
use iced::{Background, Border, Element, Length, Theme, Widget};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::button;

use super::frame::{SP, glyph, lib};
use crate::dock::DockMessage;
use crate::library::messages::{LibraryMessage, PickerMsg};
use crate::library::state::{LibraryPanelKind, LibraryRowStyle};
use crate::panels::graphite::hint_tooltip;
use crate::styles::tokens::{Overlay, overlay, resolve};
use crate::styles::{ChromeMetrics, Radii, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;
const CM: ChromeMetrics = ChromeMetrics::DEFAULT;
const RADII: Radii = Radii::GRAPHITE;

/// Side of one switch button.
pub(super) const SWITCH_BUTTON: f32 = CM.row - SP.xs;
/// Height of a thumbnail row: two text lines beside the tile.
pub(super) const THUMB_ROW_H: f32 = 2.0 * CM.row;
/// Height of the thumbnail tile.
const TILE_H: f32 = CM.row + SP.xs;
/// Width of the thumbnail tile: a little wider than tall, like a drawing.
const TILE_W: f32 = TILE_H + SP.m;
/// Side of the glyph inside the tile.
const TILE_GLYPH: f32 = TS.label;

const COMPACT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><path d="M2 4h12M2 8h12M2 12h12" stroke="currentColor" stroke-width="1.5" fill="none"/></svg>"#;
const THUMBS: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><rect x="2" y="2.5" width="4" height="4" rx="1" stroke="currentColor" fill="none"/><rect x="2" y="9.5" width="4" height="4" rx="1" stroke="currentColor" fill="none"/><path d="M8 4.5h6M8 11.5h6" stroke="currentColor" stroke-width="1.5"/></svg>"#;
const SYMBOL_TILE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><path d="M4.5 3v10l8-5z M1.5 5.5h3M1.5 10.5h3M12.5 8h2" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round"/></svg>"#;
const FOOTPRINT_TILE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><rect x="2" y="5" width="4" height="6" rx=".5" fill="currentColor"/><rect x="10" y="5" width="4" height="6" rx=".5" fill="currentColor"/></svg>"#;
const PART_TILE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><path d="M1 8h3M12 8h3" stroke="currentColor" stroke-width="1.2"/><rect x="4" y="5.5" width="8" height="5" fill="none" stroke="currentColor" stroke-width="1.2"/></svg>"#;

/// How one list row draws: on one line, or as a thumbnail row of `kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RowLook {
    Compact,
    Thumbnail(LibraryPanelKind),
}

impl RowLook {
    /// The look of a `kind` row under the panel's `style`.
    pub(super) fn of(style: LibraryRowStyle, kind: LibraryPanelKind) -> Self {
        match style {
            LibraryRowStyle::Compact => Self::Compact,
            LibraryRowStyle::Thumbnail => Self::Thumbnail(kind),
        }
    }
}

/// The switch: Compact rows, then Rows with preview thumbnail, the shown
/// style marked.
pub(super) fn switch<'a>(
    style: LibraryRowStyle,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, DockMessage> {
    let pair = [
        (
            LibraryRowStyle::Compact,
            COMPACT,
            ids::LIBRARY_PANEL_ROWS_COMPACT,
        ),
        (
            LibraryRowStyle::Thumbnail,
            THUMBS,
            ids::LIBRARY_PANEL_ROWS_THUMBNAIL,
        ),
    ];
    let mut group: Row<Element<'a, DockMessage>> = Row::new().spacing(0);
    for (value, bytes, tip) in pair {
        group = group.push(hint_tooltip(
            switch_button(bytes, value == style, value, tokens),
            t!(i18n, tip),
            tokens,
            Position::Bottom,
        ));
    }
    let edge = ti(tokens.border);
    container(group)
        .style(move |_: &Theme| container::Style {
            border: Border {
                width: 1.0,
                color: edge,
                radius: RADII.small.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .clip(true)
        .boxed()
}

fn switch_button<'a>(
    bytes: &'static [u8],
    on: bool,
    value: LibraryRowStyle,
    tokens: &ThemeTokens,
) -> Element<'a, DockMessage> {
    let ink = if on {
        ti(tokens.text)
    } else {
        resolve(tokens).ink3
    };
    let hover = overlay(tokens, Overlay::Hover);
    button(container(glyph(bytes, TS.body, ink)).center(Length::Fill))
        .width(SWITCH_BUTTON)
        .height(SWITCH_BUTTON)
        .padding(0)
        .on_press(lib(LibraryMessage::Picker(PickerMsg::PanelRowStylePicked(
            value,
        ))))
        .style(move |_: &Theme, status: button::Status| button::Style {
            background: (on || matches!(status, button::Status::Hovered | button::Status::Pressed))
                .then_some(Background::Color(hover)),
            text_color: ink,
            ..button::Style::default()
        })
        .boxed()
}

/// The tile a thumbnail row of `kind` starts with: the kind's glyph on a
/// sunken card.
pub(super) fn tile<'a>(kind: LibraryPanelKind, tokens: &ThemeTokens) -> Element<'a, DockMessage> {
    let r = resolve(tokens);
    let bytes = match kind {
        LibraryPanelKind::Symbols => SYMBOL_TILE,
        LibraryPanelKind::Footprints => FOOTPRINT_TILE,
        LibraryPanelKind::Parts => PART_TILE,
    };
    let (fill, edge) = (r.sunken, r.line_soft);
    container(glyph(bytes, TILE_GLYPH, r.ink3))
        .center_x(Length::Fixed(TILE_W))
        .center_y(Length::Fixed(TILE_H))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                width: 1.0,
                color: edge,
                radius: RADII.small.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}
