//! The one expand/collapse chevron every tree and collapsible list shares.
//!
//! A single right-pointing stroke glyph; an open row turns it a quarter turn
//! clockwise, so the two states can never drift apart. The box is a square
//! derived from the row's font size, so every surface gets the same hit
//! target for the same text size.

use std::f32::consts::FRAC_PI_2;
use std::sync::OnceLock;

use iced::widget::svg;
use iced::{Color, Radians, Rotation, Theme};

const CHEVRON_RIGHT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 12 12"><path d="M4.5 2.75L7.75 6L4.5 9.25" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>"#;

/// Side of the chevron box in em of the row's font size.
pub const SIDE_EM: f32 = 1.25;

/// Side of the chevron box for a row set at `font` px.
pub const fn side(font: f32) -> f32 {
    font * SIDE_EM
}

/// Turn of the glyph: none when collapsed, a quarter turn when open.
pub fn turn(open: bool) -> Radians {
    Radians(if open { FRAC_PI_2 } else { 0.0 })
}

fn handle() -> svg::Handle {
    static HANDLE: OnceLock<svg::Handle> = OnceLock::new();
    HANDLE
        .get_or_init(|| svg::Handle::from_memory(CHEVRON_RIGHT))
        .clone()
}

/// The chevron for a row set at `font` px, tinted `ink`.
pub fn chevron(open: bool, font: f32, ink: Color) -> svg::Svg<'static> {
    let side = side(font);
    svg(handle())
        .width(side)
        .height(side)
        .rotation(Rotation::Floating(turn(open)))
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_scales_with_the_font() {
        assert_eq!(side(12.0), 12.0 * SIDE_EM);
        assert!(side(12.0) > side(10.5));
        assert!(side(10.5) > 10.0, "larger than the old 8-10 px glyph");
    }

    #[test]
    fn open_is_a_quarter_turn_closed_is_none() {
        assert_eq!(turn(false).0, 0.0);
        assert!((turn(true).0 - FRAC_PI_2).abs() < f32::EPSILON);
    }
}
