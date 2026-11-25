//! Lay out a widget tree in a test and read back where its labels landed
//! (#623).
//!
//! The regression suite drives handlers through `Signex::update` and never
//! builds a laid-out widget tree, so no test could see a size or a position:
//! a button squeezed to a sliver with its label wrapped onto five lines
//! (#604) passed every gate. This lays an [`Element`] out against a window
//! size with a headless vello_cpu renderer, so text is measured with real
//! fonts, and reports the bounds of every text widget in it.
//!
//! The probe measures with the app's own fonts: the faces `main` registers
//! ([`crate::fonts::bundled::registered`]) are loaded once into iced's
//! global font system, and the default font is the UI font at its default
//! family (Inter), as `main` sets it when no other is saved. Widths do not
//! depend on the machine's fonts, so a layout that passes here passes on
//! CI. Still assert on relations rather than pixel counts: a label sits on
//! one line, a button is at least as wide as its label.

use std::sync::OnceLock;

use iced::advanced::widget::operation::{Text, TextInput};
use iced::advanced::widget::{Id, Operation, Tree};
use iced::advanced::{Layout, Widget as _, layout, renderer};
use iced::{Element, Pixels, Rectangle, Size, Theme};

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
    element: Element<'_, M, Theme, iced::Renderer>,
    window: Size,
) -> Vec<LaidOutText> {
    let mut collect = CollectTexts(Vec::new());
    operate_laid_out(element, window, &mut collect);
    collect.0
}

/// Every text input of `element` laid out in a window of `window`, in tree
/// order, with what it shows: its value, or its placeholder while empty.
/// [`lay_out_texts`] does not list them: an input is not a text widget.
pub(crate) fn lay_out_inputs<M>(
    element: Element<'_, M, Theme, iced::Renderer>,
    window: Size,
) -> Vec<LaidOutText> {
    let mut collect = CollectInputs(Vec::new());
    operate_laid_out(element, window, &mut collect);
    collect.0
}

