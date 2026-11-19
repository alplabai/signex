//! The Rules tab's line icons (Run DRC, find, add, the info banner, the
//! Table and Text switch), drawn in one ink.

use iced::widget::svg;
use iced::{Color, Element, Theme, Widget as _};

const PLAY: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><path d="M5 3.5v9l7.5-4.5z" fill="currentColor"/></svg>"#;
const SEARCH: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><circle cx="7" cy="7" r="4"/><path d="M10 10l3.5 3.5"/></svg>"#;
const PLUS: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="M8 3v10M3 8h10"/></svg>"#;
const INFO: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"><circle cx="8" cy="8" r="6"/><path d="M8 7.2v3.6M8 5.1v.1"/></svg>"#;
const TARGET: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"><circle cx="8" cy="8" r="4.5"/><path d="M8 1.5v3M8 11.5v3M1.5 8h3M11.5 8h3"/></svg>"#;

/// Which icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Icon {
    Play,
    Search,
    Plus,
    Info,
    Target,
}

impl Icon {
    fn bytes(self) -> &'static [u8] {
        match self {
            Icon::Play => PLAY,
            Icon::Search => SEARCH,
            Icon::Plus => PLUS,
            Icon::Info => INFO,
            Icon::Target => TARGET,
        }
    }
}

/// `icon` `side` px square in `ink`.
pub(super) fn icon<'a, M: 'a>(icon: Icon, side: f32, ink: Color) -> Element<'a, M> {
    svg(svg::Handle::from_memory(icon.bytes()))
        .width(side)
        .height(side)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..svg::Style::default()
        })
        .boxed()
}
