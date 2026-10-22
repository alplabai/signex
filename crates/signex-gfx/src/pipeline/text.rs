//! Text pipeline foundation.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

use crate::primitive::text::{
    LINE_HEIGHT_EM, TextHAlign, TextItem, TextSizePolicy, TextVAlign, text_px,
};

/// Rendered em size in logical pixels, from the one shared sizing rule.
///
/// This used to compute `(size_mm * scale).max(1.0)` — treating `size_mm` as
/// the em size and ignoring the caller's readability limits. `size_mm` is the
/// glyph height the sheet asks for, so that rendered every label about 28%
/// short of the CPU replay and broke the millimetre contract the model
/// guarantees (a 10 pt import stopped measuring 50 mils).
fn text_size_px(item: &TextItem, scale_px_per_mm: f32, policy: TextSizePolicy) -> f32 {
    text_px(item.size_mm, scale_px_per_mm, policy)
}

fn text_position_px(item: &TextItem, scale_px_per_mm: f32, offset_px: [f32; 2]) -> [f32; 2] {
    // Full screen mapping `screen_px = world_mm * scale + offset_px`. Unlike the
    // instanced primitives — which stay in world space and let the camera ortho
    // apply pan on the GPU — glyphon rasterises text CPU-side in screen space,
    // so the pan term must be added here or the labels stay pinned while the
    // geometry they annotate scrolls out from under them.
    [
        item.position[0] * scale_px_per_mm + offset_px[0],
        item.position[1] * scale_px_per_mm + offset_px[1],
    ]
}

fn normalize_rotation_radians(rotation_rad: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    let mut wrapped = rotation_rad % tau;

    if wrapped <= -std::f32::consts::PI {
        wrapped += tau;
    } else if wrapped > std::f32::consts::PI {
        wrapped -= tau;
    }

    wrapped
}

fn alignment_offset_px(size_px: [f32; 2], h_align: TextHAlign, v_align: TextVAlign) -> [f32; 2] {
    let x = match h_align {
        TextHAlign::Left => 0.0,
        TextHAlign::Center => -0.5 * size_px[0],
        TextHAlign::Right => -size_px[0],
    };

    let y = match v_align {
        TextVAlign::Top => 0.0,
        TextVAlign::Center => -0.5 * size_px[1],
        TextVAlign::Bottom => -size_px[1],
    };

    [x, y]
}

fn rotated_offset_px(offset_px: [f32; 2], rotation_rad: f32) -> [f32; 2] {
    let angle = normalize_rotation_radians(rotation_rad);
    let (sin_theta, cos_theta) = angle.sin_cos();

    [
        offset_px[0] * cos_theta - offset_px[1] * sin_theta,
        offset_px[0] * sin_theta + offset_px[1] * cos_theta,
    ]
}