/// Lay `element` out in a window of `window` and run `operation` over it.
fn operate_laid_out<M>(
    mut element: Element<'_, M, Theme, iced::Renderer>,
    window: Size,
    operation: &mut dyn Operation,
) {
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
    element.operate(
        &mut tree,
        Layout::new(size),
        &Rectangle::with_size(size),
        &renderer,
        operation,
    );
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

/// The messages left clicks on the centres of `labels` publish, in order,
/// over one laid-out `element`, so a widget's state carries from one click
/// to the next (a control a click left open takes the following press).
pub(crate) fn messages_on_clicks<M>(
    mut element: Element<'_, M, Theme, iced::Renderer>,
    window: Size,
    labels: &[&str],
) -> Vec<M> {
    use iced::advanced::mouse;
    let renderer = headless_renderer();
    let mut tree = Tree::new(&element);
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
    let mut published = Vec::new();
    for label in labels {
        let at = find(&collect.0, label).bounds.center();
        let click = [
            mouse::Event::CursorMoved { position: at },
            mouse::Event::ButtonPressed(mouse::Button::Left),
            mouse::Event::ButtonReleased(mouse::Button::Left),
        ]
        .map(iced::Event::Mouse);
        published.extend(deliver(
            &mut element,
            &mut tree,
            &renderer,
            window,
            at,
            &click,
        ));
    }
    published
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

/// The messages the cursor publishes moving through `points` in order over
/// one laid-out `element`, so a widget's state carries from one point to
/// the next (a cursor that was inside and now is not).
pub(crate) fn messages_for_moves<M>(
    mut element: Element<'_, M, Theme, iced::Renderer>,
    window: Size,
    points: &[iced::Point],
) -> Vec<M> {
    let renderer = headless_renderer();
    let mut tree = Tree::new(&element);
    tree.diff(&mut element);
    element.layout(
        &mut tree,
        &renderer,
        &layout::Limits::new(Size::ZERO, window),
    );
    points
        .iter()
        .flat_map(|&at| {
            let moved = iced::Event::Mouse(iced::mouse::Event::CursorMoved { position: at });
            deliver(&mut element, &mut tree, &renderer, window, at, &[moved])
        })
        .collect()
}

/// What the cursor finds at one point: the texts of the overlays a hover
/// there opens (its tooltips) and the messages a left click there publishes.
pub(crate) struct PointProbe<M> {
    pub tips: Vec<String>,
    pub clicked: Vec<M>,
}

/// Hover, then click, at each of `points` in turn over one laid-out
/// `element`. Moving on closes the tooltip the last hover opened, as the
/// cursor does in the app.
pub(crate) fn probe_points<M>(
    mut element: Element<'_, M, Theme, iced::Renderer>,
    window: Size,
    points: &[iced::Point],
) -> Vec<PointProbe<M>> {
    use iced::advanced::Overlay as _;
    use iced::advanced::mouse;
    let renderer = headless_renderer();
    let mut tree = Tree::new(&element);
    tree.diff(&mut element);
    element.layout(
        &mut tree,
        &renderer,
        &layout::Limits::new(Size::ZERO, window),
    );
    let viewport = Rectangle::with_size(window);
    let mut probes = Vec::with_capacity(points.len());
    for &at in points {
        let moved = iced::Event::Mouse(mouse::Event::CursorMoved { position: at });
        // A hover publishes nothing the probe asks about.
        let _ = deliver(&mut element, &mut tree, &renderer, window, at, &[moved]);
        let mut tips = CollectTexts(Vec::new());
        let layout = Layout::new(tree.size);
        let overlays = element.overlay(
            &mut tree,
            layout,
            &renderer,
            &viewport,
            iced::Vector::ZERO,
            window,
        );
        for mut overlay in overlays {
            overlay.as_overlay_mut().operate(&renderer, &mut tips);
        }
        let click = [
            mouse::Event::ButtonPressed(mouse::Button::Left),
            mouse::Event::ButtonReleased(mouse::Button::Left),
        ]
        .map(iced::Event::Mouse);
        let clicked = deliver(&mut element, &mut tree, &renderer, window, at, &click);
        probes.push(PointProbe {
            tips: tips.0.into_iter().map(|text| text.text).collect(),
            clicked,
        });
    }
    probes
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
    // As the runtime does before every frame: a redraw sets each widget's
    // status (a button that never saw one draws as disabled).
    {
        use iced::advanced::shell::{Bus, Waker};
        let mut bus = Bus::new();
        let mut shell =
            iced::advanced::Shell::new(&iced::window::Headless, Waker::noop(), &mut bus);
        let redraw = iced::Event::Window(iced::window::Event::RedrawRequested(
            std::time::Instant::now(),
        ));
        element.update(
            &mut tree,
            &redraw,
            Layout::new(size),
            iced::advanced::mouse::Cursor::Unavailable,
            &renderer,
            &mut shell,
            &Rectangle::with_size(window),
        );
    }
    // As the compositor does before every frame.
    iced::advanced::Renderer::reset(&mut renderer, Rectangle::with_size(window));
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

/// Load the app's faces into iced's global font system, which the vello_cpu
/// renderer draws with and every paragraph is measured with. Once per test
/// process.
pub(crate) fn load_app_fonts() {
    static LOADED: OnceLock<()> = OnceLock::new();
    LOADED.get_or_init(|| {
        let mut fonts = iced::advanced::graphics::text::font_system()
            .write()
            .expect("font system");
        for face in crate::fonts::bundled::registered() {
            fonts.load_font(std::borrow::Cow::Borrowed(face));
        }
    });
}

/// The headless software renderer, with the app's fonts loaded and the UI
/// font as its default.
pub(crate) fn headless_renderer() -> iced::Renderer {
    load_app_fonts();
    iced::Renderer::Secondary(iced_vello_cpu::Renderer::new(renderer::Settings {
        font: signex_widgets::ui_font::regular(),
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

struct CollectInputs(Vec<LaidOutText>);

impl<T> Operation<T> for CollectInputs {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<T>)) {
        operate(self);
    }

    fn text_input(&mut self, _id: Option<&Id>, bounds: Rectangle, state: &mut dyn TextInput) {
        self.0.push(LaidOutText {
            text: state.text().into_owned(),
            bounds,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::Widget as _;
    use signex_widgets::{raw, text};

    #[test]
    fn a_label_reports_its_bounds_and_wraps_when_squeezed() {
        let single = natural::<()>(
            text(raw("Start a fresh preferences file")).boxed(),
            "Start a fresh preferences file",
        );
        assert!(single.width > 50.0 && single.height > 0.0, "{single:?}");

        let narrow = lay_out_texts::<()>(
            text(raw("Start a fresh preferences file")).boxed(),
            Size::new(single.width / 3.0, 500.0),
        );
        let narrow = find(&narrow, "Start a fresh preferences file").bounds;
        assert!(
            narrow.height > single.height * 1.5,
            "squeezed text wraps: {narrow:?}"
        );
    }
}
