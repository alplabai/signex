//! The Remove from Project modal (#2262): the intro, notes on what a blocker
//! withholds, and the Exclude / Delete radio cards. A card click selects it;
//! the footer button carries out the selected card. The second stage asks to
//! save unsaved edits first (#738).

use super::*;
use iced::widget::{Space, column, container, row};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, button::Status, raw, text};

use super::widgets::modal_title_header;
use crate::app::state::ModalId;
use crate::styles::{
    CalloutTone, DialogButtonKind, DialogIcon, Radii, chrome_px, dialog_body, dialog_button,
    dialog_callout, dialog_footer, dialog_intro, ti, tokens,
};

pub(crate) mod content;
use content::{RemoveCard, RemoveNote};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// A length the mockup gives in px at the default base.
const fn px(at_default_base: f32) -> f32 {
    chrome_px(at_default_base * 11.0 / 12.0)
}

/// Gap between a card's radio and its text.
const CARD_GAP: f32 = px(10.0);
const CARD_PAD_Y: f32 = px(11.0);
const CARD_PAD_X: f32 = px(12.0);
/// The radio's ring and the dot of a selected one.
const RADIO: f32 = px(14.0);
const RADIO_DOT: f32 = px(8.0);
/// The radio sits this much lower than the card title's top.
const RADIO_DROP: f32 = px(2.0);
const TITLE_GAP: f32 = px(2.0);
/// Gap above the warning inside a card.
const WARNING_GAP: f32 = px(8.0);
/// A selected card's border: the 1 px border plus the mockup's 1 px inset
/// ring. A border never moves the content, so selecting shifts nothing.
const SELECTED_BORDER: f32 = 2.0;
/// How much of the soft accent / soft error fill a selected card mixes over
/// the panel.
const EXCLUDE_MIX: f32 = 0.45;
const DELETE_MIX: f32 = 0.60;

impl Signex {
    pub(in crate::app::view) fn view_remove_dialog(&self) -> Element<'_, Message> {
        let dialog = self.view_remove_dialog_body();
        self.wrap_dialog(ModalId::RemoveDialog, dialog, super::sizes::REMOVE)
    }

    pub(in crate::app::view) fn view_remove_dialog_body(&self) -> Element<'_, Message> {
        let Some(ref st) = self.ui_state.remove_dialog else {
            return container(Space::new()).boxed();
        };

        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let title = if st.unsaved_prompt {
            ids::DIALOG_REMOVE_UNSAVED_TITLE
        } else {
            ids::DIALOG_REMOVE_TITLE
        };
        let header_content: Element<'_, Message> = modal_title_header(
            t!(i18n, title).into_string(),
            Message::Remove(RemoveMsg::Close),
            tokens,
        );
        let header = self.modal_header(header_content, ModalId::RemoveDialog);
        let width = self.dialog_width(ModalId::RemoveDialog, super::sizes::REMOVE);

        if st.unsaved_prompt {
            let (body, can_discard) = self.exclude_dirty_prompt(st);
            return exclude_dirty_card(i18n, body, can_discard, header, width, tokens);
        }

        let content = self.remove_content(st, i18n);
        let mut buttons = vec![dialog_button(
            t!(i18n, ids::DIALOG_BUTTON_CANCEL),
            DialogButtonKind::Cancel,
            Some(Message::Remove(RemoveMsg::Close)),
            tokens,
        )];
        if let (Some((label, kind)), Some(choice)) = (content.footer_action(), content.selected) {
            buttons.push(dialog_button(
                t!(i18n, label),
                kind,
                Some(Message::Remove(RemoveMsg::Confirm(choice))),
                tokens,
            ));
        }

        let mut body = column![dialog_intro(
            DialogIcon::Trash,
            t!(i18n, ids::DIALOG_REMOVE_QUESTION, "name" => st.display_name.as_str()).into_string(),
            Some(t!(i18n, ids::DIALOG_REMOVE_QUESTION_DETAIL).into_string()),
            tokens,
        )]
        .spacing(SP.m);
        for note in content.notes {
            body = body.push(note_callout(note, tokens));
        }
        let selected = content.selected;
        for card in content.cards {
            let on = selected == Some(card.choice);
            body = body.push(option_card(i18n, card, on, tokens));
        }

        container(column![header, dialog_body(body), dialog_footer(buttons)].width(width))
            .style(crate::styles::modal_card(tokens))
            .clip(true)
            .boxed()
    }
}

/// A note between the intro and the cards, in an info callout.
fn note_callout<'a>(note: RemoveNote, tokens: &ThemeTokens) -> Element<'a, Message> {
    let body: Element<'a, Message> = match note {
        RemoveNote::Said(said) => text(said).size(TS.body).boxed(),
        RemoveNote::Library { path, usage } => {
            let mut lines = column![
                text(UserText::new(path))
                    .size(TS.body)
                    .font(super::graphite::mono())
            ]
            .spacing(SP.xs);
            if let Some(usage) = usage {
                // raw: the usage line is built in English by the Remove
                // handler, which its own tests read (#2061).
                lines = lines.push(text(raw(usage)).size(TS.body).boxed());
            }
            lines.boxed()
        }
    };
    dialog_callout(CalloutTone::Info, body, tokens)
}

