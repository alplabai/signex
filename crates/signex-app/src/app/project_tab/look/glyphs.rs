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
    Folder,
    Tag,
    File,
    Layers,
    Shield,
    Sliders,
    Gear,
    Branch,
    Target,
}

impl Glyph {
    /// Every glyph, in declaration order.
    pub(in crate::app::project_tab) const ALL: [Glyph; 25] = [
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
        Glyph::Folder,
        Glyph::Tag,
        Glyph::File,
        Glyph::Layers,
        Glyph::Shield,
        Glyph::Sliders,
        Glyph::Gear,
        Glyph::Branch,
        Glyph::Target,
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
            Glyph::Folder => FOLDER,
            Glyph::Tag => TAG,
            Glyph::File => FILE,
            Glyph::Layers => LAYERS,
            Glyph::Shield => SHIELD,
            Glyph::Sliders => SLIDERS,
            Glyph::Gear => GEAR,
            Glyph::Branch => BRANCH,
            Glyph::Target => TARGET,
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
const FOLDER: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/></svg>"##;
const TAG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 4h8l8 8-8 8-8-8z"/><path d="M8.5 8.5h.01"/></svg>"##;
const FILE: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M6 3h8l4 4v14H6z"/><path d="M14 3v4h4M9 13h6M9 17h6"/></svg>"##;
const LAYERS: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 4l9 5-9 5-9-5z"/><path d="M3 14l9 5 9-5"/></svg>"##;
const SHIELD: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3l8 3v6c0 4.5-3.2 7.6-8 9-4.8-1.4-8-4.5-8-9V6z"/><path d="M9 12l2 2 4-4"/></svg>"##;
const SLIDERS: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round"><path d="M4 7h9M17 7h3M4 17h3M11 17h9"/><circle cx="15" cy="7" r="2"/><circle cx="9" cy="17" r="2"/></svg>"##;
const GEAR: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round"><circle cx="12" cy="12" r="3.5"/><path d="M12 3v3M12 18v3M3 12h3M18 12h3M5.6 5.6l2.2 2.2M16.2 16.2l2.2 2.2M5.6 18.4l2.2-2.2M16.2 7.8l2.2-2.2"/></svg>"##;
const BRANCH: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="7" cy="5" r="2"/><circle cx="7" cy="19" r="2"/><circle cx="17" cy="7" r="2"/><path d="M7 7v10M17 9c0 4-10 2-10 6"/></svg>"##;
const TARGET: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round"><circle cx="12" cy="12" r="9"/><circle cx="12" cy="12" r="4"/><path d="M12 12h.01"/></svg>"##;

/// The glyphs parsed once, in [`Glyph::ALL`] order.
static HANDLES: LazyLock<[svg::Handle; 25]> =
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
