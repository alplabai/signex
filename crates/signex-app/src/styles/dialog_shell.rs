//! The dialog shell's leading block: an icon tile chosen by the dialog's
//! kind on the matching soft fill, then a title line and secondary text.
//! Graphite only; other themes keep their message line (the header there
//! already names the dialog).

use std::sync::LazyLock;

use iced::widget::{Row, column, container, svg};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_types::theme::ThemeTokens;
use signex_widgets::{raw, text};

use super::{PillTone, Radii, Spacing, TypeScale, chrome_px, pill_colors, ti, tokens};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// What a dialog is about; picks its icon tile and the tile's fill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogIcon {
    /// A neutral notice or question.
    Info,
    /// Something will be lost or skipped unless the user acts.
    Warn,
    /// An operation failed.
    Error,
    /// A library or a component in one.
    Library,
    /// A document on disk (sheet, project, export).
    File,
    /// Options or preferences.
    Settings,
    /// A copy is made (Copy to Library).
    Copy,
    /// Something is deleted.
    Trash,
    /// A file or folder is not where it was expected.
    Folder,
    /// The version-control history of a library.
    Git,
    /// A file leaves the project (Remove from Project).
    RemoveFile,
    /// A project is put under version control (Enable Version Control).
    VersionControl,
    /// An existing schematic sheet.
    Sheet,
    /// A schematic sheet about to be made.
    NewSheet,
}

impl DialogIcon {
    /// Every kind, in [`Self::index`] order.
    pub const ALL: [Self; 14] = [
        Self::Info,
        Self::Warn,
        Self::Error,
        Self::Library,
        Self::File,
        Self::Settings,
        Self::Copy,
        Self::Trash,
        Self::Folder,
        Self::Git,
        Self::RemoveFile,
        Self::VersionControl,
        Self::Sheet,
        Self::NewSheet,
    ];

    /// Tone of the tile: its soft fill and its ink.
    pub fn tone(self) -> PillTone {
        match self {
            Self::Info | Self::File | Self::Copy => PillTone::Info,
            Self::Warn | Self::Git | Self::RemoveFile => PillTone::Warn,
            Self::Error | Self::Trash | Self::Folder => PillTone::Err,
            Self::Library | Self::Settings | Self::VersionControl | Self::NewSheet => {
                PillTone::Accent
            }
            Self::Sheet => PillTone::Neutral,
        }
    }

    /// The glyph alone, `size` px square, drawn in `color`: for a list row
    /// or a banner, where no tile sits behind it.
    pub fn glyph_element<'a, M: 'a>(self, size: f32, color: iced::Color) -> Element<'a, M> {
        tinted_glyph(self.glyph_handle(), size, color)
    }

    /// The glyph without its tile, for a callout that draws it inline.
    pub fn glyph_handle(self) -> svg::Handle {
        GLYPHS[self.index()].clone()
    }

    /// The 24 x 24 stroke glyph, drawn in the tile's ink.
    fn index(self) -> usize {
        match self {
            Self::Info => 0,
            Self::Warn => 1,
            Self::Error => 2,
            Self::Library => 3,
            Self::File => 4,
            Self::Settings => 5,
            Self::Copy => 6,
            Self::Trash => 7,
            Self::Folder => 8,
            Self::Git => 9,
            Self::RemoveFile => 10,
            Self::VersionControl => 11,
            Self::Sheet => 12,
            Self::NewSheet => 13,
        }
    }

    fn glyph(self) -> &'static [u8] {
        match self {
            Self::Info => GLYPH_INFO,
            Self::Warn => GLYPH_WARN,
            Self::Error => GLYPH_ERROR,
            Self::Library => GLYPH_LIBRARY,
            Self::File => GLYPH_FILE,
            Self::Settings => GLYPH_SETTINGS,
            Self::Copy => GLYPH_COPY,
            Self::Trash => GLYPH_TRASH,
            Self::Folder => GLYPH_FOLDER,
            Self::Git => GLYPH_GIT,
            Self::RemoveFile => GLYPH_REMOVE_FILE,
            Self::VersionControl => GLYPH_VERSION_CONTROL,
            Self::Sheet => GLYPH_SHEET,
            Self::NewSheet => GLYPH_NEW_SHEET,
        }
    }
}

