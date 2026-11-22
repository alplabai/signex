//! Segmented control: a sunken track holding equal-height segments, the
//! selected one drawn as a raised chip. Used as a mode switch (a panel's
//! modes, a two-page toggle). Stateless — the caller owns the selection and
//! maps each pick to a message. Sizes derive from the text size.

use crate::widest_face::WidestFace;
use crate::{FitRow, Label, button, text};
use iced::alignment::{Horizontal, Vertical};
use iced::font::Weight;
use iced::widget::container;
use iced::{
    Background, Border, Color, Element, Font, Length, Padding, Shadow, Theme, Vector, Widget,
};

// Graphite sizes at a 12 px body, kept as ratios of the text size.
const SEGMENT_HEIGHT_EM: f32 = 22.0 / 12.0;
const TRACK_PAD_EM: f32 = 2.0 / 12.0;
const SEGMENT_GAP_EM: f32 = 2.0 / 12.0;
const SEGMENT_PAD_EM: f32 = 10.0 / 12.0;
const TRACK_RADIUS_EM: f32 = 7.0 / 12.0;
const SHADOW_OFFSET_EM: f32 = 1.0 / 12.0;
const SHADOW_BLUR_EM: f32 = 2.0 / 12.0;

/// The face of the selected segment's label.
const ACTIVE_FONT: Font = Font {
    weight: Weight::Medium,
    ..Font::DEFAULT
};

/// Colours for [`segmented`]; they come from the caller's tokens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SegmentedStyle {
    /// Sunken track fill.
    pub track: Color,
    /// Fill of the selected segment (the raised chip).
    pub active_fill: Color,
    /// Drop shadow under the selected segment.
    pub shadow: Color,
    /// Label colour of unselected segments.
    pub ink: Color,
    /// Label colour of the selected segment.
    pub active_ink: Color,
    /// Body text size; every metric derives from it.
    pub text_size: f32,
    /// Take the full width, segments sharing what their labels leave spare,
    /// instead of hugging the labels. Either way the segments fold onto more
    /// lines rather than overlap.
    pub fill_width: bool,
}

impl SegmentedStyle {
    /// The given colours at `text_size`, hugging the labels.
    pub fn new(
        track: Color,
        active_fill: Color,
        shadow: Color,
        ink: Color,
        active_ink: Color,
        text_size: f32,
    ) -> Self {
        Self {
            track,
            active_fill,
            shadow,
            ink,
            active_ink,
            text_size,
            fill_width: false,
        }
    }

    pub fn fill_width(mut self, fill_width: bool) -> Self {
        self.fill_width = fill_width;
        self
    }

    /// The same style with `ink` on the unselected segments.
    pub fn resting_ink(mut self, ink: Color) -> Self {
        self.ink = ink;
        self
    }
}

/// Sizes of a segmented control, derived from its text size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SegmentedMetrics {
    pub segment_height: f32,
    pub track_pad: f32,
    pub track_height: f32,
    pub gap: f32,
    pub segment_pad_h: f32,
    pub track_radius: f32,
    pub segment_radius: f32,
    pub shadow_offset: f32,
    pub shadow_blur: f32,
}

impl SegmentedMetrics {
    pub fn from_text_size(text_size: f32) -> Self {
        let segment_height = text_size * SEGMENT_HEIGHT_EM;
        let track_pad = text_size * TRACK_PAD_EM;
        let track_radius = text_size * TRACK_RADIUS_EM;
        Self {
            segment_height,
            track_pad,
            track_height: segment_height + 2.0 * track_pad,
            gap: text_size * SEGMENT_GAP_EM,
            segment_pad_h: text_size * SEGMENT_PAD_EM,
            track_radius,
            segment_radius: segment_radius(track_radius, track_pad),
            shadow_offset: text_size * SHADOW_OFFSET_EM,
            shadow_blur: text_size * SHADOW_BLUR_EM,
        }
    }
}

/// Corner radius of a segment, concentric with the track and never negative.
pub fn segment_radius(track_radius: f32, track_pad: f32) -> f32 {
    (track_radius - track_pad).max(0.0)
}

