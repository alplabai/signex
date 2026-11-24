//! The row style of the Library panel and the Library Browser (mockup rows
//! 118 and 119): the two-button compact / thumbnail switch, and the tile a
//! thumbnail row starts with. One style serves all three kinds and the
//! Browser. The tile is the row's own drawing: a symbol's body (a part shows
//! its symbol) or a footprint's pads on the board ground.

use std::sync::Arc;

use iced::widget::tooltip::Position;
use iced::widget::{Row, container};
use iced::{Background, Border, Element, Length, Theme, Widget};
use signex_i18n::{I18n, ids, t};
use signex_types::schematic::LibSymbol;
use signex_types::theme::ThemeTokens;
use signex_widgets::button;

use super::footprint_tile::{FootprintTile, footprint_tile};
use super::frame::{SP, glyph};
use crate::library::state::LibraryRowStyle;
use crate::panels::components_panel::thumbs::{ThumbLook, symbol_thumb};
use crate::panels::graphite::hint_tooltip;
use crate::styles::board_palette::BoardPalette;
use crate::styles::tokens::{Overlay, overlay, resolve};
use crate::styles::{ChromeMetrics, Radii, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;
const CM: ChromeMetrics = ChromeMetrics::DEFAULT;
const RADII: Radii = Radii::GRAPHITE;

/// Side of one switch button.
pub(crate) const SWITCH_BUTTON: f32 = CM.row - SP.xs;
/// Height of a thumbnail row: two text lines beside the tile.
pub(super) const THUMB_ROW_H: f32 = 2.0 * CM.row;
/// Height of the thumbnail tile.
pub(crate) const TILE_H: f32 = CM.row + SP.xs;
/// Width of the thumbnail tile: a little wider than tall, like a drawing.
pub(crate) const TILE_W: f32 = TILE_H + SP.m;
/// Side of the glyph a tile shows while its drawing is not read yet.
const TILE_GLYPH: f32 = TS.label;

const COMPACT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><path d="M2 4h12M2 8h12M2 12h12" stroke="currentColor" stroke-width="1.5" fill="none"/></svg>"#;
const THUMBS: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><rect x="2" y="2.5" width="4" height="4" rx="1" stroke="currentColor" fill="none"/><rect x="2" y="9.5" width="4" height="4" rx="1" stroke="currentColor" fill="none"/><path d="M8 4.5h6M8 11.5h6" stroke="currentColor" stroke-width="1.5"/></svg>"#;
const SYMBOL_TILE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><path d="M4.5 3v10l8-5z M1.5 5.5h3M1.5 10.5h3M12.5 8h2" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round"/></svg>"#;
const PART_TILE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><path d="M1 8h3M12 8h3" stroke="currentColor" stroke-width="1.2"/><rect x="4" y="5.5" width="8" height="5" fill="none" stroke="currentColor" stroke-width="1.2"/></svg>"#;

/// How one list row draws: on one line, or as a thumbnail row whose tile
/// is drawn with the given look.
#[derive(Debug, Clone, Copy)]
pub(crate) enum RowLook {
    Compact,
    Thumbnail(ThumbLook),
}

impl RowLook {
    /// The look of a row under `style`; a thumbnail tile draws with `look`.
    pub(crate) fn of(style: LibraryRowStyle, look: ThumbLook) -> Self {
        match style {
            LibraryRowStyle::Compact => Self::Compact,
            LibraryRowStyle::Thumbnail => Self::Thumbnail(look),
        }
    }
}

/// The look the panel's thumbnail tiles draw symbols with: the theme's
/// schematic canvas colours and the window's pixel scale.
pub(super) fn thumb_look(ctx: &crate::panels::PanelContext) -> ThumbLook {
    ThumbLook {
        colors: ctx.canvas_colors,
        device_px_per_logical_px: ctx.device_px_per_logical_px,
    }
}

/// What a thumbnail row's tile draws; `None` while the drawing is not at
/// hand (a symbol still being read), which shows the kind's glyph.
#[derive(Debug, Clone)]
pub(crate) enum RowDrawing {
    Symbol(Option<Arc<LibSymbol>>),
    Footprint(Option<Arc<FootprintTile>>),
    /// A part: its symbol.
    Part(Option<Arc<LibSymbol>>),
}

/// The switch: Compact rows, then Rows with preview thumbnail, the shown
/// style marked. A press sends `on_pick` with the button's style.
pub(crate) fn switch<'a, M: Clone + 'a>(
    style: LibraryRowStyle,
    on_pick: impl Fn(LibraryRowStyle) -> M,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, M> {
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
    let mut group: Row<Element<'a, M>> = Row::new().spacing(0);
    for (value, bytes, tip) in pair {
        group = group.push(hint_tooltip(
            switch_button(bytes, value == style, on_pick(value), tokens),
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

fn switch_button<'a, M: Clone + 'a>(
    bytes: &'static [u8],
    on: bool,
    message: M,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
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
        .on_press(message)
        .style(move |_: &Theme, status: button::Status| button::Style {
            background: (on || matches!(status, button::Status::Hovered | button::Status::Pressed))
                .then_some(Background::Color(hover)),
            text_color: ink,
            ..button::Style::default()
        })
        .boxed()
}

/// The tile a thumbnail row starts with: a footprint's pads on the board
/// ground (the same in both themes), a symbol's body on a sunken card, or
/// the kind's glyph while the symbol is not read yet.
pub(crate) fn tile<'a, M: 'a>(
    drawing: RowDrawing,
    look: ThumbLook,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let r = resolve(tokens);
    let (content, fill): (Element<'a, M>, _) = match drawing {
        RowDrawing::Footprint(Some(pads)) => return footprint_tile(pads, TILE_W, TILE_H),
        RowDrawing::Footprint(None) => (
            iced::widget::Space::new().boxed(),
            BoardPalette::new().canvas,
        ),
        RowDrawing::Symbol(Some(symbol)) | RowDrawing::Part(Some(symbol)) => (
            symbol_thumb(
                symbol,
                look.colors,
                TILE_W,
                TILE_H,
                look.device_px_per_logical_px,
            ),
            r.sunken,
        ),
        RowDrawing::Symbol(None) => (glyph(SYMBOL_TILE, TILE_GLYPH, r.ink3).boxed(), r.sunken),
        RowDrawing::Part(None) => (glyph(PART_TILE, TILE_GLYPH, r.ink3).boxed(), r.sunken),
    };
    let edge = r.line_soft;
    container(content)
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
