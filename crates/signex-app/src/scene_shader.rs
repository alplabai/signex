//! Generic GPU render path for any `signex_gfx::scene::Scene`.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: iced/wgpu public docs, IPC-2612-1, IEEE 315, IEC 60617.
//!
//! Bridges the `signex_gfx` render pipelines into iced's `shader` widget so a
//! `Scene` draws on the GPU instead of being tessellated into a
//! `canvas::Frame` on the CPU. The pipelines are primitive-agnostic — they are
//! driven purely by a `Scene` plus a screen-space pan/zoom transform — so every
//! editor surface can share this one renderer.
//!
//! Provenance: the PCB path landed in #308. A second, near-identical copy of
//! this module (`schematic_shader`, from #169 PR 2) sat unmounted beside it
//! until #625 folded it in here; the two had already drifted apart on upload
//! skipping, overlay compositing, draw order and error reporting.
//!
//! Mounted by: [`crate::pcb_canvas`] (the PCB editor), via `app/view/mod.rs`.
//! The schematic and Symbol Editor surfaces are not mounted yet — see #199
//! and #642.
//!
//! iced's shader `Primitive::draw` composites into the shared render pass over
//! whatever was already drawn behind the widget (it never clears its own
//! region), so the caller is responsible for painting the background + grid on
//! a layer *below* this shader in a `stack!`.

use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use iced::widget::shader::{self, Viewport};
use iced::{Rectangle, mouse};

use signex_gfx::camera::{CameraGpu, CameraUniform};
use signex_gfx::pipeline::arc::ArcPipeline;
use signex_gfx::pipeline::circle::CirclePipeline;
use signex_gfx::pipeline::line::LinePipeline;
use signex_gfx::pipeline::polygon::PolygonPipeline;
use signex_gfx::pipeline::text::GlyphonTextPipeline;
use signex_gfx::scene::{GPU_SCENE_DRAW_ORDER, Scene, SceneBucket};
use signex_gfx::wgpu;

use crate::app::Message;

/// One editor surface that draws its `Scene` on the GPU.
///
/// This exists purely to give each surface its own pipeline slot. iced stores
/// a primitive's pipeline in a map keyed by the **primitive's** `TypeId`
/// (`iced_wgpu::primitive::Storage::store::<P, _>`, called from
/// `BlackBox::<P>::prepare`), and that map lives in an
/// `Arc<RwLock<primitive::Storage>>` on a cloned `Engine` — one map for the
/// whole process, not one per window. iced also prepares *every* primitive in
/// a frame before drawing *any* of them. So two widgets emitting the same
/// primitive type share one instance-buffer set and one camera, and the second
/// `prepare` overwrites the first before either draws.
///
/// Making [`ScenePrimitive`] generic over this trait means
/// `ScenePrimitive<PcbSurface>` and a future `ScenePrimitive<SchematicSurface>`
/// are distinct types, so they get distinct pipeline slots.
///
/// **To add a surface:** declare a marker here, implement this trait, and mount
/// `SceneShaderProgram::<YourSurface>::new(...)`. Do not reach for a second
/// copy of this module — that is what #625 removed.
///
/// **Known gap.** The type-level split separates *surfaces*, not *instances of
/// one surface*. Two undocked schematic windows would both emit
/// `ScenePrimitive<SchematicSurface>` and collide exactly as above. Fixing that
/// needs a runtime key (window id) selecting per-instance buffers inside the
/// pipeline, which means splitting the `signex_gfx` pipelines into a shared
/// program plus per-key instance buffers. Not needed while the PCB is the only
/// mounted surface — it has a single `PcbCanvas`, not one per window — but it
/// must be solved before the schematic mounts (#199).
pub trait SceneSurface: 'static + Send + Sync + std::fmt::Debug {
    /// Name used in this surface's one-shot text-failure warnings.
    const LABEL: &'static str;
}

/// The PCB editor's scene surface.
#[derive(Debug, Clone, Copy)]
pub struct PcbSurface;

impl SceneSurface for PcbSurface {
    const LABEL: &'static str = "PCB";
}

/// World coordinate (mm) at the render pass origin (top-left).
///
/// The screen mapping is `screen_px = world_mm * scale + offset_px` — the same
/// mapping [`crate::schematic_runtime::ScreenTransform::world_to_screen`]
/// applies — so the world point drawn at the top-left corner is
/// `-offset / scale`. Returns the origin unchanged when the scale is
/// degenerate.
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

