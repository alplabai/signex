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

use iced::advanced::widget::{Id, Operation, Tree};
use iced::advanced::{Layout, layout};
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
    let mut tree = Tree::new(element.as_widget());
    let widget = element.as_widget_mut();
    let node = widget.layout(
        &mut tree,
        &renderer,
        &layout::Limits::new(Size::ZERO, window),
    );
    let mut collect = CollectTexts(Vec::new());
    widget.operate(&mut tree, Layout::new(&node), &renderer, &mut collect);
    collect.0
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

fn headless_renderer() -> iced::Renderer {
    iced::Renderer::Secondary(iced_tiny_skia::Renderer::new(Font::DEFAULT, Pixels(16.0)))
}

struct CollectTexts(Vec<LaidOutText>);

impl<T> Operation<T> for CollectTexts {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<T>)) {
        operate(self);
    }

    fn text(&mut self, _id: Option<&Id>, bounds: Rectangle, text: &str) {
        self.0.push(LaidOutText {
            text: text.to_owned(),
            bounds,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::widget::text;

    #[test]
    fn a_label_reports_its_bounds_and_wraps_when_squeezed() {
        let single = natural::<()>(
            text("Start a fresh preferences file").into(),
            "Start a fresh preferences file",
        );
        assert!(single.width > 50.0 && single.height > 0.0, "{single:?}");

        let narrow = lay_out_texts::<()>(
            text("Start a fresh preferences file").into(),
            Size::new(single.width / 3.0, 500.0),
        );
        let narrow = find(&narrow, "Start a fresh preferences file").bounds;
        assert!(
            narrow.height > single.height * 1.5,
            "squeezed text wraps: {narrow:?}"
        );
    }
}
