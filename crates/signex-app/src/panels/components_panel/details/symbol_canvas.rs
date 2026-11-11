//! The Symbol preview: the symbol as the schematic canvas draws it, placed
//! once on the real paper over the real grid (no sheet border or title
//! block), through the same lowering, scene and replay. Fitted to the box on
//! first show, with its own zoom/pan camera: left-drag pans, Ctrl/Cmd+wheel zooms
//! (a plain wheel scrolls the panel around it). Shared by the Components
//! panel and the Component Preview dialog's Symbol pane.

use std::cell::RefCell;

use iced::widget::canvas::{self, Action, Geometry};
use iced::widget::container;
use iced::{Element, Length, Point, Rectangle, Size, Theme, Vector, Widget, mouse};
use signex_types::schematic::{Aabb, LibSymbol, SchematicSheet, Symbol};
use signex_types::theme::CanvasColors;

use crate::canvas::Camera;
use crate::canvas::paper::{PaperLook, paint_paper_and_grid};
use crate::render_config::{GridStyle, to_iced};
use crate::schematic_runtime::{
    SceneOverrides, ScreenTransform, build_schematic_scene, schematic_draw_options_with,
    symbol_bounds,
};

/// Share of the box the symbol's extent fills; the rest is margin.
const FILL_RATIO: f32 = 0.8;
/// Pin and field text is drawn outside the extent; the margin is at least
/// this many times the minimum text size so it stays inside the box.
const TEXT_MARGIN_EM: f32 = 2.0;
/// Default schematic text height, mm; sets the largest sheet-like zoom.
const TEXT_HEIGHT_MM: f32 = 1.27;
/// The most a small part is magnified, in text heights at the max text size.
const MAX_SCALE_TEXT_HEIGHTS: f32 = 4.0;
/// The preview is a thumbnail, so its text keeps a readability clamp the sheet no longer has.
const PREVIEW_TEXT_SIZE: signex_gfx::primitive::text::TextSizePolicy =
    signex_gfx::primitive::text::TextSizePolicy::new(6.0, 64.0);
/// The replay options of the preview: the sheet's, with the thumbnail's text
/// clamp.
fn preview_draw_options(
    transform: &ScreenTransform,
) -> crate::renderer_scene_canvas::SceneDrawOptions {
    schematic_draw_options_with(transform, PREVIEW_TEXT_SIZE)
}

/// The part is placed on a multiple of this, mm, near the paper's centre.
const PLACEMENT_GRID_MM: f64 = 2.54;

/// The user's visible grid, as the schematic editor draws it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SheetGrid {
    /// The schematic's grid toggle; off draws the paper without a grid.
    pub visible: bool,
    pub pitch_mm: f32,
    pub style: GridStyle,
}

/// What the cached drawing was built from; a change drops the cache.
type Shown = (LibSymbol, CanvasColors, SheetGrid);

#[derive(Default)]
struct PreviewState {
    camera: Camera,
    /// The symbol the camera was fitted for; an edited or different symbol
    /// has another key and refits.
    fitted_for: Option<FitKey>,
    /// The box size the camera was fitted for; another size refits.
    fitted_size: Option<Size>,
    /// The user zoomed or panned since the last fit: a resize keeps the view.
    user_moved: bool,
    /// Last cursor position while a drag-pan is running.
    pan_from: Option<Point>,
    /// Ctrl/Cmd state from the last modifiers event; the wheel zooms with it.
    ctrl_held: bool,
    cache: canvas::Cache,
    shown: RefCell<Option<Shown>>,
}

/// A symbol id plus its body extent in micrometres (min x, min y, max x,
/// max y), so a changed body refits even under an unchanged id.
type FitKey = (String, [i64; 4]);

impl PreviewState {
    fn is_fitted_for(&self, key: &FitKey, size: Size) -> bool {
        self.fitted_for.as_ref() == Some(key) && (self.user_moved || self.fitted_size == Some(size))
    }
}

struct SymbolSheetPreview {
    lib: LibSymbol,
    colors: CanvasColors,
    grid: SheetGrid,
    device_px_per_logical_px: f32,
    /// Per-wheel-notch zoom factor.
    zoom_step: f32,
}

