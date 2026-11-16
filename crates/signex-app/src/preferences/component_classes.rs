//! Default Component Classes section — the seed class-registry editor
//! (add / edit / remove / reset). Moved verbatim from the former
//! single-file `preferences` module.

use super::*;
use iced::widget::{Column, Space, Widget, column, container, row};
use iced::{Element, Length};
use signex_i18n::{I18n, ids, t};
use signex_widgets::{button, text, text_input};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

pub(super) fn content_component_classes<'a>(
    classes: &'a [crate::fonts::ComponentClassEntry],
    i18n: &I18n,
) -> Element<'a, PrefMsg> {
    let header = column![
        text(t!(i18n, ids::PREFERENCES_CLASSES_TITLE))
            .size(TS.heading)
            .style(text_primary),
        text(t!(i18n, ids::PREFERENCES_CLASSES_HELP))
            .size(TS.body)
            .style(text_muted),
    ]
    .spacing(6);

    let column_header = row![
        container(
            text(t!(i18n, ids::PREFERENCES_CLASSES_KEY))
                .size(TS.body)
                .style(text_muted)
        )
        .width(Length::FillPortion(2)),
        container(
            text(t!(i18n, ids::PREFERENCES_CLASSES_LABEL))
                .size(TS.body)
                .style(text_muted)
        )
        .width(Length::FillPortion(3)),
        container(Space::new()).width(80),
    ]
    .spacing(SP.m)
    .padding([SP.s, 0.0]);

    let mut rows: Vec<Element<'a, PrefMsg>> = Vec::with_capacity(classes.len());
    for (idx, entry) in classes.iter().enumerate() {
        let key_input = text_input(
            t!(i18n, ids::PLACEHOLDER_COMPONENT_CLASS_KEY),
            entry.key.as_str(),
        )
        .on_input(move |s| PrefMsg::ComponentClassEditKey { index: idx, key: s })
        .padding(5)
        .size(TS.label)
        .width(Length::FillPortion(2));
        let label_input = text_input(
            t!(i18n, ids::PLACEHOLDER_COMPONENT_CLASS_LABEL),
            entry.label.as_str(),
        )
        .on_input(move |s| PrefMsg::ComponentClassEditLabel {
            index: idx,
            label: s,
        })
        .padding(5)
        .size(TS.label)
        .width(Length::FillPortion(3));
        let remove_btn = button(
            container(text(t!(i18n, ids::PREFERENCES_CLASSES_REMOVE)).size(TS.body))
                .padding([SP.s, 10.0])
                .center_x(Length::Fill),
        )
        .on_press(PrefMsg::ComponentClassRemove { index: idx })
        .style(danger_button_style)
        .width(80);

        rows.push(
            row![key_input, label_input, remove_btn]
                .spacing(SP.m)
                .align_y(iced::Alignment::Center)
                .boxed(),
        );
    }

    let body: Element<'a, PrefMsg> = if rows.is_empty() {
        text(t!(i18n, ids::PREFERENCES_CLASSES_EMPTY))
            .size(TS.body)
            .style(text_muted)
            .boxed()
    } else {
        Column::with_children(rows).spacing(6).boxed()
    };

    let add_btn = button(
        container(text(t!(i18n, ids::PREFERENCES_CLASSES_ADD)).size(TS.body)).padding([5.0, SP.l]),
    )
    .on_press(PrefMsg::ComponentClassAdd)
    .style(primary_button_style);

    let reset_btn = button(
        container(text(t!(i18n, ids::PREFERENCES_CLASSES_RESET)).size(TS.body))
            .padding([5.0, SP.l]),
    )
    .on_press(PrefMsg::ComponentClassResetDefaults)
    .style(secondary_button_style);

    let toolbar = row![
        add_btn,
        Space::new().width(8),
        reset_btn,
        Space::new().width(Length::Fill),
    ]
    .spacing(0)
    .align_y(iced::Alignment::Center);

    column![header, column_header, body, Space::new().height(8), toolbar]
        .spacing(10)
        .padding(20)
        .boxed()
}
