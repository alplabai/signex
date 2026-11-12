//! #2262: the command palette dims the app with the theme's modal scrim, the
//! one dialogs use, on Graphite Light as on Graphite Dark.

use iced::{Color, Size};
use signex_types::theme::{ThemeId, theme_tokens};

use crate::app::Signex;
use crate::layout_probe::draw_pixels;
use crate::styles::ti;
use crate::styles::tokens::{composite_over, contrast_ratio, resolve};

const THEMES: [ThemeId; 2] = [ThemeId::GraphiteLight, ThemeId::GraphiteDark];

/// The colour of the palette's backdrop layer under `id`, as drawn over the
/// probe's black background.
fn drawn_backdrop(id: ThemeId) -> [u8; 3] {
    let (mut app, _task) = Signex::new();
    app.document_state.panel_ctx.tokens = theme_tokens(id);
    app.ui_state.command_palette.open = true;
    let window = iced::window::Id::unique();
    let backdrop = app
        .command_palette_overlay(window)
        .into_iter()
        .next()
        .expect("an open palette paints its backdrop first");
    let pixels = draw_pixels(backdrop, Size::new(8.0, 8.0));
    [pixels[0], pixels[1], pixels[2]]
}

fn to_rgb8(c: Color) -> [u8; 3] {
    let channel = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    [channel(c.r), channel(c.g), channel(c.b)]
}

#[test]
fn the_palette_backdrop_is_the_theme_scrim() {
    for id in THEMES {
        let expected = to_rgb8(composite_over(
            resolve(&theme_tokens(id)).scrim,
            Color::BLACK,
        ));
        let drawn = drawn_backdrop(id);
        for (d, e) in drawn.iter().zip(expected) {
            assert!(
                d.abs_diff(e) <= 2,
                "{id:?}: backdrop {drawn:?}, theme scrim over black {expected:?}"
            );
        }
    }
}

/// How visibly `scrim` dims `surface`: the contrast between the surface and
/// the surface under the scrim.
fn dimming(scrim: Color, surface: Color) -> f32 {
    contrast_ratio(composite_over(scrim, surface), surface)
}

#[test]
fn light_dims_the_panels_at_least_as_visibly_as_dark() {
    let [light, dark] = THEMES.map(|id| {
        let t = theme_tokens(id);
        dimming(resolve(&t).scrim, ti(t.panel_bg))
    });
    assert!(
        light >= dark,
        "the Light scrim dims less than the Dark one: {light} < {dark}"
    );
}