/// Whether the cached drawing is stale for `current`; records it as shown
/// when it is. A first draw has nothing shown, so it rebuilds.
fn take_rebuild(shown: &mut Option<Shown>, current: &Shown) -> bool {
    let unchanged = shown.as_ref() == Some(current);
    if !unchanged {
        *shown = Some(current.clone());
    }
    !unchanged
}

/// A sheet holding `lib` placed once near the paper's centre, unit 1, normal
/// body.
fn preview_sheet(lib: &LibSymbol) -> (SchematicSheet, Symbol) {
    let mut sheet = SchematicSheet::default();
    let (width, height) = sheet.paper_mm();
    let on_grid = |mid: f64| (mid / PLACEMENT_GRID_MM).round() * PLACEMENT_GRID_MM;
    let symbol = Symbol {
        uuid: uuid::Uuid::nil(),
        lib_id: lib.id.clone(),
        position: signex_types::schematic::Point::new(on_grid(width / 2.0), on_grid(height / 2.0)),
        ..Symbol::default()
    };
    sheet.lib_symbols.insert(lib.id.clone(), lib.clone());
    sheet.symbols.push(symbol.clone());
    (sheet, symbol)
}

/// The margin kept free on each side of the extent along one axis, px.
fn fit_margin(side: f32) -> f32 {
    (side * (1.0 - FILL_RATIO) / 2.0).max(PREVIEW_TEXT_SIZE.min_px * TEXT_MARGIN_EM)
}

/// The largest scale, px per mm, a part is fitted to.
fn max_scale() -> f32 {
    PREVIEW_TEXT_SIZE.max_px / (TEXT_HEIGHT_MM * MAX_SCALE_TEXT_HEIGHTS)
}

/// The transform that centres `extent` in `size` and scales it to fit.
fn fit_transform(extent: Aabb, size: Size, device_px_per_logical_px: f32) -> ScreenTransform {
    let width = (extent.max_x - extent.min_x).max(f64::EPSILON) as f32;
    let height = (extent.max_y - extent.min_y).max(f64::EPSILON) as f32;
    let free_w = (size.width - 2.0 * fit_margin(size.width)).max(1.0);
    let free_h = (size.height - 2.0 * fit_margin(size.height)).max(1.0);
    let scale = (free_w / width).min(free_h / height).min(max_scale());
    let mid_x = ((extent.min_x + extent.max_x) / 2.0) as f32;
    let mid_y = ((extent.min_y + extent.max_y) / 2.0) as f32;
    ScreenTransform {
        offset_x: size.width / 2.0 - mid_x * scale,
        offset_y: size.height / 2.0 - mid_y * scale,
        scale,
        device_px_per_logical_px,
    }
}

/// Where a drag-pan's pointer is, in the same space as the press that
/// started it: the widget cursor, which a scrolled pane shifts by its scroll
/// offset, never the event's window position, which it does not. Observed,
/// so a drag over a scrollbar or a layer above keeps going.
pub(super) fn drag_position(cursor: mouse::Cursor) -> Option<Point> {
    cursor.observe().position()
}

/// The camera `transform` stands for.
fn camera_of(transform: &ScreenTransform) -> Camera {
    Camera {
        offset: Point::new(transform.offset_x, transform.offset_y),
        scale: transform.scale,
    }
}

impl SymbolSheetPreview {
    /// The camera fitting the part in a box of `size`.
    fn fitted_camera(&self, size: Size) -> Camera {
        let (sheet, symbol) = preview_sheet(&self.lib);
        camera_of(&fit_transform(
            symbol_bounds(&sheet, &symbol),
            size,
            self.device_px_per_logical_px,
        ))
    }

    /// What the fit is keyed on: the symbol id and its body extent.
    fn fit_key(&self) -> FitKey {
        let (sheet, symbol) = preview_sheet(&self.lib);
        let extent = symbol_bounds(&sheet, &symbol);
        let um = |mm: f64| (mm * 1000.0).round() as i64;
        (
            self.lib.id.clone(),
            [
                um(extent.min_x),
                um(extent.min_y),
                um(extent.max_x),
                um(extent.max_y),
            ],
        )
    }

    fn paper_look(&self) -> PaperLook {
        PaperLook {
            backdrop: to_iced(&self.colors.background),
            paper: to_iced(&self.colors.paper),
            grid: to_iced(&self.colors.grid),
            grid_visible: self.grid.visible,
            grid_mm: self.grid.pitch_mm,
            grid_style: self.grid.style,
            device_px_per_logical_px: self.device_px_per_logical_px,
            border: true,
            fine_dots: false,
        }
    }

