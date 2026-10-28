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
//! `drag_layers`) and the shader draws nothing until the drag ends
//! ([`DragOnCpu`]). Nothing is drawn twice; a GPU drag split is a later slice.

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
        shader(SchematicShaderProgram::new(slot, prefs))
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
        if self.drag_on_cpu.is_live(self.move_epoch, &self.selected) {
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

/// What the overlay last saw of a move gesture, for the shader to read.
///
/// The drag state lives in the overlay's `Program::State`, which the shader
/// cannot reach, so `update` leaves a mark here. A click-click float can be
/// ended by an app message (Delete, Undo) that no canvas event announces, so
/// the mark keeps what the overlay judges staleness by, the move epoch and the
/// selection, and [`Self::is_live`] re-judges at draw time instead of trusting
/// the last event.
#[derive(Default)]
pub(crate) struct DragOnCpu(std::cell::RefCell<Option<Mark>>);

struct Mark {
    /// `Some` for a click-click float: the epoch and selection it was seen with.
    float: Option<(u64, super::move_pick::SelectionSet)>,
}

/// What `update` saw of the move gesture.
pub(super) enum Gesture<'a> {
    Idle,
    Drag,
    Float {
        epoch: u64,
        selected: &'a [signex_types::schematic::SelectedItem],
    },
}

impl DragOnCpu {
    /// Whether the overlay is drawing a drag the shader must stand aside for.
    pub(super) fn is_live(
        &self,
        move_epoch: u64,
        selected: &[signex_types::schematic::SelectedItem],
    ) -> bool {
        match &*self.0.borrow() {
            None => false,
            Some(Mark { float: None }) => true,
            Some(Mark {
                float: Some((epoch, set)),
            }) => *epoch == move_epoch && super::move_pick::same_selection(set, selected),
        }
    }

    pub(super) fn publish(&self, gesture: Gesture<'_>) {
        let mut mark = self.0.borrow_mut();
        *mark = match gesture {
            Gesture::Idle => None,
            Gesture::Drag => Some(Mark { float: None }),
            Gesture::Float { epoch, selected } => {
                let unchanged = matches!(&*mark, Some(Mark { float: Some((e, set)) })
                    if *e == epoch && super::move_pick::same_selection(set, selected));
                if unchanged {
                    return;
                }
                Some(Mark {
                    float: Some((epoch, selected.iter().cloned().collect())),
                })
            }
        };
    }

    /// Forget the gesture: the canvas it belonged to is gone or reloaded.
    pub(super) fn clear(&self) {
        *self.0.borrow_mut() = None;
    }
}

impl SchematicCanvas<'_> {
    /// Leave the shader a mark of the move gesture after `action`'s event, and
    /// make sure a redraw follows when that turned it on or off.
    ///
    /// An event that only captures (Esc) requests no redraw, and the shader
    /// returning would otherwise stay unseen until the mouse moves.
    // iced forwards every event here, `RedrawRequested` before each draw
    // included, so the mark is re-judged right before the shader reads it.
    pub(super) fn publish_drag(
        &self,
        state: &CanvasState,
        action: Option<canvas::Action<Message>>,
    ) -> Option<canvas::Action<Message>> {
        let was_live = self.drag_on_cpu.is_live(self.move_epoch, &self.selected);
        let gesture = if self.drag_delta(state).is_none() {
            Gesture::Idle
        } else if state.is_floating() {
            Gesture::Float {
                epoch: self.move_epoch,
                selected: &self.selected,
            }
        } else {
            Gesture::Drag
        };
        self.drag_on_cpu.publish(gesture);
        if was_live == self.drag_on_cpu.is_live(self.move_epoch, &self.selected) {
            return action;
        }
        Some(with_redraw(action))
    }
}

/// `action` with a redraw guaranteed, keeping its message and capture.
///
/// The action's own redraw request is dropped for `NextFrame`: this is only
/// reached when the gesture just flipped, and the next frame is the soonest the
/// shader can show it, so a later `RedrawRequest::At` would only delay that.
fn with_redraw(action: Option<canvas::Action<Message>>) -> canvas::Action<Message> {
    let Some(action) = action else {
        return canvas::Action::request_redraw();
    };
    let (message, _, status) = action.into_inner();
    let rebuilt = match message {
        Some(message) => canvas::Action::publish(message),
        None => canvas::Action::request_redraw(),
    };
    if status == iced::event::Status::Captured {
        rebuilt.and_capture()
    } else {
        rebuilt
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
