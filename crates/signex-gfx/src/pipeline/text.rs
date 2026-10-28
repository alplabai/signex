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

/// The items the glyph atlas can draw; rotated ones belong to the CPU layer.
fn atlas_drawable(texts: &[TextItem]) -> impl Iterator<Item = &TextItem> {
    texts.iter().filter(|text| text.is_axis_aligned())
}

/// Top-left of the glyph run for an axis-aligned item. Rotation is not part of
/// this maths: a cryoglyph `TextArea` cannot rotate, so rotated items never
/// reach it (`TextItem::is_axis_aligned`) and are drawn by the CPU canvas layer.
fn anchored_top_left_px(item: &TextItem, anchor_px: [f32; 2], size_px: [f32; 2]) -> [f32; 2] {
    let offset_px = alignment_offset_px(size_px, item.h_align, item.v_align);

    [anchor_px[0] + offset_px[0], anchor_px[1] + offset_px[1]]
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

/// Widest advance a glyph can take, in ems. Real faces stay under 1.5; the
/// margin keeps the pre-shaping estimate an upper bound, never a guess.
const MAX_GLYPH_ADVANCE_EM: f32 = 3.0;
/// A tab can span this many ems (cosmic-text's default is 8 spaces).
const MAX_TAB_ADVANCE_EM: f32 = 8.0;
/// Safety factor on the laid-out height of a block.
const HEIGHT_SLACK: f32 = 2.0;

/// Whether `item` could reach the viewport, decided **before** shaping.
///
/// Shaping is the expensive part of `upload`, so an item that is certainly
/// off-screen must not pay for it. The size of the shaped block is bounded
/// from the text alone (character count, line count, `font_px`); the anchor
/// rect is then evaluated at size 0 and at that bound, and the union is
/// tested against the viewport. The anchor offset is linear in the block
/// size, so those two ends cover every size in between.
///
/// Returns `true` (keep, shape it) whenever the text might wrap, because a
/// wrapped block's height depends on shaping. Never returns `false` for an
/// item the post-shaping test would keep, **assuming** no glyph advances more
/// than `MAX_GLYPH_ADVANCE_EM` ems. That is a property of the fonts, not
/// something this code enforces; a few codepoints in fallback faces (e.g.
/// U+FDFD) are wider, so a label made of them at a viewport edge could be
/// culled while partly visible.
fn may_intersect_viewport(
    item: &TextItem,
    font_px: f32,
    line_height_px: f32,
    anchor_px: [f32; 2],
    viewport: cryoglyph::TextBounds,
) -> bool {
    let (mut chars, mut tabs, mut lines) = (0usize, 0usize, 1usize);
    for ch in item.content.chars() {
        match ch {
            // The shaper (cosmic-text) also breaks on a lone '\r'. Counting
            // both halves of "\r\n" only enlarges the bound, which is safe.
            '\n' | '\r' => lines += 1,
            '\t' => tabs += 1,
            _ => chars += 1,
        }
    }
    let width_max =
        (chars as f32 * MAX_GLYPH_ADVANCE_EM + tabs as f32 * MAX_TAB_ADVANCE_EM) * font_px;
    if !width_max.is_finite() || width_max > viewport.right as f32 {
        // Might wrap against the viewport width; only shaping knows.
        return true;
    }
    let max_size = [width_max, lines as f32 * line_height_px * HEIGHT_SLACK];
    let small = rect_from_top_left_size(
        anchored_top_left_px(item, anchor_px, [0.0, 0.0]),
        [0.0, 0.0],
    );
    let large = rect_from_top_left_size(anchored_top_left_px(item, anchor_px, max_size), max_size);
    let union = RectPx {
        left: small.left.min(large.left),
        top: small.top.min(large.top),
        right: small.right.max(large.right),
        bottom: small.bottom.max(large.bottom),
    };
    rect_intersects_viewport(union, viewport)
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

pub(crate) fn measure_text_bounds_px(buffer: &cryoglyph::Buffer) -> [f32; 2] {
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
pub(crate) fn shaped_buffer(
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

/// `family` is the caller's canvas font family name, used for an item that
/// names none of its own (`TextItem::family`, already checked against the
/// installed fonts). It used to be a
/// hardcoded `Family::SansSerif`, which rendered schematic text in whatever
/// the system served instead of the app's monospace canvas face — different
/// typeface *and* different advance widths, so anything sized from a glyph
/// estimate came out wrong too.
///
/// `italic` maps to `Style::Italic`. cosmic-text 0.15's `Attrs::matches`
/// keeps only faces whose style equals the request, so this relies on the
/// app bundling an italic face of the canvas family; without one the
/// request leaves the family for some system italic (#654). The CPU replay
/// (`renderer_scene_canvas::canvas_font`) maps the same two flags, so both
/// paths pick the same face.
pub(crate) fn attrs_for_item<'a>(item: &'a TextItem, family: &'a str) -> cryoglyph::Attrs<'a> {
    let mut attrs = cryoglyph::Attrs::new().family(cryoglyph::Family::Name(item.family_or(family)));
    if item.bold {
        attrs = attrs.weight(cryoglyph::Weight::BOLD);
    }
    if item.italic {
        attrs = attrs.style(cryoglyph::Style::Italic);
    }
    attrs
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
        for text in atlas_drawable(texts) {
            let font_px = text_size_px(text, scale_px_per_mm, size_policy);
            let line_height_px = (font_px * LINE_HEIGHT_EM).max(1.0);
            let anchor_px = text_position_px(text, scale_px_per_mm, offset_px);
            if !may_intersect_viewport(text, font_px, line_height_px, anchor_px, bounds) {
                continue;
            }
            let metrics = cryoglyph::Metrics::new(font_px, line_height_px);
            let attrs = attrs_for_item(text, family);
            let buffer = shaped_buffer(
                font_system,
                text,
                metrics,
                &attrs,
                [viewport_width, viewport_height],
            );

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
        alignment_offset_px, anchored_top_left_px, atlas_drawable, attrs_for_item,
        overlap_ratio_by_smaller_area, rect_from_top_left_size, rect_intersects_viewport,
        shaped_buffer, text_position_px, text_size_px, to_glyphon_color, viewport_bounds,
    };
    use crate::primitive::text::{TextHAlign, TextItem, TextVAlign};

    #[test]
    fn pre_shaping_cull_never_drops_an_item_shaping_would_keep() {
        use super::{may_intersect_viewport, measure_text_bounds_px};
        let mut fs = test_font_system();
        let bounds = viewport_bounds([800, 600]);
        let font_px = 16.0;
        let line_h = font_px * crate::primitive::text::LINE_HEIGHT_EM;
        let mut culled = 0;
        for content in ["R1", "NET_A_LONG_LABEL", "a\nbb\nccc", "x\ty", "a\rb\rc\rd"] {
            for h in [TextHAlign::Left, TextHAlign::Center, TextHAlign::Right] {
                for v in [TextVAlign::Top, TextVAlign::Center, TextVAlign::Bottom] {
                    let item = TextItem {
                        content: content.to_string(),
                        position: [0.0, 0.0],
                        size_mm: 1.0,
                        color: [1.0; 4],
                        bold: false,
                        italic: false,
                        rotation: 0.0,
                        h_align: h,
                        v_align: v,
                        ..Default::default()
                    };
                    let attrs = attrs_for_item(&item, "Iosevka");
                    let metrics = cryoglyph::Metrics::new(font_px, line_h);
                    let buffer = shaped_buffer(&mut fs, &item, metrics, &attrs, [800.0, 600.0]);
                    let size = measure_text_bounds_px(&buffer);
                    for x in (-900..=1700).step_by(50) {
                        for y in (-700..=1300).step_by(50) {
                            let anchor = [x as f32, y as f32];
                            let top_left = anchored_top_left_px(&item, anchor, size);
                            let kept = rect_intersects_viewport(
                                rect_from_top_left_size(top_left, size),
                                bounds,
                            );
                            let pre =
                                may_intersect_viewport(&item, font_px, line_h, anchor, bounds);
                            assert!(pre || !kept, "{content:?} {h:?} {v:?} at {anchor:?}");
                            culled += usize::from(!pre);
                        }
                    }
                }
            }
        }
        assert!(culled > 0, "the pre-cull must actually cull far items");
    }

    #[test]
    fn pre_shaping_cull_defers_possible_wrapping_to_shaping() {
        use super::may_intersect_viewport;
        let item = TextItem {
            content: "w".repeat(400),
            position: [0.0, 0.0],
            size_mm: 1.0,
            color: [1.0; 4],
            bold: false,
            italic: false,
            rotation: 0.0,
            h_align: TextHAlign::Left,
            v_align: TextVAlign::Top,
            ..Default::default()
        };
        let bounds = viewport_bounds([800, 600]);
        assert!(may_intersect_viewport(
            &item,
            16.0,
            19.2,
            [5000.0, 5000.0],
            bounds
        ));
    }

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
            family: None,
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
            family: None,
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
            family: None,
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
        assert_eq!(attrs.style, cryoglyph::Style::Italic);
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
    fn anchored_position_ignores_rotation_the_atlas_cannot_draw() {
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
            family: None,
        };

        let anchored = anchored_top_left_px(&item, [100.0, 50.0], [20.0, 10.0]);
        // Right/Bottom offsets are (-w, -h), unrotated.
        assert!((anchored[0] - 80.0).abs() < 0.0001);
        assert!((anchored[1] - 40.0).abs() < 0.0001);
    }

    #[test]
    fn rotated_items_are_not_staged_for_the_atlas() {
        let mut item = TextItem {
            content: "PIN1".to_string(),
            position: [0.0, 0.0],
            size_mm: 1.0,
            color: [1.0, 1.0, 1.0, 1.0],
            bold: false,
            italic: false,
            rotation: 0.0,
            h_align: TextHAlign::Left,
            v_align: TextVAlign::Top,
            family: None,
        };
        assert_eq!(atlas_drawable(std::slice::from_ref(&item)).count(), 1);
        for angle in [std::f32::consts::FRAC_PI_2, -std::f32::consts::FRAC_PI_2] {
            item.rotation = angle;
            assert_eq!(atlas_drawable(std::slice::from_ref(&item)).count(), 0);
        }
    }

    #[test]
    fn an_italic_item_shapes_with_the_italic_canvas_face() {
        let mut fs = hermetic_font_system(&[
            include_bytes!("../../tests/fonts/Iosevka-Regular.ttf"),
            include_bytes!("../../tests/fonts/Iosevka-Italic.ttf"),
        ]);
        // Only the two test faces: a system Iosevka must not take part.
        assert_eq!(fs.db().faces().count(), 2);

        assert_eq!(
            face_of(&mut fs, true),
            (
                "Iosevka-Italic".to_string(),
                cryoglyph::fontdb::Style::Italic
            )
        );
        assert_eq!(
            face_of(&mut fs, false),
            ("Iosevka".to_string(), cryoglyph::fontdb::Style::Normal)
        );
    }

    /// An item's own family reaches the shaper; one that names none draws in
    /// the caller's canvas family.
    #[test]
    fn an_items_own_family_beats_the_canvas_default() {
        let mut fs = hermetic_font_system(&[
            include_bytes!("../../tests/fonts/Iosevka-Regular.ttf"),
            include_bytes!("../../../signex-app/assets/fonts/Roboto-Regular.ttf"),
        ]);
        let mut item = TextItem {
            content: "NET_A".to_string(),
            ..Default::default()
        };
        let face = |fs: &mut cryoglyph::FontSystem, item: &TextItem| {
            let attrs = attrs_for_item(item, "Iosevka");
            let metrics = cryoglyph::Metrics::new(20.0, 24.0);
            let buffer = shaped_buffer(fs, item, metrics, &attrs, [800.0, 600.0]);
            let font_id = buffer.layout_runs().next().expect("one run").glyphs[0].font_id;
            fs.db()
                .face(font_id)
                .expect("shaped face")
                .post_script_name
                .clone()
        };

        assert_eq!(face(&mut fs, &item), "Iosevka");
        item.family = Some("Roboto".into());
        assert_eq!(face(&mut fs, &item), "Roboto-Regular");
    }

    #[test]
    fn an_italic_item_with_no_italic_face_shapes_with_the_upright_face() {
        let mut fs = test_font_system();

        assert_eq!(
            face_of(&mut fs, true),
            ("Iosevka".to_string(), cryoglyph::fontdb::Style::Normal)
        );
    }

    /// PostScript name and style of the face the first glyph of an
    /// `"Iosevka"` item shapes with.
    fn face_of(fs: &mut cryoglyph::FontSystem, italic: bool) -> (String, cryoglyph::fontdb::Style) {
        let item = TextItem {
            content: "NET_A".to_string(),
            position: [0.0, 0.0],
            size_mm: 1.0,
            color: [1.0; 4],
            bold: false,
            italic,
            rotation: 0.0,
            h_align: TextHAlign::Left,
            v_align: TextVAlign::Top,
            family: None,
        };
        let attrs = attrs_for_item(&item, "Iosevka");
        let metrics = cryoglyph::Metrics::new(20.0, 24.0);
        let buffer = shaped_buffer(fs, &item, metrics, &attrs, [800.0, 600.0]);
        let font_id = buffer.layout_runs().next().expect("one run").glyphs[0].font_id;
        let face = fs.db().face(font_id).expect("shaped face");
        (face.post_script_name.clone(), face.style)
    }

    /// A font system holding only `fonts`. `FontSystem::new_with_fonts` also
    /// loads the system fonts, so a machine with Iosevka installed would
    /// shape with its own faces instead of the test copies.
    fn hermetic_font_system(fonts: &[&'static [u8]]) -> cryoglyph::FontSystem {
        let mut db = cryoglyph::fontdb::Database::new();
        for font in fonts {
            db.load_font_source(cryoglyph::fontdb::Source::Binary(std::sync::Arc::new(
                *font,
            )));
        }
        cryoglyph::FontSystem::new_with_locale_and_db("en-US".to_string(), db)
    }

    fn test_font_system() -> cryoglyph::FontSystem {
        hermetic_font_system(&[include_bytes!("../../tests/fonts/Iosevka-Regular.ttf")])
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
            family: None,
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
