//! Zoom and pan of the Components panel previews, held in the model per
//! preview. A view is relative to the preview's fit, so it survives a resize
//! and needs no drawing data to update. The canvases only report gestures.

use iced::advanced::mouse::{Click, click::Kind};
use iced::widget::canvas::Action;
use iced::{Point, Rectangle, Vector, mouse};

use crate::canvas::{Camera, WheelMode};

/// Lowest zoom relative to the fit.
const MIN_ZOOM: f32 = 0.125;
/// Highest zoom relative to the fit.
const MAX_ZOOM: f32 = 32.0;
/// Pixels of a smooth-scroll delta that count as one wheel notch.
const PIXELS_PER_NOTCH: f32 = 50.0;

/// Which preview of the details card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewPane {
    Symbol,
    Footprint,
}

/// What the user did on a preview. Points are in the preview's own box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PreviewGesture {
    WheelScrolled { anchor: Point, notches: f32 },
    Pinched { anchor: Point, delta: f32 },
    Dragged(Vector),
    FitRequested,
}

/// One preview's view on top of its fit: `screen = fitted * zoom + pan`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreviewView {
    zoom: f32,
    pan: Vector,
}

impl Default for PreviewView {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: Vector::new(0.0, 0.0),
        }
    }
}

impl PreviewView {
    /// Zoom relative to the fit; `1.0` is fitted.
    #[cfg(test)]
    fn zoom(self) -> f32 {
        self.zoom
    }

    /// The camera `(scale, offset)` this view makes of a fitted camera.
    pub fn place(self, scale: f32, offset: Point) -> (f32, Point) {
        (
            scale * self.zoom,
            Point::new(
                offset.x * self.zoom + self.pan.x,
                offset.y * self.zoom + self.pan.y,
            ),
        )
    }

    /// Zoomed by `factor`, keeping the point under `anchor` where it is.
    fn zoomed_about(self, anchor: Point, factor: f32) -> Self {
        if !factor.is_finite() || factor <= 0.0 {
            return self;
        }
        let zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let applied = zoom / self.zoom;
        Self {
            zoom,
            pan: Vector::new(
                anchor.x - (anchor.x - self.pan.x) * applied,
                anchor.y - (anchor.y - self.pan.y) * applied,
            ),
        }
    }

    fn after(self, gesture: PreviewGesture, zoom_step: f32) -> Self {
        match gesture {
            PreviewGesture::WheelScrolled { anchor, notches } => {
                self.zoomed_about(anchor, Camera::zoom_factor_for_scroll(zoom_step, notches))
            }
            PreviewGesture::Pinched { anchor, delta } => {
                self.zoomed_about(anchor, Camera::pinch_factor(delta))
            }
            PreviewGesture::Dragged(delta) => Self {
                pan: self.pan + delta,
                ..self
            },
            PreviewGesture::FitRequested => Self::default(),
        }
    }
}

/// The views of the card's two previews.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PreviewViews {
    pub symbol: PreviewView,
    pub footprint: PreviewView,
}

impl PreviewViews {
    pub fn of(&self, pane: PreviewPane) -> PreviewView {
        match pane {
            PreviewPane::Symbol => self.symbol,
            PreviewPane::Footprint => self.footprint,
        }
    }

    /// Apply `gesture` to `pane`'s view alone.
    pub fn apply(&mut self, pane: PreviewPane, gesture: PreviewGesture, zoom_step: f32) {
        let view = match pane {
            PreviewPane::Symbol => &mut self.symbol,
            PreviewPane::Footprint => &mut self.footprint,
        };
        *view = view.after(gesture, zoom_step);
    }
}

/// The pointer state of a gesture in progress (widget state: it is input
/// being read, not a view of the part).
#[derive(Default)]
pub(super) struct GestureInput {
    /// Last cursor position while a drag-pan is running.
    pan_from: Option<Point>,
    /// Ctrl/Cmd state from the last modifiers event.
    is_ctrl_held: bool,
    last_click: Option<Click>,
}

