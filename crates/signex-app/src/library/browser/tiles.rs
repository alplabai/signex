//! The symbol and footprint tiles of a component preview, shared by the
//! Library Browser's preview pane (mockup row 118) and the Place Component
//! picker's preview column (mockup row 129), so both say the same thing
//! for the same row.

use iced::widget::Space;
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{text, theme_ext};

use super::PreviewLook;
use crate::library::editor::kit;
use crate::library::messages::LibraryMessage;
use crate::panels::components_panel::details::ComponentDetails;

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;

/// What the tiles of one row draw from.
pub(crate) struct TileSource<'a> {
    /// The read of the row, once it is that row's.
    pub(crate) details: Option<&'a ComponentDetails>,
    /// The row's read is running, or has not started yet.
    pub(crate) loading: bool,
    /// The row binds no symbol (`ComponentRow::is_symbol_unbound`).
    pub(crate) symbol_unbound: bool,
    /// The row binds no footprint (`ComponentRow::is_footprint_unbound`).
    pub(crate) footprint_unbound: bool,
}

/// A tile's text in place of a drawing: amber for an unbound side, muted
/// while loading or when the read failed.
fn tile_note<'a>(
    message: Localized,
    warn: bool,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let ink = if warn {
        theme_ext::warning_color(tokens)
    } else {
        crate::styles::tokens::resolve(tokens).ink4
    };
    text(message)
        .size(TS.caption)
        .color(ink)
        .align_x(iced::alignment::Horizontal::Center)
        .boxed()
}

/// What a bound side without a drawing says: loading, else unreadable.
fn missing_drawing<'a>(
    source: &TileSource<'_>,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let id = if source.loading {
        ids::LIBRARY_BROWSER_PREVIEW_LOADING
    } else {
        ids::LIBRARY_BROWSER_PREVIEW_UNREADABLE
    };
    tile_note(t!(i18n, id), false, tokens)
}

/// The row's symbol tile, `height` tall. `None` (no row) leaves it empty.
pub(crate) fn symbol_tile<'a>(
    source: Option<&TileSource<'a>>,
    look: PreviewLook,
    height: f32,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let content: Element<'a, LibraryMessage> = match source {
        None => Space::new().boxed(),
        Some(s) if s.symbol_unbound => {
            tile_note(t!(i18n, ids::LIBRARY_BROWSER_NO_SYMBOL_BOUND), true, tokens)
        }
        Some(s) => match s.details.and_then(|d| d.symbol.as_ref()) {
            Some(symbol) => {
                crate::panels::components_panel::details::symbol_canvas::symbol_preview(
                    symbol.clone(),
                    look.canvas_colors,
                    look.grid,
                    height - 2.0 * crate::styles::HAIRLINE,
                    look.device_px_per_logical_px,
                )
                .map(|()| LibraryMessage::Noop)
                .boxed()
            }
            None => missing_drawing(s, tokens, i18n),
        },
    };
    let tag = Some(t!(i18n, ids::LIBRARY_PREVIEW_SYMBOL));
    kit::preview_tile(content, height, None, tag, tokens)
}

/// The row's footprint tile on the board ground, `height` tall. `None` (no
/// row) leaves it empty.
pub(crate) fn footprint_tile<'a>(
    source: Option<&TileSource<'a>>,
    height: f32,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let content: Element<'a, LibraryMessage> = match source {
        None => Space::new().boxed(),
        Some(s) if s.footprint_unbound => tile_note(
            t!(i18n, ids::LIBRARY_BROWSER_NO_FOOTPRINT_BOUND),
            true,
            tokens,
        ),
        Some(s) => match s.details.and_then(|d| d.footprint.as_ref()) {
            Some(model) => {
                let program =
                    crate::panels::components_panel::details::footprint_canvas::FootprintPreview::new(
                        model,
                    )
                    .with_theme_tokens(tokens);
                iced::widget::canvas(program)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .boxed()
            }
            None => missing_drawing(s, tokens, i18n),
        },
    };
    // The board palette and ground, as the PCB canvas paints them.
    let ground = crate::library::editor::footprint::graphite_look_for(tokens).bg;
    let tag = Some(t!(i18n, ids::LIBRARY_PREVIEW_FOOTPRINT));
    kit::preview_tile(content, height, Some(ground), tag, tokens)
}