/// Report the first glyph-atlas failure of each kind and stay silent after.
///
/// A dropped frame of text beats panicking the render thread, but a swallowed
/// failure that never reaches the Messages panel is not reporting either. This
/// runs once per frame, so it must not log at frame rate.
fn log_text_error_once(
    flag: &AtomicBool,
    surface: &str,
    stage: &str,
    error: impl std::fmt::Display,
) {
    if !flag.swap(true, Ordering::Relaxed) {
        tracing::warn!(
            target: "signex::gfx",
            "{surface} GPU text {stage} failed ({error}); dropping this frame's text"
        );
    }
}

/// The set of `signex_gfx` pipelines plus the camera, created once by iced and
/// reused across frames.
///
/// iced stores one of these per primitive type (see [`SceneSurface`]), so each
/// surface gets its own buffers, camera and warn-once flags.
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
    /// First-failure latches for the two glyph-atlas error paths. `AtomicBool`
    /// rather than `bool` because `Primitive::draw` only borrows the pipeline
    /// immutably, and iced requires a `Pipeline` to be `Send + Sync`.
    text_upload_warned: AtomicBool,
    text_draw_warned: AtomicBool,
}

impl shader::Pipeline for ScenePipeline {
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
            uploaded_generation: None,
            text_upload_warned: AtomicBool::new(false),
            text_draw_warned: AtomicBool::new(false),
        }
    }

    fn trim(&mut self) {
        // CORRECTNESS-CRITICAL, not memory hygiene. iced calls this at the end
        // of every frame and the trait's own body is a no-op
        // (`iced_wgpu::primitive::Pipeline::trim`), so omitting this override
        // is silently legal and silently wrong: `PrepareError::AtlasFull` is
        // the one error `prepare` can hit below, and it is swallowed there on
        // the grounds that the next frame retries. Without this call nothing
        // ever releases atlas pages, so that failure never clears and text
        // stays gone for the rest of the session (#599 / #609).
        //
        // The instance/vertex buffers are intentionally left resident — they
        // only ever grow to the scene's high-water mark.
        self.text.trim_atlas();
    }
}

/// One frame's worth of scene geometry handed to the GPU. Cheap to build each
/// frame — it is the same instance data the CPU path already produces.
///
/// `S` selects the pipeline slot; see [`SceneSurface`].
#[derive(Debug)]
pub struct ScenePrimitive<S: SceneSurface> {
    /// Shared with the owning [`SceneShaderProgram`] and the source's scene
    /// cache: building the primitive each frame is an `Arc` refcount bump, not
    /// a deep copy of the geometry.
    pub scene: Arc<Scene>,
    /// Identity of `scene`'s geometry, used to skip redundant GPU uploads.
    /// `Some(g)` comes from a cached source (the PCB `gpu_scene` cache) that
    /// bumps `g` only when the geometry actually changes, so equal generations
    /// across frames mean "same geometry, don't re-upload". `None` marks an
    /// uncached source that must upload every frame — which is the honest
    /// answer for any surface whose scene depends on the camera (viewport
    /// culling or zoom-derived stroke widths), not a missing optimisation.
    pub generation: Option<u64>,
    /// Screen-space pan offset in logical pixels.
    pub offset_px: [f32; 2],
    /// Zoom in logical pixels per millimetre.
    pub scale_px_per_mm: f32,
    surface: PhantomData<S>,
}

