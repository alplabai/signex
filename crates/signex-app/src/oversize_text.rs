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
pub(crate) const MAX_OVERSIZE_TEXTS_PER_FRAME: usize = 64;
const MAX_OVERSIZE_TEXT_CHARS: usize = 256;

/// What one frame has spent of the oversize-text bounds.
pub(crate) struct OversizeBudget {
    drawn: usize,
    /// A text was left out or cut this frame.
    capped: bool,
    /// The warnings this budget sent to Messages.
    warnings: Vec<&'static str>,
    /// Counts the cap notices; the warning goes out on the first.
    latch: &'static std::sync::atomic::AtomicUsize,
}

impl Default for OversizeBudget {
    fn default() -> Self {
        Self::with_latch(&OVERSIZE_WARNINGS)
    }
}

/// Messages warnings issued; one per run, not one per frame.
///
/// A tolerated process-global latch, like the diagnostics ring: this is
/// written from `canvas::Program::draw`, which takes `&self` and has no
/// message channel to `update`, and the camera decides what is oversize, so
/// `update` cannot count it either. It only gates a log line, and opening a
/// sheet re-arms it ([`rearm_warning`]).
pub(crate) static OVERSIZE_WARNINGS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// Let the next sheet with too much oversize text warn again.
pub(crate) fn rearm_warning() {
    OVERSIZE_WARNINGS.store(0, std::sync::atomic::Ordering::Relaxed);
}

impl OversizeBudget {
    /// A budget that counts its cap notices on `latch` instead of the
    /// process-global one.
    fn with_latch(latch: &'static std::sync::atomic::AtomicUsize) -> Self {
        Self {
            drawn: 0,
            capped: false,
            warnings: Vec::new(),
            latch,
        }
    }

    /// The content to draw for the next oversize text, cut to the length
    /// bound, or `None` once the per-frame count is spent.
    pub(crate) fn admit<'a>(&mut self, content: &'a str) -> Option<&'a str> {
        if self.drawn >= MAX_OVERSIZE_TEXTS_PER_FRAME {
            self.note_capped(CAP_WARNING);
            return None;
        }
        self.drawn += 1;
        let shown = drawn_part(content);
        if shown.len() < content.len() {
            self.note_capped(CAP_WARNING);
        }
        Some(shown)
    }

    /// The content each oversize text draws, `None` for one left out or not
    /// oversize (`None` in `texts`). Texts in the viewport claim the
    /// per-frame count before texts in the pan margin, each in document
    /// order, so the drawn set is stable from frame to frame (#2084).
    pub(crate) fn admit_all<'a>(
        &mut self,
        texts: &[Option<(&'a str, Reach)>],
    ) -> Vec<Option<&'a str>> {
        let mut shown = vec![None; texts.len()];
        for class in [Reach::Viewport, Reach::Margin] {
            for (slot, &(content, reach)) in shown
                .iter_mut()
                .zip(texts)
                .filter_map(|(slot, text)| Some((slot, text.as_ref()?)))
            {
                if reach == class {
                    *slot = match class {
                        Reach::Viewport => self.admit(content),
                        Reach::Margin => self.admit_margin(content),
                    };
                }
            }
        }
        shown
    }

    /// [`Self::admit`] for a text in the pan margin: when the count runs out
    /// on the margin first, the warning says so.
    fn admit_margin<'a>(&mut self, content: &'a str) -> Option<&'a str> {
        if self.drawn < MAX_OVERSIZE_TEXTS_PER_FRAME {
            return self.admit(content);
        }
        self.note_capped(if !self.capped {
            MARGIN_CAP_WARNING
        } else {
            CAP_WARNING
        });
        None
    }

    /// Record a cap; the first one after the latch was armed warns.
    fn note_capped(&mut self, warning: &'static str) {
        self.capped = true;
        if first_notice(self.latch) {
            self.warnings.push(warning);
            crate::diagnostics::log_warning(warning);
        }
    }
}

const CAP_WARNING: &str =
    "This sheet has more or longer oversize text than can be drawn; the rest is left out";

const MARGIN_CAP_WARNING: &str = "This sheet has more oversize text just outside the view \
     than can be drawn; the text in view is drawn, some around it is left out";

