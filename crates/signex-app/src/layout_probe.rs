//! Lay out a widget tree in a test and read back where its labels landed
//! (#623).
//!
//! The regression suite drives handlers through `Signex::update` and never
//! builds a laid-out widget tree, so no test could see a size or a position:
//! a button squeezed to a sliver with its label wrapped onto five lines
//! (#604) passed every gate. This lays an [`Element`] out against a window
//! size with a headless tiny-skia renderer, so text is measured with real
//! fonts, and reports the bounds of every text widget in it.
//!
//! The fonts are whatever the system and iced's defaults provide, not the
//! app's bundled faces, so widths are close to, not identical with, the
//! running app's. Assert on relations a few pixels cannot flip: a label
//! sits on one line, a button is at least as wide as its label.

use iced::advanced::widget::operation::Text;
use iced::advanced::widget::{Id, Operation, Tree};
use iced::advanced::{Layout, Widget as _, layout, renderer};
use iced::{Element, Font, Pixels, Rectangle, Size, Theme};

/// One text widget as laid out: its content and its bounds in window
/// coordinates.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LaidOutText {
    pub text: String,
    pub bounds: Rectangle,
}

/// Every text widget of `element` laid out in a window of `window`, in
/// tree order.
pub(crate) fn lay_out_texts<M>(
    mut element: Element<'_, M, Theme, iced::Renderer>,
    window: Size,
) -> Vec<LaidOutText> {
    let renderer = headless_renderer();
    let mut tree = Tree::new(&element);
    // The fork's widgets build their child trees in `diff`, not in
    // `Tree::new`: lay out only after reconciling.
    tree.diff(&mut element);
    element.layout(
        &mut tree,
        &renderer,
        &layout::Limits::new(Size::ZERO, window),
    );
    let size = tree.size;
    let mut collect = CollectTexts(Vec::new());
    element.operate(
        &mut tree,
        Layout::new(size),
        &Rectangle::with_size(size),
        &renderer,
        &mut collect,
    );
    collect.0
}

/// The messages a left click on the centre of the text `label` publishes,
/// from a press and a release delivered to the laid-out `element`.
pub(crate) fn messages_on_click<M>(
    element: Element<'_, M, Theme, iced::Renderer>,
    window: Size,
    label: &str,
) -> Vec<M> {
    messages_on_click_at(element, window, Some(label), iced::Point::ORIGIN)
}

/// [`messages_on_click`] at the centre of `label`, or at `point` when no
/// label is given.
pub(crate) fn messages_on_click_at<M>(
    mut element: Element<'_, M, Theme, iced::Renderer>,
    window: Size,
    label: Option<&str>,
    point: iced::Point,
) -> Vec<M> {
    use iced::advanced::mouse;
    let renderer = headless_renderer();
    let mut tree = Tree::new(&element);
    // The fork's widgets build their child trees in `diff`, not in
    // `Tree::new`: lay out only after reconciling.
    tree.diff(&mut element);
    element.layout(
        &mut tree,
        &renderer,
        &layout::Limits::new(Size::ZERO, window),
    );
    let size = tree.size;
    let layout = Layout::new(size);
    let mut collect = CollectTexts(Vec::new());
    element.operate(
        &mut tree,
        layout,
        &Rectangle::with_size(size),
        &renderer,
        &mut collect,
    );
    let at = label.map_or(point, |label| find(&collect.0, label).bounds.center());
    deliver(
        &mut element,
        &mut tree,
        &renderer,
        window,
        at,
        &[
            mouse::Event::CursorMoved { position: at },
            mouse::Event::ButtonPressed(mouse::Button::Left),
            mouse::Event::ButtonReleased(mouse::Button::Left),
        ]
        .map(iced::Event::Mouse),
    )
}

/// The messages one wheel `delta` at `point` publishes from the laid-out
/// `element`.
pub(crate) fn messages_on_scroll_at<M>(
    element: Element<'_, M, Theme, iced::Renderer>,
    window: Size,
    point: iced::Point,
    delta: iced::mouse::ScrollDelta,
) -> Vec<M> {
    messages_for_events(
        element,
        window,
        point,
        &[iced::Event::Mouse(iced::mouse::Event::WheelScrolled {
            delta,
        })],
    )
}

/// The messages `events` publish from the laid-out `element`, delivered in
/// one tree with the cursor at `point` (moved there first), so a widget's
/// state carries from one event to the next (a held modifier, a press).
pub(crate) fn messages_for_events<M>(
    mut element: Element<'_, M, Theme, iced::Renderer>,
    window: Size,
    point: iced::Point,
    events: &[iced::Event],
) -> Vec<M> {
    let renderer = headless_renderer();
    let mut tree = Tree::new(&element);
    tree.diff(&mut element);
    element.layout(
        &mut tree,
        &renderer,
        &layout::Limits::new(Size::ZERO, window),
    );
    let moved = iced::Event::Mouse(iced::mouse::Event::CursorMoved { position: point });
    let events: Vec<iced::Event> = std::iter::once(moved)
        .chain(events.iter().cloned())
        .collect();
    deliver(&mut element, &mut tree, &renderer, window, point, &events)
}

