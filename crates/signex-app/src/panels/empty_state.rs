//! Shared Graphite empty-state card: a title, a line of explanation and an
//! optional muted caption, on the card surface. Used by panels whose backend
//! is not built yet (Signex AI, Selection Filter).

use iced::widget::{column, container};
use iced::{Element, Length, Widget as _};
use signex_i18n::Localized;
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use crate::styles::{self, TypeScale, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;

/// The card, 12 px padded, filling the panel width.
pub fn empty_state_card<'a, M: 'a>(
    tokens: &ThemeTokens,
    title: Localized,
    body: Localized,
    caption: Option<Localized>,
) -> Element<'a, M> {
    let mut lines = column![
        text(title).size(TS.label).color(ti(tokens.text)),
        text(body).size(TS.dense).color(ti(tokens.text_secondary)),
    ]
    .spacing(6);
    if let Some(caption) = caption {
        lines = lines.push(
            text(caption)
                .size(TS.caption)
                .color(resolve(tokens).ink4)
                .boxed(),
        );
    }
    container(lines)
        .padding(12)
        .width(Length::Fill)
        .style(styles::card(tokens))
        .boxed()
}
