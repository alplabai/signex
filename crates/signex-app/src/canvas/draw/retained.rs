//! The content layer and the background, retained across pans (#2078, #2102).
//!
//! The geometry is tessellated once, at the camera it was built at, over a
//! clip rectangle larger than the viewport. A pan then moves it by a
//! translation (see `canvas::surface`) instead of tessellating again. Rules
//! in `docs/RENDERING_RULES.md`, "sch-pan-reuse".

use super::super::*;
use crate::schematic_runtime::{ScreenTransform, pan_reach};

/// Tolerance, in device pixels, for a pan to count as a whole number of them.
/// Widened to the f32 resolution of the offsets when they are large (a sheet
/// zoomed far in), where 1e-3 is below one step of the camera.
const WHOLE_PX_TOLERANCE: f64 = 1e-3;

/// What the retained geometry was built from.
#[derive(Clone, Copy)]
struct BuiltAt {
    offset: (f32, f32),
    scale: f32,
    device_px_per_logical_px: f32,
    viewport: iced::Size,
    /// Generation of the scene it replays; a new scene is new geometry.
    scene_generation: Option<u64>,
}

#[derive(Default)]
pub(in crate::canvas) struct RetainedContent {
    cache: canvas::Cache,
    built: Option<BuiltAt>,
    /// Test-only: how often the geometry was tessellated.
    #[cfg(test)]
    rebuilds: std::cell::Cell<u32>,
}

impl RetainedContent {
    /// Drop the geometry; the next draw tessellates.
    pub(in crate::canvas) fn clear(&mut self) {
        self.cache.clear();
        self.built = None;
    }

    #[cfg(test)]
    pub(in crate::canvas) fn rebuilds(&self) -> u32 {
        self.rebuilds.get()
    }

    /// The translation that carries the retained geometry to `live`, `None`
    /// when it cannot be reused: another zoom, display or viewport, a pan
    /// past the clip rectangle, or one that is not a whole number of device
    /// pixels (the hairline snap works on that grid and only commutes with
    /// whole-pixel shifts).
    fn shift_to(&self, live: &ScreenTransform, viewport: iced::Size) -> Option<iced::Vector> {
        let built = self.built?;
        let dpr = live.device_px_per_logical_px;
        if (built.scale - live.scale).abs() >= 0.0001
            || built.device_px_per_logical_px != dpr
            || built.viewport != viewport
        {
            return None;
        }
        let reach = clip_margin(viewport, dpr);
        let axis = |from: f32, to: f32, limit: f32| {
            // Exact: the difference of two f32 fits an f64.
            let moved = (f64::from(to) - f64::from(from)) * f64::from(dpr);
            let whole = moved.round();
            let step = f64::from(f32::EPSILON) * f64::from(to.abs().max(from.abs()));
            let tolerance = WHOLE_PX_TOLERANCE.max(2.0 * step * f64::from(dpr));
            let shift = (whole / f64::from(dpr)) as f32;
            ((moved - whole).abs() < tolerance && shift.abs() <= limit).then_some(shift)
        };
        Some(iced::Vector::new(
            axis(built.offset.0, live.offset_x, reach.x)?,
            axis(built.offset.1, live.offset_y, reach.y)?,
        ))
    }
}

/// How far the clip rectangle reaches past the viewport on each side: the
/// pan reach cut down to whole device pixels.
fn clip_margin(viewport: iced::Size, device_px_per_logical_px: f32) -> iced::Vector {
    let reach = pan_reach(viewport);
    let whole = |v: f32| (v * device_px_per_logical_px).floor() / device_px_per_logical_px;
    iced::Vector::new(whole(reach.x), whole(reach.y))
}

fn clip_rect(viewport: iced::Size, device_px_per_logical_px: f32) -> Rectangle {
    let margin = clip_margin(viewport, device_px_per_logical_px);
    Rectangle::new(
        iced::Point::new(-margin.x, -margin.y),
        iced::Size::new(
            viewport.width + 2.0 * margin.x,
            viewport.height + 2.0 * margin.y,
        ),
    )
}

/// The retained layer and the translation to draw it under.
pub(in crate::canvas) struct RetainedLayer {
    pub(in crate::canvas) geometry: canvas::Geometry,
    pub(in crate::canvas) shift: iced::Vector,
}