impl<S: SceneSurface> shader::Primitive for ScenePrimitive<S> {
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
            // Overlay geometry uploads into each pipeline's dedicated overlay
            // buffer (never folded into the base buckets above — see
            // `pcb_canvas::gpu_scene`), so `Self::draw`'s later overlay pass
            // always composites on top of every base bucket.
            pipeline
                .polygon
                .upload_overlay(device, queue, &self.scene.overlay_polygons);
            pipeline
                .line
                .upload_overlay(device, queue, &self.scene.overlay_lines);
            pipeline
                .circle
                .upload_overlay(device, queue, &self.scene.overlay_circles);
            pipeline.uploaded_generation = self.generation;
        }

        // Text is NOT guarded: glyphon rasterises glyphs in screen-pixel space
        // (`size_mm * scale_px_per_mm` plus the pan offset), so it must be
        // re-prepared every frame that pan/zoom/viewport changes. The pan term
        // is passed explicitly because glyphon works in screen space and, unlike
        // the instanced primitives, does not go through the camera ortho.
        //
        // Prep can fail with `PrepareError::AtlasFull`. Swallowing it costs
        // this frame's text and nothing else *because* `ScenePipeline::trim`
        // releases unused atlas pages after every frame — delete that override
        // and this swallow makes text loss permanent for the session instead.
        // A panic here would take the render thread down.
        if let Err(error) = pipeline.text.upload(
            device,
            queue,
            &self.scene.texts,
            scale_px,
            [vp_px[0] as u32, vp_px[1] as u32],
            [self.offset_px[0] * dpi, self.offset_px[1] * dpi],
        ) {
            log_text_error_once(&pipeline.text_upload_warned, S::LABEL, "upload", error);
        }
    }

    fn draw(&self, pipeline: &Self::Pipeline, render_pass: &mut wgpu::RenderPass<'_>) -> bool {
        let camera = pipeline.camera.bind_group();
        // Composite in the shared `GPU_SCENE_DRAW_ORDER` (fills, then strokes,
        // then text on top). The order lives in one const so it can be diffed
        // against the CPU `pcb_canvas::draw_scene` order: polygons draw *first*
        // here but *last* on the CPU — a known base-bucket z-order divergence
        // the `scene::order` parity test pins until visual authority
        // reconciles it (reserved for Caner/Hakan). GPU parity stays
        // unconfirmed on hardware.
        for &bucket in GPU_SCENE_DRAW_ORDER {
            match bucket {
                SceneBucket::Polygons => pipeline.polygon.draw(render_pass, camera),
                SceneBucket::Lines => pipeline.line.draw(render_pass, camera),
                SceneBucket::Arcs => pipeline.arc.draw(render_pass, camera),
                SceneBucket::Circles => pipeline.circle.draw(render_pass, camera),
                SceneBucket::Texts => {
                    // Same rationale as the `upload` in `prepare`: a failed
                    // text draw loses text for one frame, not the render pass.
                    if let Err(error) = pipeline.text.draw(render_pass) {
                        log_text_error_once(&pipeline.text_draw_warned, S::LABEL, "draw", error);
                    }
                }
                // Not composited here — overlays get their own pass below
                // (always after every base bucket). The ERC buckets are
                // schematic-only AND currently unpopulated by any production
                // path: `SchematicRenderer::build_scene` fills them only under
                // `DirtyFlags::OVERLAY`, and the one production caller that
                // passes that flag (`schematic_runtime::overlay::draw_erc_markers`)
                // routes its markers through `OverlayInputs` instead. Schematic
                // ERC marks and PCB DRC marks are both overlay geometry, so the
                // overlay pass below already carries them.
                //
                // Handled for exhaustiveness so adding a Scene bucket forces a
                // decision here. Note this does NOT fire when a new *surface*
                // is mounted — that path needs its own review.
                SceneBucket::OverlayLines
                | SceneBucket::OverlayCircles
                | SceneBucket::OverlayPolygons
                | SceneBucket::ErcMarkerLines
                | SceneBucket::ErcMarkerCircles
                | SceneBucket::ErcMarkerPolygons => {}
            }
        }

        // Overlay pass: strictly after every base bucket above, so overlay
        // content (active-layer zone highlight, selection highlight, DRC
        // markers, ratsnest) always renders on top — matching the CPU
        // `pcb_canvas::draw_scene` overlay pass, which draws
        // OverlayLines/OverlayCircles/OverlayPolygons last. Order within the
        // pass mirrors the CPU's (lines, then circles, then polygon fills +
        // strokes on top).
        pipeline.line.draw_overlay(render_pass, camera);
        pipeline.circle.draw_overlay(render_pass, camera);
        pipeline.polygon.draw_overlay(render_pass, camera);

        true
    }
}

/// The `shader::Program` mounted when a view routes its `Scene` through the
/// GPU. Holds the built `Scene` and the active pan/zoom; pointer handling
/// stays on the CPU `canvas` layer stacked beneath this shader, so `update`
/// is the default no-op and never captures — events fall through to the
/// canvas below.
pub struct SceneShaderProgram<S: SceneSurface> {
    scene: Arc<Scene>,
    generation: Option<u64>,
    offset_px: [f32; 2],
    scale_px_per_mm: f32,
    surface: PhantomData<S>,
}

