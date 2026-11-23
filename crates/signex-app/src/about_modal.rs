//! Help ▸ About Signex: the mark and the name, the version block (version,
//! licence, OS), links to the documentation and the issue tracker, and the
//! "Copy version info" action. Also the scale and glyph theme the other
//! Help cards (Keyboard Shortcuts, the first-run card) share. Styled from
//! `ThemeTokens` only.

use std::sync::LazyLock;

use iced::font::Weight;
use iced::widget::{Space, column, container, row, svg};
use iced::{Alignment, Background, Border, Element, Font, Length, Padding, Theme, Widget};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::{button, raw, text};

use crate::about::{ExternalLink, LICENSE_NAME, VERSION, os_text};
use crate::app::view::dialogs::modal_title_header;
use crate::app::{Message, OverlayMsg};
use crate::styles::{self, DialogButtonKind, Radii, TypeScale, chrome_px, ti};

const TS: TypeScale = TypeScale::DEFAULT;

/// A length of the Graphite mockups, authored in px at the 12 px base, so
/// it scales with the chrome.
pub(crate) const fn px(at_default_base: f32) -> f32 {
    chrome_px(at_default_base * 11.0 / 12.0)
}

/// Theme the chrome glyphs are fetched for. Every glyph is tinted with an
/// ink token, which hides the theme's accent swap, so any theme gives the
/// same.
pub(crate) const GLYPH_THEME: ThemeId = ThemeId::GraphiteLight;

const CARD_W: f32 = px(400.0);
const MARK_SIDE: f32 = px(44.0);
/// Gap between the mark and the name, and between the version block's
/// columns and the two links.
const WIDE_GAP: f32 = px(14.0);
/// Gap between the version block's lines.
const LINE_GAP: f32 = px(3.0);
const LINK_GLYPH: f32 = px(12.0);

/// The Signex mark, as in the menu bar and the dialog title bar.
static MARK: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!("../assets/icons/chrome/signex_mark.svg").as_slice())
});

/// The dialog as a card in the main window, sized to itself: the overlay
/// centres it over the click-outside layer.
pub fn view<'a>(i18n: &I18n, tokens: &'a ThemeTokens) -> Element<'a, Message> {
    build(i18n, tokens, false)
}

/// The dialog as the content of its own OS window: no header (the title
/// bar has the title and the close) and filling the window.
pub fn window_view<'a>(i18n: &I18n, tokens: &'a ThemeTokens) -> Element<'a, Message> {
    build(i18n, tokens, true)
}

fn build<'a>(i18n: &I18n, tokens: &'a ThemeTokens, windowed: bool) -> Element<'a, Message> {
    let body = container(column![
        identity(i18n, tokens),
        Space::new().height(px(16.0)),
        version_block(i18n, tokens),
        Space::new().height(px(12.0)),
        row![
            external_link(
                i18n,
                tokens,
                ids::DIALOG_ABOUT_DOCUMENTATION,
                ExternalLink::Documentation
            ),
            external_link(
                i18n,
                tokens,
                ids::DIALOG_ABOUT_REPORT_ISSUE,
                ExternalLink::IssueTracker
            ),
        ]
        .spacing(WIDE_GAP),
    ])
    .padding(Padding {
        top: px(22.0),
        right: px(22.0),
        bottom: px(16.0),
        left: px(22.0),
    })
    .width(Length::Fill);

    let footer = styles::dialog_footer_band(
        vec![styles::dialog_icon_button(
            crate::icons::icon_chrome_copy(GLYPH_THEME),
            t!(i18n, ids::DIALOG_ABOUT_COPY_VERSION),
            DialogButtonKind::Secondary,
            Some(Message::Overlay(OverlayMsg::CopyAboutInfo)),
            tokens,
        )],
        vec![styles::dialog_button(
            t!(i18n, ids::DIALOG_BUTTON_CLOSE),
            DialogButtonKind::Primary,
            Some(Message::Overlay(OverlayMsg::CloseAbout)),
            tokens,
        )],
        Padding::from([px(12.0), px(16.0)]),
        tokens,
    );

    if windowed {
        return container(column![body, Space::new().height(Length::Fill), footer])
            .width(Length::Fill)
            .height(Length::Fill)
            .style(styles::modal_card(tokens))
            .boxed();
    }
    let header = modal_title_header(
        t!(i18n, ids::DIALOG_TITLE_ABOUT).into_string(),
        Message::Overlay(OverlayMsg::CloseAbout),
        tokens,
    );
    container(column![header, body, footer])
        .width(Length::Fixed(CARD_W))
        .clip(true)
        .style(styles::modal_card(tokens))
        .boxed()
}

/// The mark beside the product name and the tagline.
fn identity<'a>(i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    let muted = styles::tokens::resolve(tokens).ink3;
    row![
        svg(MARK.clone()).width(MARK_SIDE).height(MARK_SIDE),
        column![
            text(t!(i18n, ids::DIALOG_ABOUT_NAME))
                .size(TS.display)
                .font(Font {
                    weight: Weight::Semibold,
                    ..signex_widgets::ui_font::regular()
                })
                .color(ti(tokens.text)),
            text(t!(i18n, ids::DIALOG_ABOUT_TAGLINE))
                .size(TS.body)
                .color(muted),
        ],
    ]
    .spacing(WIDE_GAP)
    .align_y(Alignment::Center)
    .boxed()
}