impl SchematicCanvas<'_> {
    /// The content geometry for `transform`. With `allow_shift` it may be one
    /// built at an earlier offset, to be drawn translated by `shift`; without
    /// it, `shift` is always zero.
    pub(in crate::canvas) fn retained_content(
        &self,
        renderer: &Renderer,
        bounds: Rectangle,
        snapshot: Option<&crate::schematic_runtime::SchematicRenderSnapshot>,
        focus_set: Option<&std::collections::HashSet<uuid::Uuid>>,
        transform: &ScreenTransform,
        allow_shift: bool,
    ) -> RetainedLayer {
        let viewport = bounds.size();
        // A GPU-mounted sheet replays only its rotated and oversize texts here.
        let mut retained = match self.mode {
            ContentMode::Cpu => self.content_surface.borrow_mut(),
            ContentMode::GpuOverlay => self.gpu_text_surface.borrow_mut(),
        };
        let mut shift = retained
            .shift_to(transform, viewport)
            .filter(|s| allow_shift || (s.x == 0.0 && s.y == 0.0));
        // A moved camera needs the scene that covers it; if the scene cache
        // had to build another, the geometry is of the old one.
        if shift.is_some_and(|s| s.x != 0.0 || s.y != 0.0) {
            let generation = snapshot.and_then(|snapshot| {
                self.content_scene.borrow_mut().generation_for(
                    snapshot,
                    transform,
                    &self.prefs.canvas_colors,
                    bounds,
                    focus_set,
                    self.scene_overrides(),
                )
            });
            if retained.built.map(|b| b.scene_generation) != Some(generation) {
                shift = None;
            }
        }
        let rebuild = shift.is_none();
        if rebuild {
            retained.cache.clear();
        }
        let clip = clip_rect(viewport, transform.device_px_per_logical_px);
        let geometry = retained.cache.draw_with_bounds(renderer, clip, |frame| {
            #[cfg(test)]
            retained.rebuilds.set(retained.rebuilds.get() + 1);
            if let Some(snapshot) = snapshot {
                self.render_content_scene(
                    frame,
                    clip.position(),
                    snapshot,
                    transform,
                    bounds,
                    focus_set,
                );
            }
        });
        if rebuild {
            retained.built = Some(BuiltAt {
                offset: (transform.offset_x, transform.offset_y),
                scale: transform.scale,
                device_px_per_logical_px: transform.device_px_per_logical_px,
                viewport,
                scene_generation: self.content_scene.borrow().generation(),
            });
        }
        RetainedLayer {
            geometry,
            shift: shift.unwrap_or(iced::Vector::ZERO),
        }
    }
}

/// What the background is drawn from besides the camera. A change rebuilds
/// the layer without any `clear` call.
#[derive(Clone, PartialEq)]
struct BackgroundInputs {
    look: super::super::paper::PaperLook,
    paper_mm: (f32, f32),
    frame_color: Color,
    sheet_frame: Vec<signex_output::template::FramePrimitive>,
}

/// The sheet background (backdrop, paper, grid, frame), retained like the
/// content: a grid of tens of thousands of dots costs milliseconds to
/// tessellate, and a pan only moves it.
#[derive(Default)]
pub(in crate::canvas) struct RetainedBackground {
    layer: RetainedContent,
    inputs: Option<BackgroundInputs>,
}

impl SchematicCanvas<'_> {
    /// The background for the live camera, as a layer and the translation to
    /// draw it under. It is painted into a frame whose origin is the corner
    /// of the clip rectangle, at the camera moved by that corner.
    pub(in crate::canvas) fn retained_background(
        &self,
        renderer: &Renderer,
        bounds: Rectangle,
    ) -> RetainedLayer {
        let viewport = bounds.size();
        let live = {
            let cam = self.camera();
            ScreenTransform {
                offset_x: cam.offset.x,
                offset_y: cam.offset.y,
                scale: cam.scale,
                device_px_per_logical_px: self.prefs.device_px_per_logical_px,
            }
        };
        let inputs = BackgroundInputs {
            look: self.paper_look(),
            paper_mm: (self.paper_width_mm, self.paper_height_mm),
            frame_color: crate::render_config::to_iced(&self.prefs.canvas_colors.body),
            sheet_frame: Vec::new(),
        };
        let mut retained = self.background_surface.borrow_mut();
        // The frame list is the one input worth comparing before cloning.
        let changed = retained.inputs.as_ref().is_none_or(|old| {
            old.look != inputs.look
                || old.paper_mm != inputs.paper_mm
                || old.frame_color != inputs.frame_color
                || old.sheet_frame != self.sheet_frame
        });
        if changed {
            retained.layer.clear();
            retained.inputs = Some(BackgroundInputs {
                sheet_frame: self.sheet_frame.clone(),
                ..inputs
            });
        }
        let shift = retained.layer.shift_to(&live, viewport);
        let rebuild = shift.is_none();
        if rebuild {
            retained.layer.cache.clear();
        }
        let clip = clip_rect(viewport, live.device_px_per_logical_px);
        let geometry = retained
            .layer
            .cache
            .draw_with_bounds(renderer, clip, |frame| {
                #[cfg(test)]
                self.bg_tessellations.set(self.bg_tessellations.get() + 1);
                let camera = Camera {
                    offset: iced::Point::new(live.offset_x - clip.x, live.offset_y - clip.y),
                    scale: live.scale,
                };
                frame.translate(iced::Vector::new(clip.x, clip.y));
                self.paint_background_at(frame, &camera, Rectangle::with_size(clip.size()));
            });
        if rebuild {
            retained.layer.built = Some(BuiltAt {
                offset: (live.offset_x, live.offset_y),
                scale: live.scale,
                device_px_per_logical_px: live.device_px_per_logical_px,
                viewport,
                scene_generation: None,
            });
        }
        RetainedLayer {
            geometry,
            shift: shift.unwrap_or(iced::Vector::ZERO),
        }
    }
}
