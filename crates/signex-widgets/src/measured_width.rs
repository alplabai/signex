//! Content as wide as another element needs: the `measure` element is laid
//! out at its natural width (every fill part hugging its contents) and the
//! content takes that width, never less than a floor. Only the content is
//! drawn. Used where a column must fit one line of measured text, so the
//! width follows the font instead of a pixel count.

use iced::advanced::layout::{Layout, Limits};
use iced::advanced::widget::{self, Tree, Widget};
use iced::advanced::{Shell, overlay, renderer};
use iced::mouse::{self, Cursor};
use iced::{Element, Event, Length, Rectangle, Size, Vector};

/// `content` laid out as wide as `measure`'s natural width, at least
/// `min_width`.
pub struct MeasuredWidth<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    /// `[content, measure]`.
    parts: [Element<'a, Message, Theme, Renderer>; 2],
    min_width: f32,
}

impl<'a, Message, Theme, Renderer> MeasuredWidth<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    /// `content` as wide as `measure` lays out on its own, with no floor.
    pub fn new(
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        measure: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        Self {
            parts: [content.into(), measure.into()],
            min_width: 0.0,
        }
    }

    /// Never narrower than `min_width`.
    pub fn min_width(mut self, min_width: f32) -> Self {
        self.min_width = min_width;
        self
    }
}

/// The width the content takes: the natural width rounded up to a whole
/// pixel, at least `min_width`, inside the `limits`.
pub fn resolved_width(natural: f32, min_width: f32, limits: &Limits) -> f32 {
    let wanted = natural.ceil().max(min_width);
    let width = if limits.infinite.width {
        wanted
    } else {
        wanted.min(limits.max.width)
    };
    width.max(limits.min.width)
}

impl<Message, Theme, Renderer> widget::Meta for MeasuredWidth<'_, Message, Theme, Renderer> where
    Renderer: iced::advanced::Renderer
{
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for MeasuredWidth<'_, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(self.parts.as_mut_slice());
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Shrink,
            height: self.parts[0].size().height,
        }
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) {
        // Natural width: fill parts compress to their contents and the
        // width is unbounded, so nothing wraps or folds.
        let natural = Limits::with_flags(
            Size::new(0.0, limits.min.height),
            limits.max,
            Size::new(true, limits.compression.height),
            Size::new(true, limits.infinite.height),
        );
        let [content, measure] = &mut self.parts;
        measure.layout(&mut tree.children[1], renderer, &natural);
        let width = resolved_width(tree.children[1].size.width, self.min_width, limits);
        let exact = Limits::with_flags(
            Size::new(width, limits.min.height),
            Size::new(width, limits.max.height),
            Size::new(false, limits.compression.height),
            Size::new(false, limits.infinite.height),
        );
        let child = &mut tree.children[0];
        content.layout(child, renderer, &exact);
        child.translation = Vector::ZERO;
        tree.size = Size::new(width, child.size.height);
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        if let Some((layout, state)) = layout.iter_mut(&mut tree.children).next() {
            self.parts[0].operate(state, layout, viewport, renderer, operation);
        }
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout,
        cursor: Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if let Some((layout, state)) = layout.iter_mut(&mut tree.children).next() {
            self.parts[0].update(state, event, layout, cursor, renderer, shell, viewport);
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        layout
            .iter(&tree.children)
            .next()
            .map(|(layout, state)| {
                self.parts[0].mouse_interaction(state, layout, cursor, viewport, renderer)
            })
            .unwrap_or_default()
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
        if let Some((layout, state)) = layout.iter(&tree.children).next() {
            self.parts[0].draw(state, renderer, theme, style, layout, cursor, viewport);
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
        window: Size,
    ) -> Vec<overlay::Element<'b, Message, Theme, Renderer>> {
        match layout.iter_mut(&mut tree.children).next() {
            Some((layout, state)) => {
                self.parts[0].overlay(state, layout, renderer, viewport, translation, window)
            }
            None => Vec::new(),
        }
    }
}

impl<'a, Message, Theme, Renderer> From<MeasuredWidth<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn from(width: MeasuredWidth<'a, Message, Theme, Renderer>) -> Self {
        Widget::_boxed(width)
    }
}

// iced's headless `()` renderer exists only with debug assertions.
#[cfg(all(test, debug_assertions))]
mod tests {
    use super::*;
    use iced::Theme;
    use iced::widget::{Space, container};

    type Part = Element<'static, (), Theme, ()>;

    fn laid_out(measure: f32, min_width: f32, room: f32) -> (f32, f32) {
        let content: Part =
            iced::Widget::boxed(container(Space::new().width(10).height(10)).width(Length::Fill));
        let measure: Part = iced::Widget::boxed(Space::new().width(measure).height(10));
        let mut element: Element<'static, (), Theme, ()> = MeasuredWidth::new(content, measure)
            .min_width(min_width)
            .into();
        let mut tree = Tree::new(&element);
        tree.diff(&mut element);
        let limits = Limits::new(Size::ZERO, Size::new(room, 100.0));
        element.layout(&mut tree, &(), &limits);
        (tree.size.width, tree.children[0].size.width)
    }

    #[test]
    fn the_content_takes_the_measured_width() {
        assert_eq!(laid_out(150.0, 100.0, 400.0), (150.0, 150.0));
    }

    #[test]
    fn a_narrow_measure_keeps_the_floor() {
        assert_eq!(laid_out(60.0, 100.0, 400.0), (100.0, 100.0));
    }

    #[test]
    fn the_width_is_rounded_up_and_stays_inside_the_room() {
        assert_eq!(laid_out(150.2, 0.0, 400.0).0, 151.0);
        assert_eq!(laid_out(500.0, 100.0, 400.0).0, 400.0);
    }
}