const GLYPH_INFO: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round"><circle cx="12" cy="12" r="9"/><path d="M12 11v5M12 8h.01"/></svg>"##;
const GLYPH_WARN: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3l10 18H2z"/><path d="M12 10v5M12 18h.01"/></svg>"##;
const GLYPH_ERROR: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round"><circle cx="12" cy="12" r="9"/><path d="M9 9l6 6M15 9l-6 6"/></svg>"##;
const GLYPH_LIBRARY: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 4h4v16H4zM10 4h4v16h-4z"/><path d="M16 5l3.5-1 3 15.5-3.5 1z"/></svg>"##;
const GLYPH_FILE: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M6 3h8l4 4v14H6z"/><path d="M14 3v4h4M9 13h6M9 17h6"/></svg>"##;
const GLYPH_SETTINGS: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round"><path d="M4 7h10M18 7h2M4 17h4M12 17h8"/><circle cx="16" cy="7" r="2"/><circle cx="10" cy="17" r="2"/></svg>"##;
const GLYPH_COPY: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="7.5" y="7.5" width="12.75" height="12.75" rx="2.25"/><path d="M4.5 15.75V5.25a1.5 1.5 0 0 1 1.5-1.5h9.75"/></svg>"##;
const GLYPH_TRASH: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4.5 6.75h15M9.75 6.75V4.5h4.5v2.25M6.75 6.75l1.05 12.75h8.4l1.05-12.75"/></svg>"##;
const GLYPH_FOLDER: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6.75v12h18V8.25h-9L9.75 5.25H3z"/></svg>"##;
const GLYPH_GIT: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round"><circle cx="6.75" cy="6" r="2.4"/><circle cx="6.75" cy="18" r="2.4"/><circle cx="17.25" cy="9.75" r="2.4"/><path d="M6.75 8.4v7.2M17.25 12.15c0 3-4.5 3-9 4.5"/></svg>"##;

const GLYPH_VERSION_CONTROL: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="6" cy="5" r="2"/><circle cx="6" cy="19" r="2"/><circle cx="18" cy="7" r="2"/><path d="M6 7v10M18 9c0 5-12 3-12 8"/></svg>"##;
const GLYPH_REMOVE_FILE: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13"/><path d="M10 11v6M14 11v6"/></svg>"##;
const GLYPH_SHEET: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M5 3h14v18H5z"/><path d="M8 8h3v3H8zM11 9.5h5M8 15h8"/></svg>"##;
const GLYPH_NEW_SHEET: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M13 21H5V3h14v9"/><path d="M8 8h3v3H8zM11 9.5h5M18 15v6M15 18h6"/></svg>"##;

/// The glyphs parsed once, in [`DialogIcon::index`] order.
static GLYPHS: LazyLock<[svg::Handle; 14]> =
    LazyLock::new(|| DialogIcon::ALL.map(|icon| svg::Handle::from_memory(icon.glyph())));

/// Side of the icon tile (28 px at the 11 px base).
pub const DIALOG_ICON_TILE: f32 = chrome_px(28.0 * 11.0 / 12.0);

/// The icon tile: `icon`'s glyph in its ink on its soft fill, card radius.
pub fn dialog_icon_tile<'a, M: 'a>(icon: DialogIcon, tokens: &ThemeTokens) -> Element<'a, M> {
    let (fill, ink) = pill_colors(tokens, icon.tone());
    let glyph = DIALOG_ICON_TILE * 15.0 / 28.0;
    container(
        svg(GLYPHS[icon.index()].clone())
            .width(Length::Fixed(glyph))
            .height(Length::Fixed(glyph))
            .style(move |_: &Theme, _| svg::Style {
                color: Some(ink),
                ..Default::default()
            }),
    )
    .width(Length::Fixed(DIALOG_ICON_TILE))
    .height(Length::Fixed(DIALOG_ICON_TILE))
    .center_x(Length::Fixed(DIALOG_ICON_TILE))
    .center_y(Length::Fixed(DIALOG_ICON_TILE))
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            radius: Radii::GRAPHITE.button.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