/// Deliver `events` with the cursor at `at` to a laid-out `element` and
/// collect what it publishes.
fn deliver<M>(
    element: &mut Element<'_, M, Theme, iced::Renderer>,
    tree: &mut Tree,
    renderer: &iced::Renderer,
    window: Size,
    at: iced::Point,
    events: &[iced::Event],
) -> Vec<M> {
    use iced::advanced::Shell;
    use iced::advanced::mouse;
    use iced::advanced::shell::{Bus, Waker};
    let layout = Layout::new(tree.size);
    let cursor = mouse::Cursor::Available(at);
    let mut bus = Bus::new();
    let viewport = Rectangle::with_size(window);
    for event in events {
        // One shell per event, as the runtime does: a captured press would
        // otherwise mask the release.
        let mut shell = Shell::new(&iced::window::Headless, Waker::noop(), &mut bus);
        element.update(tree, event, layout, cursor, renderer, &mut shell, &viewport);
    }
    bus.drain().map(|(message, _)| message).collect()
}

/// The size the root of `element` lays out to in a window of `window`.
pub(crate) fn root_size<M>(
    mut element: Element<'_, M, Theme, iced::Renderer>,
    window: Size,
) -> Size {
    let renderer = headless_renderer();
    let mut tree = Tree::new(&element);
    // The fork's widgets build their child trees in `diff`, not in
    // `Tree::new`: lay out only after reconciling.
    tree.diff(&mut element);
    element.layout(
        &mut tree,
        &renderer,
        &layout::Limits::new(Size::ZERO, window),
    );
    tree.size
}

/// `element` drawn by the headless software renderer into RGBA pixels, for
/// comparing what two views look like (a selected control, not just a label).
pub(crate) fn draw_pixels<M>(
    mut element: Element<'_, M, Theme, iced::Renderer>,
    window: Size,
) -> Vec<u8> {
    use iced::advanced::renderer::Headless as _;
    let mut renderer = headless_renderer();
    let mut tree = Tree::new(&element);
    tree.diff(&mut element);
    element.layout(
        &mut tree,
        &renderer,
        &layout::Limits::new(Size::ZERO, window),
    );
    let size = tree.size;
    element.draw(
        &tree,
        &mut renderer,
        &Theme::Dark,
        &renderer::Style {
            text_color: iced::Color::WHITE,
        },
        Layout::new(size),
        iced::advanced::mouse::Cursor::Unavailable,
        &Rectangle::with_size(window),
    );
    let iced::Renderer::Secondary(software) = &mut renderer else {
        panic!("the probe renderer is the software one");
    };
    software.screenshot(
        Size::new(window.width as u32, window.height as u32),
        1.0,
        iced::Color::BLACK,
    )
}

/// The text widget whose content is exactly `label`.
pub(crate) fn find<'t>(texts: &'t [LaidOutText], label: &str) -> &'t LaidOutText {
    texts
        .iter()
        .find(|t| t.text == label)
        .unwrap_or_else(|| panic!("no text {label:?} in the laid-out tree: {texts:#?}"))
}

/// How `label` lays out on its own with no width limit: one line, at its
/// natural width. Build it the way the view does (same size, same font)
/// so the comparison is like for like.
pub(crate) fn natural<M>(element: Element<'_, M, Theme, iced::Renderer>, label: &str) -> Rectangle {
    let texts = lay_out_texts(element, Size::new(100_000.0, 100_000.0));
    find(&texts, label).bounds
}

pub(crate) fn headless_renderer() -> iced::Renderer {
    iced::Renderer::Secondary(iced_tiny_skia::Renderer::new(renderer::Settings {
        font: Font::DEFAULT,
        text_size: Pixels(16.0),
        ..renderer::Settings::default()
    }))
}

struct CollectTexts(Vec<LaidOutText>);

impl<T> Operation<T> for CollectTexts {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<T>)) {
        operate(self);
    }

    fn text(&mut self, _id: Option<&Id>, bounds: Rectangle, text: &mut dyn Text) {
        self.0.push(LaidOutText {
            text: text.text().into_owned(),
            bounds,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::Widget as _;
    use iced::widget::text;

    #[test]
    fn a_label_reports_its_bounds_and_wraps_when_squeezed() {
        let single = natural::<()>(
            text("Start a fresh preferences file").boxed(),
            "Start a fresh preferences file",
        );
        assert!(single.width > 50.0 && single.height > 0.0, "{single:?}");

        let narrow = lay_out_texts::<()>(
            text("Start a fresh preferences file").boxed(),
            Size::new(single.width / 3.0, 500.0),
        );
        let narrow = find(&narrow, "Start a fresh preferences file").bounds;
        assert!(
            narrow.height > single.height * 1.5,
            "squeezed text wraps: {narrow:?}"
        );
    }
}
