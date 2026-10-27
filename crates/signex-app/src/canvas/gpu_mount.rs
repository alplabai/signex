//! The schematic on the GPU path (#199 S2).
//!
//! Mounted only when [`crate::feature_flags::SCHEMATIC_GPU_RENDER_ENABLED`] is
//! on, as three layers bottom to top:
//!
//! 1. [`SchematicBackdrop`] — paper, grid and sheet frame. No `update`:
//!    `stack!` hands every event to every child, so a second handler would
//!    place two symbols per click.
//! 2. [`SchematicShaderProgram`] — the sheet content on the GPU. Its scene
//!    is built here, in `draw`, because only `draw` knows the cull bounds.
//! 3. The interactive [`SchematicCanvas`] in [`ContentMode::GpuOverlay`] — the
//!    only layer with `update`. Its content layer draws just the rotated texts
//!    the GPU glyph atlas cannot (`TextItem::is_axis_aligned`), then dim,
//!    selection, ghosts, previews and the HUD as ever.
//!
//! The PCB's order (interactive canvas at the bottom) is not reused: it would
//! bury selection and ghosts under the content.
//!
//! **A move-drag falls back to the CPU, wholly.** The drag state lives in the
//! overlay's `Program::State`, which the shader cannot see, so the overlay
//! draws the sheet itself (the retained static layer plus the moving items,
//! `drag_layers`) and the shader draws nothing until the drag ends. Nothing is
//! drawn twice; a GPU drag split is a later slice.

use std::sync::Arc;

use iced::widget::{shader, stack};
use iced::{Element, Length};
use signex_gfx::scene::Scene;

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
        shader(SchematicShaderProgram(SchematicCanvas::new(slot, prefs)))
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

    /// The scene the shader draws, with its generation.
    ///
    /// Empty (and uncached) while a drag draws on the CPU, and when there is
    /// nothing to draw. Goes through the same [`ContentSceneCache`] the CPU
    /// replay uses, so the overlay's rotated-text pass reads the same scene.
    ///
    /// [`ContentSceneCache`]: crate::schematic_runtime::ContentSceneCache
    fn gpu_scene(&self, bounds: Rectangle) -> (Arc<Scene>, Option<u64>) {
        let empty = || (Arc::new(Scene::default()), None);
        if self.drag_on_cpu.get() {
            return empty();
        }
        let Some(snapshot) = self.active_snapshot() else {
            return empty();
        };
        let cam = self.camera();
        let transform = crate::schematic_runtime::ScreenTransform {
            offset_x: cam.offset.x,
            offset_y: cam.offset.y,
            scale: cam.scale,
        };
        let focus_set = self.auto_focus_set();
        let mut cache = self.content_scene.borrow_mut();
        let scene = cache
            .scene_for(
                snapshot,
                &transform,
                &self.prefs.canvas_colors,
                crate::schematic_runtime::frame_local_bounds(bounds),
                focus_set.as_ref(),
                self.scene_overrides(),
            )
            .cloned();
        match scene {
            Some(scene) => (scene, cache.generation()),
            None => empty(),
        }
    }
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