fn anchored_top_left_px(item: &TextItem, anchor_px: [f32; 2], size_px: [f32; 2]) -> [f32; 2] {
    let offset_px = alignment_offset_px(size_px, item.h_align, item.v_align);
    let rotated_offset = rotated_offset_px(offset_px, item.rotation);

    [
        anchor_px[0] + rotated_offset[0],
        anchor_px[1] + rotated_offset[1],
    ]
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct RectPx {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

fn rect_from_top_left_size(top_left_px: [f32; 2], size_px: [f32; 2]) -> RectPx {
    RectPx {
        left: top_left_px[0],
        top: top_left_px[1],
        right: top_left_px[0] + size_px[0].max(0.0),
        bottom: top_left_px[1] + size_px[1].max(0.0),
    }
}

fn rect_intersects_viewport(rect: RectPx, viewport: cryoglyph::TextBounds) -> bool {
    let viewport_left = viewport.left as f32;
    let viewport_top = viewport.top as f32;
    let viewport_right = viewport.right as f32;
    let viewport_bottom = viewport.bottom as f32;

    rect.right > viewport_left
        && rect.left < viewport_right
        && rect.bottom > viewport_top
        && rect.top < viewport_bottom
}

#[cfg(test)]
fn rect_overlap_area_px(a: RectPx, b: RectPx) -> f32 {
    let overlap_left = a.left.max(b.left);
    let overlap_top = a.top.max(b.top);
    let overlap_right = a.right.min(b.right);
    let overlap_bottom = a.bottom.min(b.bottom);

    let overlap_width = (overlap_right - overlap_left).max(0.0);
    let overlap_height = (overlap_bottom - overlap_top).max(0.0);

    overlap_width * overlap_height
}

#[cfg(test)]
fn rect_area_px(rect: RectPx) -> f32 {
    let width = (rect.right - rect.left).max(0.0);
    let height = (rect.bottom - rect.top).max(0.0);
    width * height
}

#[cfg(test)]
fn overlap_ratio_by_smaller_area(a: RectPx, b: RectPx) -> f32 {
    let min_area = rect_area_px(a).min(rect_area_px(b));
    if min_area <= f32::EPSILON {
        return 0.0;
    }

    rect_overlap_area_px(a, b) / min_area
}

fn viewport_bounds(viewport_size_px: [u32; 2]) -> cryoglyph::TextBounds {
    cryoglyph::TextBounds {
        left: 0,
        top: 0,
        right: viewport_size_px[0] as i32,
        bottom: viewport_size_px[1] as i32,
    }
}

fn measure_text_bounds_px(buffer: &cryoglyph::Buffer) -> [f32; 2] {
    let mut max_width = 0.0_f32;
    let mut max_bottom = 0.0_f32;
    let mut has_lines = false;

    for run in buffer.layout_runs() {
        has_lines = true;
        max_width = max_width.max(run.line_w);
        max_bottom = max_bottom.max(run.line_top + run.line_height);
    }

    if has_lines {
        [max_width.max(0.0), max_bottom.max(1.0)]
    } else {
        [0.0, 1.0]
    }
}

/// Shape `item`'s content into a buffer.
fn shaped_buffer(
    font_system: &mut cryoglyph::FontSystem,
    item: &TextItem,
    metrics: cryoglyph::Metrics,
    attrs: &cryoglyph::Attrs<'_>,
    viewport_px: [f32; 2],
) -> cryoglyph::Buffer {
    let mut buffer = cryoglyph::Buffer::new(font_system, metrics);
    buffer.set_size(font_system, Some(viewport_px[0]), Some(viewport_px[1]));
    buffer.set_text(
        font_system,
        &item.content,
        attrs,
        cryoglyph::Shaping::Advanced,
        None,
    );
    buffer.shape_until_scroll(font_system, false);

    // Alignment is a no-op for a lone line or when every line is equally wide.
    let widths: Vec<f32> = buffer.layout_runs().map(|run| run.line_w).collect();
    let all_equal = widths.windows(2).all(|pair| pair[0] == pair[1]);
    if item.h_align == TextHAlign::Left || widths.len() < 2 || all_equal {
        return buffer;
    }

    // The block offset only anchors the bounding box; each line then aligns
    // inside it, as the CPU replay does. Rounded up so a float tie on the
    // widest line cannot re-wrap it.
    let block_w = measure_text_bounds_px(&buffer)[0].ceil();
    buffer.set_size(font_system, Some(block_w), Some(viewport_px[1]));
    let align = Some(cosmic_align(item.h_align));
    for line in buffer.lines.iter_mut() {
        line.set_align(align);
    }
    buffer.shape_until_scroll(font_system, false);
    buffer
}

fn cosmic_align(h_align: TextHAlign) -> cryoglyph::cosmic_text::Align {
    match h_align {
        TextHAlign::Left => cryoglyph::cosmic_text::Align::Left,
        TextHAlign::Center => cryoglyph::cosmic_text::Align::Center,
        TextHAlign::Right => cryoglyph::cosmic_text::Align::Right,
    }
}

/// `family` is the caller's canvas font family name. It used to be a
/// hardcoded `Family::SansSerif`, which rendered schematic text in whatever
/// the system served instead of the app's monospace canvas face — different
/// typeface *and* different advance widths, so anything sized from a glyph
/// estimate came out wrong too.
///
/// `italic` is deliberately not mapped. The canvas family ships no italic
/// face, and cosmic-text 0.15's `Attrs::matches` requires an exact style
/// match, so asking for one skips the canvas family entirely and lands on
/// some system italic — a different typeface, and different from the CPU
/// replay, which draws the same item upright. Map it once an italic face is
/// bundled, on both sides together.
fn attrs_for_item(item: &TextItem, family: &'static str) -> cryoglyph::Attrs<'static> {
    let attrs = cryoglyph::Attrs::new().family(cryoglyph::Family::Name(family));

    if item.bold {
        attrs.weight(cryoglyph::Weight::BOLD)
    } else {
        attrs
    }
}

fn to_glyphon_color(color: [f32; 4]) -> cryoglyph::Color {
    let map_channel = |channel: f32| -> u8 { (channel.clamp(0.0, 1.0) * 255.0).round() as u8 };

    cryoglyph::Color::rgba(
        map_channel(color[0]),
        map_channel(color[1]),
        map_channel(color[2]),
        map_channel(color[3]),
    )
}

#[derive(Clone, Copy, Debug)]
struct GlyphonPreparedText {
    left: f32,
    top: f32,
    scale: f32,
    bounds: cryoglyph::TextBounds,
    default_color: cryoglyph::Color,
}

/// Production text path using glyphon atlas, shaping, and cached glyph rendering.
/// The `FontSystem` is **not** owned here — [`Self::upload`] borrows the
/// caller's. Owning one meant this pipeline shaped text against a font system
/// that had never been given the app's faces, so it could only ever fall back
/// to something else, and its fallback chain for non-Latin content could
/// diverge from the CPU path's silently.
pub struct GlyphonTextPipeline {
    swash_cache: cryoglyph::SwashCache,
    viewport: cryoglyph::Viewport,
    atlas: cryoglyph::TextAtlas,
    text_renderer: cryoglyph::TextRenderer,
    buffers: Vec<cryoglyph::Buffer>,
    prepared_texts: Vec<GlyphonPreparedText>,
    text_count: u32,
    viewport_size_px: [u32; 2],
}

impl GlyphonTextPipeline {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_format: wgpu::TextureFormat,
    ) -> Self {
        let swash_cache = cryoglyph::SwashCache::new();
        let cache = cryoglyph::Cache::new(device);
        let viewport = cryoglyph::Viewport::new(device, &cache);
        let mut atlas = cryoglyph::TextAtlas::new(device, queue, &cache, target_format);
        let text_renderer = cryoglyph::TextRenderer::new(
            &mut atlas,
            device,
            wgpu::MultisampleState::default(),
            None,
        );

        Self {
            swash_cache,
            viewport,
            atlas,
            text_renderer,
            buffers: Vec::new(),
            prepared_texts: Vec::new(),
            text_count: 0,
            viewport_size_px: [0, 0],
        }
    }

    /// Shape and stage every item for the next [`Self::draw`].
    ///
    /// `font_system` is the caller's — pass the same one the CPU text path
    /// shapes against, so both measure with the same faces and the same
    /// fallback chain. `family` names the canvas font; `size_policy` carries
    /// the surface's readability limits, applied in **logical** pixels, so
    /// `scale_px_per_mm` must be the logical scale with the DPI factor already
    /// folded in by the caller only where it belongs.
    #[expect(
        clippy::too_many_arguments,
        reason = "every argument is one axis of a text frame: what to draw, \
                  how big, where, in which faces, and inside what viewport"
    )]
    pub fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        font_system: &mut cryoglyph::FontSystem,
        texts: &[TextItem],
        scale_px_per_mm: f32,
        size_policy: TextSizePolicy,
        family: &'static str,
        viewport_size_px: [u32; 2],
        offset_px: [f32; 2],
    ) -> Result<(), cryoglyph::PrepareError> {
        self.text_count = 0;
        self.viewport_size_px = viewport_size_px;
        self.buffers.clear();
        self.prepared_texts.clear();

        self.viewport.update(
            queue,
            cryoglyph::Resolution {
                width: viewport_size_px[0],
                height: viewport_size_px[1],
            },
        );

        if texts.is_empty() {
            return Ok(());
        }

        let bounds = viewport_bounds(viewport_size_px);
        let viewport_width = viewport_size_px[0] as f32;
        let viewport_height = viewport_size_px[1] as f32;

        // Rotated items are the CPU canvas layer's to draw — see
        // `TextItem::is_axis_aligned`. Staging them here drew them unrotated.
        for text in texts.iter().filter(|text| text.is_axis_aligned()) {
            let font_px = text_size_px(text, scale_px_per_mm, size_policy);
            let metrics = cryoglyph::Metrics::new(font_px, (font_px * LINE_HEIGHT_EM).max(1.0));
            let attrs = attrs_for_item(text, family);
            let buffer = shaped_buffer(
                font_system,
                text,
                metrics,
                &attrs,
                [viewport_width, viewport_height],
            );

            let anchor_px = text_position_px(text, scale_px_per_mm, offset_px);
            let text_bounds_px = measure_text_bounds_px(&buffer);
            let top_left_px = anchored_top_left_px(text, anchor_px, text_bounds_px);
            let text_rect = rect_from_top_left_size(top_left_px, text_bounds_px);
            if !rect_intersects_viewport(text_rect, bounds) {
                continue;
            }

            self.prepared_texts.push(GlyphonPreparedText {
                left: top_left_px[0],
                top: top_left_px[1],
                scale: 1.0,
                bounds,
                default_color: to_glyphon_color(text.color),
            });
            self.buffers.push(buffer);
        }

        self.text_count = self.prepared_texts.len() as u32;
        if self.text_count == 0 {
            return Ok(());
        }

        let text_areas =
            self.buffers
                .iter()
                .zip(self.prepared_texts.iter())
                .map(|(buffer, prepared)| cryoglyph::TextArea {
                    buffer,
                    left: prepared.left,
                    top: prepared.top,
                    scale: prepared.scale,
                    bounds: prepared.bounds,
                    default_color: prepared.default_color,
                });

        // cryoglyph's `prepare` uploads rasterized glyphs into the atlas
        // texture through a `CommandEncoder` (glyphon 0.11 did this via the
        // queue directly). This pipeline owns its device/queue, so it runs a
        // dedicated encoder and submits it immediately.
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("signex_gfx_text_prepare"),
        });
        let result = self.text_renderer.prepare(
            device,
            queue,
            &mut encoder,
            font_system,
            &mut self.atlas,
            &self.viewport,
            text_areas,
            &mut self.swash_cache,
        );
        queue.submit(std::iter::once(encoder.finish()));
        result
    }

    pub fn draw(
        &self,
        render_pass: &mut wgpu::RenderPass<'_>,
    ) -> Result<(), cryoglyph::RenderError> {
        if self.text_count == 0 {
            return Ok(());
        }

        self.text_renderer
            .render(&self.atlas, &self.viewport, render_pass)
    }

    pub fn trim_atlas(&mut self) {
        self.atlas.trim();
    }

    pub fn text_count(&self) -> u32 {
        self.text_count
    }

    pub fn viewport_size_px(&self) -> [u32; 2] {
        self.viewport_size_px
    }
}

