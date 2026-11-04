//! Text above the glyph-atlas ceiling, as filled polygons in the scene (#2022).
//!
//! The scene build turns each such text into `GpuPolygon` fills, so the CPU
//! replay and the GPU polygon pipeline draw the same geometry from one source,
//! at true size at any zoom, rotated or not. The glyph outlines are shaped and
//! flattened once at a reference size and scaled, so a zoom or pan re-uses
//! them.

use iced::{Point, Rectangle};
use signex_gfx::pipeline::text_measure::OutlineCmd;
use signex_gfx::primitive::polygon::GpuPolygon;
use signex_gfx::primitive::text::{TextHAlign, TextItem, TextRaster, TextVAlign};
use signex_gfx::scene::Scene;

use crate::renderer_scene_canvas::SceneDrawOptions;

/// Em the cached outlines are shaped at; the live size scales them.
const OUTLINE_REF_PX: f32 = 1000.0;

// shortcut: a fixed per-scene count, far above any real sheet (a handful of
// titles). Hostile files of thousands of giant texts stop costing here.
pub(crate) const MAX_OUTLINE_TEXTS_PER_SCENE: usize = 256;

// shortcut: fixed bounds on what one text and one scene may convert, far above
// any real sheet. Past them the remaining glyphs or texts are left out and one
// warning reaches Messages, so a hostile file cannot make a frame cost more.
const MAX_OUTLINE_VERTICES_PER_TEXT: usize = 200_000;
const MAX_OUTLINE_VERTICES_PER_SCENE: usize = 1_000_000;

/// Outlines kept at most: twice the texts a scene converts, so one rebuild
/// never evicts what it is about to read. At the cap half of them go.
const OUTLINE_CACHE_CAP: usize = 2 * MAX_OUTLINE_TEXTS_PER_SCENE;

/// A text's glyphs as polygons at [`OUTLINE_REF_PX`], origin at the top-left
/// of its box, and the box size.
struct RefOutline {
    polygons: Vec<GpuPolygon>,
    size_px: [f32; 2],
}

type CachedOutline = std::rc::Rc<RefOutline>;

thread_local! {
    static OUTLINE_CACHE: std::cell::RefCell<
        std::collections::HashMap<(String, String), Option<CachedOutline>>,
    > = std::cell::RefCell::new(std::collections::HashMap::new());
}

fn cached_outline(content: &str, family: &str) -> Option<CachedOutline> {
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
        let (polygons, cut) = glyph_polygons(&outlines.glyphs);
        if outlines.truncated || cut {
            crate::diagnostics::log_warning(format!(
                "A text of {} characters is too long to draw as outlines at this size; the end of it is left out",
                content.chars().count()
            ));
        }
        std::rc::Rc::new(RefOutline {
            polygons,
            size_px: outlines.size_px,
        })
    });
    OUTLINE_CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() >= OUTLINE_CACHE_CAP {
            let evict: Vec<_> = c.keys().take(OUTLINE_CACHE_CAP / 2).cloned().collect();
            for key in evict {
                c.remove(&key);
            }
        }
        c.insert(key, built.clone());
    });
    built
}

/// Straight segments per curve: one per 8 px of control polygon, at least 4.
fn segments(length_px: f32) -> usize {
    ((length_px / 8.0).ceil() as usize).clamp(4, 64)
}

fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// Every contour of `cmds`, curves flattened.
fn contours(cmds: &[OutlineCmd]) -> Vec<Vec<[f32; 2]>> {
    let mut out: Vec<Vec<[f32; 2]>> = Vec::new();
    let mut at = [0.0_f32; 2];
    for cmd in cmds {
        match *cmd {
            OutlineCmd::Move(p) => {
                out.push(vec![p]);
                at = p;
            }
            OutlineCmd::Line(p) => {
                if let Some(c) = out.last_mut() {
                    c.push(p);
                }
                at = p;
            }
            OutlineCmd::Quad(c1, p) => {
                let n = segments(dist(at, c1) + dist(c1, p));
                if let Some(c) = out.last_mut() {
                    for i in 1..=n {
                        let t = i as f32 / n as f32;
                        let u = 1.0 - t;
                        c.push([
                            u * u * at[0] + 2.0 * u * t * c1[0] + t * t * p[0],
                            u * u * at[1] + 2.0 * u * t * c1[1] + t * t * p[1],
                        ]);
                    }
                }
                at = p;
            }
            OutlineCmd::Cubic(c1, c2, p) => {
                let n = segments(dist(at, c1) + dist(c1, c2) + dist(c2, p));
                if let Some(c) = out.last_mut() {
                    for i in 1..=n {
                        let t = i as f32 / n as f32;
                        let u = 1.0 - t;
                        let (a, b, d, e) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                        c.push([
                            a * at[0] + b * c1[0] + d * c2[0] + e * p[0],
                            a * at[1] + b * c1[1] + d * c2[1] + e * p[1],
                        ]);
                    }
                }
                at = p;
            }
            OutlineCmd::Close => {}
        }
    }
    out.retain(|c| c.len() >= 3);
    out
}

fn signed_area(c: &[[f32; 2]]) -> f32 {
    let mut sum = 0.0;
    for i in 0..c.len() {
        let (a, b) = (c[i], c[(i + 1) % c.len()]);
        sum += a[0] * b[1] - b[0] * a[1];
    }
    sum * 0.5
}

fn contains(contour: &[[f32; 2]], p: [f32; 2]) -> bool {
    let mut inside = false;
    for i in 0..contour.len() {
        let (a, b) = (contour[i], contour[(i + 1) % contour.len()]);
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            inside = !inside;
        }
    }
    inside
}

/// An outer contour with what is computed once about it.
struct Outer {
    polygon: GpuPolygon,
    area: f32,
    min: [f32; 2],
    max: [f32; 2],
}

impl Outer {
    fn new(vertices: Vec<[f32; 2]>) -> Self {
        let area = signed_area(&vertices).abs();
        let (mut min, mut max) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);
        for v in &vertices {
            for axis in 0..2 {
                min[axis] = min[axis].min(v[axis]);
                max[axis] = max[axis].max(v[axis]);
            }
        }
        Self {
            polygon: GpuPolygon {
                vertices,
                ..Default::default()
            },
            area,
            min,
            max,
        }
    }

    fn may_hold(&self, p: [f32; 2]) -> bool {
        (0..2).all(|axis| p[axis] >= self.min[axis] && p[axis] <= self.max[axis])
    }
}

/// One glyph's polygons: an outer contour with the holes inside it. Outers
/// turn the way the glyph's largest contour turns; each other contour is a
/// hole of the smallest outer of THIS glyph that holds it.
fn glyph_group(cmds: &[OutlineCmd]) -> Vec<GpuPolygon> {
    let all = contours(cmds);
    let Some(largest) = all
        .iter()
        .map(|c| signed_area(c))
        .max_by(|a, b| a.abs().total_cmp(&b.abs()))
    else {
        return Vec::new();
    };
    let (outers, holes): (Vec<_>, Vec<_>) = all
        .into_iter()
        .partition(|c| signed_area(c).signum() == largest.signum());
    let mut outers: Vec<Outer> = outers.into_iter().map(Outer::new).collect();
    let mut orphans = Vec::new();
    for hole in holes {
        let owner = outers
            .iter_mut()
            .filter(|o| o.may_hold(hole[0]) && contains(&o.polygon.vertices, hole[0]))
            .min_by(|a, b| a.area.total_cmp(&b.area));
        match owner {
            Some(o) => o.polygon.holes.push(hole),
            None => orphans.push(hole),
        }
    }
    outers
        .into_iter()
        .map(|o| o.polygon)
        .chain(orphans.into_iter().map(|vertices| GpuPolygon {
            vertices,
            ..Default::default()
        }))
        .collect()
}

fn vertex_count(polygons: &[GpuPolygon]) -> usize {
    polygons
        .iter()
        .map(|p| p.vertices.len() + p.holes.iter().map(Vec::len).sum::<usize>())
        .sum()
}

