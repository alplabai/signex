//! Stat tile samples in both Graphite themes: a read-only summary (with a
//! text-valued tile), a filter row with one tile on, and a clean run where
//! every count is zero. Presses are ignored, like the chip samples.

use iced::widget::{Column, column, container};
use iced::{Background, Border, Element, Length, Theme, Widget};
use signex_i18n::I18n;
use signex_types::theme::{ThemeId, ThemeTokens, theme_tokens};
use signex_widgets::stat_tiles::{Density, StatTile, Tone, stat_tiles};
use signex_widgets::theme_ext::{HAIRLINE, Radii, Spacing, TypeScale, to_color};
use signex_widgets::{Label, raw};

use crate::catalog::Message;

/// Body size of the Graphite type scale.
const BODY: f32 = TypeScale::DEFAULT.body;
const SP: Spacing = Spacing::DEFAULT;

/// Sample text for this page: fixture content, not catalog copy. The one
/// `raw` site of the page.
fn sample(text: &'static str) -> Label<'static> {
    raw(text)
}

fn pressed() -> Message {
    Message::TreeInteracted
}

/// The summary row of the annotate dialog: inert, one value is a word.
fn summary() -> Vec<StatTile<'static, Message>> {
    vec![
        StatTile::count(38, sample("parts change")),
        StatTile::count(4, sample("sheets")),
        StatTile::text(sample("IEEE 315"), sample("standard")),
    ]
}

/// The filter row of the update dialog, with Change on.
fn filters() -> Vec<StatTile<'static, Message>> {
    vec![
        StatTile::count(14, sample("Add"))
            .tone(Tone::Ok)
            .on_press(pressed()),
        StatTile::count(9, sample("Change"))
            .tone(Tone::Info)
            .on(true)
            .on_press(pressed()),
        StatTile::count(2, sample("Remove")).on_press(pressed()),
        StatTile::count(3, sample("Problems"))
            .tone(Tone::Err)
            .on_press(pressed()),
    ]
}

/// A clean check run: zero counts keep the neutral ink.
fn clean() -> Vec<StatTile<'static, Message>> {
    vec![
        StatTile::count(0, sample("errors")).tone(Tone::Err),
        StatTile::count(0, sample("warnings")).tone(Tone::Warn),
        StatTile::count(0, sample("info")).tone(Tone::Info),
    ]
}

fn theme_column<'a>(theme: ThemeId, i18n: &I18n) -> Element<'a, Message> {
    let tokens = theme_tokens(theme);
    let rows: Column<Element<'a, Message>> = column![
        stat_tiles(summary(), Density::Regular, &tokens, BODY, i18n),
        stat_tiles(filters(), Density::Regular, &tokens, BODY, i18n),
        stat_tiles(clean(), Density::Compact, &tokens, BODY, i18n),
    ]
    .spacing(SP.l);
    let paper = to_color(&tokens.paper);
    let edge = to_color(&tokens.border);
    container(rows)
        .padding(SP.l)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(paper)),
            border: Border {
                color: edge,
                width: HAIRLINE,
                radius: Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// Both Graphite themes side by side, whichever theme the catalog shows.
pub(crate) fn view<'a>(_tokens: &ThemeTokens) -> Element<'a, Message> {
    let i18n = crate::locale::en_us();
    let mut row = iced::widget::Row::new().spacing(SP.xl);
    for theme in ThemeId::BUILTINS {
        row = row.push(theme_column(*theme, i18n));
    }
    row.boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_cover_plain_filter_and_zero_states() {
        assert!(summary().iter().all(|t| !t.is_clickable()));
        let filters = filters();
        assert!(filters.iter().all(StatTile::is_clickable));
        assert_eq!(filters.iter().filter(|t| t.is_on).count(), 1);
        assert!(
            clean()
                .iter()
                .all(|t| t.effective_tone() == Tone::Neutral && t.tone != Tone::Neutral)
        );
    }
}
