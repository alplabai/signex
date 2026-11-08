//! Footprint preview: the footprint editor's own rendering (backdrop, grid,
//! origin, silk, courtyard, pads with holes) over a read-only
//! [`PreviewFootprint`], with its own zoom/pan camera. Fitted to the box on
//! first show, and again when the box changes size while the user has not
//! zoomed or panned. Shared by the Components panel and the Component Preview
//! dialog. Left-drag pans; Ctrl+wheel zooms (a plain wheel scrolls the
//! panel around it).

use iced::widget::canvas::{self, Action, Cache, Geometry};
use iced::{Color, Point, Rectangle, Renderer, Theme, Vector, mouse};

use crate::canvas::Camera;
use crate::library::editor::footprint::canvas::{
    BACKDROP_BG, BACKDROP_GRID, BackdropLook, FootprintCanvasState, PreviewFootprint,
    draw_backdrop, draw_courtyard, draw_fab, draw_origin, draw_pads, draw_silk, graphite_look_for,
};
use crate::library::editor::footprint::layers::LayerPalette;
use crate::library::editor::footprint::state::SnapOptions;

/// Lowest zoom of the preview, px per mm: low enough to fit a large part
/// into the small details box.
const MIN_PREVIEW_SCALE: f32 = 0.5;

pub struct FootprintPreview<'a> {
    model: &'a PreviewFootprint,
    /// Per-wheel-notch zoom factor.
    zoom_step: f32,
    /// Graphite ground colour and layer palette; `None` keeps the legacy
    /// black board.
    graphite: Option<(iced::Color, LayerPalette)>,
}

impl<'a> FootprintPreview<'a> {
    pub fn new(model: &'a PreviewFootprint) -> Self {
        Self {
            model,
            zoom_step: Camera::zoom_step_factor(Camera::ZOOM_STEP_PERCENT_DEFAULT),
            graphite: None,
        }
    }

    /// Paint with the Graphite paper / charcoal canvas look and the preview
    /// palette (crisp pads, neutral courtyard) when `tokens` is a Graphite
    /// theme; any other theme keeps the legacy board.
    pub fn with_theme_tokens(mut self, tokens: &signex_types::theme::ThemeTokens) -> Self {
        self.graphite = graphite_look_for(tokens).map(|l| (l.bg, l.palette().for_preview()));
        self
    }

    /// The Graphite ground colour, `None` on a legacy theme.
    #[cfg(test)]
    pub(crate) fn ground(&self) -> Option<Color> {
        self.graphite.map(|(bg, _)| bg)
    }
}

/// Zoom `camera` about `anchor` for one pinch event, with the shared pinch
/// factor and the preview's own scale floor. False when nothing changed.
fn pinch_zoom(camera: &mut FootprintCanvasState, anchor: Point, delta: f32) -> bool {
    let factor = Camera::pinch_factor(delta);
    if factor == 1.0 {
        return false;
    }
    let before = camera.scale;
    camera.zoom_about(anchor, factor, MIN_PREVIEW_SCALE);
    camera.scale != before
}

/// The preview's own camera and geometry cache (widget state).
#[derive(Default)]
pub struct PreviewState {
    camera: FootprintCanvasState,
    /// The footprint the camera was fitted for.
    fitted_for: Option<(uuid::Uuid, i64)>,
    /// The box size the camera was fitted for; another size refits.
    fitted_size: Option<iced::Size>,
    /// The user zoomed or panned since the last fit: a resize keeps the view.
    user_moved: bool,
    /// Last cursor position while a drag-pan is running.
    pan_from: Option<Point>,
    /// Ctrl state from the last modifiers event; the wheel zooms with it.
    ctrl_held: bool,
    cache: Cache,
    /// Ground colour the cached drawing was built with; another colour
    /// (a theme switch) drops the cache.
    shown_bg: std::cell::Cell<Option<Color>>,
}

impl PreviewState {
    /// Current view scale, px per mm.
    #[cfg(test)]
    pub fn scale(&self) -> f32 {
        self.camera.scale
    }

    /// Current view offset, px.
    #[cfg(test)]
    pub fn offset(&self) -> Point {
        self.camera.offset
    }

    fn is_fitted_for(&self, model: &PreviewFootprint, size: iced::Size) -> bool {
        self.fitted_for == Some(model.key())
            && (self.user_moved || self.fitted_size == Some(size))
            && self.camera.did_initial_fit
    }
}

/// A camera fitted to the model's bbox in `viewport`, or the origin
/// centred for an empty footprint.
pub(crate) fn fitted_camera(model: &PreviewFootprint, viewport: Rectangle) -> FootprintCanvasState {
    let mut camera = FootprintCanvasState::default();
    match model.bbox_mm() {
        Some(bbox) => camera.fit_to_bounds_min(bbox, viewport, MIN_PREVIEW_SCALE),
        None => camera.offset = Point::new(viewport.width / 2.0, viewport.height / 2.0),
    }
    camera.did_initial_fit = true;
    camera
}

