//! The board 3D view's GPU mount (#2390): the board's 3D scene through
//! iced's `shader` widget and `signex_gfx`'s mesh pipeline, and the pointer
//! gestures over it (left drag turns, right or middle drag or Shift left
//! drag slides, the wheel zooms, a left click picks).
//!
//! The program holds what one frame needs: the scene, its generation (the
//! pipeline uploads only a new one), the camera and the parts to wash in
//! the selection colour. The camera lives in the app's `Board3dSession`;
//! gestures go back as messages, never mutate it here.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: iced/wgpu public docs.

use std::ops::Range;
use std::sync::Arc;

use iced::widget::shader::{self, Viewport};
use iced::{Event, Point, Rectangle, keyboard, mouse};
use signex_gfx::camera3d::OrbitCamera;
use signex_gfx::pipeline::mesh::{MeshPipeline, PixelRect};
use signex_renderer::board3d::BoardScene3d;

// iced's own wgpu re-export, the one the `Primitive` signatures use.
use iced::wgpu;

use crate::app::{Board3dGesture, Board3dMsg, Message, PcbMsg};

/// How far, in logical pixels, a press may move and still be a click.
const CLICK_SLOP_PX: f32 = 3.0;
/// Pixels of a smooth-scroll wheel that make one zoom step.
const WHEEL_PX_PER_STEP: f32 = 50.0;

/// One frame of the board 3D view.
pub struct Board3dProgram {
    pub scene: Arc<BoardScene3d>,
    pub generation: u64,
    pub camera: OrbitCamera,
    /// The selected parts' triangles.
    pub highlights: Vec<Range<u32>>,
    /// The selection wash.
    pub highlight: [f32; 4],
}

/// A press in progress over the view.
#[derive(Debug, Clone, Copy)]
struct Drag {
    button: mouse::Button,
    from: Point,
    last: Point,
    moved: bool,
}

/// The pointer state of the view's widget: the press in progress and
/// whether Shift is held (mouse events carry no modifiers).
#[derive(Debug, Default)]
pub struct Board3dPointer {
    drag: Option<Drag>,
    shift: bool,
}

impl Board3dProgram {
    fn publish(bounds: Rectangle, gesture: Board3dGesture) -> shader::Action<Message> {
        shader::Action::publish(Message::Pcb(PcbMsg::Board3d(Board3dMsg::Pointer {
            viewport: [bounds.width, bounds.height],
            gesture,
        })))
        .and_capture()
    }
}

impl shader::Program<Message> for Board3dProgram {
    type State = Board3dPointer;
    type Primitive = Board3dPrimitive;

    fn update(
        &self,
        state: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<shader::Action<Message>> {
        match event {
            Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                state.shift = modifiers.shift();
                None
            }
            Event::Mouse(mouse::Event::ButtonPressed(button)) => {
                if !matches!(
                    button,
                    mouse::Button::Left | mouse::Button::Right | mouse::Button::Middle
                ) {
                    return None;
                }
                let at = cursor.position_in(bounds)?;
                state.drag = Some(Drag {
                    button: *button,
                    from: at,
                    last: at,
                    moved: false,
                });
                Some(shader::Action::capture())
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let at = cursor.position_from(bounds.position())?;
                let drag = state.drag.as_mut()?;
                if !drag.moved && at.distance(drag.from) < CLICK_SLOP_PX {
                    return None;
                }
                drag.moved = true;
                let (dx, dy) = (at.x - drag.last.x, at.y - drag.last.y);
                drag.last = at;
                let gesture = match drag.button {
                    mouse::Button::Left if !state.shift => Board3dGesture::Orbited { dx, dy },
                    _ => Board3dGesture::Panned { dx, dy },
                };
                Some(Self::publish(bounds, gesture))
            }
            Event::Mouse(mouse::Event::ButtonReleased(button)) => {
                let drag = state.drag.filter(|drag| drag.button == *button)?;
                state.drag = None;
                if drag.button == mouse::Button::Left && !drag.moved {
                    let at = [drag.from.x, drag.from.y];
                    return Some(Self::publish(bounds, Board3dGesture::Clicked(at)));
                }
                Some(shader::Action::capture())
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                cursor.position_in(bounds)?;
                let steps = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => *y,
                    mouse::ScrollDelta::Pixels { y, .. } => *y / WHEEL_PX_PER_STEP,
                };
                Some(Self::publish(bounds, Board3dGesture::Zoomed(steps)))
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        _state: &Self::State,
        _cursor: mouse::Cursor,
        _bounds: Rectangle,
    ) -> Self::Primitive {
        Board3dPrimitive {
            scene: Arc::clone(&self.scene),
            generation: self.generation,
            camera: self.camera,
            highlights: self.highlights.clone(),
            highlight: self.highlight,
        }
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        match state.drag {
            Some(drag) if drag.moved => mouse::Interaction::Grabbing,
            _ if cursor.is_over(bounds) => mouse::Interaction::Grab,
            _ => mouse::Interaction::default(),
        }
    }
}

/// What one frame hands the GPU.
#[derive(Debug)]
pub struct Board3dPrimitive {
    scene: Arc<BoardScene3d>,
    generation: u64,
    camera: OrbitCamera,
    highlights: Vec<Range<u32>>,
    highlight: [f32; 4],
}

/// The mesh pipeline and where this frame's view sits on the target.
pub struct Board3dPipeline {
    mesh: MeshPipeline,
    area: PixelRect,
}

impl shader::Pipeline for Board3dPipeline {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        Self {
            mesh: MeshPipeline::new(device, format),
            area: PixelRect {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            },
        }
    }
}

impl shader::Primitive for Board3dPrimitive {
    type Pipeline = Board3dPipeline;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bounds: &Rectangle,
        viewport: &Viewport,
    ) {
        // iced renders in physical pixels; the camera works in the view's
        // logical shape, which is the same aspect.
        let dpi = viewport.scale_factor();
        pipeline.area = PixelRect {
            x: bounds.x * dpi,
            y: bounds.y * dpi,
            width: bounds.width * dpi,
            height: bounds.height * dpi,
        };
        pipeline
            .mesh
            .upload(device, queue, &self.scene.scene, self.generation);
        let uniform = self
            .camera
            .uniform(bounds.width, bounds.height.max(1.0), self.highlight);
        pipeline.mesh.set_uniform(queue, &uniform);
        let size = viewport.physical_size();
        pipeline
            .mesh
            .prepare_depth(device, [size.width, size.height]);
    }

    fn render(
        &self,
        pipeline: &Self::Pipeline,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clip_bounds: &Rectangle<u32>,
    ) {
        if pipeline.area.width <= 0.0 || pipeline.area.height <= 0.0 {
            return;
        }
        let clip = [
            clip_bounds.x,
            clip_bounds.y,
            clip_bounds.width,
            clip_bounds.height,
        ];
        pipeline
            .mesh
            .render(encoder, target, pipeline.area, clip, &self.highlights);
    }
}
