//! Footprint preview over a read-only [`PreviewFootprint`]: the footprint
//! editor's backdrop, silk, fab and courtyard in the board palette, the pads
//! as the PCB canvas draws them (copper and drills, #2348), pad numbers, a
//! size readout and a scale bar, with its own zoom/pan camera. Fitted to the
//! box on first show, and again when the box changes size while the user has
//! not zoomed or panned. Shared by the Components panel and the Component Preview
//! dialog. Left-drag pans; Ctrl/Cmd+wheel zooms (a plain wheel scrolls the
//! panel around it). The Components panel card wraps it in [`viewed_footprint`],
//! whose view lives in the model and follows the Mouse wheel preference.

use iced::widget::canvas::{self, Action, Cache, Geometry};
use iced::{Color, Point, Rectangle, Renderer, Theme, Vector, Widget, mouse};

use super::preview_view::{GestureInput, PreviewGesture, PreviewView};
use crate::canvas::{Camera, WheelMode};
use crate::library::editor::footprint::canvas::{
    BACKDROP_BG, BACKDROP_GRID, BackdropLook, FIT_PAD, FootprintCanvasState, PreviewFootprint,
    draw_back_layers, draw_backdrop, draw_courtyard, draw_fab, draw_origin, draw_pad_bodies,
    draw_pad_numbers, draw_silk, graphite_look_for,
};
use crate::library::editor::footprint::layers::LayerPalette;
use crate::library::editor::footprint::state::SnapOptions;

mod footprint_scale;

/// Lowest zoom of the preview, px per mm: low enough to fit a large part
/// into the small details box.
const MIN_PREVIEW_SCALE: f32 = 0.5;

pub struct FootprintPreview<'a> {
    model: &'a PreviewFootprint,
    /// Per-wheel-notch zoom factor.
    zoom_step: f32,
    /// Board ground colour and the board palette; `None` until
    /// [`Self::with_theme_tokens`] sets them, which paints the black board.
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

    /// Paint on the board ground with the board palette, as the PCB canvas
    /// does in every theme.
    pub fn with_theme_tokens(mut self, tokens: &signex_types::theme::ThemeTokens) -> Self {
        self.graphite = Some((graphite_look_for(tokens).bg, LayerPalette::Board));
        self
    }

    /// The Graphite ground colour, `None` before the theme is applied.
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
    /// Ctrl/Cmd state from the last modifiers event; the wheel zooms with it.
    ctrl_held: bool,
    cache: Cache,
    /// Ground colour the cached drawing was built with; another colour
    /// (a theme switch) drops the cache.
    shown_bg: std::cell::Cell<Option<Color>>,
    /// The view the cached drawing was built through.
    shown_view: std::cell::Cell<PreviewView>,
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

    /// Fit the camera to `model` in a box of `size` unless it already is.
    fn refit(&mut self, model: &PreviewFootprint, size: iced::Size) {
        if size.width <= 0.0 || size.height <= 0.0 || self.is_fitted_for(model, size) {
            return;
        }
        self.camera = fitted_camera(model, Rectangle::new(Point::ORIGIN, size));
        self.fitted_for = Some(model.key());
        self.fitted_size = Some(size);
        self.user_moved = false;
        self.pan_from = None;
        self.cache.clear();
    }
}

/// Replay options of the board pad scene at `scale` px per mm. The pads are
/// fills with no stroke or text, so neither the display scale nor a text
/// clamp changes them.
fn board_pad_options(scale: f32) -> crate::renderer_scene_canvas::SceneDrawOptions {
    let text = signex_gfx::primitive::text::TextSizePolicy::UNCLAMPED;
    crate::renderer_scene_canvas::SceneDrawOptions {
        scale_px_per_mm: scale,
        device_px_per_logical_px: 1.0,
        text_min_px: text.min_px,
        text_max_px: text.max_px,
        frame_origin: Point::ORIGIN,
    }
}