/// Version, licence (a link to the licence text) and OS, the three lines
/// "Copy version info" copies, on a bordered panel; values in mono.
fn version_block<'a>(i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    let r = styles::tokens::resolve(tokens);
    let ink = ti(tokens.text);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let key = |id: MessageId| text(t!(i18n, id)).size(TS.body).color(r.ink4);
    // raw: the licence's SPDX name, not language.
    let licence = text(raw(LICENSE_NAME))
        .size(TS.body)
        .font(mono)
        .color(r.accent_text);
    let values = column![
        // raw: the version number, not language.
        text(raw(VERSION)).size(TS.body).font(mono).color(ink),
        button(licence)
            .padding(0)
            .on_press(Message::Overlay(OverlayMsg::OpenLink(
                ExternalLink::License
            )))
            .style(styles::ghost_button(tokens)),
        // raw: the OS and architecture identifiers, not language.
        text(raw(os_text())).size(TS.body).font(mono).color(ink),
    ]
    .spacing(LINE_GAP);
    let keys = column![
        key(ids::DIALOG_ABOUT_VERSION),
        key(ids::DIALOG_ABOUT_LICENSE),
        key(ids::DIALOG_ABOUT_OS),
    ]
    .spacing(LINE_GAP);
    let (panel, line) = (ti(tokens.panel_bg), ti(tokens.border));
    container(row![keys, values].spacing(WIDE_GAP))
        .padding([px(10.0), px(12.0)])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(panel)),
            border: Border {
                width: 1.0,
                color: line,
                radius: Radii::GRAPHITE.button.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// A link to an external page: accent text and the external-link glyph.
fn external_link<'a>(
    i18n: &I18n,
    tokens: &ThemeTokens,
    label: MessageId,
    link: ExternalLink,
) -> Element<'a, Message> {
    let accent = styles::tokens::resolve(tokens).accent_text;
    button(
        row![
            text(t!(i18n, label)).size(TS.body).color(accent),
            styles::tinted_glyph(
                crate::icons::icon_chrome_external_link(GLYPH_THEME),
                LINK_GLYPH,
                accent
            ),
        ]
        .spacing(px(5.0))
        .align_y(Alignment::Center),
    )
    .padding(0)
    .on_press(Message::Overlay(OverlayMsg::OpenLink(link)))
    .style(styles::ghost_button(tokens))
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout_probe::{lay_out_texts, messages_on_click};
    use signex_types::theme::theme_tokens;

    const WINDOW: iced::Size = iced::Size::new(1400.0, 900.0);

    fn texts(element: Element<'_, Message>) -> Vec<String> {
        lay_out_texts(element, WINDOW)
            .into_iter()
            .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
            .collect()
    }

    #[test]
    fn the_dialog_shows_the_catalog_texts_and_the_version_block() {
        let i18n = I18n::en_us_ref();
        let os = os_text();
        for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let tokens = theme_tokens(id);
            let shown = texts(view(i18n, &tokens));
            for expected in [
                "About Signex",
                "Signex",
                "AI-first electronics design automation",
                "Version",
                "License",
                "OS",
                VERSION,
                LICENSE_NAME,
                os.as_str(),
                "Documentation",
                "Report an Issue",
                "Copy version info",
                "Close",
            ] {
                assert!(
                    shown.iter().any(|t| t == expected),
                    "{id:?}: {expected:?} not in {shown:?}"
                );
            }
            assert!(
                !shown.iter().any(|t| t.contains("Licensed under")),
                "the licence is a row of the version block now"
            );
        }
    }

    #[test]
    fn the_window_form_has_no_title_but_the_same_body() {
        let tokens = theme_tokens(ThemeId::GraphiteLight);
        let shown = texts(window_view(I18n::en_us_ref(), &tokens));
        assert!(!shown.iter().any(|t| t == "About Signex"), "{shown:?}");
        for expected in ["Documentation", "Copy version info", "Close"] {
            assert!(shown.iter().any(|t| t == expected), "{expected:?}");
        }
    }

    #[test]
    fn each_link_and_button_sends_its_message() {
        let tokens = theme_tokens(ThemeId::GraphiteDark);
        let i18n = I18n::en_us_ref();
        let click = |label: &str| messages_on_click(view(i18n, &tokens), WINDOW, label);
        let opened = |label: &str, want: ExternalLink| {
            click(label)
                .iter()
                .any(|m| matches!(m, Message::Overlay(OverlayMsg::OpenLink(l)) if *l == want))
        };
        assert!(opened("Documentation", ExternalLink::Documentation));
        assert!(opened("Report an Issue", ExternalLink::IssueTracker));
        assert!(opened(LICENSE_NAME, ExternalLink::License));
        assert!(
            click("Copy version info")
                .iter()
                .any(|m| matches!(m, Message::Overlay(OverlayMsg::CopyAboutInfo)))
        );
        assert!(
            click("Close")
                .iter()
                .any(|m| matches!(m, Message::Overlay(OverlayMsg::CloseAbout)))
        );
    }
}