/// Where an oversize text that can touch the frame lies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reach {
    /// It can touch the viewport.
    Viewport,
    /// Only the pan margin the retained layer draws past the viewport.
    Margin,
}

/// Whether this is the first notice on `latch` since it was armed.
fn first_notice(latch: &std::sync::atomic::AtomicUsize) -> bool {
    latch.fetch_add(1, std::sync::atomic::Ordering::Relaxed) == 0
}

/// No glyph of any face in use is wider than this many em; a box built on it
/// holds any shaping of the text, so it can cull without shaping.
// shortcut: a flat 2 em per char, wider than any real glyph; tighten only if a
// face with wider glyphs shows up.
const MAX_GLYPH_ADVANCE_EM: f32 = 2.0;

/// The part of `content` that is drawn, per the length bound.
fn drawn_part(content: &str) -> &str {
    match content.char_indices().nth(MAX_OVERSIZE_TEXT_CHARS) {
        Some((end, _)) => &content[..end],
        None => content,
    }
}

/// Whether an oversize text can touch the viewport, only the pan margin of
/// the frame, or neither (`None`, culled). Only the part that is drawn is
/// measured, and a text whose widest possible box misses is culled before
/// any shaping, so a frame never shapes more than a few short lines per
/// near-screen text. Its corners are rotated about the anchor.
///
/// The frame's top-left is `options.frame_origin`, in the coordinates
/// `position` is in: the retained content layer is drawn into a frame that
/// reaches a viewport past the viewport on every side, and the fork's
/// `Frame::size()` does not say where it starts.
pub(crate) fn reach(
    frame: &canvas::Frame,
    options: crate::renderer_scene_canvas::SceneDrawOptions,
    text: &TextItem,
    position: Point,
    size_px: f32,
) -> Option<Reach> {
    let rect = iced::Rectangle::new(options.frame_origin, frame.size());
    reach_in(rect, text, position, size_px)
}

/// [`reach`] for a frame covering `frame`.
fn reach_in(
    frame: iced::Rectangle,
    text: &TextItem,
    position: Point,
    size_px: f32,
) -> Option<Reach> {
    let width = shaped_width(frame, text, position, size_px)?;
    let touches = |area| box_touches(area, text, position, size_px, width);
    if touches(viewport_of(frame)) {
        Some(Reach::Viewport)
    } else {
        touches(frame).then_some(Reach::Margin)
    }
}

/// The viewport inside `frame`: the screen origin, with the frame's margin
/// cut off every side. The retained layer's clip reaches the same distance
/// past each side (`canvas::draw::retained::clip_rect`); a frame that starts
/// at the origin is all viewport.
fn viewport_of(frame: iced::Rectangle) -> iced::Rectangle {
    iced::Rectangle::new(
        Point::ORIGIN,
        iced::Size::new(frame.width + 2.0 * frame.x, frame.height + 2.0 * frame.y),
    )
}

/// The shaped width in px of what `text` draws, shaped once per text (#2254),
/// or `None` when even its widest possible box misses `frame`, before any
/// shaping. The viewport lies inside `frame`, so this bound holds for it too.
fn shaped_width(
    frame: iced::Rectangle,
    text: &TextItem,
    position: Point,
    size_px: f32,
) -> Option<f32> {
    let shown = drawn_part(&text.content);
    let upper = shown.chars().count() as f32 * MAX_GLYPH_ADVANCE_EM * size_px;
    if !box_touches(frame, text, position, size_px, upper) {
        return None;
    }
    let family = text.family_or(crate::render_config::CANVAS_FONT_FAMILY);
    let width = shown
        .split('\n')
        .map(|line| {
            signex_types::font::shaped_advance_em(line, Some(family))
                .unwrap_or(line.chars().count() as f32 * 0.5)
        })
        .fold(0.0_f32, f32::max)
        * size_px;
    Some(width)
}