/// A camera fitted to the model's bbox in `viewport`, or the origin
/// centred for an empty footprint.
pub(crate) fn fitted_camera(model: &PreviewFootprint, viewport: Rectangle) -> FootprintCanvasState {
    let mut camera = FootprintCanvasState::default();
    match model.bbox_mm() {
        Some(bbox) => camera.fit_to_bounds_min(
            bbox,
            footprint_scale::drawing_area(viewport, bbox, FIT_PAD),
            MIN_PREVIEW_SCALE,
        ),
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
        state.refit(self.model, bounds.size());
        if let iced::Event::Keyboard(iced::keyboard::Event::ModifiersChanged(mods)) = event {
            state.ctrl_held = mods.command();
            return None;
        }
        let iced::Event::Mouse(event) = event else {
            return None;
        };
        match event {
            mouse::Event::WheelScrolled { delta } => {
                // Only Ctrl/Cmd+wheel zooms; a plain wheel scrolls the panel.
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
        self.paint(state, renderer, bounds, PreviewView::default())
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

impl FootprintPreview<'_> {
    /// Draw through `view`, on top of the camera in `state`.
    fn paint(
        &self,
        state: &PreviewState,
        renderer: &Renderer,
        bounds: Rectangle,
        view: PreviewView,
    ) -> Vec<Geometry> {
        // Until `update` has fitted this footprint, draw the fitted view
        // without caching it against the stale camera.
        let fitted = state.is_fitted_for(self.model, bounds.size());
        if !fitted {
            state.cache.clear();
        }
        let base = if fitted {
            FootprintCanvasState::with_view(state.camera.scale, state.camera.offset)
        } else {
            fitted_camera(self.model, Rectangle::new(Point::ORIGIN, bounds.size()))
        };
        let (scale, offset) = view.place(base.scale, base.offset);
        let camera = FootprintCanvasState::with_view(scale, offset);
        if state.shown_view.replace(view) != view {
            state.cache.clear();
        }
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
                grid_origin: None,
            };
            draw_backdrop(frame, &camera, area, look);
            // The legacy yellow crosshair does not read on the paper ground.
            if self.graphite.is_none() {
                draw_origin(frame, &camera);
            }
            let mut layers = self.model.layer_input();
            if let Some((_, palette)) = self.graphite {
                layers.palette = palette;
            }
            draw_back_layers(
                frame,
                &camera,
                self.model.silk_b(),
                self.model.fab_b(),
                layers.palette,
            );
            draw_fab(frame, &camera, self.model.fab_f(), layers.palette);
            draw_silk(frame, &camera, &layers);
            draw_courtyard(frame, &camera, &layers);
            crate::renderer_scene_canvas::draw_scene_with_world_to_screen(
                frame,
                self.model.board_pads(),
                |p| camera.world_to_screen((f64::from(p[0]), f64::from(p[1]))),
                board_pad_options(camera.scale),
            );
            let mut unheld = layers;
            unheld.pads = self.model.unheld_pads();
            draw_pad_bodies(frame, &camera, &unheld);
            // Numbers last so no pad covers a neighbour's number.
            draw_pad_numbers(frame, &camera, &layers);
            footprint_scale::draw_readouts(frame, area, camera.scale, self.model.bbox_mm());
        });
        vec![geometry]
    }
}

/// The preview with its view held in the model: gestures are published, the
/// widget keeps only the fit, the cache and the pointer.
struct ViewedFootprint<'a> {
    preview: FootprintPreview<'a>,
    view: PreviewView,
    wheel_mode: WheelMode,
}

#[derive(Default)]
struct ViewedState {
    inner: PreviewState,
    input: GestureInput,
}

impl canvas::Program<PreviewGesture> for ViewedFootprint<'_> {
    type State = ViewedState;

    fn update(
        &self,
        state: &mut ViewedState,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<PreviewGesture>> {
        state.inner.refit(self.preview.model, bounds.size());
        state.input.read(event, bounds, cursor, self.wheel_mode)
    }

    fn draw(
        &self,
        state: &ViewedState,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        self.preview
            .paint(&state.inner, renderer, bounds, self.view)
    }

    fn mouse_interaction(
        &self,
        state: &ViewedState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        state.input.interaction(bounds, cursor)
    }
}

/// The footprint preview of the details card, `height` tall, through `view`.
pub(super) fn viewed_footprint<'a>(
    model: &'a PreviewFootprint,
    tokens: &signex_types::theme::ThemeTokens,
    view: PreviewView,
    wheel_mode: WheelMode,
    height: f32,
) -> iced::Element<'a, PreviewGesture> {
    iced::widget::canvas(ViewedFootprint {
        preview: FootprintPreview::new(model).with_theme_tokens(tokens),
        view,
        wheel_mode,
    })
    .width(iced::Length::Fill)
    .height(height)
    .boxed()
}

#[cfg(test)]
#[path = "footprint_canvas_tests.rs"]
mod tests;