    /// Paper, then grid, then the part, through `camera`.
    fn paint(&self, frame: &mut canvas::Frame, camera: &Camera, size: Size) {
        let (sheet, _) = preview_sheet(&self.lib);
        let area = Rectangle::with_size(size);
        paint_paper_and_grid(
            frame,
            camera,
            area,
            {
                let (w, h) = sheet.paper_mm();
                (w as f32, h as f32)
            },
            &self.paper_look(),
        );
        let transform = ScreenTransform {
            offset_x: camera.offset.x,
            offset_y: camera.offset.y,
            scale: camera.scale,
            device_px_per_logical_px: self.device_px_per_logical_px,
        };
        if let Some(scene) = build_schematic_scene(
            &sheet,
            &transform,
            &self.colors,
            area,
            None,
            SceneOverrides::default(),
        ) {
            crate::renderer_scene_canvas::draw_scene_with_world_to_screen(
                frame,
                &scene,
                |point| transform.world_to_screen((point[0] as f64, point[1] as f64)),
                preview_draw_options(&transform),
            );
        }
    }
}

impl canvas::Program<()> for SymbolSheetPreview {
    type State = PreviewState;

    fn update(
        &self,
        state: &mut PreviewState,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<()>> {
        let size_ok = bounds.width > 0.0 && bounds.height > 0.0;
        let key = self.fit_key();
        if size_ok && !state.is_fitted_for(&key, bounds.size()) {
            state.camera = self.fitted_camera(bounds.size());
            state.fitted_for = Some(key);
            state.fitted_size = Some(bounds.size());
            state.user_moved = false;
            state.pan_from = None;
            state.cache.clear();
        }
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
                if !state.camera.zoom_at(
                    anchor,
                    lines,
                    self.zoom_step,
                    Rectangle::with_size(bounds.size()),
                ) {
                    return None;
                }
                state.user_moved = true;
                state.cache.clear();
                Some(Action::request_redraw().and_capture())
            }
            // Previews keep wheel and pinch zoom only: the opt-in two-finger
            // pan is an editor-canvas preference and is not offered here.
            mouse::Event::PinchGesture { delta } => {
                let anchor = cursor.position_in(bounds)?;
                if !state
                    .camera
                    .zoom_by_factor(anchor, Camera::pinch_factor(*delta))
                {
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
                let position = drag_position(cursor)?;
                let delta = Vector::new(position.x - last.x, position.y - last.y);
                state.camera.pan(delta.x, delta.y);
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
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        // Until `update` has fitted this symbol at this size, draw the
        // fitted view without caching it against the stale camera.
        let fitted = state.is_fitted_for(&self.fit_key(), bounds.size());
        if !fitted {
            state.cache.clear();
        }
        let current = (self.lib.clone(), self.colors, self.grid);
        if take_rebuild(&mut state.shown.borrow_mut(), &current) {
            state.cache.clear();
        }
        let camera = if fitted {
            state.camera.clone()
        } else {
            self.fitted_camera(bounds.size())
        };
        let geometry = state.cache.draw(renderer, bounds.size(), |frame| {
            if bounds.width > 0.0 && bounds.height > 0.0 {
                self.paint(frame, &camera, bounds.size());
            }
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

/// The Symbol preview of `lib`, `height` tall and as wide as its parent,
/// clipped to its box; strokes snap to the window's device pixel grid.
pub(crate) fn symbol_preview(
    lib: LibSymbol,
    colors: CanvasColors,
    grid: SheetGrid,
    height: f32,
    device_px_per_logical_px: f32,
) -> Element<'static, ()> {
    let canvas = canvas::Canvas::new(SymbolSheetPreview {
        lib,
        colors,
        grid,
        device_px_per_logical_px,
        zoom_step: Camera::zoom_step_factor(Camera::ZOOM_STEP_PERCENT_DEFAULT),
    })
    .width(Length::Fill)
    .height(height);
    container::<_, Theme>(canvas)
        .width(Length::Fill)
        .height(height)
        .clip(true)
        .boxed()
}

#[cfg(test)]
#[path = "symbol_canvas_tests.rs"]
mod tests;