impl<M> canvas::Program<M> for FootprintPreview<'_> {
    type State = PreviewState;

    fn update(
        &self,
        state: &mut PreviewState,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<M>> {
        let size_ok = bounds.width > 0.0 && bounds.height > 0.0;
        if size_ok && !state.is_fitted_for(self.model, bounds.size()) {
            state.camera = fitted_camera(self.model, Rectangle::new(Point::ORIGIN, bounds.size()));
            state.fitted_for = Some(self.model.key());
            state.fitted_size = Some(bounds.size());
            state.user_moved = false;
            state.pan_from = None;
            state.cache.clear();
        }
        if let iced::Event::Keyboard(iced::keyboard::Event::ModifiersChanged(mods)) = event {
            state.ctrl_held = mods.control();
            return None;
        }
        let iced::Event::Mouse(event) = event else {
            return None;
        };
        match event {
            mouse::Event::WheelScrolled { delta } => {
                // Only Ctrl+wheel zooms; a plain wheel scrolls the panel.
                if !state.ctrl_held {
                    return None;
                }
                let anchor = cursor.position_in(bounds)?;
                let lines = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => *y,
                    mouse::ScrollDelta::Pixels { y, .. } => *y / 50.0,
                };
                let factor = Camera::zoom_factor_for_scroll(self.zoom_step, lines);
                if lines == 0.0 || !factor.is_finite() || factor == 1.0 {
                    return None;
                }
                state.camera.zoom_about(anchor, factor, MIN_PREVIEW_SCALE);
                state.user_moved = true;
                state.cache.clear();
                Some(Action::request_redraw().and_capture())
            }
            // Previews keep wheel and pinch zoom only: the opt-in two-finger
            // pan is an editor-canvas preference and is not offered here.
            mouse::Event::PinchGesture { delta } => {
                let anchor = cursor.position_in(bounds)?;
                if !pinch_zoom(&mut state.camera, anchor, *delta) {
                    return None;
                }
                state.user_moved = true;
                state.cache.clear();
                Some(Action::request_redraw().and_capture())
            }
            mouse::Event::ButtonPressed(mouse::Button::Left) => {
                cursor.position_in(bounds)?;
                state.pan_from = cursor.position();
                Some(Action::capture())
            }
            mouse::Event::CursorMoved { .. } => {
                let last = state.pan_from?;
                let position = super::symbol_canvas::drag_position(cursor)?;
                state
                    .camera
                    .pan_by(Vector::new(position.x - last.x, position.y - last.y));
                state.pan_from = Some(position);
                state.user_moved = true;
                state.cache.clear();
                Some(Action::request_redraw().and_capture())
            }
            mouse::Event::ButtonReleased(_) => {
                state.pan_from.take()?;
                Some(Action::capture())
            }
            mouse::Event::CursorLeft => {
                state.pan_from = None;
                None
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        state: &PreviewState,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        // Until `update` has fitted this footprint, draw the fitted view
        // without caching it against the stale camera.
        let fitted = state.is_fitted_for(self.model, bounds.size());
        if !fitted {
            state.cache.clear();
        }
        let camera = if fitted {
            FootprintCanvasState::with_view(state.camera.scale, state.camera.offset)
        } else {
            fitted_camera(self.model, Rectangle::new(Point::ORIGIN, bounds.size()))
        };
        let snap = SnapOptions::default();
        let bg = self.graphite.map_or(BACKDROP_BG, |(bg, _)| bg);
        if state.shown_bg.replace(Some(bg)) != Some(bg) {
            state.cache.clear();
        }
        let geometry = state.cache.draw(renderer, bounds.size(), |frame| {
            let area = Rectangle::new(Point::ORIGIN, bounds.size());
            let look = BackdropLook {
                bg,
                grid: BACKDROP_GRID,
                sketch: false,
                snap: &snap,
            };
            draw_backdrop(frame, &camera, area, look);
            // The legacy yellow crosshair does not read on the paper ground.
            if self.graphite.is_none() {
                draw_origin(frame, &camera, false);
            }
            let mut layers = self.model.layer_input();
            if let Some((_, palette)) = self.graphite {
                layers.palette = palette;
            }
            draw_fab(frame, &camera, self.model.fab_f(), layers.palette);
            draw_silk(frame, &camera, &layers);
            draw_courtyard(frame, &camera, &layers);
            draw_pads(frame, &camera, &layers);
        });
        vec![geometry]
    }

    fn mouse_interaction(
        &self,
        state: &PreviewState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.pan_from.is_some() {
            mouse::Interaction::Grabbing
        } else if cursor.is_over(bounds) {
            mouse::Interaction::Grab
        } else {
            mouse::Interaction::None
        }
    }
}

#[cfg(test)]
#[path = "footprint_canvas_tests.rs"]
mod tests;
