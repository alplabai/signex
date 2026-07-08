//! Text pipeline foundation.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

use crate::primitive::text::{TextHAlign, TextItem, TextVAlign};

fn text_size_px(item: &TextItem, scale_px_per_mm: f32) -> f32 {
    (item.size_mm.max(0.01) * scale_px_per_mm.max(0.01)).max(1.0)
}

fn text_position_px(item: &TextItem, scale_px_per_mm: f32) -> [f32; 2] {
    [
        item.position[0] * scale_px_per_mm,
        item.position[1] * scale_px_per_mm,
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

fn rect_intersects_viewport(rect: RectPx, viewport: glyphon::TextBounds) -> bool {
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

fn viewport_bounds(viewport_size_px: [u32; 2]) -> glyphon::TextBounds {
    glyphon::TextBounds {
        left: 0,
        top: 0,
        right: viewport_size_px[0] as i32,
        bottom: viewport_size_px[1] as i32,
    }
}

fn measure_text_bounds_px(buffer: &glyphon::Buffer) -> [f32; 2] {
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

fn attrs_for_item(item: &TextItem) -> glyphon::Attrs<'static> {
    let mut attrs = glyphon::Attrs::new().family(glyphon::Family::SansSerif);

    if item.bold {
        attrs = attrs.weight(glyphon::Weight::BOLD);
    }

    if item.italic {
        attrs = attrs.style(glyphon::Style::Italic);
    }

    attrs
}

fn to_glyphon_color(color: [f32; 4]) -> glyphon::Color {
    let map_channel = |channel: f32| -> u8 { (channel.clamp(0.0, 1.0) * 255.0).round() as u8 };

    glyphon::Color::rgba(
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
    bounds: glyphon::TextBounds,
    default_color: glyphon::Color,
}

/// Production text path using glyphon atlas, shaping, and cached glyph rendering.
pub struct GlyphonTextPipeline {
    font_system: glyphon::FontSystem,
    swash_cache: glyphon::SwashCache,
    viewport: glyphon::Viewport,
    atlas: glyphon::TextAtlas,
    text_renderer: glyphon::TextRenderer,
    buffers: Vec<glyphon::Buffer>,
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
        let font_system = glyphon::FontSystem::new();
        let swash_cache = glyphon::SwashCache::new();
        let cache = glyphon::Cache::new(device);
        let viewport = glyphon::Viewport::new(device, &cache);
        let mut atlas = glyphon::TextAtlas::new(device, queue, &cache, target_format);
        let text_renderer =
            glyphon::TextRenderer::new(&mut atlas, device, wgpu::MultisampleState::default(), None);

        Self {
            font_system,
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

    pub fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texts: &[TextItem],
        scale_px_per_mm: f32,
        viewport_size_px: [u32; 2],
    ) -> Result<(), glyphon::PrepareError> {
        self.text_count = 0;
        self.viewport_size_px = viewport_size_px;
        self.buffers.clear();
        self.prepared_texts.clear();

        self.viewport.update(
            queue,
            glyphon::Resolution {
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

        for text in texts {
            let font_px = text_size_px(text, scale_px_per_mm);
            let metrics = glyphon::Metrics::new(font_px, (font_px * 1.35).max(1.0));
            let attrs = attrs_for_item(text);
            let mut buffer = glyphon::Buffer::new(&mut self.font_system, metrics);

            buffer.set_size(
                &mut self.font_system,
                Some(viewport_width),
                Some(viewport_height),
            );
            buffer.set_text(
                &mut self.font_system,
                &text.content,
                &attrs,
                glyphon::Shaping::Advanced,
                None,
            );
            buffer.shape_until_scroll(&mut self.font_system, false);

            let anchor_px = text_position_px(text, scale_px_per_mm);
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
                .map(|(buffer, prepared)| glyphon::TextArea {
                    buffer,
                    left: prepared.left,
                    top: prepared.top,
                    scale: prepared.scale,
                    bounds: prepared.bounds,
                    default_color: prepared.default_color,
                    custom_glyphs: &[],
                });

        self.text_renderer.prepare(
            device,
            queue,
            &mut self.font_system,
            &mut self.atlas,
            &self.viewport,
            text_areas,
            &mut self.swash_cache,
        )
    }

    pub fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
    ) -> Result<(), glyphon::RenderError> {
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
    use super::{
        alignment_offset_px, anchored_top_left_px, attrs_for_item, normalize_rotation_radians,
        overlap_ratio_by_smaller_area, rect_from_top_left_size, rect_intersects_viewport,
        text_position_px, text_size_px, to_glyphon_color, viewport_bounds,
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

        let size_px = text_size_px(&item, 32.0);
        let position_px = text_position_px(&item, 32.0);
        let bounds = viewport_bounds([128, 96]);

        assert_eq!(size_px, 32.0);
        assert_eq!(position_px, [80.0, 128.0]);
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

        let size_px = text_size_px(&item, 0.0);
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

        let attrs = attrs_for_item(&item);
        let color = to_glyphon_color(item.color);

        assert_eq!(attrs.weight, glyphon::Weight::BOLD);
        assert_eq!(attrs.style, glyphon::Style::Italic);
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