/// Whether `text`, `width` px wide, anchored at `position`, can touch a frame
/// covering `frame`.
fn box_touches(
    frame: iced::Rectangle,
    text: &TextItem,
    position: Point,
    size_px: f32,
    width: f32,
) -> bool {
    let height = drawn_part(&text.content).split('\n').count() as f32
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
    max_x + pad >= frame.x
        && min_x - pad <= frame.x + frame.width
        && max_y + pad >= frame.y
        && min_y - pad <= frame.y + frame.height
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A hostile sheet of 10 000 on-screen oversize texts of 10 000 characters
    /// each asks iced for at most the per-frame count, each at most the length
    /// bound, and warns once.
    #[test]
    fn a_flood_of_long_oversize_texts_is_bounded_and_warns_once() {
        static LATCH: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let long = "W".repeat(10_000);
        let mut budget = OversizeBudget::with_latch(&LATCH);
        let drawn: Vec<&str> = (0..10_000).filter_map(|_| budget.admit(&long)).collect();
        assert_eq!(drawn.len(), MAX_OVERSIZE_TEXTS_PER_FRAME);
        assert!(
            drawn
                .iter()
                .all(|c| c.chars().count() <= MAX_OVERSIZE_TEXT_CHARS)
        );
        // The budget's own flag, not the process-global warning latch, which
        // other tests re-arm in parallel.
        assert!(budget.capped, "the bound was hit");
        // This test's own latch: every cap notice reached it, and only the
        // first of them warns.
        assert!(LATCH.load(std::sync::atomic::Ordering::Relaxed) > 1);
        assert_eq!(budget.warnings, [CAP_WARNING], "many caps, one warning");
        let fresh = std::sync::atomic::AtomicUsize::new(0);
        assert!(first_notice(&fresh), "the first notice warns");
        assert!(!first_notice(&fresh), "later ones do not");
        assert!(!first_notice(&fresh));
        let short = "R1";
        let mut next_frame = OversizeBudget::default();
        assert_eq!(
            next_frame.admit(short),
            Some(short),
            "a new frame starts clean"
        );
        assert!(!next_frame.capped);
    }

    const FRAME: iced::Rectangle = iced::Rectangle {
        x: 0.0,
        y: 0.0,
        width: 200.0,
        height: 100.0,
    };

    /// Whether an oversize text can touch a frame covering `frame`.
    fn on_screen_in(
        frame: iced::Rectangle,
        text: &TextItem,
        position: Point,
        size_px: f32,
    ) -> bool {
        reach_in(frame, text, position, size_px).is_some()
    }

    fn note(rotation: f32) -> TextItem {
        TextItem {
            content: "NOTE".into(),
            rotation,
            ..Default::default()
        }
    }

    /// Off-screen oversize text never reaches iced: a box fully off the frame
    /// is culled, one whose one-em padding reaches the frame is kept, and a
    /// rotated box is judged by its rotated corners.
    #[test]
    fn oversize_text_off_the_frame_is_culled_and_a_visible_tail_is_kept() {
        let em = 10.0;
        let at = |x: f32, y: f32, rotation: f32| {
            box_touches(FRAME, &note(rotation), Point::new(x, y), em, 100.0)
        };
        assert!(at(10.0, 10.0, 0.0), "on the frame");
        assert!(!at(-300.0, 10.0, 0.0), "far left");
        assert!(!at(500.0, 10.0, 0.0), "far right");
        assert!(!at(10.0, -300.0, 0.0), "far above");
        assert!(
            at(-105.0, 10.0, 0.0),
            "the box ends 5 px left, inside the em of padding"
        );
        assert!(!at(-115.0, 10.0, 0.0), "the box ends 15 px left, past it");
        // A 100 px run turned a quarter turn hangs 100 px below its anchor.
        let quarter = std::f32::consts::FRAC_PI_2;
        assert!(!at(50.0, -60.0, 0.0), "flat above the frame");
        assert!(
            at(50.0, -60.0, quarter),
            "turned, it reaches into the frame"
        );
    }

    /// The full predicate culls without needing a measured width: a text
    /// far from the frame is off, one on it is on.
    #[test]
    fn on_screen_culls_a_far_text_and_keeps_a_near_one() {
        assert!(!on_screen_in(
            FRAME,
            &note(0.0),
            Point::new(10_000.0, 10_000.0),
            10.0
        ));
        assert!(on_screen_in(
            FRAME,
            &note(0.0),
            Point::new(10.0, 10.0),
            10.0
        ));
    }

    /// The retained content layer's frame starts a viewport left of and above
    /// the viewport: a text in that margin is inside the frame and is kept,
    /// one past its far side is not.
    #[test]
    fn a_frame_that_does_not_start_at_the_origin_keeps_its_margin() {
        let retained = iced::Rectangle::new(
            Point::new(-1500.0, -1000.0),
            iced::Size::new(4500.0, 3000.0),
        );
        let text = TextItem {
            content: "H".into(),
            ..Default::default()
        };
        let at = |x: f32, y: f32| on_screen_in(retained, &text, Point::new(x, y), 10.0);
        assert!(at(-1300.0, 500.0), "the left margin is inside the frame");
        assert!(at(500.0, -900.0), "so is the top margin");
        assert!(!at(4000.0, 500.0), "past the right edge it is culled");
        assert!(!at(-1700.0, 500.0), "and past the left one");
    }

    /// The retained frame reaches a viewport past the viewport on every side,
    /// so the viewport is the frame less that margin; a frame starting at the
    /// origin is all viewport.
    #[test]
    fn a_text_is_in_the_viewport_or_in_the_pan_margin() {
        let retained = iced::Rectangle::new(
            Point::new(-1500.0, -1000.0),
            iced::Size::new(4500.0, 3000.0),
        );
        let at = |frame: iced::Rectangle, x: f32, y: f32| {
            reach_in(frame, &note(0.0), Point::new(x, y), 10.0)
        };
        assert_eq!(at(retained, 500.0, 500.0), Some(Reach::Viewport));
        assert_eq!(at(retained, -1300.0, 500.0), Some(Reach::Margin), "left");
        assert_eq!(at(retained, 1700.0, 500.0), Some(Reach::Margin), "right");
        assert_eq!(at(retained, 500.0, 1300.0), Some(Reach::Margin), "below");
        assert_eq!(at(retained, 4000.0, 500.0), None, "past the frame");
        assert_eq!(at(FRAME, 10.0, 10.0), Some(Reach::Viewport));
    }

    /// Oversize texts in the pan margin ahead of a visible one in document
    /// order cannot spend the budget it needs (#2084): the viewport claims
    /// first, the margin gets the rest in document order, and the warning
    /// names the margin as what was left out.
    #[test]
    fn margin_texts_ahead_of_a_visible_one_do_not_take_its_place() {
        static LATCH: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let margin_count = MAX_OVERSIZE_TEXTS_PER_FRAME + 6;
        let mut texts: Vec<Option<(&str, Reach)>> = vec![Some(("M", Reach::Margin)); margin_count];
        texts.insert(3, None);
        texts.push(Some(("V", Reach::Viewport)));
        let mut budget = OversizeBudget::with_latch(&LATCH);
        let shown = budget.admit_all(&texts);
        assert_eq!(shown.len(), texts.len(), "one answer per text, in order");
        assert_eq!(shown.last(), Some(&Some("V")), "the visible text is drawn");
        assert_eq!(shown[3], None, "a text that is not oversize stays out");
        let margin_shown: Vec<usize> = (0..shown.len() - 1)
            .filter(|&i| shown[i].is_some())
            .collect();
        let expected: Vec<usize> = (0..texts.len() - 1)
            .filter(|&i| i != 3)
            .take(MAX_OVERSIZE_TEXTS_PER_FRAME - 1)
            .collect();
        assert_eq!(margin_shown, expected, "the margin's first texts, in order");
        assert_eq!(
            budget.warnings,
            [MARGIN_CAP_WARNING],
            "the cap fell on the margin"
        );
    }

    /// More visible oversize texts than the budget: the visible ones fill it
    /// in document order, nothing in the margin is drawn, and the warning is
    /// the plain cap.
    #[test]
    fn a_full_viewport_leaves_the_margin_out() {
        static LATCH: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let mut texts = vec![Some(("M", Reach::Margin))];
        texts.extend(vec![
            Some(("V", Reach::Viewport));
            MAX_OVERSIZE_TEXTS_PER_FRAME + 1
        ]);
        let mut budget = OversizeBudget::with_latch(&LATCH);
        let shown = budget.admit_all(&texts);
        assert_eq!(shown[0], None, "the margin text is left out");
        assert!(
            shown[1..=MAX_OVERSIZE_TEXTS_PER_FRAME]
                .iter()
                .all(Option::is_some)
        );
        assert_eq!(shown.last(), Some(&None), "the last visible one is over");
        assert_eq!(
            budget.warnings,
            [CAP_WARNING],
            "visible text was capped first"
        );
    }
}