#[cfg(test)]
mod tests {
    use super::TextSizePolicy;
    use super::{
        alignment_offset_px, anchored_top_left_px, attrs_for_item, normalize_rotation_radians,
        overlap_ratio_by_smaller_area, rect_from_top_left_size, rect_intersects_viewport,
        shaped_buffer, text_position_px, text_size_px, to_glyphon_color, viewport_bounds,
    };
    use crate::primitive::text::{TextHAlign, TextItem, TextVAlign};

    #[test]
    fn glyphon_helpers_map_size_position_and_bounds() {
        let item = TextItem {
            content: "R12".to_string(),
            position: [2.5, 4.0],
            size_mm: 1.0,
            color: [1.0, 1.0, 1.0, 1.0],
            bold: false,
            italic: false,
            rotation: 0.0,
            h_align: TextHAlign::Left,
            v_align: TextVAlign::Top,
        };

        // Wide bounds so this pins the conversion, not a clamp.
        let policy = TextSizePolicy::new(1.0, 512.0);
        let size_px = text_size_px(&item, 32.0, policy);
        let position_px = text_position_px(&item, 32.0, [0.0, 0.0]);
        let panned_px = text_position_px(&item, 32.0, [15.0, -7.0]);
        let bounds = viewport_bounds([128, 96]);

        // 1.0 mm of glyph height is 1.0 / MM_PER_EM em, then scaled. This
        // used to assert 32.0 — `size_mm` taken as the em size directly,
        // which rendered every label short of the CPU replay.
        assert!(
            (size_px - 44.444_443).abs() < 1e-3,
            "expected ~44.444 px, got {size_px}"
        );
        assert_eq!(position_px, [80.0, 128.0]);
        // Pan translates the anchor by the screen-space offset.
        assert_eq!(panned_px, [95.0, 121.0]);
        assert_eq!(bounds.left, 0);
        assert_eq!(bounds.top, 0);
        assert_eq!(bounds.right, 128);
        assert_eq!(bounds.bottom, 96);
    }