/// `icon`'s bare glyph, `side` px square, in its tone's ink and with no
/// tile: the severity mark of a dialog header.
pub fn dialog_glyph<'a, M: 'a>(
    icon: DialogIcon,
    side: f32,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let (_, ink) = pill_colors(tokens, icon.tone());
    tinted_glyph(GLYPHS[icon.index()].clone(), side, ink)
}

/// A monochrome SVG glyph, `side` px square, drawn in `ink`. The one way
/// the dialogs, Help cards and hover cards draw a glyph.
pub fn tinted_glyph<'a, M: 'a>(handle: svg::Handle, side: f32, ink: iced::Color) -> Element<'a, M> {
    svg(handle)
        .width(Length::Fixed(side))
        .height(Length::Fixed(side))
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        })
        .boxed()
}

/// Height the Graphite intro adds over the plain message line it replaces:
/// the title line and the gap under it (a one-line title).
pub const DIALOG_INTRO_EXTRA: f32 = TS.title * super::LINE_HEIGHT + TITLE_GAP;

/// Gap between the title line and the secondary text.
const TITLE_GAP: f32 = SP.xs + 1.0;

/// The semibold UI face of a dialog's title line.
fn title_font() -> iced::Font {
    iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    }
}

/// The leading block of a dialog body: the tile, then `title` with
/// `secondary` under it (left out when empty, so no gap is left).
pub fn dialog_intro<'a, M: 'a>(
    icon: DialogIcon,
    title: impl Into<String>,
    secondary: Option<String>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let ink3 = tokens::resolve(tokens).ink3;
    let secondary = secondary
        .filter(|s| !s.trim().is_empty())
        .map(|s| text(raw(s)).size(TS.body).color(ink3).boxed());
    dialog_intro_with(icon, title, secondary, tokens)
}

