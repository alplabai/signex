//! GPU render path for the schematic canvas (issue #169 PR 2).
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: iced/wgpu public docs, IPC-2612-1, IEEE 315, IEC 60617.
//!
//! Bridges the existing `signex_gfx` render pipelines into iced's
//! `shader` widget so the schematic draws on the GPU instead of tessellating
//! a `canvas::Frame` on the CPU. Both paths consume the **same**
//! `signex_gfx::scene::Scene` (built by `SchematicRenderer::build_scene`), so
//! they are comparable by construction.
//!
//! This is the render layer only, and nothing mounts it yet: no view
//! constructs a [`SchematicShaderProgram`], and the `SCHEMATIC_GPU_RENDER`
//! flag this doc used to link to was never added to [`crate::feature_flags`]
//! (which carries `FOOTPRINT_EDITOR_ENABLED` and `PCB_GPU_RENDER` only).
//! Pointer interaction and presentation overlays (selection, ghost preview,
//! ERC marks) still live on the CPU `canvas` program; wiring those onto the
//! GPU surface is a follow-up. See
//! [`crate::schematic_runtime::ScreenTransform`] for the world→screen
//! mapping this camera mirrors.

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
use crate::ignore::IgnoreResult;
use crate::schematic_runtime::ScreenTransform;

/// World coordinate (mm) at the render pass origin (top-left).
///
/// `ScreenTransform` maps `screen_px = world_mm * scale + offset_px`, so the
/// world point drawn at the top-left corner is `-offset / scale`. Returns the
/// origin unchanged when the scale is degenerate.
fn world_origin_mm(offset_px: [f32; 2], scale_px_per_mm: f32) -> [f32; 2] {
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
/// reused across frames (iced stores this keyed by [`SchematicPrimitive`]).
pub struct SchematicPipeline {
    camera: CameraGpu,
    line: LinePipeline,
    circle: CirclePipeline,
    arc: ArcPipeline,
    polygon: PolygonPipeline,
    text: GlyphonTextPipeline,
}

impl shader::Pipeline for SchematicPipeline {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        // The camera bind-group layout is shared by every instanced pipeline.
        let camera = CameraGpu::new(device, CameraUniform::ortho([1.0, 1.0], [0.0, 0.0], 1.0));
        let layout = camera.bind_group_layout();
        Self {
            line: LinePipeline::new(device, format, layout),
            circle: CirclePipeline::new(device, format, layout),
            arc: ArcPipeline::new(device, format, layout),
            polygon: PolygonPipeline::new(device, format, layout),
            text: GlyphonTextPipeline::new(device, queue, format),
            camera,
        }
    }

    fn trim(&mut self) {
        // iced calls this at the end of every frame; the trait's default body
        // is a no-op. Release glyph-atlas pages that fell out of use, so the
        // atlas cannot fill permanently — without this, the one
        // `PrepareError` variant (`AtlasFull`) that `prepare` can hit below
        // never clears and schematic text stays gone for the rest of the
        // session. Mirrors `scene_shader::ScenePipeline::trim`, which is
        // there for exactly this reason. The instance/vertex buffers are
        // intentionally left resident — they only ever grow to the scene's
        // high-water mark.
        self.text.trim_atlas();
    }
}

/// One frame's worth of schematic geometry handed to the GPU. Cheap to build
/// each frame — it is the same instance data the CPU path already produces.
#[derive(Debug)]
pub struct SchematicPrimitive {
    scene: Scene,
    /// Screen-space pan offset in logical pixels (from `ScreenTransform`).
    offset_px: [f32; 2],
    /// Zoom in logical pixels per millimetre.
    scale_px_per_mm: f32,
}

impl shader::Primitive for SchematicPrimitive {
    type Pipeline = SchematicPipeline;

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

        pipeline
            .camera
            .update(queue, CameraUniform::ortho(vp_px, offset_mm, scale_px));