impl GestureInput {
    /// The gesture `event` makes. Under the Scroll wheel preference a plain
    /// wheel is left to the panel around the preview.
    pub(super) fn read(
        &mut self,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
        wheel_mode: WheelMode,
    ) -> Option<Action<PreviewGesture>> {
        if let iced::Event::Keyboard(iced::keyboard::Event::ModifiersChanged(mods)) = event {
            self.is_ctrl_held = mods.command();
            return None;
        }
        let iced::Event::Mouse(event) = event else {
            return None;
        };
        match event {
            mouse::Event::WheelScrolled { delta } => {
                if wheel_mode == WheelMode::Scroll && !self.is_ctrl_held {
                    return None;
                }
                let anchor = cursor.position_in(bounds)?;
                let notches = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => *y,
                    mouse::ScrollDelta::Pixels { y, .. } => *y / PIXELS_PER_NOTCH,
                };
                if notches == 0.0 || !notches.is_finite() {
                    return None;
                }
                let gesture = PreviewGesture::WheelScrolled { anchor, notches };
                Some(Action::publish(gesture).and_capture())
            }
            mouse::Event::PinchGesture { delta } => {
                let anchor = cursor.position_in(bounds)?;
                let gesture = PreviewGesture::Pinched {
                    anchor,
                    delta: *delta,
                };
                Some(Action::publish(gesture).and_capture())
            }
            mouse::Event::ButtonPressed(mouse::Button::Left) => {
                let at = cursor.position_in(bounds)?;
                let click = Click::new(at, mouse::Button::Left, self.last_click);
                self.last_click = Some(click);
                if click.kind() == Kind::Double {
                    self.pan_from = None;
                    return Some(Action::publish(PreviewGesture::FitRequested).and_capture());
                }
                self.pan_from = cursor.position();
                Some(Action::capture())
            }
            mouse::Event::CursorMoved { .. } => {
                let last = self.pan_from?;
                let position = super::symbol_canvas::drag_position(cursor)?;
                self.pan_from = Some(position);
                let delta = Vector::new(position.x - last.x, position.y - last.y);
                Some(Action::publish(PreviewGesture::Dragged(delta)).and_capture())
            }
            mouse::Event::ButtonReleased(_) => {
                self.pan_from.take()?;
                Some(Action::capture())
            }
            mouse::Event::CursorLeft => {
                self.pan_from = None;
                None
            }
            _ => None,
        }
    }

    pub(super) fn interaction(
        &self,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if self.pan_from.is_some() {
            mouse::Interaction::Grabbing
        } else if cursor.is_over(bounds) {
            mouse::Interaction::Grab
        } else {
            mouse::Interaction::None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Message, Signex};
    use crate::library::messages::LibraryMessage;

    const BOX: Rectangle = Rectangle {
        x: 0.0,
        y: 0.0,
        width: 200.0,
        height: 120.0,
    };
    const ANCHOR: Point = Point::new(40.0, 30.0);

    fn send(app: &mut Signex, pane: PreviewPane, gesture: PreviewGesture) {
        let _ = app.update(Message::Library(
            LibraryMessage::ComponentsPanelPreviewMoved(pane, gesture),
        ));
    }

    fn views(app: &Signex) -> PreviewViews {
        app.library.components_panel.preview_views
    }

    fn tick() -> PreviewGesture {
        PreviewGesture::WheelScrolled {
            anchor: ANCHOR,
            notches: 1.0,
        }
    }

    #[test]
    fn a_wheel_tick_over_the_footprint_preview_changes_its_zoom() {
        let (mut app, _boot) = Signex::new();
        send(&mut app, PreviewPane::Footprint, tick());
        assert!(views(&app).footprint.zoom() > 1.0);
    }

    #[test]
    fn each_preview_keeps_its_own_zoom() {
        let (mut app, _boot) = Signex::new();
        send(&mut app, PreviewPane::Footprint, tick());
        let footprint = views(&app).footprint;
        assert_eq!(views(&app).symbol, PreviewView::default());
        send(&mut app, PreviewPane::Symbol, tick());
        send(&mut app, PreviewPane::Symbol, tick());
        assert!(views(&app).symbol.zoom() > footprint.zoom());
        assert_eq!(views(&app).footprint, footprint);
    }

    #[test]
    fn fit_resets_only_its_own_preview() {
        let (mut app, _boot) = Signex::new();
        send(&mut app, PreviewPane::Footprint, tick());
        send(&mut app, PreviewPane::Symbol, tick());
        let symbol = views(&app).symbol;
        send(
            &mut app,
            PreviewPane::Footprint,
            PreviewGesture::FitRequested,
        );
        assert_eq!(views(&app).footprint, PreviewView::default());
        assert_eq!(views(&app).symbol, symbol);
    }

    #[test]
    fn a_zoom_keeps_the_point_under_the_cursor() {
        let (scale, offset) = (10.0, Point::new(25.0, 15.0));
        let world = Point::new((ANCHOR.x - offset.x) / scale, (ANCHOR.y - offset.y) / scale);
        let view = PreviewView::default().zoomed_about(ANCHOR, 1.5);
        let (zoomed, moved) = view.place(scale, offset);
        assert!((world.x * zoomed + moved.x - ANCHOR.x).abs() < 1e-3);
        assert!((world.y * zoomed + moved.y - ANCHOR.y).abs() < 1e-3);
    }

    #[test]
    fn the_zoom_stays_in_range_and_ignores_a_bad_factor() {
        let mut view = PreviewView::default();
        for _ in 0..40 {
            view = view.zoomed_about(ANCHOR, 2.0);
        }
        assert_eq!(view.zoom(), MAX_ZOOM);
        assert_eq!(view.zoomed_about(ANCHOR, f32::NAN), view);
        assert_eq!(view.zoomed_about(ANCHOR, 0.0), view);
    }

    fn wheel() -> iced::Event {
        iced::Event::Mouse(mouse::Event::WheelScrolled {
            delta: mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 },
        })
    }

    fn published(action: Option<Action<PreviewGesture>>) -> Option<PreviewGesture> {
        action?.into_inner().0
    }

    #[test]
    fn the_wheel_follows_the_mouse_wheel_preference() {
        let over = mouse::Cursor::Available(ANCHOR);
        let mut input = GestureInput::default();
        assert!(matches!(
            published(input.read(&wheel(), BOX, over, WheelMode::Zoom)),
            Some(PreviewGesture::WheelScrolled { .. })
        ));
        assert!(
            input.read(&wheel(), BOX, over, WheelMode::Scroll).is_none(),
            "a plain wheel scrolls the panel"
        );
        let ctrl = if cfg!(target_os = "macos") {
            iced::keyboard::Modifiers::LOGO
        } else {
            iced::keyboard::Modifiers::CTRL
        };
        let held = iced::Event::Keyboard(iced::keyboard::Event::ModifiersChanged(ctrl));
        let _ = input.read(&held, BOX, over, WheelMode::Scroll);
        assert!(matches!(
            published(input.read(&wheel(), BOX, over, WheelMode::Scroll)),
            Some(PreviewGesture::WheelScrolled { .. })
        ));
    }

    #[test]
    fn a_double_click_asks_for_the_fit() {
        let over = mouse::Cursor::Available(ANCHOR);
        let press = iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
        let release = iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left));
        let mut input = GestureInput::default();
        assert_eq!(
            published(input.read(&press, BOX, over, WheelMode::Zoom)),
            None
        );
        let _ = input.read(&release, BOX, over, WheelMode::Zoom);
        assert_eq!(
            published(input.read(&press, BOX, over, WheelMode::Zoom)),
            Some(PreviewGesture::FitRequested)
        );
    }
}
