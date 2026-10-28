//! The GPU side of dirty-gated scene uploads for [`crate::scene_shader`].
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: iced/wgpu public docs, IPC-2612-1, IEEE 315, IEC 60617.
//!
//! [`signex_gfx::scene::apply_dirty_uploads_with_culling`] decides *what* to
//! upload; this adapter is the [`SceneUploadTarget`] that sends each category to
//! its pipeline buffer.

use std::convert::Infallible;
use std::sync::atomic::AtomicBool;

use iced::Rectangle;
use signex_gfx::pipeline::arc::ArcPipeline;
use signex_gfx::pipeline::circle::CirclePipeline;
use signex_gfx::pipeline::line::LinePipeline;
use signex_gfx::pipeline::ordered::OrderedPipeline;
use signex_gfx::pipeline::polygon::PolygonPipeline;
use signex_gfx::pipeline::text::GlyphonTextPipeline;
use signex_gfx::primitive::arc::Arc as GpuArc;
use signex_gfx::primitive::circle::Circle;
use signex_gfx::primitive::line::LineSegment;
use signex_gfx::primitive::ordered::OrderedItem;
use signex_gfx::primitive::polygon::GpuPolygon;
use signex_gfx::primitive::text::{TextItem, TextSizePolicy};
use signex_gfx::scene::{SceneUploadTarget, TextUploadParams, ViewportAabbMm};
use signex_gfx::wgpu;

use crate::scene_shader::world_origin_mm;

/// Per-frame text inputs that are not part of [`TextUploadParams`].
pub struct TextFrame<'a> {
    pub size_policy: TextSizePolicy,
    /// Pan offset in physical pixels.
    pub offset_px: [f32; 2],
    pub warned: &'a AtomicBool,
    pub surface_label: &'static str,
}

/// Borrows one surface's pipelines for the duration of a frame's uploads.
pub struct GpuUploadTarget<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub line: &'a mut LinePipeline,
    pub circle: &'a mut CirclePipeline,
    pub arc: &'a mut ArcPipeline,
    pub polygon: &'a mut PolygonPipeline,
    /// Drawings behind the circuit, in file paint order (#1579).
    pub ordered: &'a mut OrderedPipeline,
    /// Drawings placed above the circuit (#1583).
    pub ordered_front: &'a mut OrderedPipeline,
    pub text: &'a mut GlyphonTextPipeline,
    pub text_frame: TextFrame<'a>,
}

impl SceneUploadTarget for GpuUploadTarget<'_> {
    /// Text failures are reported once inside [`Self::upload_texts`] and cost one
    /// frame of text, never the render pass.
    type TextError = Infallible;

    fn upload_lines(&mut self, lines: &[LineSegment]) {
        self.line.upload(self.device, self.queue, lines);
    }

    fn upload_circles(&mut self, circles: &[Circle]) {
        self.circle.upload(self.device, self.queue, circles);
    }

    fn upload_arcs(&mut self, arcs: &[GpuArc]) {
        self.arc.upload(self.device, self.queue, arcs);
    }

    fn upload_polygons(&mut self, polygons: &[GpuPolygon]) {
        self.polygon.upload(self.device, self.queue, polygons);
    }

    fn upload_top_polygons(&mut self, polygons: &[GpuPolygon]) {
        self.polygon.upload_top(self.device, self.queue, polygons);
    }

    fn upload_drawings(&mut self, drawings: &[OrderedItem]) {
        self.ordered.upload(self.device, self.queue, drawings);
    }

    fn upload_drawings_front(&mut self, drawings: &[OrderedItem]) {
        self.ordered_front.upload(self.device, self.queue, drawings);
    }

    fn upload_texts(
        &mut self,
        texts: &[TextItem],
        params: TextUploadParams,
    ) -> Result<(), Self::TextError> {
        // The font system is iced's own, so canvas text uses the faces the app
        // loaded. Safe to take the write lock: iced's text prepare is a
        // sibling of custom-primitive prepare, not a parent.
        let mut font_system = iced::advanced::graphics::text::font_system()
            .write()
            .expect("iced font system poisoned");
        if let Err(error) = self.text.upload(
            self.device,
            self.queue,
            font_system.raw(),
            texts,
            params.scale_px_per_mm,
            self.text_frame.size_policy,
            crate::render_config::CANVAS_FONT_FAMILY,
            params.viewport_size_px,
            self.text_frame.offset_px,
        ) {
            crate::scene_shader::log_text_error_once(
                self.text_frame.warned,
                self.text_frame.surface_label,
                "upload",
                error,
            );
        }
        Ok(())
    }

    // The grid is painted by the canvas layer beneath the shader, and theme
    // colours travel inside the primitives (a recolour dirties its category),
    // so neither has GPU state to refresh here.
    fn refresh_grid(&mut self) {}

    fn upload_overlay_lines(&mut self, lines: &[LineSegment]) {
        self.line.upload_overlay(self.device, self.queue, lines);
    }

    fn upload_overlay_circles(&mut self, circles: &[Circle]) {
        self.circle.upload_overlay(self.device, self.queue, circles);
    }

    fn upload_overlay_polygons(&mut self, polygons: &[GpuPolygon]) {
        self.polygon
            .upload_overlay(self.device, self.queue, polygons);
    }

    // No production path fills the ERC marker buckets; their markers travel as
    // overlay geometry. See the note in `ScenePrimitive::draw`.
    fn upload_erc_marker_lines(&mut self, _lines: &[LineSegment]) {}
    fn upload_erc_marker_circles(&mut self, _circles: &[Circle]) {}
    fn upload_erc_marker_polygons(&mut self, _polygons: &[GpuPolygon]) {}
    fn refresh_theme(&mut self) {}
}

/// The world rectangle (mm) currently visible through `bounds`, or `None`
/// while the view is degenerate (zero size or non-positive scale), in which
/// case nothing can be culled.
pub fn visible_world_rect(
    bounds: &Rectangle,
    offset_px: [f32; 2],
    scale_px_per_mm: f32,
) -> Option<ViewportAabbMm> {
    let finite = scale_px_per_mm.is_finite() && offset_px.iter().all(|v| v.is_finite());
    if !(finite && scale_px_per_mm > 0.0 && bounds.width > 0.0 && bounds.height > 0.0) {
        return None;
    }
    let origin = world_origin_mm(offset_px, scale_px_per_mm);
    Some(ViewportAabbMm::new(
        origin,
        [
            origin[0] + bounds.width / scale_px_per_mm,
            origin[1] + bounds.height / scale_px_per_mm,
        ],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_rect_is_the_widget_in_world_mm() {
        // 4 px/mm, panned 80/40 px: top-left shows world (-20, -10); a 400x200
        // widget spans 100 x 50 mm from there.
        let bounds = Rectangle::new(iced::Point::ORIGIN, iced::Size::new(400.0, 200.0));
        let rect = visible_world_rect(&bounds, [80.0, 40.0], 4.0).unwrap();
        assert_eq!(rect, ViewportAabbMm::new([-20.0, -10.0], [80.0, 40.0]));
    }

    #[test]
    fn degenerate_views_are_not_culled() {
        let bounds = Rectangle::new(iced::Point::ORIGIN, iced::Size::new(400.0, 200.0));
        assert!(visible_world_rect(&bounds, [0.0, 0.0], 0.0).is_none());
        assert!(visible_world_rect(&Rectangle::default(), [0.0, 0.0], 4.0).is_none());
        assert!(visible_world_rect(&bounds, [0.0, 0.0], f32::NAN).is_none());
        assert!(visible_world_rect(&bounds, [f32::INFINITY, 0.0], 4.0).is_none());
    }
}
