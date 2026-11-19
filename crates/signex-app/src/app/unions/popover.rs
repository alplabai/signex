//! The Create Union popover (#158, mockup m158 "Create union"): a 300 px
//! card near the selection with the object count, a Name field (focused on
//! open), a Tags field of removable pills and an "add tag" input, the
//! "Lock after creating" box, and Cancel / Create. A pure view.

use std::sync::LazyLock;

use iced::widget::{Column, Row, Space, column, container, row};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{CheckboxLabel as _, button, checkbox, text, text_input};

use super::state::{CreateUnionPopover, UnionPopoverMsg};
use crate::styles::{
    DialogButtonKind, Radii, Spacing, TypeScale, chrome_px, dialog_button, ti, tokens,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Card width (mockup: 300 px).
const CARD_W: f32 = chrome_px(300.0 * 11.0 / 12.0);
/// Width of the Name / Tags label column (mockup: 56 px).
const LABEL_W: f32 = chrome_px(56.0 * 11.0 / 12.0);
/// Width of the "add tag" input (mockup: 60 px).
const TAG_INPUT_W: f32 = chrome_px(60.0 * 11.0 / 12.0);

/// The Name field, focused when the popover opens.
pub static UNION_NAME_INPUT_ID: LazyLock<iced::widget::Id> =
    LazyLock::new(|| iced::widget::Id::new("signex.union_name"));

/// A tag pill (mockup `.ttag`: 16 px high, sunken, small radius) with an
/// optional × that removes it.
pub(crate) fn tag_pill<'a, M: Clone + 'a>(
    label: String,
    on_remove: Option<M>,
    i18n: &I18n,
    theme_tokens: &ThemeTokens,
) -> Element<'a, M> {
    let r = tokens::resolve(theme_tokens);
    let mut line: Row<Element<'a, M>> = Row::new().spacing(SP.xs).align_y(Alignment::Center);
    line = line.push(
        text(UserText::new(label))
            .size(TS.caption)
            .color(ti(theme_tokens.text_secondary))
            .boxed(),
    );
    if let Some(msg) = on_remove {
        line = line.push(
            button(
                text(t!(i18n, ids::UNIONS_TAG_REMOVE))
                    .size(TS.caption)
                    .color(r.ink3),
            )
            .padding(0)
            .on_press(msg)
            .style(crate::styles::ghost_button(theme_tokens))
            .boxed(),
        );
    }
    let fill = r.sunken;
    container(line)
        .padding([0.0, SP.s])
        .height(Length::Fixed(chrome_px(16.0 * 11.0 / 12.0)))
        .align_y(iced::alignment::Vertical::Center)
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

fn label<'a>(
    i18n: &I18n,
    id: signex_i18n::MessageId,
    theme_tokens: &ThemeTokens,
) -> Element<'a, UnionPopoverMsg> {
    text(t!(i18n, id))
        .size(TS.label)
        .color(tokens::muted_text(theme_tokens))
        .width(Length::Fixed(LABEL_W))
        .boxed()
}

/// The popover card.
pub fn view<'a>(
    popover: &'a CreateUnionPopover,
    i18n: &I18n,
    theme_tokens: &ThemeTokens,
) -> Element<'a, UnionPopoverMsg> {
    let title = text(t!(i18n, ids::UNIONS_CREATE_TITLE, "count" => popover.members.len() as i64))
        .size(TS.label)
        .color(ti(theme_tokens.text))
        .boxed();
    let submit = popover.can_create().then_some(UnionPopoverMsg::Confirmed);
    let name = row![
        label(i18n, ids::UNIONS_CREATE_NAME, theme_tokens),
        text_input(UserText::new(String::new()), &popover.name)
            .id(UNION_NAME_INPUT_ID.clone())
            .on_input(UnionPopoverMsg::NameEdited)
            .on_submit_maybe(submit.clone())
            .size(TS.body)
            .width(Length::Fill),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);

    let mut pills: Row<Element<'a, UnionPopoverMsg>> =
        Row::new().spacing(SP.xs).align_y(Alignment::Center);
    for (at, tag) in popover.tags.iter().enumerate() {
        pills = pills.push(tag_pill(
            tag.clone(),
            Some(UnionPopoverMsg::TagRemoved(at)),
            i18n,
            theme_tokens,
        ));
    }
    pills = pills.push(
        text_input(t!(i18n, ids::UNIONS_CREATE_ADD_TAG), &popover.tag_input)
            .on_input(UnionPopoverMsg::TagInputEdited)
            .on_submit(UnionPopoverMsg::TagSubmitted)
            .size(TS.caption)
            .width(Length::Fixed(TAG_INPUT_W))
            .boxed(),
    );
    let tags = row![
        label(i18n, ids::UNIONS_CREATE_TAGS, theme_tokens),
        pills.wrap().vertical_spacing(SP.xs),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);

    let lock = checkbox(popover.lock)
        .style(crate::panels::form_look::checkbox_style)
        .size(TS.title)
        .labelled(t!(i18n, ids::UNIONS_CREATE_LOCK))
        .text_size(TS.label)
        .on_toggle(UnionPopoverMsg::LockToggled);

    let buttons = row![
        Space::new().width(Length::Fill),
        dialog_button(
            t!(i18n, ids::DIALOG_BUTTON_CANCEL),
            DialogButtonKind::Cancel,
            Some(UnionPopoverMsg::Cancelled),
            theme_tokens,
        ),
        dialog_button(
            t!(i18n, ids::UNIONS_CREATE_CONFIRM),
            DialogButtonKind::Primary,
            submit,
            theme_tokens,
        ),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);

    let body: Column<Element<'a, UnionPopoverMsg>> = column![title, name, tags, lock, buttons]
        .spacing(SP.m)
        .width(Length::Fixed(CARD_W));
    container(body)
        .padding(SP.l)
        .style(crate::styles::context_menu(theme_tokens))
        .boxed()
}
