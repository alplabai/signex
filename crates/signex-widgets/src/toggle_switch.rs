//! Toggle switch: a pill track with a sliding knob. Stateless — the caller
//! owns the on/off value and passes the message a press should emit.

use iced::alignment::Horizontal;
use iced::widget::{Space, button, container};
use iced::{Background, Border, Color, Element, Length, Theme};

/// Track width in px.
pub const TRACK_WIDTH: f32 = 32.0;
/// Track height in px.
pub const TRACK_HEIGHT: f32 = 18.0;
/// Gap between the track edge and the knob.
pub const KNOB_INSET: f32 = 2.0;

/// Colours for [`toggle_switch`]; they come from the caller's tokens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToggleSwitchStyle {
    pub on_track: Color,
    pub off_track: Color,
    pub knob: Color,
}

impl ToggleSwitchStyle {
    pub fn new(on_track: Color, off_track: Color, knob: Color) -> Self {
        Self {
            on_track,
            off_track,
            knob,
        }
    }
}

/// Knob diameter for a track of `track_height`.
pub fn knob_diameter(track_height: f32) -> f32 {
    (track_height - 2.0 * KNOB_INSET).max(0.0)
}

/// Which end of the track the knob rests at.
pub fn knob_side(is_on: bool) -> Horizontal {
    if is_on {
        Horizontal::Right
    } else {
        Horizontal::Left
    }
}

/// Fade applied to a disabled switch.
const DISABLED_ALPHA: f32 = 0.5;

fn faded(c: Color, enabled: bool) -> Color {
    if enabled {
        c
    } else {
        Color {
            a: c.a * DISABLED_ALPHA,
            ..c
        }
    }
}

/// Toggle switch showing `is_on`. `on_press` is the message a press emits
/// (typically the flipped value); `None` renders a faded, inert switch.
pub fn toggle_switch<'a, M: Clone + 'a>(
    is_on: bool,
    style: ToggleSwitchStyle,
    on_press: Option<M>,
) -> Element<'a, M> {
    let enabled = on_press.is_some();
    let d = knob_diameter(TRACK_HEIGHT);
    let knob_c = faded(style.knob, enabled);
    let track_c = faded(
        if is_on {
            style.on_track
        } else {
            style.off_track
        },
        enabled,
    );

    let knob =
        container(Space::new().width(d).height(d)).style(move |_: &Theme| container::Style {
            background: Some(Background::Color(knob_c)),
            border: Border {
                radius: (d / 2.0).into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    let track = container(knob)
        .width(TRACK_WIDTH)
        .height(TRACK_HEIGHT)
        .padding(KNOB_INSET)
        .align_x(knob_side(is_on))
        .align_y(iced::alignment::Vertical::Center)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(track_c)),
            border: Border {
                radius: (TRACK_HEIGHT / 2.0).into(),
                ..Border::default()
            },
            ..container::Style::default()
        });

    let mut b = button(track)
        .padding(0.0)
        .width(Length::Shrink)
        .style(|_: &Theme, _| button::Style::default());
    if let Some(m) = on_press {
        b = b.on_press(m);
    }
    b.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knob_fits_inside_the_track_with_an_inset() {
        assert_eq!(knob_diameter(TRACK_HEIGHT), 14.0);
        assert_eq!(knob_diameter(1.0), 0.0);
    }

    #[test]
    fn knob_rests_right_when_on_and_left_when_off() {
        assert_eq!(knob_side(true), Horizontal::Right);
        assert_eq!(knob_side(false), Horizontal::Left);
    }

    #[test]
    fn disabled_colours_are_half_transparent() {
        let c = Color::from_rgb(1.0, 0.0, 0.0);
        assert_eq!(faded(c, true).a, 1.0);
        assert_eq!(faded(c, false).a, 0.5);
    }

    #[test]
    fn builds_enabled_and_disabled_elements() {
        let s = ToggleSwitchStyle::new(Color::BLACK, Color::WHITE, Color::WHITE);
        let _: Element<'_, bool> = toggle_switch(true, s, Some(false));
        let _: Element<'_, bool> = toggle_switch(false, s, None);
    }
}
