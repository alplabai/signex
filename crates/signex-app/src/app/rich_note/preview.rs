//! The editor's live preview (mockup row 203): the note as it will draw on
//! the sheet, as vector paths at the preview's scale. Ctrl+click on a link
//! opens it; a plain click does nothing.

use iced::widget::canvas;
use iced::{Color, Point, Rectangle, Renderer, Theme, keyboard, mouse};
use signex_notebook::note::{NoteDrawing, NotePaint, NotePathOp, NoteShapeKind};

use super::RichNoteMsg;
use crate::app::Message;

/// The preview canvas of one drawing.
pub(super) struct NotePreview<'a> {
    pub drawing: &'a NoteDrawing,
    /// The colour Typst's default black draws in.
    pub ink: Color,
    /// Screen pixels per mm.
    pub px_per_mm: f32,
}

impl NotePreview<'_> {
    fn point(&self, p: [f64; 2]) -> Point {
        Point::new(p[0] as f32 * self.px_per_mm, p[1] as f32 * self.px_per_mm)
    }

    /// The link under the preview point `at` (pixels from its top-left).
    pub(super) fn link_at(&self, at: Point) -> Option<&str> {
        let (x, y) = (
            f64::from(at.x / self.px_per_mm),
            f64::from(at.y / self.px_per_mm),
        );
        self.drawing
            .links
            .iter()
            .find(|link| link.contains(x, y))
            .map(|link| link.target.as_str())
    }
}

impl canvas::Program<Message> for NotePreview<'_> {
    /// The modifiers held, so a click knows whether Ctrl is down.
    type State = keyboard::Modifiers;

    fn update(
        &self,
        state: &mut Self::State,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        match event {
            iced::Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                *state = *modifiers;
                None
            }
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                if state.command() =>
            {
                let at = cursor.position_in(bounds)?;
                let target = self.link_at(at)?.to_string();
                Some(
                    canvas::Action::publish(Message::from(RichNoteMsg::LinkClicked { target }))
                        .and_capture(),
                )
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        for shape in &self.drawing.shapes {
            let path = canvas::Path::new(|b| {
                for op in &shape.path {
                    match *op {
                        NotePathOp::Move(p) => b.move_to(self.point(p)),
                        NotePathOp::Line(p) => b.line_to(self.point(p)),
                        NotePathOp::Quad(c, p) => {
                            b.quadratic_curve_to(self.point(c), self.point(p));
                        }
                        NotePathOp::Cubic(c1, c2, p) => {
                            b.bezier_curve_to(self.point(c1), self.point(c2), self.point(p));
                        }
                        NotePathOp::Close => b.close(),
                    }
                }
            });
            let color = match shape.paint {
                NotePaint::Ink => self.ink,
                NotePaint::Rgba([r, g, b, a]) => Color::from_rgba8(r, g, b, f32::from(a) / 255.0),
            };
            match shape.kind {
                NoteShapeKind::Fill { .. } => frame.fill(&path, color),
                NoteShapeKind::Stroke { width_mm } => frame.stroke(
                    &path,
                    canvas::Stroke::default()
                        .with_color(color)
                        .with_width(width_mm as f32 * self.px_per_mm),
                ),
            }
        }
        vec![frame.into_geometry()]
    }
}
