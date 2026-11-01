//! The schematic on the GPU path (#199): a five-layer `stack!`, mounted only
//! when [`crate::feature_flags::SCHEMATIC_GPU_RENDER_ENABLED`] is on. Layer
//! order, the move-drag split and the reasons are in
//! `docs/architecture/schematic-gpu-mount.md`.

use std::sync::Arc;

use iced::widget::{shader, stack};
use iced::{Element, Length};
use signex_gfx::scene::Scene;

use super::gpu_drag::{SchematicDragShaderProgram, SchematicStaticTextLayer};
use super::*;
use crate::scene_shader::{ScenePrimitive, SchematicSurface};

/// What a [`SchematicCanvas`] draws for the sheet content.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ContentMode {
    /// Everything, on the CPU, background included.
    #[default]
    Cpu,
    /// Only what the GPU shader beneath cannot draw; no background.
    GpuOverlay,
}

/// The GPU-mounted schematic of one window's `slot`.
pub(crate) fn view<'a>(slot: &'a CanvasSlot, prefs: CanvasViewPrefs<'a>) -> Element<'a, Message> {
    stack![
        canvas(SchematicBackdrop::new(slot, prefs))
            .width(Length::Fill)
            .height(Length::Fill),
        shader(SchematicShaderProgram::new(slot, prefs))
            .width(Length::Fill)
            .height(Length::Fill),
        canvas(SchematicStaticTextLayer::new(slot, prefs))
            .width(Length::Fill)
            .height(Length::Fill),
        shader(SchematicDragShaderProgram::new(slot, prefs))
            .width(Length::Fill)
            .height(Length::Fill),
        canvas(SchematicCanvas::new(slot, prefs).gpu_overlay())
            .width(Length::Fill)
            .height(Length::Fill),
    ]
    .into()
}

impl SchematicCanvas<'_> {
    /// A canvas that leaves the sheet content to the GPU shader beneath it.
    pub(crate) fn gpu_overlay(mut self) -> Self {
        self.mode = ContentMode::GpuOverlay;
        self
    }

    /// The live pan/zoom as the transform the scene is built and replayed with.
    pub(super) fn screen_transform(&self) -> crate::schematic_runtime::ScreenTransform {
        let cam = self.camera();
        crate::schematic_runtime::ScreenTransform {
            offset_x: cam.offset.x,
            offset_y: cam.offset.y,
            scale: cam.scale,
            device_px_per_logical_px: self.prefs.device_px_per_logical_px,
        }
    }

    /// The scene the shader draws, with its generation: the sheet content, or
    /// during a move-drag its static layer.
    ///
    /// Goes through the same [`ContentSceneCache`] the CPU replay uses, so the
    /// overlay's rotated-text pass reads the same scene.
    ///
    /// [`ContentSceneCache`]: crate::schematic_runtime::ContentSceneCache
    fn gpu_scene(&self, bounds: Rectangle) -> (Arc<Scene>, Option<u64>) {
        let Some(snapshot) = self.active_snapshot() else {
            return (empty_scene(), None);
        };
        let transform = self.screen_transform();
        let bounds = crate::schematic_runtime::frame_local_bounds(bounds);
        if let Some(frame) = self.live_drag() {
            let (scene, generation) = self.drag_static_scene(snapshot, &frame, &transform, bounds);
            return (scene.unwrap_or_else(empty_scene), generation);
        }
        let focus_set = self.auto_focus_set();
        let mut cache = self.content_scene.borrow_mut();
        let scene = cache
            .scene_for(
                snapshot,
                &transform,
                &self.prefs.canvas_colors,
                bounds,
                focus_set.as_ref(),
                self.scene_overrides(),
            )
            .cloned();
        match scene {
            Some(scene) => (scene, cache.generation()),
            None => (empty_scene(), None),
        }
    }
}

/// A shared empty scene, so an idle layer allocates nothing per frame.
pub(super) fn empty_scene() -> Arc<Scene> {
    static EMPTY: std::sync::OnceLock<Arc<Scene>> = std::sync::OnceLock::new();
    Arc::clone(EMPTY.get_or_init(Default::default))
}

/// Paper, grid and sheet frame, and nothing that reacts to input.
pub(crate) struct SchematicBackdrop<'a>(SchematicCanvas<'a>);

impl<'a> SchematicBackdrop<'a> {
    pub(crate) fn new(slot: &'a CanvasSlot, prefs: CanvasViewPrefs<'a>) -> Self {
        Self(SchematicCanvas::new(slot, prefs))
    }
}

impl canvas::Program<Message> for SchematicBackdrop<'_> {
    type State = ();

    // No `update`: the default returns `None`, so this layer never emits a
    // message and never captures an event.

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        vec![self.0.draw_background(renderer, bounds)]
    }
}

/// The sheet content as one GPU primitive of the [`SchematicSurface`] slot.
pub(crate) struct SchematicShaderProgram<'a>(SchematicCanvas<'a>);

impl<'a> SchematicShaderProgram<'a> {
    pub(crate) fn new(slot: &'a CanvasSlot, prefs: CanvasViewPrefs<'a>) -> Self {
        Self(SchematicCanvas::new(slot, prefs))
    }
}

impl shader::Program<Message> for SchematicShaderProgram<'_> {
    type State = ();
    type Primitive = ScenePrimitive<SchematicSurface>;

    fn draw(
        &self,
        _state: &Self::State,
        _cursor: mouse::Cursor,
        bounds: Rectangle,
    ) -> Self::Primitive {
        let cam = self.0.camera();
        let (scene, generation) = self.0.gpu_scene(bounds);
        ScenePrimitive::new(scene, generation, [cam.offset.x, cam.offset.y], cam.scale)
    }
}