impl<S: SceneSurface> SceneShaderProgram<S> {
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
            surface: PhantomData,
        }
    }
}

impl<S: SceneSurface> shader::Program<Message> for SceneShaderProgram<S> {
    type State = ();
    type Primitive = ScenePrimitive<S>;

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
            surface: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::widget::shader::Program;
    use signex_gfx::primitive::line::LineSegment;

    /// This module's own source, embedded at compile time. Building a
    /// [`ScenePipeline`] needs a live `wgpu::Device`, so the `trim`
    /// override below cannot be exercised at runtime here — this scans
    /// for it instead. It pins that the override exists, not that it
    /// works; the behaviour it guards is glyphon's.
    ///
    /// Ported from `schematic_shader.rs` by #625, where it guarded the
    /// *unmounted* copy while this — the mounted one — had no guard at all.
    const SRC: &str = include_str!("scene_shader.rs");

    /// The `impl shader::Pipeline for ScenePipeline` block alone.
    /// The impl's own closing brace is the first `}` at column 0 after
    /// the opener; every method inside it closes at an indent.
    fn pipeline_impl_block() -> &'static str {
        let start = SRC
            .find("impl shader::Pipeline for ScenePipeline {")
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
    /// atlas never drains and every label stays gone for the rest of
    /// the session (#599).
    #[test]
    fn the_pipeline_overrides_trim_so_a_full_glyph_atlas_can_recover() {
        let block = pipeline_impl_block();
        assert!(
            block.contains("fn trim(&mut self)"),
            "ScenePipeline must override shader::Pipeline::trim"
        );
        assert!(
            block.contains("self.text.trim_atlas();"),
            "the trim override must release glyph-atlas pages"
        );
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

    /// The mapping this mirrors is `ScreenTransform::world_to_screen`
    /// (`screen_px = world_mm * scale + offset_px`), so a surface feeding this
    /// program from a `ScreenTransform` passes `offset_x` / `offset_y` as
    /// `offset_px` and `scale` as `scale_px_per_mm`. Ported from
    /// `schematic_shader`'s round-trip test (#625) — that file held the only
    /// check tying the two together.
    #[test]
    fn world_origin_matches_the_screen_transform_mapping() {
        let (offset_x, offset_y, scale) = (80.0_f32, 40.0_f32, 4.0_f32);
        let transform = crate::schematic_runtime::ScreenTransform {
            offset_x,
            offset_y,
            scale,
        };
        // The world point that lands on the widget's top-left pixel.
        let origin = world_origin_mm([offset_x, offset_y], scale);
        let back = transform.world_to_screen((origin[0] as f64, origin[1] as f64));
        assert!(back.x.abs() < 1e-3, "origin must map to screen x = 0");
        assert!(back.y.abs() < 1e-3, "origin must map to screen y = 0");
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

        let program =
            SceneShaderProgram::<PcbSurface>::new(Arc::new(scene), Some(7), [5.0, 6.0], 3.0);
        let primitive = program.draw(
            &(),
            iced::mouse::Cursor::Unavailable,
            iced::Rectangle::default(),
        );

        assert_eq!(primitive.scene.lines.len(), 1);
        assert_eq!(primitive.generation, Some(7));
        assert_eq!(primitive.offset_px, [5.0, 6.0]);
        assert_eq!(primitive.scale_px_per_mm, 3.0);
    }

    /// The whole point of the [`SceneSurface`] parameter: iced keys a stored
    /// pipeline by the primitive's `TypeId`, so two surfaces must not share
    /// one. Pins that adding a marker actually produces a distinct type.
    #[test]
    fn each_surface_gets_its_own_primitive_type() {
        #[derive(Debug, Clone, Copy)]
        struct OtherSurface;
        impl SceneSurface for OtherSurface {
            const LABEL: &'static str = "other";
        }

        assert_ne!(
            std::any::TypeId::of::<ScenePrimitive<PcbSurface>>(),
            std::any::TypeId::of::<ScenePrimitive<OtherSurface>>(),
            "two surfaces sharing a TypeId would share one instance buffer set \
             and one camera, and the second prepare would overwrite the first"
        );
    }
}
