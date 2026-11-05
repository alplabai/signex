//! The CPU schematic canvas widget.
//!
//! Mirrors iced's `Canvas` widget (tag, state, size, layout, update and
//! mouse interaction are the same and defer to [`canvas::Program`]); only
//! `draw` differs. It clips to the canvas and draws the retained content layer
//! under an extra translation, so a pan moves that geometry instead of
//! tessellating it again (#2078). See `docs/RENDERING_RULES.md`, "sch-pan-reuse".

use iced::advanced::graphics::geometry::Renderer as _;
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer::Renderer as _;
use iced::advanced::widget::{self, Tree, tree};
use iced::advanced::{Shell, Widget, renderer};
use iced::widget::canvas::Program as _;
use iced::{Event, Length, Size, Vector, window};

use super::*;

/// The layers of one frame, bottom to top.
pub(super) struct SurfaceLayers {
    pub(super) layers: Vec<canvas::Geometry>,
    /// The retained content layer, when it is to be drawn translated:
    /// its index in `layers` and the translation.
    pub(super) content_shift: Option<(usize, Vector)>,
}

impl SchematicCanvas<'_> {
    /// The layers of a frame in z-order. `allow_shift` lets the content layer
    /// stand for an earlier camera offset, drawn translated by
    /// `content_shift`; a caller that draws the layers as they are passes
    /// `false`.
    pub(super) fn draw_surface(
        &self,
        state: &CanvasState,
        renderer: &Renderer,
        bounds: Rectangle,
        cursor: mouse::Cursor,
        allow_shift: bool,
    ) -> SurfaceLayers {
        let mut layers = Vec::with_capacity(4);
        let mut content_shift = None;

        // Layer 1: background (grid + paper). A GPU-mounted sheet has the
        // backdrop canvas for it beneath the shader.
        if self.mode == ContentMode::Cpu {
            layers.push(self.draw_background(renderer, bounds));
        }

        let drag_offset = self.drag_delta(state);

        // A move-drag draws the sheet in two layers (#1335): the items the
        // drag leaves alone in a retained geometry, and each frame only the
        // moving items, shifted. With "keep wires" the moving items include
        // the rubber-banded wires computed when the cursor moved
        // (`refresh_drag_ghost`); until then the plain shift stands in.
        //
        // A GPU-mounted sheet reads the frame `update` published, so the
        // shaders and this layer agree on what is moving.
        let drag_frame = match self.mode {
            ContentMode::Cpu => self.drag_frame(state, drag_offset),
            ContentMode::GpuOverlay => self.live_drag(),
        };
        let Some(drag_frame) = drag_frame else {
            if self.drag_layers.borrow().is_active() {
                self.drag_layers.borrow_mut().clear();
            }
            let snapshot = self.active_snapshot();

            // Layer 2: content (schematic elements). A GPU-mounted sheet has
            // its content on the shaders, and its rotated and oversize texts on a layer
            // of their own beneath the drag shader.
            if self.mode == ContentMode::Cpu {
                let content =
                    self.draw_content(renderer, bounds, snapshot, drag_offset, allow_shift);
                if content.shift != Vector::ZERO {
                    content_shift = Some((layers.len(), content.shift));
                }
                layers.push(content.geometry);
            }

            // Layer 2.5: AutoFocus dim
            if let Some(dim) = self.draw_autofocus_dim(renderer, bounds, snapshot) {
                layers.push(dim);
            }

            // Layer 3: selection overlay
            if let Some(selection) = self.draw_selection(renderer, bounds, snapshot, drag_offset) {
                layers.push(selection);
            }

            // Layer 4: overlay (cursor HUD, previews, drag guides)
            layers.push(self.draw_overlay(state, renderer, bounds, cursor));
            return SurfaceLayers {
                layers,
                content_shift,
            };
        };
        let moving = Some(&*drag_frame.moving);

        // Layer 2: content, static layer then moving items on top
        match self.mode {
            ContentMode::Cpu => {
                layers.extend(self.draw_drag_content(renderer, bounds, &drag_frame))
            }
            // The static layer's rotated and oversize texts have their own layer below the
            // drag shader; only the moving ones belong above it.
            ContentMode::GpuOverlay => {
                layers.push(self.draw_moving_canvas_texts(renderer, bounds));
            }
        }

        // Layer 2.5: AutoFocus dim
        if let Some(dim) = self.draw_autofocus_dim(renderer, bounds, moving) {
            layers.push(dim);
        }

        // Layer 3: selection overlay
        if let Some(selection) = self.draw_selection(renderer, bounds, moving, drag_offset) {
            layers.push(selection);
        }

        // Layer 4: overlay (cursor HUD, previews, drag guides)
        layers.push(self.draw_overlay(state, renderer, bounds, cursor));

        SurfaceLayers {
            layers,
            content_shift,
        }
    }
}