/// A segment's label in `font`, or the renderer's default font for `None`.
fn label_face<'a, M: 'a>(
    label: Label<'a>,
    size: f32,
    ink: Color,
    font: Option<Font>,
) -> Element<'a, M> {
    let face = text(label)
        .size(size)
        .color(ink)
        .height(Length::Fill)
        .align_y(Vertical::Center)
        .align_x(Horizontal::Center)
        .wrapping(iced::widget::text::Wrapping::None)
        .ellipsis(iced::widget::text::Ellipsis::End);
    match font {
        Some(font) => face.font(font).into(),
        None => face.into(),
    }
}

/// Segmented control over `options` (value, label). `on_select` maps the
/// picked value to the caller's message; picking the selected one re-emits it.
pub fn segmented<'a, V, M>(
    options: impl IntoIterator<Item = (V, Label<'a>)>,
    selected: V,
    style: SegmentedStyle,
    on_select: impl Fn(V) -> M + 'a,
) -> Element<'a, M>
where
    V: Copy + PartialEq + 'a,
    M: Clone + 'a,
{
    let m = SegmentedMetrics::from_text_size(style.text_size);
    let track_width = if style.fill_width {
        Length::Fill
    } else {
        Length::Shrink
    };

    let mut cells: Vec<Element<'a, M>> = Vec::new();
    for (value, label) in options {
        let is_active = value == selected;
        let ink = if is_active {
            style.active_ink
        } else {
            style.ink
        };
        // Both faces are measured, so picking a segment cannot refold the row.
        let plain = label_face(label.clone(), style.text_size, ink, None);
        let medium = label_face(label, style.text_size, ink, Some(ACTIVE_FONT));
        let label: Element<'a, M> = WidestFace::new(plain, medium, is_active).into();
        let seg = button(label)
            .padding(Padding::from([0.0, m.segment_pad_h]))
            .height(m.segment_height)
            .on_press(on_select(value))
            .style(move |_: &Theme, _| button::Style {
                background: is_active.then_some(Background::Color(style.active_fill)),
                text_color: ink,
                border: Border {
                    radius: m.segment_radius.into(),
                    ..Border::default()
                },
                shadow: if is_active {
                    Shadow {
                        color: style.shadow,
                        offset: Vector::new(0.0, m.shadow_offset),
                        blur_radius: m.shadow_blur,
                    }
                } else {
                    Shadow::default()
                },
                ..button::Style::default()
            });
        cells.push(seg.boxed());
    }

    let cells = FitRow::new(cells).spacing(m.gap).fill(style.fill_width);
    container(cells)
        .padding(m.track_pad)
        .width(track_width)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(style.track)),
            border: Border {
                radius: m.track_radius.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fixture text for the tests.
    fn sample(text: &str) -> Label<'static> {
        crate::raw(text.to_owned())
    }
    use crate::Label;

    #[test]
    fn metrics_at_the_12_px_body_match_the_mockup() {
        let m = SegmentedMetrics::from_text_size(12.0);
        assert!((m.segment_height - 22.0).abs() < 1e-4);
        assert!((m.track_height - 26.0).abs() < 1e-4);
        assert!((m.track_radius - 7.0).abs() < 1e-4);
        assert!((m.segment_radius - 5.0).abs() < 1e-4);
    }

    #[test]
    fn metrics_scale_with_the_text_size() {
        let a = SegmentedMetrics::from_text_size(12.0);
        let b = SegmentedMetrics::from_text_size(18.0);
        assert!((b.track_height - 1.5 * a.track_height).abs() < 1e-4);
        assert!((b.segment_pad_h - 1.5 * a.segment_pad_h).abs() < 1e-4);
    }

    #[test]
    fn segment_radius_is_concentric_and_never_negative() {
        assert_eq!(segment_radius(7.0, 2.0), 5.0);
        assert_eq!(segment_radius(1.0, 2.0), 0.0);
    }

    #[derive(Clone, Copy, PartialEq, Debug)]
    enum Mode {
        Nets,
        NetClasses,
        Polygons,
    }

    #[test]
    fn builds_an_element_for_each_selection() {
        let style = SegmentedStyle::new(
            Color::BLACK,
            Color::WHITE,
            Color::BLACK,
            Color::WHITE,
            Color::BLACK,
            12.0,
        )
        .fill_width(true);
        for sel in [Mode::Nets, Mode::NetClasses, Mode::Polygons] {
            let _: Element<'_, Mode> = segmented(
                [
                    (Mode::Nets, sample("Nets")),
                    (Mode::NetClasses, sample("Net Classes")),
                    (Mode::Polygons, sample("Polygons")),
                ],
                sel,
                style,
                |m| m,
            );
        }
    }
}
