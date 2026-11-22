//! The Library panel with nothing to browse (mockup row 119): a centred
//! card that says so, with Open Library… as its action.

use iced::widget::{column, container};
use iced::{Alignment, Border, Element, Length, Theme, Widget};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text};

use super::frame::{SP, TS, lib};
use crate::dock::DockMessage;
use crate::library::messages::LibraryMessage;
use crate::styles::tokens::resolve;
use crate::styles::{Radii, ti};

/// The card: title, one line of explanation and the primary action.
pub(super) fn card<'a>(tokens: &ThemeTokens, i18n: &I18n) -> Element<'a, DockMessage> {
    let r = resolve(tokens);
    let edge = r.line_strong;
    let open = button(text(t!(i18n, ids::LIBRARY_PANEL_OPEN_LIBRARY_ACTION)).size(TS.body))
        .padding([SP.s, SP.l])
        .on_press(lib(LibraryMessage::OpenLibraryDialog))
        .style(crate::styles::primary_button(tokens));
    let body = column![
        text(t!(i18n, ids::LIBRARY_PANEL_EMPTY_TITLE))
            .size(TS.body)
            .font(iced::Font {
                weight: iced::font::Weight::Medium,
                ..signex_widgets::ui_font::regular()
            })
            .color(ti(tokens.text))
            .boxed(),
        text(t!(i18n, ids::LIBRARY_PANEL_EMPTY_BODY))
            .size(TS.body)
            .color(r.ink3)
            .align_x(iced::alignment::Horizontal::Center)
            .boxed(),
        container(open).padding([SP.m, 0.0]).boxed(),
    ]
    .spacing(SP.xs)
    .align_x(Alignment::Center)
    .width(Length::Fill);
    container(
        container(body)
            .padding([SP.xl, SP.l])
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: None,
                border: Border {
                    width: 1.0,
                    color: edge,
                    radius: Radii::GRAPHITE.card.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            }),
    )
    .padding(SP.m)
    .width(Length::Fill)
    .boxed()
}
