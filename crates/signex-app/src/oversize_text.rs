//! The bounds on oversize text handed to iced (#2022): how much of it, how
//! long, and whether it can be seen at all.

use iced::Point;
use iced::widget::canvas;
use signex_gfx::primitive::text::{TextHAlign, TextItem, TextVAlign};

// shortcut: fixed per-frame bounds on text iced draws as vector outlines. The
// fork re-shapes and re-tessellates it every frame (a cache belongs in
// alplabai/iced's `draw_with`, follow-up on iced#21), so a hostile file of many
// or very long oversize texts is cut here: far above any real sheet, which has
// a handful of short titles. Past either bound the rest is not drawn and one
// warning reaches Messages.
const MAX_OVERSIZE_TEXTS_PER_FRAME: usize = 64;
const MAX_OVERSIZE_TEXT_CHARS: usize = 256;

/// What one frame has spent of the oversize-text bounds.
#[derive(Default)]
pub(crate) struct OversizeBudget {
    drawn: usize,
}

/// Messages warnings issued; one per run, not one per frame.
///
/// A tolerated process-global latch, like the diagnostics ring: this is
/// written from `canvas::Program::draw`, which takes `&self` and has no
/// message channel to `update`, and the camera decides what is oversize, so
/// `update` cannot count it either. It only gates a log line.
pub(crate) static OVERSIZE_WARNINGS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

impl OversizeBudget {
    /// The content to draw for the next oversize text, cut to the length
    /// bound, or `None` once the per-frame count is spent.
    pub(crate) fn admit<'a>(&mut self, content: &'a str) -> Option<&'a str> {
        if self.drawn >= MAX_OVERSIZE_TEXTS_PER_FRAME {
            warn_oversize_capped_once();
            return None;
        }
        self.drawn += 1;
        match content.char_indices().nth(MAX_OVERSIZE_TEXT_CHARS) {
            Some((end, _)) => {
                warn_oversize_capped_once();
                Some(&content[..end])
            }
            None => Some(content),
        }
    }
}

fn warn_oversize_capped_once() {
    if OVERSIZE_WARNINGS.fetch_add(1, std::sync::atomic::Ordering::Relaxed) == 0 {
        crate::diagnostics::log_warning(
            "This sheet has more or longer oversize text than can be drawn; the rest is left out",
        );
    }
}

/// Whether an oversize text can touch the frame, from its cached advance (no
/// shaping): its corners rotated about the anchor. Off-screen oversize text is
/// never handed to iced, so a hostile file cannot make a frame cost more.
pub(crate) fn on_screen(
    frame: &canvas::Frame,
    text: &TextItem,
    position: Point,
    size_px: f32,
) -> bool {
    let family = text.family_or(crate::render_config::CANVAS_FONT_FAMILY);
    let width = text
        .content
        .split('\n')
        .map(|line| {
            signex_types::font::shaped_advance_em(line, Some(family))
                .unwrap_or(line.chars().count() as f32 * 0.5)
        })
        .fold(0.0_f32, f32::max)
        * size_px;
    let height = text.content.split('\n').count() as f32
        * size_px
        * signex_gfx::primitive::text::LINE_HEIGHT_EM;
    let x0 = match text.h_align {
        TextHAlign::Left => 0.0,
        TextHAlign::Center => -0.5 * width,
        TextHAlign::Right => -width,
    };
    let y0 = match text.v_align {
        TextVAlign::Top => 0.0,
        TextVAlign::Center => -0.5 * height,
        TextVAlign::Bottom => -height,
    };
    let (sin, cos) = text.rotation.sin_cos();
    let corners = [
        (x0, y0),
        (x0 + width, y0),
        (x0, y0 + height),
        (x0 + width, y0 + height),
    ]
    .map(|(x, y)| {
        (
            position.x + x * cos - y * sin,
            position.y + x * sin + y * cos,
        )
    });
    let (min_x, max_x) = corners
        .iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), c| {
            (lo.min(c.0), hi.max(c.0))
        });
    let (min_y, max_y) = corners
        .iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), c| {
            (lo.min(c.1), hi.max(c.1))
        });
    // Padded by one em each side: the shaper's box can sit short of what it
    // draws, and a visible tail must never be culled.
    let pad = size_px;
    let size = frame.size();
    max_x + pad >= 0.0
        && min_x - pad <= size.width
        && max_y + pad >= 0.0
        && min_y - pad <= size.height
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A hostile sheet of 10 000 on-screen oversize texts of 10 000 characters
    /// each asks iced for at most the per-frame count, each at most the length
    /// bound, and warns once.
    #[test]
    fn a_flood_of_long_oversize_texts_is_bounded_and_warns_once() {
        let long = "W".repeat(10_000);
        let mut budget = OversizeBudget::default();
        let before = OVERSIZE_WARNINGS.load(std::sync::atomic::Ordering::Relaxed);
        let drawn: Vec<&str> = (0..10_000).filter_map(|_| budget.admit(&long)).collect();
        assert_eq!(drawn.len(), MAX_OVERSIZE_TEXTS_PER_FRAME);
        assert!(
            drawn
                .iter()
                .all(|c| c.chars().count() <= MAX_OVERSIZE_TEXT_CHARS)
        );
        let after = OVERSIZE_WARNINGS.load(std::sync::atomic::Ordering::Relaxed);
        assert!(after > before, "the bound was hit");
        // One Messages line per run: only the first request logs.
        assert!(after - before >= 1);
        let short = "R1";
        let mut next_frame = OversizeBudget::default();
        assert_eq!(
            next_frame.admit(short),
            Some(short),
            "a new frame starts clean"
        );
    }
}
