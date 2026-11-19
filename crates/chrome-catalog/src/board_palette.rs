//! Board canvas palette and line-art inks, to check against the mockup
//! (#2523). Board colours are the same in both themes; the line-art inks
//! follow the selected chrome theme.

use iced::Widget;
use iced::widget::{Space, column, container, row};
use iced::{Background, Border, Color, Element, Length, Theme};
use signex_types::theme::{ThemeId, ThemeTokens, canvas_colors, theme_tokens};
use signex_widgets::{Label, text};

use crate::catalog::Message;
use crate::theme;
use signex_i18n::UserText;

/// Sample text for this page: fixture content, not UI copy.
fn sample_text(text: impl Into<String>) -> Label<'static> {
    UserText::new(text).into()
}

/// The body text size the catalog samples are sized from.
const BODY: f32 = 12.0;
/// A swatch chip, one text line square.
const CHIP: f32 = BODY;
const GAP: f32 = BODY * 8.0 / 11.0;
const PAD: f32 = BODY * 12.0 / 11.0;

pub(crate) fn view<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    column![board_legend(), line_art(tokens)]
        .spacing(GAP)
        .boxed()
}

/// The board legend on the board canvas ground; a sided pair shows its
/// darker bottom chip after the top.
fn board_legend<'a>() -> Element<'a, Message> {
    let board = canvas_colors(ThemeId::GraphiteDark);
    let layers = board.board_layer_colors();
    let t = layers.technical;
    let entries = [
        ("Top", layers.top_copper, None),
        ("Bottom", layers.bottom_copper, None),
        ("Mid 1", layers.layer_cycle[0], None),
        ("Mid 2", layers.layer_cycle[1], None),
        ("Mask", layers.solder_mask, None),
        ("Paste", layers.paste, None),
        ("Courtyard", t.courtyard.top, Some(t.courtyard.bottom)),
        ("Fab", t.fab.top, Some(t.fab.bottom)),
        ("3D Body", t.body_3d.top, Some(t.body_3d.bottom)),
        ("Keep-Out", board.keepout_color(), None),
        ("Board edge", board.board_edge_color(), None),
    ];
    let ground = theme::color(theme_tokens(ThemeId::GraphiteDark).bg);
    let label = theme::color(theme_tokens(ThemeId::GraphiteDark).text);
    let chips = entries.into_iter().map(|(name, top, bottom)| {
        let mut pair = row![chip(theme::color(top))].spacing(GAP / 4.0);
        if let Some(bottom) = bottom {
            pair = pair.push(chip(theme::color(bottom)));
        }
        column![pair, text(sample_text(name)).size(BODY).color(label)]
            .spacing(GAP / 2.0)
            .boxed()
    });
    container(row(chips).spacing(PAD).wrap())
        .padding(PAD)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(ground)),
            ..container::Style::default()
        })
        .boxed()
}

/// The line-art inks of the selected chrome theme, strongest first.
fn line_art<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    let e = &tokens.extended;
    let pick = |set: Option<signex_types::theme::Color>, fallback| set.unwrap_or(fallback);
    let inks = [
        ("Outline", tokens.text),
        ("Detail", tokens.text_secondary),
        ("Mark", pick(e.ink3, tokens.text_secondary)),
        ("Guide", pick(e.ink4, tokens.text_secondary)),
        ("Callout", pick(e.accent_text, tokens.accent)),
    ];
    let label = theme::color(tokens.text);
    let chips = inks.into_iter().map(|(name, ink)| {
        row![
            stroke(theme::color(ink)),
            text(sample_text(name)).size(BODY).color(label)
        ]
        .spacing(GAP / 2.0)
        .boxed()
    });
    row(chips).spacing(PAD).boxed()
}

fn chip<'a>(color: Color) -> Element<'a, Message> {
    container(Space::new())
        .width(CHIP)
        .height(CHIP)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(color)),
            ..container::Style::default()
        })
        .boxed()
}

/// A line-art sample: an outlined square in the ink, on no fill.
fn stroke<'a>(color: Color) -> Element<'a, Message> {
    container(Space::new())
        .width(CHIP)
        .height(CHIP)
        .style(move |_: &Theme| container::Style {
            border: Border {
                width: 1.0,
                color,
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}