    #[test]
    fn glyphon_helpers_clamp_small_text_size() {
        let item = TextItem {
            content: String::new(),
            position: [0.0, 0.0],
            size_mm: 0.0,
            color: [1.0, 1.0, 1.0, 1.0],
            bold: false,
            italic: false,
            rotation: 0.0,
            h_align: TextHAlign::Left,
            v_align: TextVAlign::Top,
        };

        // A degenerate scale collapses to zero, and the policy's floor is what
        // keeps it drawable — the floor is now the caller's, not a hardcoded
        // 1.0 buried in the pipeline.
        let size_px = text_size_px(&item, 0.0, TextSizePolicy::new(1.0, 512.0));
        assert_eq!(size_px, 1.0);
    }

    #[test]
    fn glyphon_helpers_map_style_and_color() {
        let item = TextItem {
            content: "NET_A".to_string(),
            position: [0.0, 0.0],
            size_mm: 1.0,
            color: [0.25, 0.5, 0.75, 0.5],
            bold: true,
            italic: true,
            rotation: 0.0,
            h_align: TextHAlign::Left,
            v_align: TextVAlign::Top,
        };

        let attrs = attrs_for_item(&item, "Iosevka");
        let color = to_glyphon_color(item.color);

        assert_eq!(
            attrs.family,
            cryoglyph::Family::Name("Iosevka"),
            "the caller's canvas family must reach the shaper — a hardcoded \
             SansSerif here is what put schematic text in the wrong typeface"
        );

        assert_eq!(attrs.weight, cryoglyph::Weight::BOLD);
        // Upright on purpose: no italic canvas face is bundled, and an exact
        // style match would otherwise leave the canvas family (#645).
        assert_eq!(attrs.style, cryoglyph::Style::Normal);
        assert_eq!(color.r(), 64);
        assert_eq!(color.g(), 128);
        assert_eq!(color.b(), 191);
        assert_eq!(color.a(), 128);
    }

