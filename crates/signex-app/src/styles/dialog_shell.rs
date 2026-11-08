//! The dialog shell's leading block: an icon tile chosen by the dialog's
//! kind on the matching soft fill, then a title line and secondary text.
//! Graphite only; other themes keep their message line (the header there
//! already names the dialog).

use std::sync::LazyLock;

use iced::widget::{Row, column, container, svg, text};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_types::theme::ThemeTokens;

use super::{PillTone, Radii, Spacing, TypeScale, chrome_px, is_graphite, pill_colors, ti, tokens};

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
}

impl DialogIcon {
    /// Tone of the tile: its soft fill and its ink.
    pub fn tone(self) -> PillTone {
        match self {
            Self::Info | Self::File => PillTone::Info,
            Self::Warn => PillTone::Warn,
            Self::Error => PillTone::Err,
            Self::Library | Self::Settings => PillTone::Accent,
        }
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
        }
    }
}

const GLYPH_INFO: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round"><circle cx="12" cy="12" r="9"/><path d="M12 11v5M12 8h.01"/></svg>"##;
const GLYPH_WARN: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3l10 18H2z"/><path d="M12 10v5M12 18h.01"/></svg>"##;
const GLYPH_ERROR: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round"><circle cx="12" cy="12" r="9"/><path d="M9 9l6 6M15 9l-6 6"/></svg>"##;
const GLYPH_LIBRARY: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 4h4v16H4zM10 4h4v16h-4z"/><path d="M16 5l3.5-1 3 15.5-3.5 1z"/></svg>"##;
const GLYPH_FILE: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M6 3h8l4 4v14H6z"/><path d="M14 3v4h4M9 13h6M9 17h6"/></svg>"##;
const GLYPH_SETTINGS: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round"><path d="M4 7h10M18 7h2M4 17h4M12 17h8"/><circle cx="16" cy="7" r="2"/><circle cx="10" cy="17" r="2"/></svg>"##;

/// The six glyphs parsed once, in [`DialogIcon::index`] order.
static GLYPHS: LazyLock<[svg::Handle; 6]> = LazyLock::new(|| {
    [
        DialogIcon::Info,
        DialogIcon::Warn,
        DialogIcon::Error,
        DialogIcon::Library,
        DialogIcon::File,
        DialogIcon::Settings,
    ]
    .map(|icon| svg::Handle::from_memory(icon.glyph()))
});

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
            .style(move |_: &Theme, _| svg::Style { color: Some(ink) }),
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

/// The leading block of a dialog body. Graphite: the tile, then `title`
/// with `secondary` under it (left out when empty, so no gap is left).
/// Other themes: the plain message line they always had, `secondary` when
/// given and `title` otherwise (their header already names the dialog).
pub fn dialog_intro<'a, M: 'a>(
    icon: DialogIcon,
    title: impl Into<String>,
    secondary: Option<String>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let title = title.into();
    let secondary = secondary.filter(|s| !s.trim().is_empty());
    if !is_graphite(tokens) {
        return super::dialog_message(secondary.unwrap_or(title), tokens);
    }
    let r = tokens::resolve(tokens);
    let mut lines = column![
        text(title)
            .size(TS.title)
            .font(title_font())
            .color(ti(tokens.text))
            .boxed()
    ]
    .spacing(TITLE_GAP);
    if let Some(s) = secondary {
        lines = lines.push(text(s).size(TS.body).color(r.ink3).boxed());
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

    const ALL: [DialogIcon; 6] = [
        DialogIcon::Info,
        DialogIcon::Warn,
        DialogIcon::Error,
        DialogIcon::Library,
        DialogIcon::File,
        DialogIcon::Settings,
    ];

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
            if !is_graphite(&t) {
                continue;
            }
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