/// The CPU schematic canvas, as a widget.
pub struct SchematicSurface<'a> {
    program: SchematicCanvas<'a>,
    last_mouse_interaction: Option<mouse::Interaction>,
}

impl<'a> SchematicSurface<'a> {
    pub fn new(program: SchematicCanvas<'a>) -> Self {
        Self {
            program,
            last_mouse_interaction: None,
        }
    }
}

impl widget::Meta for SchematicSurface<'_> {}

impl Widget<Message, Theme, Renderer> for SchematicSurface<'_> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<CanvasState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(CanvasState::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(&mut self, tree: &mut Tree, _renderer: &Renderer, limits: &layout::Limits) {
        tree.size = layout::atomic(limits, Length::Fill, Length::Fill);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let state = tree.state.downcast_mut::<CanvasState>();
        let is_redraw_request = matches!(event, Event::Window(window::Event::RedrawRequested(_)));

        if let Some(action) = self.program.update(state, event, bounds, cursor) {
            let (message, redraw_request, event_status) = action.into_inner();

            shell.request_redraw_at(redraw_request);

            if let Some(message) = message {
                shell.publish(message);
            }

            if event_status == iced::event::Status::Captured {
                shell.capture_event();
            }
        }

        if shell.redraw_request() != window::RedrawRequest::NextFrame {
            let mouse_interaction =
                self.mouse_interaction(tree, layout, cursor, viewport, renderer);

            if is_redraw_request {
                self.last_mouse_interaction = Some(mouse_interaction);
            } else if self
                .last_mouse_interaction
                .is_some_and(|last| last != mouse_interaction)
            {
                shell.request_redraw();
            }
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let state = tree.state.downcast_ref::<CanvasState>();
        self.program
            .mouse_interaction(state, layout.bounds(), cursor)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        if bounds.width < 1.0 || bounds.height < 1.0 {
            return;
        }
        let state = tree.state.downcast_ref::<CanvasState>();
        let frame = self
            .program
            .draw_surface(state, renderer, bounds, cursor, true);
        draw_layers(renderer, bounds, frame);
    }
}

/// Draw `frame` in one clipped layer: the content layer under its translation,
/// every other layer as it is. One layer for all of them keeps their order
/// against each other (within a renderer layer, text draws above every mesh).
fn draw_layers(renderer: &mut Renderer, bounds: Rectangle, frame: SurfaceLayers) {
    let SurfaceLayers {
        layers,
        content_shift,
    } = frame;
    renderer.with_layer(bounds, |renderer| {
        renderer.with_translation(Vector::new(bounds.x, bounds.y), |renderer| {
            for (index, layer) in layers.into_iter().enumerate() {
                match content_shift {
                    Some((content, shift)) if content == index => {
                        renderer.with_translation(shift, |renderer| renderer.draw_geometry(layer));
                    }
                    _ => renderer.draw_geometry(layer),
                }
            }
        });
    });
}
