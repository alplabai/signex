//! A move-drag on the GPU-mounted schematic (#199 S3).
//!
//! The drag is split the way the CPU preview splits it (`drag_layers`, #1335):
//! the items it leaves alone are the **static scene**, built once per drag and
//! again on zoom, on a pan past its margin, or on a new revision, and kept
//! resident on the GPU under one generation; the items it moves are the
//! **moving scene**, rebuilt each cursor step and uploaded every frame (it is
//! small). They are two primitive types, [`SchematicSurface`] and
//! [`SchematicDragSurface`], because iced prepares every primitive of one type
//! before rendering any of them, so two of one type in a window would overwrite
//! each other. A moving item draws above every unmoved one, as in the CPU
//! preview.
//!
//! The drag state lives in the overlay's `Program::State`, which the shaders
//! cannot reach. `update` therefore leaves the frame it computed in a
//! [`PublishedDrag`] on the slot. A click-click float can be ended by an app
//! message (Delete, Undo) that no canvas event announces, but iced forwards
//! `RedrawRequested` to `Program::update` before every draw, so `update` has
//! re-judged the gesture (`drag_delta` drops a stale float) by the time a shader
//! reads the mark. The mark therefore needs no staleness check of its own.

use std::rc::Rc;
use std::sync::Arc;

use iced::widget::shader;
use signex_gfx::scene::Scene;
use signex_types::schematic::SelectedItem;

use super::drag_layers::{DragFrame, StaticView, is_known};
use super::gpu_mount::empty_scene;
use super::*;
use crate::scene_shader::{ScenePrimitive, SchematicDragSurface};
use crate::schematic_runtime::SceneInputs;

struct Mark {
    frame: DragFrame,
    /// The scene of `frame.moving` for one set of scene inputs; built once per
    /// frame, read by the drag shader and the overlay's rotated-text pass.
    moving_scene: Option<(SceneInputs, Option<Arc<Scene>>)>,
}

/// Everything a published frame was computed from. While it holds, a new event
/// leaves the frame (and the scene built from it) as it is.
#[derive(PartialEq)]
struct FrameKey {
    offset: Option<(u64, u64)>,
    epoch: u64,
    revision: u64,
    floating: bool,
    selected: Vec<SelectedItem>,
    /// The ready rubber-banded ghost, by identity: it can become ready at an
    /// unchanged delta.
    ghost: Option<usize>,
}

#[derive(Default)]
struct Published {
    mark: Option<Mark>,
    key: Option<FrameKey>,
}

/// The move gesture the overlay last saw, for the shaders to read.
#[derive(Default)]
pub(crate) struct PublishedDrag(std::cell::RefCell<Published>);

impl PublishedDrag {
    /// The frame of the drag the overlay is drawing, if one is live.
    pub(super) fn frame(&self) -> Option<DragFrame> {
        self.0.borrow().mark.as_ref().map(|mark| mark.frame.clone())
    }

    /// Whether a drag frame is published.
    fn is_live(&self) -> bool {
        self.0.borrow().mark.is_some()
    }

    /// Whether `key` is what the published frame was computed from.
    fn is_current(&self, key: &FrameKey) -> bool {
        self.0.borrow().key.as_ref() == Some(key)
    }

    fn publish(&self, key: FrameKey, frame: Option<DragFrame>) {
        let mut published = self.0.borrow_mut();
        published.key = Some(key);
        let Some(frame) = frame else {
            published.mark = None;
            return;
        };
        // The scene built from the old frame only fits that frame.
        let moving_scene = published
            .mark
            .take()
            .filter(|old| Rc::ptr_eq(&old.frame.moving, &frame.moving))
            .and_then(|old| old.moving_scene);
        published.mark = Some(Mark {
            frame,
            moving_scene,
        });
    }

    /// Forget the gesture: the document it belonged to was replaced.
    pub(super) fn clear(&self) {
        *self.0.borrow_mut() = Published::default();
    }

    fn moving_scene(
        &self,
        view: &StaticView<'_>,
        build: impl FnOnce(&DragFrame) -> Option<Scene>,
    ) -> Option<Arc<Scene>> {
        let mut published = self.0.borrow_mut();
        let mark = published.mark.as_mut()?;
        if let Some((inputs, scene)) = &mark.moving_scene
            && inputs.fits_in_place(
                view.transform,
                view.colors,
                view.viewport,
                view.focus_set,
                view.overrides,
            )
        {
            return scene.clone();
        }
        let scene = build(&mark.frame).map(Arc::new);
        let inputs = SceneInputs::capture(
            view.transform,
            view.colors,
            view.viewport,
            view.focus_set,
            view.overrides,
        );
        mark.moving_scene = Some((inputs, scene.clone()));
        scene
    }
}

