//! A bar laid out on one line when its parts fit at their measured widths:
//! a field that gives up width first, the parts after it at their natural
//! widths, and an end part at the right end. When the line would need more
//! than the bar has, even with the field at its least, a stacked layout the
//! caller builds is shown instead.

use iced::advanced::layout::{Layout, Limits};
use iced::advanced::widget::{self, Tree, Widget, tree};
use iced::advanced::{Shell, overlay, renderer};
use iced::mouse::{self, Cursor};
use iced::{Element, Event, Length, Rectangle, Size, Vector};

/// Less than this over the width is float rounding, not overflow.
const ROUNDING: f32 = 0.01;

/// Indices of the parts in `FitBar::parts` and the tree's children.
const FIELD: usize = 0;
const MIDDLE: usize = 1;
const END: usize = 2;
const STACKED: usize = 3;

/// A field, the parts after it and an end part on one line, or `stacked`
/// when they do not fit.
pub struct FitBar<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    /// `[field, middle, end, stacked]`.
    parts: [Element<'a, Message, Theme, Renderer>; 4],
    field_least: f32,
    field_most: f32,
    spacing: f32,
    end_spacing: f32,
}

/// Which layout the last pass picked.
#[derive(Debug, Clone, Copy, Default)]
struct Picked {
    is_one_line: bool,
}

impl<'a, Message, Theme, Renderer> FitBar<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    /// `field`, `middle` and `end` on one line when they fit, else
    /// `stacked`. The field must fill the width it is given.
    pub fn new(
        field: impl Into<Element<'a, Message, Theme, Renderer>>,
        middle: impl Into<Element<'a, Message, Theme, Renderer>>,
        end: impl Into<Element<'a, Message, Theme, Renderer>>,
        stacked: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        Self {
            parts: [field.into(), middle.into(), end.into(), stacked.into()],
            field_least: 0.0,
            field_most: f32::INFINITY,
            spacing: 0.0,
            end_spacing: 0.0,
        }
    }

    /// The field's least and most width on the line.
    pub fn field_width(mut self, least: f32, most: f32) -> Self {
        self.field_least = least;
        self.field_most = most.max(least);
        self
    }

    /// The gap between the field and the middle parts.
    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;
        self
    }

    /// The least gap before the end part.
    pub fn end_spacing(mut self, end_spacing: f32) -> Self {
        self.end_spacing = end_spacing;
        self
    }
}

/// The field's width on one line of `available` when the other parts take
/// `rest`: the leftover between `least` and `most`, or `None` when even
/// `least` does not fit.
pub fn one_line_field_width(available: f32, rest: f32, least: f32, most: f32) -> Option<f32> {
    let room = available - rest;
    (room + ROUNDING >= least).then(|| room.clamp(least, most))
}

/// The parts the last layout placed.
fn shown(tree: &Tree) -> std::ops::Range<usize> {
    if tree.state.downcast_ref::<Picked>().is_one_line {
        FIELD..STACKED
    } else {
        STACKED..STACKED + 1
    }
}

