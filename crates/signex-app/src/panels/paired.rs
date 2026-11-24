//! Tab + compact dock panel pair: one model, two views (#2523).
//!
//! Some surfaces exist both as a document tab and as a compact dock panel
//! beside the canvas (length matching, the polygon list). The pair rule:
//!
//! - The state lives once, in the document or slice model (the board, the
//!   open groups, the selected member). Neither view keeps a copy.
//! - The tab view and the panel view are both pure functions of that
//!   model, so the panel is live while routing: each frame reads the board
//!   as it is now.
//! - Selecting a member in either view sends the same message, so the
//!   other view follows without a second path.
//!
//! This module holds the compact panel's header row: the panel title, a
//! spacer and the link that opens the tab view. The caller passes the ids
//! and the message, so each pair keeps its own wording. It also holds the
//! compact Length Matching dock panel (mockup rows 7 and 107).

use iced::widget::text::Wrapping;
use iced::widget::{Space, column, container, row};
use iced::{Alignment, Background, Element, Font, Length, Padding, Theme, Widget as _, font};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text};

use crate::panels::{PanelContext, PanelMsg, empty_state_card};
use crate::styles::tokens::resolve;
use crate::styles::{HAIRLINE, Spacing, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
/// Vertical padding of the header row, in em of the body text.
const PAD_Y_EM: f32 = 6.0 / 12.0;

/// The header row of a compact panel that pairs with a document tab.
#[derive(Debug, Clone)]
pub struct PairedHeader<M> {
    /// The panel's title, or its current mode.
    pub title: MessageId,
    /// The link text, e.g. "Open as tab".
    pub open_tab: MessageId,
    /// What the link sends: the message that opens (or focuses) the tab.
    pub on_open_tab: M,
}

impl<M: Clone> PairedHeader<M> {
    /// What a click on the link sends.
    pub fn open_tab_message(&self) -> M {
        self.on_open_tab.clone()
    }

    /// Title, spacer, link; a `line-soft` rule under the row.
    pub fn view<'a>(&self, tokens: &ThemeTokens, i18n: &I18n) -> Element<'a, M>
    where
        M: 'a,
    {
        let r = resolve(tokens);
        let title = text(t!(i18n, self.title))
            .size(TS.body)
            .font(Font {
                weight: font::Weight::Semibold,
                ..signex_widgets::ui_font::regular()
            })
            .color(ti(tokens.text))
            .wrapping(Wrapping::None);
        let (link_ink, link_hot) = (r.accent_text, ti(tokens.accent));
        let link = button(
            text(t!(i18n, self.open_tab))
                .size(TS.body)
                .wrapping(Wrapping::None),
        )
        .padding(0)
        .on_press(self.open_tab_message())
        .style(move |_: &Theme, status: button::Status| button::Style {
            text_color: if matches!(status, button::Status::Hovered | button::Status::Pressed) {
                link_hot
            } else {
                link_ink
            },
            ..button::Style::default()
        });
        let line_soft = r.line_soft;
        let rule = container(Space::new())
            .width(Length::Fill)
            .height(HAIRLINE)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(line_soft)),
                ..container::Style::default()
            });
        let bar = row![title, Space::new().width(Length::Fill), link]
            .spacing(SP.s)
            .align_y(Alignment::Center)
            .padding(Padding::from([TS.body * PAD_Y_EM, SP.m]))
            .width(Length::Fill);
        column![bar, rule].width(Length::Fill).boxed()
    }
}

/// The compact Length Matching panel (mockup row 107). The board model has
/// no length-matching groups yet (#2797), so the panel shows its empty
/// state; the group rows and the "Open as tab" action arrive with the tab.
pub fn length_matching_view(ctx: &PanelContext) -> Element<'_, PanelMsg> {
    let card = empty_state_card(
        &ctx.tokens,
        t!(ctx.i18n, ids::PANELS_LENGTH_MATCHING_EMPTY_TITLE),
        t!(ctx.i18n, ids::PANELS_LENGTH_MATCHING_EMPTY_BODY),
        None,
    );
    container(card).padding(SP.m).width(Length::Fill).boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_i18n::ids;
    use signex_types::theme::{ThemeId, theme_tokens};

    #[derive(Debug, Clone, PartialEq)]
    enum Msg {
        LengthMatchingTabOpened,
    }

    fn header() -> PairedHeader<Msg> {
        PairedHeader {
            title: ids::PANELS_PAIRED_LENGTH_MATCHING,
            open_tab: ids::PANELS_PAIRED_OPEN_AS_TAB,
            on_open_tab: Msg::LengthMatchingTabOpened,
        }
    }

    #[test]
    fn the_link_sends_the_open_tab_message() {
        assert_eq!(header().open_tab_message(), Msg::LengthMatchingTabOpened);
    }

    #[test]
    fn the_header_text_comes_from_the_catalog() {
        let i18n = I18n::en_us_ref();
        assert_eq!(t!(i18n, header().title).into_string(), "Length Matching");
        assert_eq!(t!(i18n, header().open_tab).into_string(), "Open as tab");
    }

    /// Mockup row 107: a board with no length-matching groups shows the
    /// panel's empty state, its text from the catalog.
    #[test]
    fn the_length_matching_panel_shows_its_empty_state() {
        let (app, _boot) = crate::app::Signex::new();
        let ctx = app.document_state.panel_ctx.clone();
        let texts: Vec<String> = crate::layout_probe::lay_out_texts(
            length_matching_view(&ctx),
            iced::Size::new(300.0, 600.0),
        )
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect();
        assert_eq!(
            texts,
            [
                "No length-matching groups",
                "This board has no length-matching groups yet. Their members show here, \
                 with their lengths, as you route.",
            ]
        );
    }

    /// The dock draws the Length Matching kind with this panel, titled
    /// Length Matching.
    #[test]
    fn the_dock_draws_the_length_matching_kind() {
        let i18n = I18n::en_us();
        let kind = crate::panels::PanelKind::LengthMatching;
        assert_eq!(t!(i18n, kind.message()).into_string(), "Length Matching");
        let (app, _boot) = crate::app::Signex::new();
        let ctx = app.document_state.panel_ctx.clone();
        let texts = crate::layout_probe::lay_out_texts(
            crate::panels::view_panel(kind, &ctx),
            iced::Size::new(300.0, 600.0),
        );
        assert!(
            texts
                .iter()
                .any(|t| t.text.contains("No length-matching groups")),
            "{texts:?}"
        );
    }

    #[test]
    fn builds_on_both_themes() {
        let i18n = I18n::en_us_ref();
        for id in ThemeId::BUILTINS {
            let _: Element<'_, Msg> = header().view(&theme_tokens(*id), i18n);
        }
    }
}
