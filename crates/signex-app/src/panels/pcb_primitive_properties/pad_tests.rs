//! The Pad panel's read-only Layer (mockup row 97): sunken, with a round
//! layer dot before the name.

use iced::Size;
use signex_types::pcb_layer::{CopperLayer, PcbLayer};

use super::layer_value;
use crate::app::Signex;
use crate::layout_probe::{draw_pixels, find, lay_out_inputs, lay_out_texts};
use crate::panels::form_look::FormLook;
use crate::panels::pcb_primitive_properties::rows::Look;
use crate::pcb_canvas::layer_style;

const WINDOW: Size = Size::new(160.0, 40.0);
const NAME: &str = "Top Layer";

/// The RGB of the pixel at (`x`, `y`) of an RGBA image `WINDOW` wide.
fn pixel(pixels: &[u8], x: f32, y: f32) -> [i32; 3] {
    let at = (y as usize * WINDOW.width as usize + x as usize) * 4;
    [
        pixels[at].into(),
        pixels[at + 1].into(),
        pixels[at + 2].into(),
    ]
}

fn rgb(c: iced::Color) -> [i32; 3] {
    [c.r, c.g, c.b].map(|v| (v * 255.0).round() as i32)
}

fn assert_close(got: [i32; 3], want: [i32; 3], what: &str) {
    let is_close = got.iter().zip(want).all(|(g, w)| (g - w).abs() <= 3);
    assert!(is_close, "{what}: {got:?} is not {want:?}");
}

/// The texts of the tooltips `element` opens with the pointer at `at`.
fn tooltip_texts(
    mut element: iced::Element<'_, crate::panels::PanelMsg>,
    at: iced::Point,
) -> Vec<String> {
    use iced::Rectangle;
    use iced::advanced::overlay::Overlay as _;
    use iced::advanced::shell::{Bus, Waker};
    use iced::advanced::widget::operation::Text;
    use iced::advanced::widget::{Id, Operation, Tree};
    use iced::advanced::{Layout, Shell, Widget as _, layout, mouse};

    struct Texts(Vec<String>);
    impl<T> Operation<T> for Texts {
        fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<T>)) {
            operate(self);
        }
        fn text(&mut self, _id: Option<&Id>, _bounds: Rectangle, text: &mut dyn Text) {
            self.0.push(text.text().into_owned());
        }
    }

    let renderer = crate::layout_probe::headless_renderer();
    let mut tree = Tree::new(&element);
    tree.diff(&mut element);
    element.layout(
        &mut tree,
        &renderer,
        &layout::Limits::new(Size::ZERO, WINDOW),
    );
    let layout = Layout::new(tree.size);
    let viewport = Rectangle::with_size(WINDOW);
    let mut bus = Bus::new();
    let moved = iced::Event::Mouse(mouse::Event::CursorMoved { position: at });
    element.update(
        &mut tree,
        &moved,
        layout,
        mouse::Cursor::Available(at),
        &renderer,
        &mut Shell::new(&iced::window::Headless, Waker::noop(), &mut bus),
        &viewport,
    );
    let mut texts = Texts(Vec::new());
    for mut overlay in element.overlay(
        &mut tree,
        layout,
        &renderer,
        &viewport,
        iced::Vector::ZERO,
        WINDOW,
    ) {
        overlay.as_overlay_mut().operate(&renderer, &mut texts);
    }
    texts
        .0
        .into_iter()
        .map(|t| t.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

/// Mockup row 97 (#2808): hovering the read-only Layer says why it takes no
/// change, and nothing says it with the pointer away.
#[test]
fn the_layer_says_moving_a_pad_is_not_supported_here_yet() {
    let (app, _boot) = Signex::new();
    let ctx = app.document_state.panel_ctx.clone();
    let look = Look::of(&ctx);
    let shown = (NAME.to_string(), Some(PcbLayer::Copper(CopperLayer::Top)));
    let texts: Vec<_> = lay_out_texts(layer_value(look, Some(&shown)), WINDOW)
        .into_iter()
        .map(|t| crate::layout_probe::LaidOutText {
            text: t.text.replace(['\u{2068}', '\u{2069}'], ""),
            ..t
        })
        .collect();
    let name = find(&texts, NAME).bounds;
    let why = signex_i18n::t!(ctx.i18n, signex_i18n::ids::PCB_PRIM_PAD_LAYER_UNSUPPORTED)
        .into_string()
        .replace(['\u{2068}', '\u{2069}'], "");
    let hovered = tooltip_texts(layer_value(look, Some(&shown)), name.center());
    assert_eq!(hovered, [why], "the tooltip on the Layer");
    let away = iced::Point::new(-1.0, -1.0);
    let quiet = tooltip_texts(layer_value(look, Some(&shown)), away);
    assert!(quiet.is_empty(), "no tooltip away from it: {quiet:?}");
}

#[test]
fn the_layer_is_sunken_with_a_round_dot_and_takes_no_typing() {
    let (app, _boot) = Signex::new();
    let ctx = app.document_state.panel_ctx.clone();
    let look = Look::of(&ctx);
    let layer = PcbLayer::Copper(CopperLayer::Top);
    let shown = (NAME.to_string(), Some(layer));

    assert!(lay_out_inputs(layer_value(look, Some(&shown)), WINDOW).is_empty());
    let texts: Vec<_> = lay_out_texts(layer_value(look, Some(&shown)), WINDOW)
        .into_iter()
        .map(|t| crate::layout_probe::LaidOutText {
            text: t.text.replace(['\u{2068}', '\u{2069}'], ""),
            ..t
        })
        .collect();
    let name = find(&texts, NAME).bounds;
    let pixels = draw_pixels(layer_value(look, Some(&shown)), WINDOW);

    // The fill right of the name is the sunken one the probe's theme gives.
    let sunken = FormLook::of(&iced::Theme::Dark).sunken;
    assert_close(
        pixel(&pixels, name.x + name.width + 6.0, name.center_y()),
        rgb(sunken),
        "the value's fill",
    );
    // The dot sits between the box's edge and the name, in the layer colour.
    let dot = layer_style::layer_colour(layer, look.tokens());
    assert_close(
        pixel(&pixels, name.x / 2.0 + 1.0, name.center_y()),
        rgb(dot),
        "the layer dot",
    );
}