impl<Message, Theme, Renderer> widget::Meta for FitBar<'_, Message, Theme, Renderer> where
    Renderer: iced::advanced::Renderer
{
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for FitBar<'_, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Picked>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(Picked::default())
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(self.parts.as_mut_slice());
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fill,
            height: Length::Shrink,
        }
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) {
        let limits = limits.width(Length::Fill).height(Length::Shrink);
        let available = limits.bounds().width;
        // Natural widths: unbounded, fill parts hugging their contents.
        let natural = Limits::with_flags(
            Size::ZERO,
            limits.max,
            Size::new(true, true),
            Size::new(true, limits.infinite.height),
        );
        let [field, middle, end, stacked] = &mut self.parts;
        middle.layout(&mut tree.children[MIDDLE], renderer, &natural);
        end.layout(&mut tree.children[END], renderer, &natural);
        let middle_size = tree.children[MIDDLE].size;
        let end_size = tree.children[END].size;
        let rest = self.spacing + middle_size.width + self.end_spacing + end_size.width;
        let picked = one_line_field_width(available, rest, self.field_least, self.field_most);
        *tree.state.downcast_mut::<Picked>() = Picked {
            is_one_line: picked.is_some(),
        };
        let Some(field_width) = picked else {
            let child = &mut tree.children[STACKED];
            stacked.layout(child, renderer, &limits);
            child.translation = Vector::ZERO;
            tree.size = limits.resolve(Length::Fill, Length::Shrink, child.size);
            return;
        };
        let exact = Limits::with_flags(
            Size::new(field_width, 0.0),
            Size::new(field_width, limits.max.height),
            Size::new(false, true),
            Size::new(false, limits.infinite.height),
        );
        field.layout(&mut tree.children[FIELD], renderer, &exact);
        let field_size = tree.children[FIELD].size;
        let height = field_size
            .height
            .max(middle_size.height)
            .max(end_size.height);
        let width = if available.is_finite() {
            available.max(field_width + rest)
        } else {
            field_width + rest
        };
        let centred = |h: f32| (height - h) / 2.0;
        tree.children[FIELD].translation = Vector::new(0.0, centred(field_size.height));
        tree.children[MIDDLE].translation =
            Vector::new(field_width + self.spacing, centred(middle_size.height));
        tree.children[END].translation =
            Vector::new(width - end_size.width, centred(end_size.height));
        tree.size = limits.resolve(Length::Fill, Length::Shrink, Size::new(width, height));
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        let range = shown(tree);
        operation.container(None, layout.bounds(), viewport);
        operation.traverse(&mut |operation| {
            self.parts[range.clone()]
                .iter_mut()
                .zip(layout.iter_mut(&mut tree.children[range.clone()]))
                .for_each(|(part, (layout, state))| {
                    part.operate(state, layout, viewport, renderer, operation);
                });
        });
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
        let range = shown(tree);
        for (part, (layout, state)) in self.parts[range.clone()]
            .iter_mut()
            .zip(layout.iter_mut(&mut tree.children[range]))
        {
            part.update(state, event, layout, cursor, renderer, shell, viewport);
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
        let range = shown(tree);
        self.parts[range.clone()]
            .iter()
            .zip(layout.iter(&tree.children[range]))
            .map(|(part, (layout, state))| {
                part.mouse_interaction(state, layout, cursor, viewport, renderer)
            })
            .max()
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
        let range = shown(tree);
        for (part, (layout, state)) in self.parts[range.clone()]
            .iter()
            .zip(layout.iter(&tree.children[range]))
        {
            part.draw(state, renderer, theme, style, layout, cursor, viewport);
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
        let range = shown(tree);
        self.parts[range.clone()]
            .iter_mut()
            .zip(layout.iter_mut(&mut tree.children[range]))
            .flat_map(|(part, (layout, state))| {
                part.overlay(state, layout, renderer, viewport, translation, window)
            })
            .collect()
    }
}

impl<'a, Message, Theme, Renderer> From<FitBar<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn from(bar: FitBar<'a, Message, Theme, Renderer>) -> Self {
        Widget::_boxed(bar)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_field_takes_the_leftover_up_to_its_most() {
        assert_eq!(
            one_line_field_width(1400.0, 700.0, 120.0, 250.0),
            Some(250.0)
        );
        assert_eq!(
            one_line_field_width(890.0, 700.0, 120.0, 250.0),
            Some(190.0)
        );
        assert_eq!(
            one_line_field_width(f32::INFINITY, 700.0, 120.0, 250.0),
            Some(250.0)
        );
    }

    #[test]
    fn the_field_gives_up_width_down_to_its_least_then_the_line_stacks() {
        assert_eq!(
            one_line_field_width(820.0, 700.0, 120.0, 250.0),
            Some(120.0)
        );
        assert_eq!(one_line_field_width(819.0, 700.0, 120.0, 250.0), None);
        assert_eq!(one_line_field_width(290.0, 700.0, 120.0, 250.0), None);
    }

    #[test]
    fn a_rounding_error_short_still_fits() {
        assert_eq!(
            one_line_field_width(819.999, 700.0, 120.0, 250.0),
            Some(120.0)
        );
    }
}

// iced's headless `()` renderer exists only with debug assertions.
#[cfg(all(test, debug_assertions))]
mod layout_tests {
    use super::*;
    use iced::Theme;
    use iced::widget::{Space, container};

    type Part = Element<'static, (), Theme, ()>;

    /// The bar's children after a layout in `room`: (picked one line, the
    /// field's width, the end part's x).
    fn laid_out(room: f32) -> (bool, f32, f32) {
        let fixed = |w: f32| -> Part { iced::Widget::boxed(Space::new().width(w).height(10)) };
        let field: Part =
            iced::Widget::boxed(container(Space::new().height(10)).width(Length::Fill));
        let mut element: Element<'static, (), Theme, ()> =
            FitBar::new(field, fixed(300.0), fixed(100.0), fixed(50.0))
                .field_width(120.0, 250.0)
                .spacing(10.0)
                .end_spacing(20.0)
                .into();
        let mut tree = Tree::new(&element);
        tree.diff(&mut element);
        let limits = Limits::new(Size::ZERO, Size::new(room, 100.0));
        element.layout(&mut tree, &(), &limits);
        let picked = tree.state.downcast_ref::<Picked>().is_one_line;
        (
            picked,
            tree.children[FIELD].size.width,
            tree.children[END].translation.x,
        )
    }

    #[test]
    fn parts_that_fit_share_one_line_with_the_end_at_the_right() {
        let (one_line, field, end_x) = laid_out(800.0);
        assert!(one_line);
        assert_eq!(field, 250.0);
        assert_eq!(end_x, 700.0);
    }

    #[test]
    fn the_field_shrinks_before_the_line_stacks() {
        let (one_line, field, _) = laid_out(600.0);
        assert!(one_line);
        assert_eq!(field, 170.0);
        let (one_line, _, _) = laid_out(549.0);
        assert!(!one_line, "120 + 430 needs 550");
    }
}
