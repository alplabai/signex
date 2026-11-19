//! The Bindings section of the Edit Component modal: one card each for the
//! symbol and the footprint the row binds, with its status, the file and
//! primitive it resolves to, and the Pick… button.

use iced::widget::{column, container, row, svg};
use iced::{Alignment, Border, Color, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_library::{PrimitiveRef, PrimitiveSummary};
use signex_types::object_id::short_id;
use signex_types::theme::ThemeTokens;
use signex_widgets::{text, theme_ext};

use super::super::messages::LibraryMessage;
use super::super::modal_chrome::secondary_btn;
use super::super::state::LibraryState;
use crate::styles::{Radii, ti};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

const CHECK_GLYPH: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 14 14"><path d="M3 7.5l2.6 2.6L11 4.5" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>"#;
const WARN_GLYPH: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 14 14"><path d="M7 1.8l5.6 10H1.4z" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"/><path d="M7 5.6v3M7 10.4h.01" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>"#;
const UNBOUND_GLYPH: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 14 14"><circle cx="7" cy="7" r="4.6" fill="none" stroke="currentColor" stroke-width="1.4" stroke-dasharray="2 2"/></svg>"#;

/// Side of a binding card's status glyph.
const GLYPH: f32 = 14.0;

/// What one binding slot of the row resolves to in the open libraries.
#[derive(Debug, Clone, Copy)]
pub enum Binding<'a> {
    /// The row binds nothing here.
    Unbound,
    /// Bound, but no open library lists the primitive (its library is not
    /// open, or the primitive is gone).
    Missing(uuid::Uuid),
    /// Bound, and listed by its library.
    Found(&'a PrimitiveSummary),
}

impl<'a> Binding<'a> {
    /// Resolve `r` in the listing `pick` chooses (symbols or footprints) of
    /// the open library the ref names: a ref into another open library
    /// resolves too.
    pub(crate) fn resolve(
        lib: &'a LibraryState,
        r: Option<&PrimitiveRef>,
        pick: impl Fn(&'a super::super::state::OpenLibrary) -> &'a [PrimitiveSummary],
    ) -> Self {
        let Some(r) = r.filter(|r| !r.uuid.is_nil()) else {
            return Self::Unbound;
        };
        lib.open_libraries
            .iter()
            .find(|l| l.library_id == r.library_id)
            .and_then(|l| pick(l).iter().find(|s| s.uuid == r.uuid))
            .map_or(Self::Missing(r.uuid), Self::Found)
    }
}

/// The texts of one binding card.
pub(super) struct CardTexts {
    pub title: Localized,
    pub unbound: Localized,
    pub pick: Localized,
}

/// One binding card: the status glyph, the slot's title over what it binds
/// (`<file> · <name>` in mono), and the Pick… button.
pub(super) fn card<'a>(
    binding: Binding<'_>,
    texts: CardTexts,
    on_pick: LibraryMessage,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let r = crate::styles::tokens::resolve(tokens);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let (glyph, ink) = match binding {
        Binding::Found(_) => (CHECK_GLYPH, theme_ext::success_color(tokens)),
        Binding::Missing(_) => (WARN_GLYPH, theme_ext::warning_color(tokens)),
        Binding::Unbound => (UNBOUND_GLYPH, r.ink4),
    };
    let what: Element<'a, LibraryMessage> = match binding {
        Binding::Found(summary) => {
            let name = UserText::new(summary.name.clone());
            let label = match summary.file.as_deref().and_then(|f| f.file_name()) {
                Some(file) => text(t!(
                    i18n,
                    ids::LIBRARY_EDIT_ROW_BINDING,
                    "file" => file.to_string_lossy().into_owned(),
                    "name" => name.as_str()
                )),
                None => text(name),
            };
            label
                .size(TS.body)
                .font(mono)
                .color(ti(tokens.text))
                .boxed()
        }
        Binding::Missing(uuid) => text(t!(
            i18n,
            ids::LIBRARY_EDIT_ROW_BINDING_MISSING,
            "id" => short_id(&uuid)
        ))
        .size(TS.body)
        .color(theme_ext::warning_color(tokens))
        .boxed(),
        Binding::Unbound => text(texts.unbound).size(TS.body).color(r.ink4).boxed(),
    };
    let line = theme_ext::border_color(tokens);
    container(
        row![
            status_glyph(glyph, ink),
            column![text(texts.title).size(TS.caption).color(r.ink3), what,]
                .spacing(SP.xs)
                .width(Length::Fill),
            secondary_btn(texts.pick, on_pick, tokens),
        ]
        .spacing(SP.m + SP.xs)
        .align_y(Alignment::Center),
    )
    .padding([SP.m, SP.m + SP.xs])
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        border: Border {
            width: 1.0,
            color: line,
            radius: Radii::GRAPHITE.card.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

/// A monochrome status glyph tinted `ink`.
fn status_glyph<'a>(bytes: &'static [u8], ink: Color) -> Element<'a, LibraryMessage> {
    svg(svg::Handle::from_memory(bytes))
        .width(GLYPH)
        .height(GLYPH)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        })
        .boxed()
}
