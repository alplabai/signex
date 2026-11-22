//! The kind tile a palette row leads with (mockup row 9): a small sunken
//! square holding the glyph of the row's source, tinted with the theme's ink.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{container, svg};
use iced::{Background, Border, Element, Length, Theme, Widget as _};
use signex_types::theme::ThemeTokens;

use crate::app::command_palette::CommandSource;
use crate::styles::{Radii, TypeScale, chrome_px, tokens};

/// Side of the tile (22 px at the 12 px base).
const TILE_SIDE: f32 = chrome_px(22.0 * 11.0 / 12.0);

const COMMAND_GLYPH: &[u8] = include_bytes!("../../../../../assets/icons/chrome/keyboard.svg");
const SYMBOL_GLYPH: &[u8] = include_bytes!("../../../../../assets/icons/component.svg");
const SHEET_GLYPH: &[u8] = include_bytes!("../../../../../assets/icons/menu/sheet.svg");
const FILE_GLYPH: &[u8] = include_bytes!("../../../../../assets/icons/menu/file.svg");
/// A net's wave.
const NET_GLYPH: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 14c2-4 5-5 8-2s6 2 8-2"/></svg>"#;

/// The glyph of a row of `source`.
fn tile_glyph(source: CommandSource) -> &'static [u8] {
    match source {
        CommandSource::Command => COMMAND_GLYPH,
        CommandSource::Symbol => SYMBOL_GLYPH,
        CommandSource::Net => NET_GLYPH,
        CommandSource::Sheet => SHEET_GLYPH,
        CommandSource::File => FILE_GLYPH,
    }
}

/// The tile of a row of `source`.
pub(super) fn kind_tile<'a, M: 'a>(
    theme_tokens: &ThemeTokens,
    source: CommandSource,
) -> Element<'a, M> {
    let r = tokens::resolve(theme_tokens);
    let (fill, ink) = (r.sunken, r.ink3);
    let side = TypeScale::DEFAULT.label;
    let glyph = svg(svg::Handle::from_memory(tile_glyph(source)))
        .width(Length::Fixed(side))
        .height(Length::Fixed(side))
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        });
    container(glyph)
        .width(Length::Fixed(TILE_SIDE))
        .height(Length::Fixed(TILE_SIDE))
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mockup row 9: a row's tile says what kind of result it is, so no two
    /// sources share a glyph.
    #[test]
    fn every_source_has_its_own_tile_glyph() {
        let sources = [
            CommandSource::Command,
            CommandSource::Symbol,
            CommandSource::Net,
            CommandSource::Sheet,
            CommandSource::File,
        ];
        for (i, a) in sources.iter().enumerate() {
            for b in &sources[i + 1..] {
                assert_ne!(tile_glyph(*a), tile_glyph(*b), "{a:?} and {b:?}");
            }
        }
    }
}
