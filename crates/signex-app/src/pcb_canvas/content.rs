//! The board content of the CPU mount, a layer of its own between the
//! background canvas and the highlight layer, where the GPU mount has its
//! shader (#2321). A widget rather than a `canvas`, so a pan draws the
//! retained geometry under a translation instead of tessellating the board
//! again, as the schematic canvas does (#2078). Rules:
//! `docs/RENDERING_RULES.md`, "pcb-cpu-content".

use iced::advanced::graphics::cache::Cached;
use iced::advanced::graphics::geometry::Renderer as _;
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer::Renderer as _;
use iced::advanced::widget::{self, Tree};
use iced::advanced::{Widget, renderer};
use iced::widget::canvas;
use iced::{Element, Length, Point, Rectangle, Renderer, Size, Theme, Vector, mouse};

use super::cull::{CULL_MARGIN_PX, WorldRect};
use super::tessellate::{available_workers, tessellate, visible_items};
use super::{BoardCamera, PcbCanvas};
use crate::app::Message;
use crate::canvas::pan_reuse::{clip_rect, whole_pixel_shift};

/// What the retained geometry was built from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct BuiltAt {
    pub(super) offset: (f32, f32),
    pub(super) scale: f32,
    pub(super) degrees: f32,
    pub(super) device_px_per_logical_px: f32,
    pub(super) viewport: Size,
    pub(super) scene_generation: u64,
}

impl BuiltAt {
    fn live(canvas: &PcbCanvas, camera: &BoardCamera, viewport: Size, dpr: f32) -> Self {
        Self {
            offset: (camera.offset().x, camera.offset().y),
            scale: camera.scale(),
            degrees: camera.rotation().degrees(),
            device_px_per_logical_px: dpr,
            viewport,
            scene_generation: canvas.scene_generation.get(),
        }
    }

    /// The same scene, zoom, turn, display and viewport: only a pan apart.
    fn same_view(&self, live: &Self) -> bool {
        (self.scale - live.scale).abs() < 0.0001
            && self.degrees == live.degrees
            && self.device_px_per_logical_px == live.device_px_per_logical_px
            && self.viewport == live.viewport
            && self.scene_generation == live.scene_generation
    }
}

/// The tessellated board, kept across frames.
pub(super) struct RetainedBoard {
    pub(super) built: BuiltAt,
    /// The screen rectangle, at the build camera, whose primitives are all
    /// in the geometry: the viewport, the whole clip rectangle after a pan
    /// build, or the clip rectangle when nothing was culled.
    pub(super) cover: Rectangle,
    pub(super) layers: Vec<RetainedLayer>,
}

/// One run's geometry, kept with its GPU buffers.
pub(super) type RetainedLayer = <canvas::Geometry as Cached>::Cache;

/// The geometry to draw for `layers`; a refcount bump each, no tessellation.
fn loaded(layers: &[RetainedLayer]) -> Vec<canvas::Geometry> {
    layers
        .iter()
        .map(<canvas::Geometry as Cached>::load)
        .collect()
}

/// What a frame does with the retained board.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Plan {
    /// Draw it moved by the shift.
    Reuse(Vector),
    /// Tessellate again; `reach` culls to the whole clip rectangle, so the
    /// pan that asked for it can go on reusing the result.
    Build { reach: bool },
}

pub(super) fn plan(retained: Option<&RetainedBoard>, live: &BuiltAt) -> Plan {
    let tight = Plan::Build { reach: false };
    let Some(retained) = retained.filter(|r| r.built.same_view(live)) else {
        return tight;
    };
    let unlimited = Vector::new(f32::INFINITY, f32::INFINITY);
    // A pan off the device pixel grid tessellates, as it always did.
    let Some(shift) = whole_pixel_shift(
        retained.built.offset,
        live.offset,
        live.device_px_per_logical_px,
        unlimited,
    ) else {
        return tight;
    };
    // The live viewport, in the pixels of the build camera.
    let seen = Rectangle::new(Point::new(-shift.x, -shift.y), live.viewport);
    if covers(retained.cover, seen) {
        Plan::Reuse(shift)
    } else {
        Plan::Build { reach: true }
    }
}