impl SchematicCanvas<'_> {
    /// The live drag, if the overlay is drawing one.
    pub(super) fn live_drag(&self) -> Option<DragFrame> {
        self.published_drag.frame()
    }

    /// Leave the shaders the drag frame of this event, and make sure a redraw
    /// follows when that turned the drag on or off.
    ///
    /// An event that only captures (Esc) requests no redraw, and the layers
    /// changing hands would otherwise stay unseen until the mouse moves.
    pub(super) fn publish_drag(
        &self,
        state: &CanvasState,
        action: Option<canvas::Action<Message>>,
    ) -> Option<canvas::Action<Message>> {
        let offset = self.drag_delta(state);
        // Idle, and nothing published to retire: no key to build (it clones the
        // selection), and this runs on every event.
        if offset.is_none() && !self.published_drag.is_live() {
            return action;
        }
        let was_live = self.published_drag.is_live();
        let revision = self.render_cache.as_ref().map_or(0, |c| c.revision());
        let key = FrameKey {
            offset: offset.map(|(dx, dy)| (dx.to_bits(), dy.to_bits())),
            epoch: self.move_epoch,
            revision,
            floating: state.is_floating(),
            selected: self.selected.clone(),
            ghost: self
                .ready_ghost_frame(state, offset, revision)
                .map(|ghost| Rc::as_ptr(&ghost.moving) as usize),
        };
        // An unknown revision never matches, as in the static layer's key.
        if is_known(revision) && self.published_drag.is_current(&key) {
            return action;
        }
        let frame = offset.and_then(|offset| self.drag_frame(state, Some(offset)));
        self.published_drag.publish(key, frame);
        if was_live == self.published_drag.is_live() {
            return action;
        }
        Some(with_redraw(action))
    }

    /// The static layer of `frame` as a scene with its generation.
    pub(super) fn drag_static_scene(
        &self,
        snapshot: &crate::schematic_runtime::SchematicRenderSnapshot,
        frame: &DragFrame,
        transform: &crate::schematic_runtime::ScreenTransform,
        bounds: Rectangle,
    ) -> (Option<Arc<Scene>>, Option<u64>) {
        let focus_set = self.auto_focus_set();
        let revision = self.render_cache.as_ref().map_or(0, |c| c.revision());
        self.drag_layers.borrow_mut().static_scene(
            snapshot,
            revision,
            &frame.skip,
            &StaticView {
                transform,
                colors: &self.prefs.canvas_colors,
                viewport: bounds.size(),
                focus_set: focus_set.as_ref(),
                overrides: self.scene_overrides(),
            },
        )
    }

    /// The moving items of the live drag as a scene for the current inputs.
    pub(super) fn drag_moving_scene(
        &self,
        transform: &crate::schematic_runtime::ScreenTransform,
        bounds: Rectangle,
    ) -> Option<Arc<Scene>> {
        let focus_set = self.auto_focus_set();
        let view = StaticView {
            transform,
            colors: &self.prefs.canvas_colors,
            viewport: bounds.size(),
            focus_set: focus_set.as_ref(),
            overrides: self.scene_overrides(),
        };
        self.published_drag.moving_scene(&view, |frame| {
            crate::schematic_runtime::build_schematic_scene(
                &frame.moving,
                transform,
                &self.prefs.canvas_colors,
                bounds,
                focus_set.as_ref(),
                self.scene_overrides(),
            )
        })
    }

    /// The rotated texts of the moving items, which the GPU glyph atlas cannot
    /// draw. Above the drag shader, as the moving items are above the rest.
    pub(in crate::canvas) fn draw_moving_rotated_texts(
        &self,
        renderer: &Renderer,
        bounds: Rectangle,
    ) -> canvas::Geometry {
        let transform = self.screen_transform();
        let bounds = crate::schematic_runtime::frame_local_bounds(bounds);
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        if let Some(scene) = self.drag_moving_scene(&transform, bounds) {
            crate::schematic_runtime::draw_rotated_text_fallback(&mut frame, &scene, &transform);
        }
        frame.into_geometry()
    }

    /// The rotated texts of everything that is not moving: the whole sheet when
    /// idle, the static layer during a drag. Drawn between the content shader
    /// and the drag shader, so a moving item passes over them like over the
    /// rest of the static layer.
    fn draw_static_rotated_texts(
        &self,
        renderer: &Renderer,
        bounds: Rectangle,
    ) -> canvas::Geometry {
        let local = crate::schematic_runtime::frame_local_bounds(bounds);
        match self.live_drag() {
            Some(drag) => self.draw_drag_static_texts(renderer, local, &drag),
            None => self.draw_idle_texts(renderer, local),
        }
    }

    /// Idle: the content scene's rotated texts, retained like the CPU content
    /// layer until the camera moves.
    fn draw_idle_texts(&self, renderer: &Renderer, local: Rectangle) -> canvas::Geometry {
        if !self.content_cache_fits_camera() {
            self.content_cache.clear();
        }
        let transform = self.screen_transform();
        let focus_set = self.auto_focus_set();
        self.content_cache.draw(renderer, local.size(), |frame| {
            let cam = self.camera();
            self.content_cache_camera
                .set((cam.offset.x, cam.offset.y, cam.scale));
            if let Some(snapshot) = self.active_snapshot() {
                self.render_content_scene(frame, snapshot, &transform, local, focus_set.as_ref());
            }
        })
    }

    /// During a drag: the static layer's rotated texts, retained with it.
    fn draw_drag_static_texts(
        &self,
        renderer: &Renderer,
        local: Rectangle,
        drag: &DragFrame,
    ) -> canvas::Geometry {
        let transform = self.screen_transform();
        let focus_set = self.auto_focus_set();
        let revision = self.render_cache.as_ref().map_or(0, |c| c.revision());
        let static_scene = self
            .active_snapshot()
            .and_then(|snapshot| self.drag_static_scene(snapshot, drag, &transform, local).0);
        let mut layers = self.drag_layers.borrow_mut();
        layers.static_is_stale(
            &StaticView {
                transform: &transform,
                colors: &self.prefs.canvas_colors,
                viewport: local.size(),
                focus_set: focus_set.as_ref(),
                overrides: self.scene_overrides(),
            },
            revision,
            &drag.skip,
        );
        layers.static_cache.draw(renderer, local.size(), |target| {
            if let Some(scene) = &static_scene {
                crate::schematic_runtime::draw_rotated_text_fallback(target, scene, &transform);
            }
        })
    }
}

