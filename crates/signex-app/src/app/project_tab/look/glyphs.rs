//! The Project tab's line glyphs: 24 x 24 stroke drawings, parsed once and
//! tinted through the svg style colour, so one source serves every theme.

use std::sync::LazyLock;

use iced::widget::svg;
use iced::{Color, Element, Length, Theme, Widget as _};

use crate::app::Message;

/// A glyph of the Project tab look.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app::project_tab) enum Glyph {
    Up,
    Down,
    Lock,
    Warn,
    Info,
    Library,
    Plus,
    Search,
    ChevronDown,
    ChevronRight,
    Close,
    Check,
    History,
    Restore,
    Play,
    Trash,
}

impl Glyph {
    /// Every glyph, in declaration order.
    pub(in crate::app::project_tab) const ALL: [Glyph; 16] = [
        Glyph::Up,
        Glyph::Down,
        Glyph::Lock,
        Glyph::Warn,
        Glyph::Info,
        Glyph::Library,
        Glyph::Plus,
        Glyph::Search,
        Glyph::ChevronDown,
        Glyph::ChevronRight,
        Glyph::Close,
        Glyph::Check,
        Glyph::History,
        Glyph::Restore,
        Glyph::Play,
        Glyph::Trash,
    ];

    /// Position in [`Glyph::ALL`] and in the parsed handles.
    pub(in crate::app::project_tab) const fn index(self) -> usize {
        self as usize
    }

    /// The SVG source, drawn in black and tinted when shown.
    pub(in crate::app::project_tab) fn source(self) -> &'static [u8] {
        match self {
            Glyph::Up => UP,
            Glyph::Down => DOWN,
            Glyph::Lock => LOCK,
            Glyph::Warn => WARN,
            Glyph::Info => INFO,
            Glyph::Library => LIBRARY,
            Glyph::Plus => PLUS,
            Glyph::Search => SEARCH,
            Glyph::ChevronDown => CHEVRON_DOWN,
            Glyph::ChevronRight => CHEVRON_RIGHT,
            Glyph::Close => CLOSE,
            Glyph::Check => CHECK,
            Glyph::History => HISTORY,
            Glyph::Restore => RESTORE,
            Glyph::Play => PLAY,
            Glyph::Trash => TRASH,
        }
    }
}

const UP: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 19V5M6 11l6-6 6 6"/></svg>"##;
const DOWN: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 5v14M6 13l6 6 6-6"/></svg>"##;
const LOCK: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="5" y="11" width="14" height="10" rx="2"/><path d="M8 11V8a4 4 0 0 1 8 0v3"/></svg>"##;
const WARN: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 4l9 16H3z"/><path d="M12 10v4M12 17h.01"/></svg>"##;
const INFO: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round"><circle cx="12" cy="12" r="9"/><path d="M12 11v6M12 7.5h.01"/></svg>"##;
const LIBRARY: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M5 4h4v16H5zM11 4h4v16h-4z"/><path d="M17 6l3-1 2 14-3 1z"/></svg>"##;
const PLUS: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round"><path d="M12 5v14M5 12h14"/></svg>"##;
const SEARCH: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round"><circle cx="11" cy="11" r="6.5"/><path d="M16 16l4.5 4.5"/></svg>"##;
const CHEVRON_DOWN: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M7 10l5 5 5-5"/></svg>"##;
const CHEVRON_RIGHT: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10 7l5 5-5 5"/></svg>"##;
const CLOSE: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round"><path d="M6 6l12 12M18 6L6 18"/></svg>"##;
const CHECK: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12.5l4.5 4.5L19 7"/></svg>"##;
const HISTORY: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 12a8 8 0 1 0 2.3-5.6"/><path d="M4 4v4h4M12 8v4l3 2"/></svg>"##;
const RESTORE: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 14L4 9l5-5"/><path d="M4 9h10a6 6 0 0 1 0 12h-3"/></svg>"##;
const PLAY: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linejoin="round"><path d="M8 5v14l11-7z"/></svg>"##;
const TRASH: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3"/></svg>"##;

/// The glyphs parsed once, in [`Glyph::ALL`] order.
static HANDLES: LazyLock<[svg::Handle; 16]> =
    LazyLock::new(|| Glyph::ALL.map(|g| svg::Handle::from_memory(g.source())));

/// `g` drawn `size` px square in `color`.
pub(in crate::app::project_tab) fn glyph<'a>(
    g: Glyph,
    size: f32,
    color: Color,
) -> Element<'a, Message> {
    svg(HANDLES[g.index()].clone())
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .style(move |_: &Theme, _| svg::Style {
            color: Some(color),
            ..Default::default()
        })
        .boxed()
}
