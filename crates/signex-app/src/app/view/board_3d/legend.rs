//! The stack legend under the view cube (mockup row 109, decision of
//! 2026-11-21): the board's layers from the laminate out, each in the
//! material the scene draws it in. It sits on a card so it reads on every
//! lighting preset.

use iced::widget::text::Wrapping;
use iced::widget::{Space, column, container, row};
use iced::{Alignment, Background, Border, Color, Element, Padding, Widget as _};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_renderer::board3d::{Board3dColors, mask_over_copper};
use signex_widgets::{text, theme_ext};

use super::{Look, M, TS, card_style, shadow};
use crate::app::Message;
use crate::pcb_canvas::layer_style::board_3d_colors;
use crate::styles::Radii;

/// One mockup pixel at the current type scale.
const EM: f32 = TS.body / 12.0;
const BAND_WIDTH: f32 = 42.0 * EM;
const SQUARE: f32 = 10.0 * EM;
/// The tallest the legend lays out; its rows are text lines and one band.
const LEGEND_HEIGHT: f32 = 190.0 * EM;
/// What the hint keeps clear at the view's bottom right.
const HINT_ROOM: f32 = 30.0 * EM;

/// How an entry's material is shown.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Swatch {
    /// A layer of the stack: a band this tall.
    Band(f32),
    /// A surface: a small square.
    Square,
}

/// One legend row.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Entry {
    name: MessageId,
    colour: [f32; 4],
    swatch: Swatch,
}

/// The rows, top to bottom: the stack through the board, then the two
/// surfaces the scene tints.
fn entries(c: &Board3dColors) -> [Entry; 9] {
    let band = |name: MessageId, colour: [f32; 4], height: f32| Entry {
        name,
        colour,
        swatch: Swatch::Band(height * EM),
    };
    let square = |name: MessageId, colour: [f32; 4]| Entry {
        name,
        colour,
        swatch: Swatch::Square,
    };
    let silk = band(ids::PCB_3D_LEGEND_SILK, c.silk, 3.0);
    let mask = band(ids::PCB_3D_LEGEND_MASK, c.mask, 5.0);
    let copper = band(ids::PCB_3D_LEGEND_COPPER, c.copper, 4.0);
    [
        silk,
        mask,
        copper,
        band(ids::PCB_3D_LEGEND_LAMINATE, c.substrate, 22.0),
        copper,
        mask,
        silk,
        square(ids::PCB_3D_LEGEND_FINISH, c.finish),
        square(
            ids::PCB_3D_LEGEND_MASK_OVER_COPPER,
            mask_over_copper(c.mask),
        ),
    ]
}

/// Whether a view `height` px tall holds the legend between the cube and
/// the hint.
pub(super) fn legend_fits(height: f32) -> bool {
    height >= M.cube_top() + M.cube + M.popover_gap * 2.0 + LEGEND_HEIGHT + HINT_ROOM
}

/// The legend card.
pub(super) fn stack_legend<'a>(i18n: &I18n, look: &Look) -> Element<'a, Message> {
    let title = text(t!(i18n, ids::PCB_3D_LEGEND_TITLE))
        .size(TS.caption)
        .color(look.text)
        .wrapping(Wrapping::None);
    let mut rows = column![title].spacing(M.bar_gap);
    for entry in entries(&board_3d_colors()) {
        rows = rows.push(entry_row(i18n, entry, look));
    }
    container(rows)
        .padding(Padding::from([M.pill_pad_v, M.pill_pad_h]))
        .style(card_style(
            look,
            Radii::GRAPHITE.small,
            shadow(look, M.bar_shadow),
        ))
        .boxed()
}

fn entry_row<'a>(i18n: &I18n, entry: Entry, look: &Look) -> Element<'a, Message> {
    row![
        swatch(entry, look),
        text(t!(i18n, entry.name))
            .size(TS.caption)
            .color(look.ink2)
            .wrapping(Wrapping::None),
    ]
    .spacing(M.button_gap)
    .align_y(Alignment::Center)
    .boxed()
}

/// The material, opaque so it matches at a glance, with a hairline so white
/// silkscreen shows on a light card.
fn swatch<'a>(entry: Entry, look: &Look) -> Element<'a, Message> {
    let (width, height) = match entry.swatch {
        Swatch::Band(height) => (BAND_WIDTH, height),
        Swatch::Square => (SQUARE, SQUARE),
    };
    let [r, g, b, _] = entry.colour;
    let fill = Color::from_rgb(r, g, b);
    let line = look.r.line_soft;
    container(Space::new())
        .width(width)
        .height(height)
        .style(move |_| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                width: theme_ext::HAIRLINE,
                color: line,
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

#[cfg(test)]
mod tests {
    use iced::Size;
    use signex_types::theme::{ThemeId, theme_tokens};

    use super::*;
    use crate::layout_probe::root_size;

    #[test]
    fn the_legend_reads_from_the_laminate_out_in_the_scene_materials() {
        let c = board_3d_colors();
        let rows = entries(&c);
        let names = rows.map(|e| e.name);
        assert_eq!(
            names[..7],
            [
                ids::PCB_3D_LEGEND_SILK,
                ids::PCB_3D_LEGEND_MASK,
                ids::PCB_3D_LEGEND_COPPER,
                ids::PCB_3D_LEGEND_LAMINATE,
                ids::PCB_3D_LEGEND_COPPER,
                ids::PCB_3D_LEGEND_MASK,
                ids::PCB_3D_LEGEND_SILK,
            ]
        );
        assert_eq!(rows[3].colour, c.substrate);
        assert_eq!(rows[7].colour, c.finish);
        assert_eq!(rows[8].colour, mask_over_copper(c.mask));
    }

    #[test]
    fn the_legend_stays_within_the_height_it_reserves() {
        for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let tokens = theme_tokens(id);
            let look = Look::of(&tokens);
            let legend = stack_legend(I18n::en_us_ref(), &look);
            let size = root_size(legend, Size::new(600.0, 600.0));
            assert!(size.height <= LEGEND_HEIGHT, "{id:?}: {size:?}");
        }
        assert!(legend_fits(600.0));
        assert!(!legend_fits(200.0));
    }
}