    #[test]
    fn alignment_helpers_map_offsets() {
        assert_eq!(
            alignment_offset_px([12.0, 4.0], TextHAlign::Left, TextVAlign::Top),
            [0.0, 0.0]
        );
        assert_eq!(
            alignment_offset_px([12.0, 4.0], TextHAlign::Center, TextVAlign::Center),
            [-6.0, -2.0]
        );
        assert_eq!(
            alignment_offset_px([12.0, 4.0], TextHAlign::Right, TextVAlign::Bottom),
            [-12.0, -4.0]
        );
    }

    #[test]
    fn anchored_position_respects_alignment_and_rotation() {
        let item = TextItem {
            content: "PIN1".to_string(),
            position: [0.0, 0.0],
            size_mm: 1.0,
            color: [1.0, 1.0, 1.0, 1.0],
            bold: false,
            italic: false,
            rotation: std::f32::consts::FRAC_PI_2,
            h_align: TextHAlign::Right,
            v_align: TextVAlign::Bottom,
        };

        let anchored = anchored_top_left_px(&item, [100.0, 50.0], [20.0, 10.0]);
        assert!((anchored[0] - 110.0).abs() < 0.0001);
        assert!((anchored[1] - 30.0).abs() < 0.0001);
    }

    fn test_font_system() -> cryoglyph::FontSystem {
        let font = include_bytes!("../../tests/fonts/Iosevka-Regular.ttf");
        cryoglyph::FontSystem::new_with_fonts([cryoglyph::fontdb::Source::Binary(
            std::sync::Arc::new(font.as_slice()),
        )])
    }