        pipeline.polygon.upload(device, queue, &self.scene.polygons);
        pipeline.line.upload(device, queue, &self.scene.lines);
        pipeline.arc.upload(device, queue, &self.scene.arcs);
        pipeline.circle.upload(device, queue, &self.scene.circles);
        // Text prep fails with `PrepareError::AtlasFull` when the glyph atlas
        // has no room for this frame's glyphs. That costs this frame's text
        // and nothing else *because* `SchematicPipeline::trim` above releases
        // unused atlas pages after every frame — delete that override and the
        // failure becomes permanent for the session instead. Deliberately not
        // logged: this runs once per frame, so reporting it would emit at
        // frame rate for as long as the atlas stayed full.
        pipeline
            .text
            .upload(
                device,
                queue,
                &self.scene.texts,
                scale_px,
                [vp_px[0] as u32, vp_px[1] as u32],
                // Screen-space pan offset in physical px. Glyphon text bypasses
                // the camera ortho (which pans the instanced primitives via
                // `offset_mm`), so the pan must be applied to text explicitly —
                // mirrors `scene_shader::ScenePrimitive::prepare`. Without it,
                // schematic GPU text stays pinned while the geometry pans.
                [self.offset_px[0] * dpi, self.offset_px[1] * dpi],
            )
            .ignore(
                "a full glyph atlas drops this frame's text; `trim` frees pages every \
                 frame so the next frame retries, and a panic here would take the \
                 render thread down",
            );
    }

    fn draw(&self, pipeline: &Self::Pipeline, render_pass: &mut wgpu::RenderPass<'_>) -> bool {
        let camera = pipeline.camera.bind_group();
        // Fills first, then strokes, then text on top — matches CPU z-order.
        pipeline.polygon.draw(render_pass, camera);
        pipeline.line.draw(render_pass, camera);
        pipeline.arc.draw(render_pass, camera);
        pipeline.circle.draw(render_pass, camera);
        // Same frame, same rationale as the `upload` in `prepare`: a failed
        // text draw costs one frame of text, not the render pass, and this
        // is per-frame so it must not log.
        pipeline
            .text
            .draw(render_pass)
            .ignore("a failed text draw loses text for one frame, not the render pass");
        true
    }
}

/// The `shader::Program` the schematic view mounts when the GPU render flag is
/// on. Holds the built `Scene` and the active camera; pointer handling stays
/// on the CPU canvas layer for now (see module docs), so `update` is the
/// default no-op.
pub struct SchematicShaderProgram {
    scene: Scene,
    offset_px: [f32; 2],
    scale_px_per_mm: f32,
}

impl SchematicShaderProgram {
    /// Build from an already-tessellated `Scene` and the current transform.
    pub fn new(scene: Scene, transform: &ScreenTransform) -> Self {
        Self {
            scene,
            offset_px: [transform.offset_x, transform.offset_y],
            scale_px_per_mm: transform.scale,
        }
    }
}

impl shader::Program<Message> for SchematicShaderProgram {
    type State = ();
    type Primitive = SchematicPrimitive;

    fn draw(
        &self,
        _state: &Self::State,
        _cursor: mouse::Cursor,
        _bounds: Rectangle,
    ) -> Self::Primitive {
        SchematicPrimitive {
            scene: self.scene.clone(),
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

    /// This module's own source, embedded at compile time. Building a
    /// [`SchematicPipeline`] needs a live `wgpu::Device`, so the `trim`
    /// override below cannot be exercised at runtime here — this scans
    /// for it instead. It pins that the override exists, not that it
    /// works; the behaviour it guards is glyphon's, and
    /// `scene_shader::ScenePipeline` runs the identical call on the GPU
    /// path that is actually mounted.
    const SRC: &str = include_str!("schematic_shader.rs");

    /// The `impl shader::Pipeline for SchematicPipeline` block alone.
    /// The impl's own closing brace is the first `}` at column 0 after
    /// the opener; every method inside it closes at an indent.
    fn pipeline_impl_block() -> &'static str {
        let start = SRC
            .find("impl shader::Pipeline for SchematicPipeline {")
            .expect("the shader::Pipeline impl moved — update this guard");
        let rest = &SRC[start..];
        let end = rest
            .find("\n}\n")
            .expect("the shader::Pipeline impl has no column-0 closing brace");
        &rest[..end]
    }

    /// `PrepareError::AtlasFull` is the only thing `upload` can fail
    /// with, and `prepare` discards it. That is a one-frame loss only
    /// while something releases atlas pages between frames — the
    /// trait's own `trim` is a no-op, so without this override the
    /// atlas never drains and every schematic label stays gone for the
    /// rest of the session (#599).
    #[test]
    fn the_pipeline_overrides_trim_so_a_full_glyph_atlas_can_recover() {
        let block = pipeline_impl_block();
        assert!(
            block.contains("fn trim(&mut self)"),
            "SchematicPipeline must override shader::Pipeline::trim"
        );
        assert!(
            block.contains("self.text.trim_atlas();"),
            "the trim override must release glyph-atlas pages, as scene_shader does"
        );
    }

    fn transform(offset_x: f32, offset_y: f32, scale: f32) -> ScreenTransform {
        ScreenTransform {
            offset_x,
            offset_y,
            scale,
        }
    }

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

        let program = SchematicShaderProgram::new(scene, &transform(5.0, 6.0, 3.0));
        let primitive = program.draw(
            &(),
            iced::mouse::Cursor::Unavailable,
            iced::Rectangle::default(),
        );

        assert_eq!(primitive.scene.lines.len(), 1);
        assert_eq!(primitive.offset_px, [5.0, 6.0]);
        assert_eq!(primitive.scale_px_per_mm, 3.0);
    }
}
