//! One of two faces of a label, sized to the wider of them, so that
//! switching face never changes the width a parent lays out.

use iced::advanced::layout::{Layout, Limits};
use iced::advanced::renderer;
use iced::advanced::widget::{self, Tree, Widget};
use iced::mouse::Cursor;
use iced::{Element, Length, Rectangle, Size};

/// Draws `faces[shown]` in the width of the wider face.
pub(crate) struct WidestFace<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    faces: [Element<'a, Message, Theme, Renderer>; 2],
    shown: usize,
}

impl<'a, Message, Theme, Renderer> WidestFace<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    /// `plain` and `emphasised` faces of one label; `is_emphasised` picks
    /// the one drawn.
    pub(crate) fn new(
        plain: Element<'a, Message, Theme, Renderer>,
        emphasised: Element<'a, Message, Theme, Renderer>,
        is_emphasised: bool,
    ) -> Self {
        Self {
            faces: [plain, emphasised],
            shown: usize::from(is_emphasised),
        }
    }
}

impl<Message, Theme, Renderer> widget::Meta for WidestFace<'_, Message, Theme, Renderer> where
    Renderer: iced::advanced::Renderer
{
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for WidestFace<'_, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(self.faces.as_mut_slice());
    }

    fn size(&self) -> Size<Length> {
        self.faces[self.shown].size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) {
        let measure = limits.loose();
        let mut widest: f32 = 0.0;
        for (face, child) in self.faces.iter_mut().zip(&mut tree.children) {
            face.layout(child, renderer, &measure);
            widest = widest.max(child.size.width);
        }
        let width = limits.resolve_width(Length::Shrink, widest);
        let exact = Limits::with_flags(
            Size::new(width, limits.min.height),
            Size::new(width, limits.max.height),
            Size::new(false, limits.compression.height),
            Size::new(false, limits.infinite.height),
        );
        for (face, child) in self.faces.iter_mut().zip(&mut tree.children) {
            face.layout(child, renderer, &exact);
        }
        tree.size = Size::new(width, tree.children[self.shown].size.height);
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        if let Some((layout, state)) = layout.iter_mut(&mut tree.children).nth(self.shown) {
            self.faces[self.shown].operate(state, layout, viewport, renderer, operation);
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        if let Some((layout, state)) = layout.iter(&tree.children).nth(self.shown) {
            self.faces[self.shown].draw(state, renderer, theme, style, layout, cursor, viewport);
        }
    }
}

impl<'a, Message, Theme, Renderer> From<WidestFace<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn from(face: WidestFace<'a, Message, Theme, Renderer>) -> Self {
        Widget::_boxed(face)
    }
}

// iced's headless `()` renderer exists only with debug assertions.
#[cfg(all(test, debug_assertions))]
mod tests {
    use super::*;
    use iced::Theme;
    use iced::widget::Space;

    fn laid_out_width(plain: f32, emphasised: f32, is_emphasised: bool, room: f32) -> f32 {
        let mut element: Element<'static, (), Theme, ()> = WidestFace::new(
            Space::new().width(plain).height(10).into(),
            Space::new().width(emphasised).height(10).into(),
            is_emphasised,
        )
        .into();
        let mut tree = Tree::new(&element);
        tree.diff(&mut element);
        let limits = Limits::new(Size::ZERO, Size::new(room, 100.0));
        element.layout(&mut tree, &(), &limits);
        tree.size.width
    }

    #[test]
    fn the_width_does_not_depend_on_the_face_shown() {
        for (plain, emphasised) in [(30.0, 50.0), (50.0, 30.0)] {
            assert_eq!(laid_out_width(plain, emphasised, false, 200.0), 50.0);
            assert_eq!(laid_out_width(plain, emphasised, true, 200.0), 50.0);
        }
    }

    #[test]
    fn the_width_stays_inside_the_room_given() {
        assert_eq!(laid_out_width(30.0, 50.0, true, 40.0), 40.0);
    }
}
