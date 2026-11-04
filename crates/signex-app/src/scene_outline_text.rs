//! Text above the glyph-atlas ceiling, drawn as filled vector outlines (#2022).
//!
//! The CPU canvas draws it here, and the GPU-mounted sheet hands it to the
//! CPU canvas, so it stays at true size at any zoom.

use iced::Point;
use iced::widget::canvas;
use signex_gfx::primitive::text::{TextHAlign, TextItem, TextVAlign};

use crate::renderer_scene_canvas::color_from_rgba;

/// Em the cached outlines are shaped at; drawing scales them to the live size,
/// so a zoom or pan never re-shapes or re-tessellates (a `Path` per text).
const OUTLINE_REF_PX: f32 = 1000.0;

// shortcut: a fixed per-frame count, far above any real sheet (a handful of
// titles). Hostile files of thousands of giant texts stop costing frames here.
pub(crate) const MAX_OUTLINE_TEXTS_PER_FRAME: usize = 256;

/// Outlines kept at most; at the cap the cache starts over.
const OUTLINE_CACHE_CAP: usize = 64;

type OutlinePath = std::rc::Rc<(canvas::Path, [f32; 2])>;

thread_local! {
    /// (content, family) to the path at [`OUTLINE_REF_PX`] and its box.
    static OUTLINE_CACHE: std::cell::RefCell<std::collections::HashMap<(String, String), Option<OutlinePath>>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

fn cached_outline(content: &str, family: &str) -> Option<OutlinePath> {
    use signex_gfx::pipeline::text_measure::OutlineCmd;
    let key = (content.to_string(), family.to_string());
    if let Some(hit) = OUTLINE_CACHE.with(|c| c.borrow().get(&key).cloned()) {
        return hit;
    }
    let built = crate::fonts::measure::text_outlines(
        content,
        family,
        OUTLINE_REF_PX,
        signex_gfx::primitive::text::LINE_HEIGHT_EM,
    )
    .map(|outlines| {
        let pt = |p: [f32; 2]| Point::new(p[0], p[1]);
        let path = canvas::Path::new(|b| {
            for cmd in &outlines.cmds {
                match *cmd {
                    OutlineCmd::Move(p) => b.move_to(pt(p)),
                    OutlineCmd::Line(p) => b.line_to(pt(p)),
                    OutlineCmd::Quad(c, p) => b.quadratic_curve_to(pt(c), pt(p)),
                    OutlineCmd::Cubic(a, c, p) => b.bezier_curve_to(pt(a), pt(c), pt(p)),
                    OutlineCmd::Close => b.close(),
                }
            }
        });
        std::rc::Rc::new((path, outlines.size_px))
    });
    OUTLINE_CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() >= OUTLINE_CACHE_CAP {
            c.clear();
        }
        c.insert(key, built.clone());
    });
    built
}

/// Box of a text in the frame, from the cached advance (no shaping of its
/// own): the text's anchor-relative corners rotated about the anchor.
fn outline_text_screen_bounds(text: &TextItem, position: Point, size_px: f32) -> iced::Rectangle {
    let family = text.family_or(crate::render_config::CANVAS_FONT_FAMILY);
    let lines = || text.content.split('\n');
    let width = lines()
        .map(|line| {
            signex_types::font::shaped_advance_em(line, Some(family))
                .unwrap_or(line.chars().count() as f32 * 0.5)
        })
        .fold(0.0_f32, f32::max)
        * size_px;
    let height = lines().count() as f32 * size_px * signex_gfx::primitive::text::LINE_HEIGHT_EM;
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
    let (sin, cos) = if text.is_axis_aligned() {
        (0.0, 1.0)
    } else {
        text.rotation.sin_cos()
    };
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
    let min_x = corners.iter().map(|c| c.0).fold(f32::INFINITY, f32::min);
    let max_x = corners
        .iter()
        .map(|c| c.0)
        .fold(f32::NEG_INFINITY, f32::max);
    let min_y = corners.iter().map(|c| c.1).fold(f32::INFINITY, f32::min);
    let max_y = corners
        .iter()
        .map(|c| c.1)
        .fold(f32::NEG_INFINITY, f32::max);
    iced::Rectangle::new(
        Point::new(min_x, min_y),
        iced::Size::new(max_x - min_x, max_y - min_y),
    )
}

/// Text above the glyph-atlas ceiling, drawn as filled glyph outlines so it
/// keeps its true size at any zoom. Off-screen text is skipped before it is
/// shaped. Bold and italic are not synthesised.
pub(crate) fn draw(frame: &mut canvas::Frame, text: &TextItem, position: Point, size_px: f32) {
    let on_screen = iced::Rectangle::with_size(frame.size());
    if !outline_text_screen_bounds(text, position, size_px).intersects(&on_screen) {
        return;
    }
    let family = text.family_or(crate::render_config::CANVAS_FONT_FAMILY);
    let Some(cached) = cached_outline(&text.content, family) else {
        return;
    };
    let (path, [w, h]) = (&cached.0, cached.1);
    let offset = iced::Vector::new(
        match text.h_align {
            TextHAlign::Left => 0.0,
            TextHAlign::Center => -0.5 * w,
            TextHAlign::Right => -w,
        },
        match text.v_align {
            TextVAlign::Top => 0.0,
            TextVAlign::Center => -0.5 * h,
            TextVAlign::Bottom => -h,
        },
    );
    let color = color_from_rgba(text.color);
    frame.with_save(|inner| {
        inner.translate(iced::Vector::new(position.x, position.y));
        if !text.is_axis_aligned() {
            inner.rotate(iced::Radians(text.rotation));
        }
        inner.scale(size_px / OUTLINE_REF_PX);
        inner.translate(offset);
        inner.fill(path, color);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(size_mm: f32) -> TextItem {
        TextItem {
            content: "R1".into(),
            size_mm,
            ..Default::default()
        }
    }

    /// Oversize text far off the frame is skipped before it is shaped; the
    /// bounds come from the advance alone.
    #[test]
    fn oversize_text_off_the_frame_is_culled_and_on_the_frame_is_kept() {
        crate::fonts::measure::install_hermetic();
        let frame = iced::Rectangle::with_size(iced::Size::new(800.0, 600.0));
        let big = item(40.0);
        let px = 5224.0;
        let on = outline_text_screen_bounds(&big, Point::new(100.0, 100.0), px);
        let off = outline_text_screen_bounds(&big, Point::new(-50_000.0, 100.0), px);
        assert!(on.intersects(&frame));
        assert!(!off.intersects(&frame));
    }

    /// A zoom or pan re-uses one tessellated path per (text, face).
    #[test]
    fn oversize_outlines_are_cached_per_text_and_face() {
        crate::fonts::measure::install_hermetic();
        let a = cached_outline("R1", "Iosevka").expect("outline");
        let b = cached_outline("R1", "Iosevka").expect("outline");
        assert!(std::rc::Rc::ptr_eq(&a, &b));
        let other = cached_outline("R2", "Iosevka").expect("outline");
        assert!(!std::rc::Rc::ptr_eq(&a, &other));
    }
}
