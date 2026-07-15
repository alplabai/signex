//! Generic GPU render path for any `signex_gfx::scene::Scene`.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: iced/wgpu public docs, IPC-2612-1, IEEE 315, IEC 60617.
//!
//! Bridges the `signex_gfx` render pipelines into iced's `shader` widget so a
//! `Scene` draws on the GPU instead of being tessellated into a
//! `canvas::Frame` on the CPU. The pipelines are primitive-agnostic — they are
//! driven purely by a `Scene` plus a screen-space pan/zoom transform — so both
//! the schematic ([`crate::schematic_shader`]) and the PCB editor
//! ([`crate::pcb_canvas`]) mount the *same* renderer here.
//!
//! iced's shader `Primitive::draw` composites into the shared render pass over
//! whatever was already drawn behind the widget (it never clears its own
//! region), so the caller is responsible for painting the background + grid on
//! a layer *below* this shader in a `stack!`.

use std::sync::Arc;

use iced::widget::shader::{self, Viewport};
use iced::{Rectangle, mouse};

use signex_gfx::camera::{CameraGpu, CameraUniform};
use signex_gfx::pipeline::arc::ArcPipeline;
use signex_gfx::pipeline::circle::CirclePipeline;
use signex_gfx::pipeline::line::LinePipeline;
use signex_gfx::pipeline::polygon::PolygonPipeline;
use signex_gfx::pipeline::text::GlyphonTextPipeline;
use signex_gfx::scene::Scene;
use signex_gfx::wgpu;

use crate::app::Message;

/// World coordinate (mm) at the render pass origin (top-left).
///
/// The screen mapping is `screen_px = world_mm * scale + offset_px`, so the
/// world point drawn at the top-left corner is `-offset / scale`. Returns the
/// origin unchanged when the scale is degenerate.
pub fn world_origin_mm(offset_px: [f32; 2], scale_px_per_mm: f32) -> [f32; 2] {
    if scale_px_per_mm > 0.0 {
        [
            -offset_px[0] / scale_px_per_mm,
            -offset_px[1] / scale_px_per_mm,
        ]
    } else {
        [0.0, 0.0]
    }
}

/// The set of `signex_gfx` pipelines plus the camera, created once by iced and
/// reused across frames (iced stores this keyed by [`ScenePrimitive`]).
pub struct ScenePipeline {
    camera: CameraGpu,
    line: LinePipeline,
    circle: CirclePipeline,
    arc: ArcPipeline,
    polygon: PolygonPipeline,
    text: GlyphonTextPipeline,
    /// Scene generation currently resident in the instance buffers, or `None`
    /// if geometry has never been uploaded. When a frame's primitive carries
    /// the same generation, `prepare` skips re-uploading identical geometry and
    /// refreshes only the camera uniform — so a pure pan/zoom moves ~64 bytes
    /// instead of the whole board. See [`ScenePrimitive::generation`].
    uploaded_generation: Option<u64>,
}

impl shader::Pipeline for ScenePipeline {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> Self {
        // The camera bind-group layout is shared by every instanced pipeline.
        let camera = CameraGpu::new(
            device,
            CameraUniform::ortho([1.0, 1.0], [0.0, 0.0], 1.0),
        );
        let layout = camera.bind_group_layout();
        Self {
            line: LinePipeline::new(device, format, layout),
            circle: CirclePipeline::new(device, format, layout),
            arc: ArcPipeline::new(device, format, layout),
            polygon: PolygonPipeline::new(device, format, layout),
            text: GlyphonTextPipeline::new(device, queue, format),
            camera,
            uploaded_generation: None,
        }
    }
}

/// One frame's worth of scene geometry handed to the GPU. Cheap to build each
/// frame — it is the same instance data the CPU path already produces.
#[derive(Debug)]
pub struct ScenePrimitive {
    /// Shared with the owning [`SceneShaderProgram`] and the `gpu_scene` cache:
    /// building the primitive each frame is an `Arc` refcount bump, not a deep
    /// copy of the geometry.
    pub scene: Arc<Scene>,
    /// Identity of `scene`'s geometry, used to skip redundant GPU uploads.
    /// `Some(g)` comes from a cached source (the PCB `gpu_scene` cache) that
    /// bumps `g` only when the geometry actually changes, so equal generations
    /// across frames mean "same geometry, don't re-upload". `None` marks an
    /// uncached source (the schematic path) that must upload every frame.
    pub generation: Option<u64>,
    /// Screen-space pan offset in logical pixels.
    pub offset_px: [f32; 2],
    /// Zoom in logical pixels per millimetre.
    pub scale_px_per_mm: f32,
}