/// [`dialog_intro`] with any element as the secondary line (a mono path, a
/// line with a link). `None` leaves no gap under the title.
pub fn dialog_intro_with<'a, M: 'a>(
    icon: DialogIcon,
    title: impl Into<String>,
    secondary: Option<Element<'a, M>>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let title = title.into();
    let mut lines = column![
        text(raw(title))
            .size(TS.title)
            .font(title_font())
            .color(ti(tokens.text))
            .boxed()
    ]
    .spacing(TITLE_GAP);
    if let Some(s) = secondary {
        lines = lines.push(s);
    }
    let mut line: Row<Element<'a, M>> = Row::new().spacing(SP.m + 2.0).align_y(Alignment::Start);
    line = line.push(dialog_icon_tile(icon, tokens));
    line = line.push(lines.width(Length::Fill).boxed());
    line.width(Length::Fill).boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::theme::ThemeId;

    const ALL: [DialogIcon; 14] = [
        DialogIcon::Info,
        DialogIcon::Warn,
        DialogIcon::Error,
        DialogIcon::Library,
        DialogIcon::File,
        DialogIcon::Settings,
        DialogIcon::Copy,
        DialogIcon::Trash,
        DialogIcon::Folder,
        DialogIcon::Git,
        DialogIcon::RemoveFile,
        DialogIcon::VersionControl,
        DialogIcon::Sheet,
        DialogIcon::NewSheet,
    ];

    /// Each kind sits at its own place in the parsed glyph table.
    #[test]
    fn every_kind_has_its_own_slot_in_the_glyph_table() {
        for (i, icon) in ALL.iter().enumerate() {
            assert_eq!(icon.index(), i, "{icon:?}");
        }
        assert_eq!(GLYPHS.len(), ALL.len());
    }

    /// The four kinds the Library confirms added use the tones their
    /// meaning asks for: a copy informs, a delete and a missing folder are
    /// errors, a missing history warns.
    #[test]
    fn the_library_confirm_kinds_use_their_tones() {
        assert_eq!(DialogIcon::Copy.tone(), PillTone::Info);
        assert_eq!(DialogIcon::Trash.tone(), PillTone::Err);
        assert_eq!(DialogIcon::Folder.tone(), PillTone::Err);
        assert_eq!(DialogIcon::Git.tone(), PillTone::Warn);
        for id in ThemeId::BUILTINS {
            let t = signex_types::theme::theme_tokens(*id);
            for icon in ALL {
                let _: Element<'_, ()> = icon.glyph_element(14.0, ti(t.text));
            }
        }
    }

    /// The parsed-glyph table is indexed by `index`: every kind sits at its
    /// own slot, so no tile paints another kind's glyph.
    #[test]
    fn every_kind_sits_at_its_index_in_the_glyph_table() {
        for (slot, icon) in DialogIcon::ALL.iter().enumerate() {
            assert_eq!(icon.index(), slot, "{icon:?}");
        }
        for icon in ALL {
            assert!(DialogIcon::ALL.contains(&icon), "{icon:?}");
        }
    }

    /// Version Control and New Sheet are accent tiles, Remove File a warn
    /// tile and Sheet the neutral sunken tile with the secondary ink.
    #[test]
    fn the_project_dialog_kinds_take_their_tones() {
        assert_eq!(DialogIcon::VersionControl.tone(), PillTone::Accent);
        assert_eq!(DialogIcon::NewSheet.tone(), PillTone::Accent);
        assert_eq!(DialogIcon::RemoveFile.tone(), PillTone::Warn);
        assert_eq!(DialogIcon::Sheet.tone(), PillTone::Neutral);
        for id in ThemeId::BUILTINS {
            let t = signex_types::theme::theme_tokens(*id);
            let r = tokens::resolve(&t);
            assert_eq!(
                pill_colors(&t, DialogIcon::Sheet.tone()),
                (r.sunken, r.ink3)
            );
        }
    }

    #[test]
    fn the_intro_takes_any_element_as_its_secondary_line() {
        for id in ThemeId::BUILTINS {
            let t = signex_types::theme::theme_tokens(*id);
            for icon in ALL {
                let path = text(signex_i18n::UserText::new(
                    "D:/work/power.snxsch".to_string(),
                ))
                .size(TS.caption)
                .boxed();
                let _: Element<'_, ()> = dialog_intro_with(icon, "Title", Some(path), &t);
                let _: Element<'_, ()> = dialog_intro_with(icon, "Title", None, &t);
            }
        }
    }

    /// Every kind draws its own glyph on the soft fill of its tone, and the
    /// three status kinds use the three status tones.
    #[test]
    fn each_dialog_kind_has_its_own_glyph_and_the_matching_soft_fill() {
        for (i, a) in ALL.iter().enumerate() {
            for b in &ALL[i + 1..] {
                assert_ne!(a.glyph(), b.glyph(), "{a:?} / {b:?}");
            }
        }
        assert_eq!(DialogIcon::Warn.tone(), PillTone::Warn);
        assert_eq!(DialogIcon::Error.tone(), PillTone::Err);
        assert_eq!(DialogIcon::Info.tone(), PillTone::Info);
        assert_eq!(DialogIcon::Library.tone(), PillTone::Accent);
        for id in ThemeId::BUILTINS {
            let t = signex_types::theme::theme_tokens(*id);
            let r = tokens::resolve(&t);
            assert_eq!(pill_colors(&t, DialogIcon::Warn.tone()).0, r.warn_soft);
            assert_eq!(pill_colors(&t, DialogIcon::Error.tone()).0, r.err_soft);
            assert_eq!(pill_colors(&t, DialogIcon::Info.tone()).0, r.info_soft);
            assert_eq!(pill_colors(&t, DialogIcon::Library.tone()).0, r.accent_soft);
        }
    }

    /// Every glyph is a parseable SVG (a broken one paints an empty tile).
    #[test]
    fn every_dialog_glyph_is_well_formed_svg() {
        for icon in ALL {
            let src = std::str::from_utf8(icon.glyph()).expect("utf-8");
            assert!(
                src.starts_with("<svg") && src.ends_with("</svg>"),
                "{icon:?}"
            );
            assert_eq!(
                src.matches('<').count(),
                src.matches('>').count(),
                "{icon:?}"
            );
        }
    }

    #[test]
    fn the_intro_builds_for_every_theme_with_and_without_secondary_text() {
        for id in ThemeId::BUILTINS {
            let t = signex_types::theme::theme_tokens(*id);
            for icon in ALL {
                let _: Element<'_, ()> = dialog_intro(icon, "Title", None, &t);
                let _: Element<'_, ()> = dialog_intro(icon, "Title", Some("why".into()), &t);
                let _: Element<'_, ()> = dialog_intro(icon, "Title", Some("  ".into()), &t);
            }
        }
    }
}