/// The polygons of every glyph, glyph by glyph, and whether the vertex cap
/// cut the rest off.
fn glyph_polygons(glyphs: &[Vec<OutlineCmd>]) -> (Vec<GpuPolygon>, bool) {
    let mut out = Vec::new();
    let mut vertices = 0;
    for glyph in glyphs {
        if vertices >= MAX_OUTLINE_VERTICES_PER_TEXT {
            return (out, true);
        }
        let group = glyph_group(glyph);
        vertices += vertex_count(&group);
        out.extend(group);
    }
    (out, false)
}

/// The text's box in screen px, from the cached advance (no shaping of its
/// own): its corners rotated about the anchor.
fn screen_bounds(text: &TextItem, position: Point, size_px: f32) -> Rectangle {
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
    let (x0, y0) = alignment_offset(text, width, height);
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
    let fold = |pick: fn(&(f32, f32)) -> f32, f: fn(f32, f32) -> f32, init: f32| {
        corners.iter().map(pick).fold(init, f)
    };
    let (min_x, max_x) = (
        fold(|c| c.0, f32::min, f32::INFINITY),
        fold(|c| c.0, f32::max, f32::NEG_INFINITY),
    );
    let (min_y, max_y) = (
        fold(|c| c.1, f32::min, f32::INFINITY),
        fold(|c| c.1, f32::max, f32::NEG_INFINITY),
    );
    Rectangle::new(
        Point::new(min_x, min_y),
        iced::Size::new(max_x - min_x, max_y - min_y),
    )
}

fn alignment_offset(text: &TextItem, w: f32, h: f32) -> (f32, f32) {
    (
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
    )
}

/// `text` as polygons in world mm: the cached glyphs scaled to the text's em,
/// aligned, rotated about the anchor and moved to it. The one geometry both
/// replays draw.
pub(crate) fn text_polygons(text: &TextItem) -> Vec<GpuPolygon> {
    let family = text.family_or(crate::render_config::CANVAS_FONT_FAMILY);
    let Some(cached) = cached_outline(&text.content, family) else {
        return Vec::new();
    };
    let em_mm = text.size_mm.max(0.1) / signex_types::font::mm_per_em(Some(family));
    let k = em_mm / OUTLINE_REF_PX;
    let (ox, oy) = alignment_offset(text, cached.size_px[0], cached.size_px[1]);
    let (sin, cos) = text.rotation.sin_cos();
    let place = |v: &[f32; 2]| {
        let (x, y) = ((v[0] + ox) * k, (v[1] + oy) * k);
        [
            text.position[0] + x * cos - y * sin,
            text.position[1] + x * sin + y * cos,
        ]
    };
    let color = text.color;
    cached
        .polygons
        .iter()
        .map(|p| GpuPolygon {
            vertices: p.vertices.iter().map(place).collect(),
            holes: p
                .holes
                .iter()
                .map(|h| h.iter().map(place).collect())
                .collect(),
            fill_color: color,
            stroke_color: None,
            stroke_width: 0.0,
        })
        .collect()
}

/// Replace the scene's texts above the atlas ceiling with their polygons, in
/// the bucket drawn just under text. Off-screen ones are dropped before they
/// are shaped; at most [`MAX_OUTLINE_TEXTS_PER_SCENE`] are kept.
pub(crate) fn convert_oversize_texts<F>(
    scene: &mut Scene,
    options: SceneDrawOptions,
    world_to_screen: F,
    viewport: Rectangle,
) where
    F: Fn([f32; 2]) -> Point,
{
    let mut kept = 0;
    let mut vertices = 0;
    let texts = std::mem::take(&mut scene.texts);
    for text in texts {
        if options.text_raster(&text) != TextRaster::Outline {
            scene.texts.push(text);
            continue;
        }
        if text.content.is_empty() {
            continue;
        }
        if kept >= MAX_OUTLINE_TEXTS_PER_SCENE || vertices >= MAX_OUTLINE_VERTICES_PER_SCENE {
            warn_scene_capped_once();
            continue;
        }
        let size_px = options.text_px(text.size_mm, text.family.as_deref());
        if !screen_bounds(&text, world_to_screen(text.position), size_px).intersects(&viewport) {
            continue;
        }
        kept += 1;
        let polygons = text_polygons(&text);
        vertices += vertex_count(&polygons);
        scene.top_polygons.extend(polygons);
    }
}