/// The rotated texts of the static layer, on a canvas of their own between the
/// content shader and the drag shader. No `update`.
pub(crate) struct SchematicStaticTextLayer<'a>(SchematicCanvas<'a>);

impl<'a> SchematicStaticTextLayer<'a> {
    pub(crate) fn new(slot: &'a CanvasSlot, prefs: CanvasViewPrefs<'a>) -> Self {
        Self(SchematicCanvas::new(slot, prefs).gpu_overlay())
    }
}

impl canvas::Program<Message> for SchematicStaticTextLayer<'_> {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        vec![self.0.draw_static_rotated_texts(renderer, bounds)]
    }
}

/// `action` with a redraw guaranteed, keeping its message and capture.
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

/// The generation an idle drag layer carries: constant, so the GPU uploads its
/// empty scene once and then skips.
fn idle_generation() -> u64 {
    static IDLE: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    *IDLE.get_or_init(crate::scene_generation::next_scene_generation)
}

/// The moving items of a move-drag as one GPU primitive of the
/// [`SchematicDragSurface`] slot. Empty when no drag is live. No `update`.
pub(crate) struct SchematicDragShaderProgram<'a>(SchematicCanvas<'a>);

impl<'a> SchematicDragShaderProgram<'a> {
    pub(crate) fn new(slot: &'a CanvasSlot, prefs: CanvasViewPrefs<'a>) -> Self {
        Self(SchematicCanvas::new(slot, prefs).gpu_overlay())
    }
}

impl shader::Program<Message> for SchematicDragShaderProgram<'_> {
    type State = ();
    type Primitive = ScenePrimitive<SchematicDragSurface>;

    fn draw(
        &self,
        _state: &Self::State,
        _cursor: mouse::Cursor,
        bounds: Rectangle,
    ) -> Self::Primitive {
        let cam = self.0.camera();
        let offset = [cam.offset.x, cam.offset.y];
        if self.0.live_drag().is_none() {
            return ScenePrimitive::new(empty_scene(), Some(idle_generation()), offset, cam.scale);
        }
        let transform = self.0.screen_transform();
        let local = crate::schematic_runtime::frame_local_bounds(bounds);
        // `None`: the moving set changes with every cursor step, so it uploads
        // every frame.
        let scene = self
            .0
            .drag_moving_scene(&transform, local)
            .unwrap_or_else(empty_scene);
        ScenePrimitive::new(scene, None, offset, cam.scale)
    }
}
