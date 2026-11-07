//! Segmented control: a sunken track holding equal-height segments, the
//! selected one drawn as a raised chip. Stateless — the caller owns the
//! selection and maps each pick to a message.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{Row, button, container, text};
use iced::{Background, Border, Color, Element, Length, Padding, Theme};

/// Track height in px.
pub const TRACK_HEIGHT: f32 = 28.0;
/// Gap between the track edge and the segments.
pub const TRACK_PAD: f32 = 2.0;
/// Track corner radius.
pub const TRACK_RADIUS: f32 = 7.0;
/// Horizontal padding inside a segment.
pub const SEGMENT_PAD_H: f32 = 12.0;

/// Colours and metrics for [`segmented`]; colours come from the caller's tokens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SegmentedStyle {
    /// Sunken track fill.
    pub track: Color,
    /// Fill of the selected segment (the raised chip).
    pub active_fill: Color,
    /// Outline of the selected segment.
    pub active_edge: Color,
    /// Label colour of unselected segments.
    pub ink: Color,
    /// Label colour of the selected segment.
    pub active_ink: Color,
    pub text_size: f32,
    pub track_height: f32,
    pub track_radius: f32,
    /// Stretch segments to share the full width instead of hugging labels.
    pub fill_width: bool,
}

impl SegmentedStyle {
    /// Default metrics (28 px track, radius 7, 12 px labels) with the given colours.
    pub fn new(
        track: Color,
        active_fill: Color,
        active_edge: Color,
        ink: Color,
        active_ink: Color,
    ) -> Self {
        Self {
            track,
            active_fill,
            active_edge,
            ink,
            active_ink,
            text_size: 12.0,
            track_height: TRACK_HEIGHT,
            track_radius: TRACK_RADIUS,
            fill_width: false,
        }
    }

    pub fn text_size(mut self, text_size: f32) -> Self {
        self.text_size = text_size;
        self
    }

    pub fn fill_width(mut self, fill_width: bool) -> Self {
        self.fill_width = fill_width;
        self
    }
}

/// Height of one segment inside a track of `track_height`.
pub fn segment_height(track_height: f32) -> f32 {
    (track_height - 2.0 * TRACK_PAD).max(0.0)
}

/// Corner radius of a segment, concentric with the track.
pub fn segment_radius(track_radius: f32) -> f32 {
    (track_radius - TRACK_PAD).max(0.0)
}

/// Segmented control over `options` (value, label). `on_select` maps the
/// picked value to the caller's message; picking the selected one re-emits it.
pub fn segmented<'a, V, M>(
    options: &[(V, &str)],
    selected: V,
    style: SegmentedStyle,
    on_select: impl Fn(V) -> M + 'a,
) -> Element<'a, M>
where
    V: Copy + PartialEq + 'a,
    M: Clone + 'a,
{
    let seg_h = segment_height(style.track_height);
    let seg_r = segment_radius(style.track_radius);
    let seg_w = if style.fill_width {
        Length::Fill
    } else {
        Length::Shrink
    };

    let mut row: Row<'a, M> = Row::new().spacing(0.0);
    for (value, label) in options {
        let is_active = *value == selected;
        let ink = if is_active {
            style.active_ink
        } else {
            style.ink
        };
        let label = text((*label).to_string())
            .size(style.text_size)
            .color(ink)
            .height(Length::Fill)
            .align_y(Vertical::Center)
            .align_x(Horizontal::Center)
            .width(seg_w);
        let seg = button(label)
            .padding(Padding::from([0.0, SEGMENT_PAD_H]))
            .width(seg_w)
            .height(seg_h)
            .on_press(on_select(*value))
            .style(move |_: &Theme, _| button::Style {
                background: is_active.then_some(Background::Color(style.active_fill)),
                text_color: ink,
                border: Border {
                    color: if is_active {
                        style.active_edge
                    } else {
                        Color::TRANSPARENT
                    },
                    width: if is_active { 1.0 } else { 0.0 },
                    radius: seg_r.into(),
                },
                ..button::Style::default()
            });
        row = row.push(seg);
    }

    container(row)
        .padding(TRACK_PAD)
        .width(if style.fill_width {
            Length::Fill
        } else {
            Length::Shrink
        })
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(style.track)),
            border: Border {
                radius: style.track_radius.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_track_leaves_a_24_px_segment() {
        assert_eq!(segment_height(TRACK_HEIGHT), 24.0);
    }

    #[test]
    fn segment_radius_is_concentric_and_never_negative() {
        assert_eq!(segment_radius(TRACK_RADIUS), 5.0);
        assert_eq!(segment_radius(1.0), 0.0);
    }

    #[test]
    fn tiny_track_clamps_segment_height_to_zero() {
        assert_eq!(segment_height(3.0), 0.0);
    }

    #[derive(Clone, Copy, PartialEq, Debug)]
    enum Mode {
        A,
        B,
    }

    #[test]
    fn builds_an_element_for_each_selection() {
        let style = SegmentedStyle::new(
            Color::BLACK,
            Color::WHITE,
            Color::WHITE,
            Color::WHITE,
            Color::BLACK,
        )
        .fill_width(true);
        for sel in [Mode::A, Mode::B] {
            let _: Element<'_, Mode> = segmented(
                &[(Mode::A, "Assistant"), (Mode::B, "Generator")],
                sel,
                style,
                |m| m,
            );
        }
    }
}
