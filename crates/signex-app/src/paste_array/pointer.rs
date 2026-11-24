//! Pointer input on the Paste Array preview (mockup row 69): a press on copy
//! 2 holds it and the spacing follows the drag; a click on a drawn value opens
//! it for typing on the drawing. It draws nothing and keeps no state: the
//! held copy lives in `PasteArrayState::preview_drag`.

use iced::widget::canvas::{self, Canvas};
use iced::{Element, Length, Point, Rectangle, Renderer, Theme, mouse};
use signex_widgets::property_illustration::Fit;

use super::PasteArrayMsg;
use super::preview::PreviewLayout;

/// The press, drag and click layer laid over the drawing.
pub(super) struct PreviewPointer {
    pub layout: PreviewLayout,
    /// Copy 2 is held.
    pub is_dragging: bool,
    /// The drawing's caption size, in pixels.
    pub text_size: f32,
}

impl PreviewPointer {
    fn fit(&self, bounds: Rectangle) -> Fit {
        Fit::new(self.layout.view_size(), bounds.size())
    }

    /// What a press at `at` (in the drawing) does.
    fn press(&self, bounds: Rectangle, at: Point) -> Option<PasteArrayMsg> {
        let fit = self.fit(bounds);
        if let Some(field) = self.layout.value_at(&fit, self.text_size, at) {
            return Some(PasteArrayMsg::PreviewValueClicked(field));
        }
        self.layout
            .is_on_copy_two(&fit, self.text_size, at)
            .then(|| PasteArrayMsg::PreviewCopyGrabbed(self.layout.to_sheet(&fit, at)))
    }
}

impl canvas::Program<PasteArrayMsg> for PreviewPointer {
    type State = ();

    fn update(
        &self,
        _state: &mut (),
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<PasteArrayMsg>> {
        let canvas::Event::Mouse(event) = event else {
            return None;
        };
        let message = match event {
            mouse::Event::ButtonPressed(mouse::Button::Left) => {
                self.press(bounds, cursor.position_in(bounds)?)
            }
            // A held copy follows the pointer out of the drawing too.
            mouse::Event::CursorMoved { position } if self.is_dragging => {
                let at = Point::new(position.x - bounds.x, position.y - bounds.y);
                Some(PasteArrayMsg::PreviewCopyDragged(
                    self.layout.to_sheet(&self.fit(bounds), at),
                ))
            }
            mouse::Event::ButtonReleased(mouse::Button::Left) if self.is_dragging => {
                Some(PasteArrayMsg::PreviewCopyDropped)
            }
            _ => None,
        }?;
        Some(canvas::Action::publish(message).and_capture())
    }

    fn draw(
        &self,
        _state: &(),
        _renderer: &Renderer,
        _theme: &Theme,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        Vec::new()
    }

    fn mouse_interaction(
        &self,
        _state: &(),
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if self.is_dragging {
            return mouse::Interaction::Grabbing;
        }
        let Some(at) = cursor.position_in(bounds) else {
            return mouse::Interaction::default();
        };
        let fit = self.fit(bounds);
        if self.layout.value_at(&fit, self.text_size, at).is_some() {
            mouse::Interaction::Text
        } else if self.layout.is_on_copy_two(&fit, self.text_size, at) {
            mouse::Interaction::Grab
        } else {
            mouse::Interaction::default()
        }
    }
}

/// The pointer layer, as tall as the drawing under it.
pub(super) fn preview_pointer<'a>(
    pointer: PreviewPointer,
    height: f32,
) -> Element<'a, PasteArrayMsg> {
    Canvas::new(pointer)
        .width(Length::Fill)
        .height(Length::Fixed(height))
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paste_array::PasteArrayField;
    use crate::paste_array::preview::{PreviewSpec, layout};
    use iced::widget::canvas::Program;
    use signex_i18n::UserText;

    fn pointer(is_dragging: bool) -> PreviewPointer {
        let text = |s: &str| UserText::new(s.to_owned()).into();
        let spec = PreviewSpec {
            positions: 4,
            dx_mm: 10.16,
            dy_mm: 0.0,
            step: 1,
            copy_size_mm: Some((8.0, 4.0)),
            grid_mm: 1.27,
            sample_label: Some("D0".into()),
            x_label: text("X 10.16 mm"),
            y_label: text("Y 0 mm"),
            more_label: text("+0 more"),
            copies_label: text("4 copies"),
            step_label: Some(text("+1 per copy")),
        };
        PreviewPointer {
            layout: layout(&spec),
            is_dragging,
            text_size: 11.0,
        }
    }

    const BOUNDS: Rectangle = Rectangle {
        x: 20.0,
        y: 40.0,
        width: 300.0,
        height: 160.0,
    };

    fn sent(p: &PreviewPointer, event: mouse::Event, at: Point) -> Option<PasteArrayMsg> {
        let cursor = mouse::Cursor::Available(at);
        p.update(&mut (), &canvas::Event::Mouse(event), BOUNDS, cursor)
            .and_then(|action| action.into_inner().0)
    }

    /// A screen point of the drawing at view point `v`.
    fn on_screen(p: &PreviewPointer, v: Point) -> Point {
        let fit = p.fit(BOUNDS);
        let local = fit.point(v);
        Point::new(local.x + BOUNDS.x, local.y + BOUNDS.y)
    }

    #[test]
    fn a_press_on_copy_two_grabs_it_and_a_press_on_a_value_edits_it() {
        let p = pointer(false);
        let (ox, oy) = p.layout.origin;
        let copy_two = on_screen(&p, Point::new((ox + 10.16 + 4.0) as f32, (oy + 2.0) as f32));
        let press = mouse::Event::ButtonPressed(mouse::Button::Left);
        let Some(PasteArrayMsg::PreviewCopyGrabbed((x, y))) = sent(&p, press, copy_two) else {
            panic!("copy 2 is grabbed");
        };
        assert!(
            (x - 14.16).abs() < 1e-3 && (y - 2.0).abs() < 1e-3,
            "{x} {y}"
        );

        let copies = p.layout.values[0];
        assert_eq!(copies.field, PasteArrayField::Copies);
        let label = on_screen(&p, Point::new(copies.at.x, copies.at.y));
        let at = Point::new(label.x + 2.0, label.y - 2.0);
        assert!(matches!(
            sent(&p, press, at),
            Some(PasteArrayMsg::PreviewValueClicked(PasteArrayField::Copies))
        ));

        let original = on_screen(&p, Point::new((ox + 4.0) as f32, (oy + 2.0) as f32));
        assert!(
            sent(&p, press, original).is_none(),
            "the original stays put"
        );
    }

    /// Only a held copy follows the pointer, out of the drawing too, and the
    /// release lets it go.
    #[test]
    fn a_held_copy_follows_the_pointer_until_released() {
        let moved = |position| mouse::Event::CursorMoved { position };
        let release = mouse::Event::ButtonReleased(mouse::Button::Left);
        let outside = Point::new(BOUNDS.x + BOUNDS.width + 50.0, BOUNDS.y);
        let idle = pointer(false);
        assert!(sent(&idle, moved(outside), outside).is_none());
        assert!(sent(&idle, release, outside).is_none());

        let held = pointer(true);
        assert!(matches!(
            sent(&held, moved(outside), outside),
            Some(PasteArrayMsg::PreviewCopyDragged(_))
        ));
        assert!(matches!(
            sent(&held, release, outside),
            Some(PasteArrayMsg::PreviewCopyDropped)
        ));
    }
}
