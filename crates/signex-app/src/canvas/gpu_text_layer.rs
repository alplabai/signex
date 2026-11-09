//! The rotated and oversize texts of a GPU-mounted sheet, which the glyph
//! atlas cannot draw, on a layer of their own between the content shader and
//! the drag shader. A widget rather than a `canvas`, so the idle layer can be
//! drawn under a translation and a pan moves it instead of rebuilding it
//! (#2128), the same way `surface` draws the CPU content layer.

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{self, Tree};
use iced::advanced::{Widget, renderer};
use iced::{Length, Size};

use super::surface::{SurfaceLayers, draw_layers};
use super::*;

/// The static rotated-text layer. No `update`: it never takes an event.
pub(crate) struct SchematicStaticTextLayer<'a>(SchematicCanvas<'a>);

impl<'a> SchematicStaticTextLayer<'a> {
    pub(crate) fn new(slot: &'a CanvasSlot, prefs: CanvasViewPrefs<'a>) -> Self {
        Self(SchematicCanvas::new(slot, prefs).gpu_overlay())
    }
}

impl widget::Meta for SchematicStaticTextLayer<'_> {}

impl Widget<Message, Theme, Renderer> for SchematicStaticTextLayer<'_> {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(&mut self, tree: &mut Tree, _renderer: &Renderer, limits: &layout::Limits) {
        tree.size = layout::atomic(limits, Length::Fill, Length::Fill);
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        if bounds.width < 1.0 || bounds.height < 1.0 {
            return;
        }
        let layer = self.0.static_text_layer(renderer, bounds);
        let content_shift = (layer.shift != iced::Vector::ZERO).then_some((0, layer.shift));
        draw_layers(
            renderer,
            bounds,
            SurfaceLayers {
                layers: vec![layer.geometry],
                content_shift,
                selection_shift: None,
                background_shift: None,
            },
        );
    }
}
