//! Every on-screen size of an illustration, derived from the caption size.
//!
//! The ratios match the approved line-art drawings, where the caption is
//! 8.4 view units and a regular stroke is one unit.

use super::primitive::TextRole;

const STROKE_EM: f32 = 0.12;
const ARROW_LENGTH_EM: f32 = 0.5;
const ARROW_HALF_WIDTH_EM: f32 = 0.2;
/// A span shorter than this puts the arrow heads outside.
const SHORT_SPAN_EM: f32 = 1.4;
/// How far the dimension line runs past a short span.
const SHORT_OVERHANG_EM: f32 = 1.07;
const LABEL_GAP_EM: f32 = 0.5;
const DASH_EM: f32 = 0.36;
const DASH_GAP_EM: f32 = 0.24;
const LEADER_DOT_EM: f32 = 0.15;
const NOTE_SCALE: f32 = 0.93;
const MONO_SCALE: f32 = 1.07;

/// Sizes in logical pixels for one caption size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    pub text_size: f32,
}

impl Metrics {
    /// Metrics for a caption of `text_size` logical pixels (the type scale's
    /// caption step).
    pub fn new(text_size: f32) -> Self {
        Self {
            text_size: text_size.max(1.0),
        }
    }

    fn em(&self, factor: f32) -> f32 {
        self.text_size * factor
    }

    pub fn stroke(&self) -> f32 {
        self.em(STROKE_EM)
    }

    pub fn arrow_length(&self) -> f32 {
        self.em(ARROW_LENGTH_EM)
    }

    pub fn arrow_half_width(&self) -> f32 {
        self.em(ARROW_HALF_WIDTH_EM)
    }

    pub fn short_span(&self) -> f32 {
        self.em(SHORT_SPAN_EM)
    }

    pub fn short_overhang(&self) -> f32 {
        self.em(SHORT_OVERHANG_EM)
    }

    pub fn label_gap(&self) -> f32 {
        self.em(LABEL_GAP_EM)
    }

    /// Dash and gap lengths of a dashed stroke.
    pub fn dash(&self) -> [f32; 2] {
        [self.em(DASH_EM), self.em(DASH_GAP_EM)]
    }

    pub fn leader_dot(&self) -> f32 {
        self.em(LEADER_DOT_EM)
    }

    pub fn text(&self, role: TextRole) -> f32 {
        match role {
            TextRole::Caption => self.text_size,
            TextRole::Note => self.em(NOTE_SCALE),
            TextRole::Mono => self.em(MONO_SCALE),
        }
    }

    /// Widget height for an illustration `height_em` captions tall.
    pub fn height(&self, height_em: f32) -> f32 {
        self.em(height_em)
    }
}
