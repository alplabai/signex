//! The pen glyph after an editable value: it says the value can be typed
//! into (mockup rows 89 and 90).

use std::sync::OnceLock;

use iced::widget::svg;
use iced::{Color, Element, Theme};

/// Side of the glyph, in ems of the text it follows.
pub(crate) const PEN_EM: f32 = 0.9;

const PEN_ICON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 12 12"><path d="M2.5 9.5l1-3 4.5-4.5 2 2-4.5 4.5z" fill="none" stroke="currentColor" stroke-width="1" stroke-linejoin="round"/></svg>"#;

fn pen_handle() -> svg::Handle {
    static HANDLE: OnceLock<svg::Handle> = OnceLock::new();
    HANDLE
        .get_or_init(|| svg::Handle::from_memory(PEN_ICON))
        .clone()
}

/// The pen in `ink`, sized to text of `size`.
pub(crate) fn pen_glyph<'a, M: 'a>(size: f32, ink: Color) -> Element<'a, M> {
    svg(pen_handle())
        .width(PEN_EM * size)
        .height(PEN_EM * size)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..svg::Style::default()
        })
        .into()
}
