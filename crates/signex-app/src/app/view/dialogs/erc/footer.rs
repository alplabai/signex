//! The ERC dialog's footer (#2413): the "Profile" menu button, Undo and Redo
//! as icon buttons, then Close and the primary Run ERC. The open Profile
//! menu is a card floating just above its button.

use iced::widget::{Column, Row, container, svg};
use iced::{Alignment, Element, Length, Padding, Shadow, Theme, Widget};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text, tooltip};

use crate::app::document::UndoTarget;
use crate::app::{ErcMsg, Message, Signex};
use crate::styles::{
    ChromeMetrics, DialogButtonKind, card, dialog_button, dialog_footer_height,
    dialog_footer_split, floating_shadow, hover_row, outline_button, ti,
};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;
/// Icon glyph size inside a footer icon button, relative to the button.
const ICON_IN_BUTTON: f32 = 0.5;

fn erc(msg: ErcMsg) -> Message {
    Message::Erc(msg)
}

/// A square outlined icon button with a tooltip; muted and inert without
/// an action.
fn icon_button<'a>(
    handle: svg::Handle,
    hint: Localized,
    on_press: Option<Message>,
    theme_tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let ink = if on_press.is_some() {
        ti(theme_tokens.text)
    } else {
        crate::styles::tokens::resolve(theme_tokens).ink4
    };
    let side = ChromeMetrics::DEFAULT.button;
    let glyph = svg(handle)
        .width(side * ICON_IN_BUTTON)
        .height(side * ICON_IN_BUTTON)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        });
    let face = container(glyph)
        .center_x(Length::Fill)
        .center_y(Length::Fill);
    let btn = button(face)
        .padding(0)
        .width(side)
        .height(side)
        .on_press_maybe(on_press)
        .style(outline_button(theme_tokens));
    let tip = container(text(hint).size(TS.caption).color(ti(theme_tokens.text)))
        .padding([SP.xs, SP.m])
        .style(card(theme_tokens));
    tooltip(btn, tip, tooltip::Position::Top).gap(SP.xs).boxed()
}

impl Signex {
    /// "Profile ▾": opens the menu of profile actions; disabled when the
    /// dialog offers none.
    fn view_erc_profile_button<'a>(&'a self, theme_tokens: &ThemeTokens) -> Element<'a, Message> {
        let i18n: &I18n = &self.ui_state.i18n;
        let enabled = !self.erc_profile_actions().is_empty();
        let ink = if enabled {
            ti(theme_tokens.text)
        } else {
            ti(theme_tokens.text_secondary)
        };
        let chevron_side = TS.caption;
        let chevron = svg(crate::icons::icon_collapse_down(
            self.ui_state.effective_theme_id(),
        ))
        .width(chevron_side)
        .height(chevron_side)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        });
        let face: Row<Element<'a, Message>> = Row::new()
            .push(
                text(t!(i18n, ids::DIALOG_ERC_PROFILE_MENU))
                    .size(TS.label)
                    .color(ink)
                    .boxed(),
            )
            .push(chevron.boxed())
            .spacing(SP.s)
            .align_y(Alignment::Center);
        button(container(face).center_y(Length::Fill))
            .padding([0.0, SP.l])
            .height(ChromeMetrics::DEFAULT.button)
            .on_press_maybe(enabled.then(|| erc(ErcMsg::ProfileMenuToggled)))
            .style(outline_button(theme_tokens))
            .boxed()
    }

    /// Profile menu, Undo / Redo (when the project stores rules) on the
    /// left; Close and Run ERC on the right.
    pub(super) fn view_erc_footer<'a>(
        &'a self,
        stores_rules: bool,
        theme_tokens: &ThemeTokens,
    ) -> Element<'a, Message> {
        let i18n: &I18n = &self.ui_state.i18n;
        let mut extras: Vec<Element<'a, Message>> =
            vec![self.view_erc_profile_button(theme_tokens)];
        if stores_rules {
            let (can_undo, can_redo) = self.undo_availability(
                &self
                    .document_state
                    .active_document_project()
                    .map_or(UndoTarget::None, |p| UndoTarget::ProjectRules(p.id)),
            );
            let theme = self.ui_state.effective_theme_id();
            extras.push(icon_button(
                crate::icons::icon_undo(theme),
                t!(i18n, ids::DIALOG_BUTTON_UNDO),
                can_undo.then_some(erc(ErcMsg::RulesUndone)),
                theme_tokens,
            ));
            extras.push(icon_button(
                crate::icons::icon_redo(theme),
                t!(i18n, ids::DIALOG_BUTTON_REDO),
                can_redo.then_some(erc(ErcMsg::RulesRedone)),
                theme_tokens,
            ));
        }
        dialog_footer_split(
            extras,
            vec![
                dialog_button(
                    t!(i18n, ids::DIALOG_BUTTON_CLOSE),
                    DialogButtonKind::Cancel,
                    Some(erc(ErcMsg::CloseDialog)),
                    theme_tokens,
                ),
                dialog_button(
                    t!(i18n, ids::DIALOG_ERC_RUN),
                    DialogButtonKind::Primary,
                    Some(erc(ErcMsg::Run)),
                    theme_tokens,
                ),
            ],
        )
    }

    /// The open Profile menu: one row per available action, in a floating
    /// card anchored to the footer's left edge, just above the Profile
    /// button. `None` while the menu is closed or offers nothing.
    pub(super) fn view_erc_profile_menu<'a>(
        &'a self,
        theme_tokens: &ThemeTokens,
    ) -> Option<Element<'a, Message>> {
        if !self.ui_state.erc_profile_menu_open {
            return None;
        }
        let actions = self.erc_profile_actions();
        if actions.is_empty() {
            return None;
        }
        let i18n: &I18n = &self.ui_state.i18n;
        let mut rows: Column<Element<'a, Message>> = Column::new().spacing(0);
        for action in actions {
            rows = rows.push(
                button(text(t!(i18n, action.label())).size(TS.body))
                    .padding([SP.s, SP.m])
                    .width(Length::Fill)
                    .on_press(erc(ErcMsg::ProfileMenuRowChosen(action)))
                    .style(hover_row(theme_tokens, false))
                    .boxed(),
            );
        }
        let base = card(theme_tokens);
        let shadow: Shadow = floating_shadow(theme_tokens);
        let menu =
            container(rows)
                .padding(SP.xs)
                .width(Length::Shrink)
                .style(move |theme: &Theme| container::Style {
                    shadow,
                    ..base(theme)
                });
        // The footer pads its buttons by SP.l; the card sits on that inset,
        // its bottom edge on the footer's top edge.
        Some(
            container(menu)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_left(Length::Fill)
                .align_bottom(Length::Fill)
                .padding(Padding {
                    left: SP.l,
                    bottom: dialog_footer_height() - SP.l + SP.xs,
                    ..Padding::ZERO
                })
                .boxed(),
        )
    }
}