fn covers(outer: Rectangle, inner: Rectangle) -> bool {
    const SLACK: f32 = 1e-3;
    inner.x >= outer.x - SLACK
        && inner.y >= outer.y - SLACK
        && inner.x + inner.width <= outer.x + outer.width + SLACK
        && inner.y + inner.height <= outer.y + outer.height + SLACK
}

/// The board area a build keeps: primitives touching `screen` (in canvas
/// pixels) at `camera`, with the usual cull slack.
pub(super) fn cull_rect(camera: &BoardCamera, screen: Rectangle) -> Option<WorldRect> {
    let mut moved = camera.clone();
    moved.pan(-screen.x, -screen.y);
    WorldRect::visible(&moved, screen.size(), CULL_MARGIN_PX)
}

impl PcbCanvas {
    /// The board geometry for a `viewport`-sized layer and the translation
    /// to draw it under: retained across a pan, rebuilt for anything else.
    /// Empty without a board.
    pub(super) fn board_content(
        &self,
        renderer: &Renderer,
        viewport: Size,
        device_px_per_logical_px: f32,
    ) -> (Vec<canvas::Geometry>, Vector) {
        let camera = self.camera.borrow();
        let live = BuiltAt::live(self, &camera, viewport, device_px_per_logical_px);
        let mut slot = self.content.borrow_mut();
        let reach = match plan(slot.as_ref(), &live) {
            Plan::Reuse(shift) => {
                let layers = slot
                    .as_ref()
                    .map_or_else(Vec::new, |retained| loaded(&retained.layers));
                return (layers, shift);
            }
            Plan::Build { reach } => reach,
        };
        let Some(scene) = self.gpu_scene() else {
            *slot = None;
            return (Vec::new(), Vector::ZERO);
        };
        let extents = self.cull_bounds(&scene);
        let clip = clip_rect(viewport, device_px_per_logical_px);
        let wanted = if reach {
            clip
        } else {
            Rectangle::with_size(viewport)
        };
        let (items, complete) = visible_items(&scene, &extents, cull_rect(&camera, wanted));
        // Compile-time check: threads need `iced::Renderer`'s frame to be
        // `Send`. If an iced update takes that away this call stops
        // compiling; `tessellate` names the in-order fallback.
        let geometry = tessellate(renderer, clip, &items, &camera, available_workers());
        let group = canvas::Group::unique();
        let layers: Vec<RetainedLayer> = geometry
            .into_iter()
            .map(|layer| layer.cache(group, None))
            .collect();
        let drawn = loaded(&layers);
        *slot = Some(RetainedBoard {
            built: live,
            cover: if complete { clip } else { wanted },
            layers,
        });
        (drawn, Vector::ZERO)
    }
}

/// The CPU mount's board layer. No `update`: it never takes an event.
pub struct BoardContentLayer<'a> {
    canvas: &'a PcbCanvas,
    device_px_per_logical_px: f32,
}

impl<'a> BoardContentLayer<'a> {
    /// The layer of `canvas` on a display of `device_px_per_logical_px`.
    pub fn new(canvas: &'a PcbCanvas, device_px_per_logical_px: f32) -> Self {
        Self {
            canvas,
            device_px_per_logical_px,
        }
    }
}

impl widget::Meta for BoardContentLayer<'_> {}

impl Widget<Message, Theme, Renderer> for BoardContentLayer<'_> {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(&mut self, tree: &mut Tree, _renderer: &Renderer, limits: &layout::Limits) {
        tree.size = layout::atomic(limits, Length::Fill, Length::Fill);
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        if bounds.width < 1.0 || bounds.height < 1.0 {
            return;
        }
        let (layers, shift) =
            self.canvas
                .board_content(renderer, bounds.size(), self.device_px_per_logical_px);
        // One clipped layer; the geometry reaches past the viewport.
        renderer.with_layer(bounds, |renderer| {
            renderer.with_translation(
                Vector::new(bounds.x + shift.x, bounds.y + shift.y),
                |renderer| {
                    for layer in layers {
                        renderer.draw_geometry(layer);
                    }
                },
            );
        });
    }
}

impl<'a> From<BoardContentLayer<'a>> for Element<'a, Message> {
    fn from(layer: BoardContentLayer<'a>) -> Self {
        Element::new(layer)
    }
}