    /// Left edge of each laid-out line's first glyph, and each line's width.
    fn line_extents(h_align: TextHAlign, content: &str) -> Vec<(f32, f32)> {
        let mut fs = test_font_system();
        let item = TextItem {
            content: content.to_string(),
            position: [0.0, 0.0],
            size_mm: 1.0,
            color: [1.0; 4],
            bold: false,
            italic: false,
            rotation: 0.0,
            h_align,
            v_align: TextVAlign::Top,
        };
        let metrics = cryoglyph::Metrics::new(20.0, 24.0);
        let attrs = attrs_for_item(&item, "Iosevka");
        let buffer = shaped_buffer(&mut fs, &item, metrics, &attrs, [800.0, 600.0]);
        buffer
            .layout_runs()
            .map(|run| (run.glyphs.first().map_or(0.0, |g| g.x), run.line_w))
            .collect()
    }

    /// Each line's x must equal its slack inside the block the shaper aligns
    /// within: the widest line's width, rounded up.
    fn assert_per_line(h_align: TextHAlign, content: &str) {
        let runs = line_extents(h_align, content);
        assert_eq!(runs.len(), 2);
        assert!(runs[0].1 != runs[1].1, "lines must differ in width");
        let block_w = runs.iter().map(|r| r.1).fold(0.0_f32, f32::max).ceil();
        for (x, line_w) in runs {
            let slack = block_w - line_w;
            let want = if h_align == TextHAlign::Center {
                slack / 2.0
            } else {
                slack
            };
            assert!((x - want).abs() < 0.01, "got {x} want {want}");
        }
    }

    #[test]
    fn centred_multiline_aligns_each_line() {
        assert_per_line(TextHAlign::Center, "WWWWWWWW\nWW");
    }

    #[test]
    fn right_multiline_aligns_each_line() {
        assert_per_line(TextHAlign::Right, "WWWWWWWW\nWW");
    }

    /// Trailing spaces: pins whatever `line_w` counts for them. The line is
    /// aligned by its measured width, which is what the block offset uses too.
    #[test]
    fn trailing_whitespace_aligns_by_measured_width() {
        // cosmic-text 0.15 counts trailing spaces in `line_w` (4 advances, not 2).
        let widths = line_extents(TextHAlign::Left, "AA  \nA");
        assert!((widths[0].1 - 4.0 * widths[1].1).abs() < 0.01);
        assert_per_line(TextHAlign::Center, "AA  \nA");
        assert_per_line(TextHAlign::Right, "AA  \nA");
    }

    #[test]
    fn left_and_single_line_are_unshifted() {
        let left = line_extents(TextHAlign::Left, "WWWWWWWW\nWW");
        assert!(left.iter().all(|r| r.0.abs() < 0.01));
        let single = line_extents(TextHAlign::Center, "WWWW");
        assert_eq!(single.len(), 1);
        assert!(single[0].0.abs() < 0.01);
    }

    #[test]
    fn rotation_normalization_is_stable() {
        let normalized = normalize_rotation_radians(std::f32::consts::TAU + 0.25);
        assert!((normalized - 0.25).abs() < 0.0001);
    }

    #[test]
    fn clipping_helper_rejects_outside_rectangles() {
        let viewport = viewport_bounds([128, 96]);
        let inside = rect_from_top_left_size([12.0, 8.0], [24.0, 10.0]);
        let outside = rect_from_top_left_size([160.0, 8.0], [24.0, 10.0]);

        assert!(rect_intersects_viewport(inside, viewport));
        assert!(!rect_intersects_viewport(outside, viewport));
    }

    #[test]
    fn overlap_ratio_detects_dense_overlap() {
        let a = rect_from_top_left_size([10.0, 10.0], [20.0, 10.0]);
        let b = rect_from_top_left_size([12.0, 11.0], [20.0, 10.0]);
        let c = rect_from_top_left_size([40.0, 40.0], [10.0, 10.0]);

        let dense_overlap = overlap_ratio_by_smaller_area(a, b);
        let disjoint_overlap = overlap_ratio_by_smaller_area(a, c);

        assert!(dense_overlap > 0.7);
        assert!(disjoint_overlap.abs() < f32::EPSILON);
    }
}