/// One radio card: the radio, the title (Delete in the error ink), what the
/// choice does, and what it leaves behind as a warning. A click selects it.
fn option_card<'a>(
    i18n: &I18n,
    card: RemoveCard,
    selected: bool,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let danger = card.choice == crate::app::RemoveChoice::DeleteFile;
    let title_ink = if danger {
        tokens::danger(tokens)
    } else {
        ti(tokens.text)
    };
    let mut lines = column![
        text(t!(i18n, card.title))
            .size(TS.body)
            .font(semibold())
            .color(title_ink),
        text(card.description).size(TS.body).color(r.ink3),
    ]
    .spacing(TITLE_GAP)
    .width(Length::Fill);
    if let Some(warning) = card.warning {
        lines = lines.push(
            container(dialog_callout(
                CalloutTone::Warn,
                text(warning).size(TS.body).boxed(),
                tokens,
            ))
            .padding(iced::Padding {
                top: WARNING_GAP - TITLE_GAP,
                ..iced::Padding::ZERO
            })
            .boxed(),
        );
    }
    let radio = container(radio_mark(selected, tokens)).padding(iced::Padding {
        top: RADIO_DROP,
        ..iced::Padding::ZERO
    });
    let face = row![radio, lines]
        .spacing(CARD_GAP)
        .align_y(Alignment::Start);
    let tokens = *tokens;
    button(face)
        .padding([CARD_PAD_Y, CARD_PAD_X])
        .width(Length::Fill)
        .on_press(Message::Remove(RemoveMsg::ChoicePicked(card.choice)))
        .style(move |_: &Theme, status| card_style(&tokens, danger, selected, status))
        .boxed()
}

fn semibold() -> iced::Font {
    iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    }
}

/// `amount` of `fill` (flattened over `base` first, so a translucent soft
/// tint mixes like its painted colour) over `base`.
fn mixed(fill: Color, base: Color, amount: f32) -> Color {
    let flat = tokens::composite_over(fill, base);
    tokens::composite_over(Color { a: amount, ..flat }, base)
}

/// Look of a card: the panel fill and a hairline at rest, the hover fill
/// under the pointer; selected, a 2 px accent (Delete: error) border and the
/// soft accent (error) fill mixed over the panel.
fn card_style(tokens: &ThemeTokens, danger: bool, selected: bool, status: Status) -> button::Style {
    let r = tokens::resolve(tokens);
    let panel = ti(tokens.panel_bg);
    let (fill, line, width) = if selected && danger {
        (
            mixed(r.err_soft, panel, DELETE_MIX),
            tokens::danger(tokens),
            SELECTED_BORDER,
        )
    } else if selected {
        (
            mixed(r.accent_soft, panel, EXCLUDE_MIX),
            ti(tokens.accent),
            SELECTED_BORDER,
        )
    } else if matches!(status, Status::Hovered | Status::Pressed) {
        (
            tokens::composite_over(tokens::hover_overlay(tokens), panel),
            ti(tokens.border),
            1.0,
        )
    } else {
        (panel, ti(tokens.border), 1.0)
    };
    button::Style {
        background: Some(Background::Color(fill)),
        text_color: ti(tokens.text),
        border: Border {
            width,
            color: line,
            radius: Radii::GRAPHITE.card.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

/// The radio: a ring in the strong line colour on the field fill; selected,
/// an accent ring around an accent dot.
fn radio_mark<'a>(selected: bool, tokens: &ThemeTokens) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let accent = ti(tokens.accent);
    let ring = if selected { accent } else { r.line_strong };
    let field = r.field;
    let dot: Element<'a, Message> = if selected {
        container(Space::new())
            .width(Length::Fixed(RADIO_DOT))
            .height(Length::Fixed(RADIO_DOT))
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(accent)),
                border: Border {
                    radius: (RADIO_DOT / 2.0).into(),
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .boxed()
    } else {
        Space::new().boxed()
    };
    container(dot)
        .width(Length::Fixed(RADIO))
        .height(Length::Fixed(RADIO))
        .center_x(Length::Fixed(RADIO))
        .center_y(Length::Fixed(RADIO))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(field)),
            border: Border {
                width: 1.0,
                color: ring,
                radius: (RADIO / 2.0).into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// Unsaved-edits stage of the Remove modal (#738): Save / Discard / Cancel.
/// Cancel is `RemoveMsg::Close`, the same message Esc resolves to.
fn exclude_dirty_card<'a>(
    i18n: &I18n,
    body: String,
    can_discard: bool,
    header: Element<'a, Message>,
    width: Length,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    // Discard is withheld when another open project shares the target.
    let mut buttons = vec![dialog_button(
        t!(i18n, ids::DIALOG_BUTTON_CANCEL),
        DialogButtonKind::Cancel,
        Some(Message::Remove(RemoveMsg::Close)),
        tokens,
    )];
    if can_discard {
        buttons.push(dialog_button(
            t!(i18n, ids::DIALOG_BUTTON_DISCARD),
            DialogButtonKind::Danger,
            Some(Message::Remove(RemoveMsg::ExcludeDirty(
                crate::app::ExcludeDirtyChoice::Discard,
            ))),
            tokens,
        ));
    }
    buttons.push(dialog_button(
        t!(i18n, ids::DIALOG_BUTTON_SAVE),
        DialogButtonKind::Primary,
        Some(Message::Remove(RemoveMsg::ExcludeDirty(
            crate::app::ExcludeDirtyChoice::Save,
        ))),
        tokens,
    ));
    container(
        column![
            header,
            dialog_body(dialog_intro(
                DialogIcon::Warn,
                t!(i18n, ids::DIALOG_EXCLUDE_DIRTY_QUESTION).into_string(),
                Some(body),
                tokens,
            )),
            dialog_footer(buttons),
        ]
        .width(width),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .boxed()
}