impl shader::Primitive for ScenePrimitive {
    type Pipeline = ScenePipeline;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bounds: &Rectangle,
        viewport: &Viewport,
    ) {
        // iced renders the shader pass in physical pixels; scale the logical
        // transform by the surface's DPI factor so world→pixel matches the
        // CPU canvas exactly.
        let dpi = viewport.scale_factor();
        let vp_px = [bounds.width * dpi, bounds.height * dpi];
        let scale_px = self.scale_px_per_mm * dpi;

        let offset_mm = world_origin_mm(self.offset_px, self.scale_px_per_mm);

        // The camera changes on every pan/zoom frame, so always refresh it.
        pipeline
            .camera
            .update(queue, CameraUniform::ortho(vp_px, offset_mm, scale_px));

        // Skip re-uploading identical geometry. The line/circle/arc/polygon
        // instances live in *world* space and the camera ortho above applies
        // pan/zoom on the GPU, so their buffers — which the reused `Pipeline`
        // keeps resident across frames — only need refreshing when the geometry
        // itself changes. When this frame's generation already matches what's
        // resident, a pure pan/zoom moved just the camera uniform. An uncached
        // source (`generation == None`) never matches and re-uploads.
        let geometry_is_current = matches!(
            (self.generation, pipeline.uploaded_generation),
            (Some(current), Some(resident)) if current == resident
        );
        if !geometry_is_current {
            pipeline.polygon.upload(device, queue, &self.scene.polygons);
            pipeline.line.upload(device, queue, &self.scene.lines);
            pipeline.arc.upload(device, queue, &self.scene.arcs);
            pipeline.circle.upload(device, queue, &self.scene.circles);
            pipeline.uploaded_generation = self.generation;
        }

        // Text is NOT guarded: glyphon rasterises glyphs in screen-pixel space
        // (`size_mm * scale_px_per_mm`, pixel positions), so it must be
        // re-prepared every frame that pan/zoom/viewport changes. Its prep can
        // also fail if the glyph atlas is exhausted; a dropped frame of text is
        // preferable to a panic on the render thread.
        let _ = pipeline.text.upload(
            device,
            queue,
            &self.scene.texts,
            scale_px,
            [vp_px[0] as u32, vp_px[1] as u32],
        );
    }

    fn draw(
        &self,
        pipeline: &Self::Pipeline,
        render_pass: &mut wgpu::RenderPass<'_>,
    ) -> bool {
        let camera = pipeline.camera.bind_group();
        // Fills, then strokes, then text on top. NOTE: polygons draw *before*
        // lines/circles here, whereas the CPU `pcb_canvas::draw_scene` draws
        // polygons *last* — a known z-order divergence to reconcile when GPU
        // visual parity is verified (the feature flags disclose parity is
        // still unconfirmed on hardware).
        pipeline.polygon.draw(render_pass, camera);
        pipeline.line.draw(render_pass, camera);
        pipeline.arc.draw(render_pass, camera);
        pipeline.circle.draw(render_pass, camera);
        let _ = pipeline.text.draw(render_pass);
        true
    }
}

/// The `shader::Program` mounted when a view routes its `Scene` through the
/// GPU. Holds the built `Scene` and the active pan/zoom; pointer handling
/// stays on the CPU `canvas` layer stacked beneath this shader, so `update`
/// is the default no-op and never captures — events fall through to the
/// canvas below.
pub struct SceneShaderProgram {
    scene: Arc<Scene>,
    generation: Option<u64>,
    offset_px: [f32; 2],
    scale_px_per_mm: f32,
}

impl SceneShaderProgram {
    /// Build from an already-tessellated `Scene` and the current screen-space
    /// transform (`offset_px` = pan in logical pixels, `scale_px_per_mm` =
    /// zoom in logical pixels per millimetre). `generation` identifies the
    /// geometry so the pipeline can skip redundant GPU uploads on pan/zoom:
    /// `Some(g)` from a cached source that bumps `g` only on real geometry
    /// changes, or `None` for an uncached source that uploads every frame.
    pub fn new(
        scene: Arc<Scene>,
        generation: Option<u64>,
        offset_px: [f32; 2],
        scale_px_per_mm: f32,
    ) -> Self {
        Self {
            scene,
            generation,
            offset_px,
            scale_px_per_mm,
        }
    }
}

impl shader::Program<Message> for SceneShaderProgram {
    type State = ();
    type Primitive = ScenePrimitive;

    fn draw(
        &self,
        _state: &Self::State,
        _cursor: mouse::Cursor,
        _bounds: Rectangle,
    ) -> Self::Primitive {
        ScenePrimitive {
            scene: Arc::clone(&self.scene),
            generation: self.generation,
            offset_px: self.offset_px,
            scale_px_per_mm: self.scale_px_per_mm,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::widget::shader::Program;
    use signex_gfx::primitive::line::LineSegment;

    #[test]
    fn world_origin_is_negative_offset_over_scale() {
        // At 4 px/mm with the view panned 80px right / 40px down, the top-left
        // corner shows world (-20mm, -10mm).
        let origin = world_origin_mm([80.0, 40.0], 4.0);
        assert_eq!(origin, [-20.0, -10.0]);
    }

    #[test]
    fn world_origin_handles_degenerate_scale() {
        assert_eq!(world_origin_mm([12.0, 34.0], 0.0), [0.0, 0.0]);
    }

    #[test]
    fn primitive_carries_the_scene_and_camera() {
        let mut scene = Scene::default();
        scene.lines.push(LineSegment {
            p0: [0.0, 0.0],
            p1: [1.0, 1.0],
            width: 0.1,
            color: [1.0, 1.0, 1.0, 1.0],
            style: 0,
            _pad: 0,
        });

        let program = SceneShaderProgram::new(Arc::new(scene), Some(7), [5.0, 6.0], 3.0);
        let primitive =
            program.draw(&(), iced::mouse::Cursor::Unavailable, iced::Rectangle::default());

        assert_eq!(primitive.scene.lines.len(), 1);
        assert_eq!(primitive.generation, Some(7));
        assert_eq!(primitive.offset_px, [5.0, 6.0]);
        assert_eq!(primitive.scale_px_per_mm, 3.0);
    }
}