/// One Messages warning per run, not one per rebuilt scene.
fn warn_scene_capped_once() {
    static WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !WARNED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        crate::diagnostics::log_warning(
            "This sheet has more oversize text than can be drawn as outlines; the rest is left out"
                .to_string(),
        );
    }
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

    fn options(scale: f32) -> SceneDrawOptions {
        SceneDrawOptions {
            scale_px_per_mm: scale,
            device_px_per_logical_px: 1.0,
            text_min_px: 0.0,
            text_max_px: f32::INFINITY,
        }
    }

    /// A glyph with a counter ("O" has one) is an outer contour with a hole.
    #[test]
    fn a_glyph_with_a_counter_is_one_polygon_with_a_hole() {
        crate::fonts::measure::install_hermetic();
        let polygons = text_polygons(&TextItem {
            content: "O".into(),
            size_mm: 5.0,
            ..Default::default()
        });
        assert_eq!(polygons.len(), 1);
        assert_eq!(polygons[0].holes.len(), 1);
    }

    /// Oversize text far off the frame is dropped before it is shaped.
    #[test]
    fn oversize_text_off_the_frame_is_dropped_and_on_the_frame_is_kept() {
        crate::fonts::measure::install_hermetic();
        let viewport = Rectangle::with_size(iced::Size::new(800.0, 600.0));
        let mut on = item(40.0);
        on.position = [1.0, 1.0];
        let mut off = item(40.0);
        off.position = [-100_000.0, 1.0];
        let mut scene = Scene {
            texts: vec![on, off],
            ..Default::default()
        };
        convert_oversize_texts(
            &mut scene,
            options(96.0),
            |p| Point::new(p[0] * 96.0, p[1] * 96.0),
            viewport,
        );
        assert!(scene.texts.is_empty());
        let xs: Vec<f32> = scene
            .top_polygons
            .iter()
            .flat_map(|p| p.vertices.iter().map(|v| v[0]))
            .collect();
        assert!(
            !xs.is_empty() && xs.iter().all(|x| *x > -1.0),
            "only the on-screen text"
        );
    }

    /// A zoom or pan re-uses one set of glyph polygons per (text, face).
    #[test]
    fn oversize_outlines_are_cached_per_text_and_face() {
        crate::fonts::measure::install_hermetic();
        let a = cached_outline("R1", "Iosevka").expect("outline");
        let b = cached_outline("R1", "Iosevka").expect("outline");
        assert!(std::rc::Rc::ptr_eq(&a, &b));
        let other = cached_outline("R2", "Iosevka").expect("outline");
        assert!(!std::rc::Rc::ptr_eq(&a, &other));
    }

    /// Hostile files stay bounded: more oversize texts than the cap become at
    /// most the cap's worth of polygons.
    #[test]
    fn a_flood_of_oversize_texts_is_capped() {
        crate::fonts::measure::install_hermetic();
        let viewport = Rectangle::with_size(iced::Size::new(800.0, 600.0));
        let mut scene = Scene {
            texts: (0..MAX_OUTLINE_TEXTS_PER_SCENE + 50)
                .map(|_| {
                    let mut t = item(40.0);
                    t.position = [1.0, 1.0];
                    t
                })
                .collect(),
            ..Default::default()
        };
        convert_oversize_texts(
            &mut scene,
            options(96.0),
            |p| Point::new(p[0], p[1]),
            viewport,
        );
        let one = text_polygons(&item(40.0)).len();
        assert_eq!(scene.top_polygons.len(), one * MAX_OUTLINE_TEXTS_PER_SCENE);
    }

    /// #2022 - on the schematic's own scene build, 40 mm text at 96 px/mm
    /// (about 5224 px, past the atlas) becomes fill polygons, none of it is
    /// left as a text item for the GPU to skip or the CPU overlay to pick up,
    /// and the polygons are exactly what both replays are handed.
    #[test]
    fn oversize_schematic_text_is_scene_fill_geometry_and_not_routed_to_the_cpu() {
        use crate::schematic_runtime::{SceneOverrides, ScreenTransform, build_schematic_scene};
        use signex_types::schematic::{HAlign, SchematicSheet, TextNote, VAlign};
        use signex_types::theme::ThemeId;
        crate::fonts::measure::install_hermetic();
        let note = |rotation: f64| TextNote {
            uuid: uuid::Uuid::now_v7(),
            text: "R1".into(),
            position: signex_types::schematic::Point::new(2.0, 2.0),
            rotation,
            font_size: 40.0,
            font_family: None,
            justify_h: HAlign::Left,
            justify_v: VAlign::Top,
            locked: false,
        };
        let sheet = SchematicSheet {
            text_notes: vec![note(0.0), note(90.0)],
            ..Default::default()
        };
        let transform = ScreenTransform {
            offset_x: 0.0,
            offset_y: 0.0,
            scale: 96.0,
            device_px_per_logical_px: 1.0,
        };
        let bounds = Rectangle::with_size(iced::Size::new(1600.0, 1200.0));
        let scene = build_schematic_scene(
            &sheet,
            &transform,
            &signex_types::theme::canvas_colors(ThemeId::Signex),
            bounds,
            None,
            SceneOverrides::default(),
        )
        .expect("scene");
        assert!(
            scene.texts.is_empty(),
            "no oversize text item left: {:?}",
            scene.texts.len()
        );
        assert!(!scene.top_polygons.is_empty(), "fill geometry in the scene");
        // Both replays draw `scene.top_polygons`; it is the shared outline
        // source, so a rebuild of one note gives the same polygons.
        let one = TextItem {
            content: "R1".into(),
            size_mm: 40.0,
            position: [2.0, 2.0],
            ..Default::default()
        };
        let expected = text_polygons(&one);
        assert!(expected.len() >= 2, "R and 1");
        assert!(
            scene.top_polygons.windows(expected.len()).any(|w| w
                .iter()
                .zip(&expected)
                .all(|(a, b)| a.vertices == b.vertices && a.holes == b.holes)),
            "the scene carries exactly the shared outline geometry"
        );
    }

    /// A long string of holed glyphs ("8@B": counters in every glyph) cannot
    /// make the conversion cost more than its caps: it returns capped output,
    /// quickly, and does not panic.
    #[test]
    fn a_long_string_of_holed_glyphs_is_capped_and_fast() {
        crate::fonts::measure::install_hermetic();
        let start = std::time::Instant::now();
        let cached = cached_outline(&"8@B".repeat(5000), "Iosevka").expect("outline");
        assert!(
            start.elapsed() < std::time::Duration::from_secs(20),
            "{:?}",
            start.elapsed()
        );
        assert!(!cached.polygons.is_empty());
        assert!(
            vertex_count(&cached.polygons) <= 2 * MAX_OUTLINE_VERTICES_PER_TEXT,
            "{}",
            vertex_count(&cached.polygons)
        );
        let glyph_count = cached
            .polygons
            .iter()
            .filter(|p| !p.holes.is_empty())
            .count();
        assert!(glyph_count <= signex_gfx::pipeline::text_measure::MAX_OUTLINE_GLYPHS * 3);
    }

    /// Holes belong to their own glyph: "88" is two outers with one hole each
    /// (a whole-string search would also test each against the other).
    #[test]
    fn holes_are_assigned_within_their_own_glyph() {
        crate::fonts::measure::install_hermetic();
        let cached = cached_outline("88", "Iosevka").expect("outline");
        assert_eq!(
            cached.polygons.iter().map(|p| p.holes.len()).sum::<usize>(),
            4
        );
        assert!(cached.polygons.iter().all(|p| p.holes.len() <= 2));
    }
}
