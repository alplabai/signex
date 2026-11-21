//! The selected chip of a segmented control: the Neutral and Accent looks in
//! Graphite Dark, an unchanged Graphite Light, and the chip as rendered.

use super::ti;
use super::tokens::{ResolvedTokens, resolve};
use iced::{Color, Element, Size};
use signex_types::theme::{
    Color as ThemeColor, ExtendedTokens, SelectedSegmentLook, ThemeId, ThemeTokens, theme_tokens,
};
use signex_widgets::{SegmentedStyle, raw, segmented};

const BODY: f32 = 12.0;
const WINDOW: Size = Size::new(240.0, 40.0);
const LOOKS: [SelectedSegmentLook; 2] = [SelectedSegmentLook::Neutral, SelectedSegmentLook::Accent];

fn hex(value: &str) -> Color {
    ti(ThemeColor::from_hex(value))
}

fn tokens(id: ThemeId, look: SelectedSegmentLook) -> ThemeTokens {
    look.apply(id, theme_tokens(id))
}

fn control(style: SegmentedStyle) -> Element<'static, bool> {
    segmented(
        [(true, raw("On".to_owned())), (false, raw("Off".to_owned()))],
        true,
        style,
        |picked| picked,
    )
}

fn pixel(pixels: &[u8], x: usize, y: usize) -> [u8; 3] {
    let i = (y * WINDOW.width as usize + x) * 4;
    [pixels[i], pixels[i + 1], pixels[i + 2]]
}

fn rgb8(c: Color) -> [u8; 3] {
    let [r, g, b, _] = c.into_rgba8();
    [r, g, b]
}

fn assert_near(got: [u8; 3], want: [u8; 3], what: &str) {
    let close = got.iter().zip(want).all(|(g, w)| g.abs_diff(w) <= 1);
    assert!(close, "{what}: got {got:?}, want {want:?}");
}

#[test]
fn graphite_dark_neutral_style_has_the_exact_fill_ink_and_shadow() {
    let r = resolve(&tokens(ThemeId::GraphiteDark, SelectedSegmentLook::Neutral));
    let style = r.segmented(BODY);
    assert_eq!(style.active_fill, hex("#3D4047"));
    assert_eq!(style.active_ink, hex("#FFFFFF"));
    assert_eq!(style.shadow, r.shadow);
    assert!(style.shadow.a > 0.0);
}

#[test]
fn graphite_dark_accent_style_has_the_exact_fill_ink_and_no_shadow() {
    let r = resolve(&tokens(ThemeId::GraphiteDark, SelectedSegmentLook::Accent));
    let style = r.segmented(BODY);
    assert_eq!(style.active_fill, hex("#C2552D"));
    assert_eq!(style.active_ink, hex("#FFFFFF"));
    assert_eq!(style.shadow.a, 0.0);
}

#[test]
fn graphite_light_style_is_the_old_chip_under_both_looks() {
    let light = theme_tokens(ThemeId::GraphiteLight);
    let r = resolve(&light);
    let old = SegmentedStyle::new(r.sunken, r.raised, r.shadow, r.ink3, ti(light.text), BODY);
    for look in LOOKS {
        let style = resolve(&tokens(ThemeId::GraphiteLight, look)).segmented(BODY);
        assert_eq!(style, old, "{look:?}");
    }
}

#[test]
fn the_look_leaves_every_other_chrome_colour_alone() {
    let neutral = resolve(&tokens(ThemeId::GraphiteDark, SelectedSegmentLook::Neutral));
    let accent = ResolvedTokens {
        selected_segment: neutral.selected_segment,
        selected_segment_ink: neutral.selected_segment_ink,
        selected_segment_shadow: neutral.selected_segment_shadow,
        ..resolve(&tokens(ThemeId::GraphiteDark, SelectedSegmentLook::Accent))
    };
    assert_eq!(accent, neutral);
    assert_eq!(
        neutral.raised,
        hex("#1D1F23"),
        "cards and menus keep raised"
    );
}

#[test]
fn a_theme_without_segment_tokens_falls_back_to_the_raised_chip() {
    let mut bare = theme_tokens(ThemeId::GraphiteDark);
    bare.extended = ExtendedTokens::NONE;
    let r = resolve(&bare);
    assert_eq!(r.selected_segment, r.raised);
    assert_eq!(r.selected_segment_ink, ti(bare.text));
    assert_eq!(r.selected_segment_shadow, r.shadow);
}

#[test]
fn a_rendered_dark_control_shows_its_chip_apart_from_the_track() {
    for (look, want) in [
        (SelectedSegmentLook::Neutral, "#3D4047"),
        (SelectedSegmentLook::Accent, "#C2552D"),
    ] {
        let r = resolve(&tokens(ThemeId::GraphiteDark, look));
        let style = r.segmented(BODY);
        let size = crate::layout_probe::root_size(control(style), WINDOW);
        let pixels = crate::layout_probe::draw_pixels(control(style), WINDOW);
        let middle = (size.height / 2.0) as usize;
        // Inside each segment's side padding, clear of its label.
        let chip = pixel(&pixels, 6, middle);
        let track = pixel(&pixels, size.width as usize - 6, middle);
        assert_near(chip, rgb8(hex(want)), "selected chip");
        assert_near(track, rgb8(hex("#222429")), "track");
        assert_ne!(chip, track, "{look:?}");
    }
}
