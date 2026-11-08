//! Default Component Classes section — the seed class-registry editor
//! (add / edit / remove / reset). Moved verbatim from the former
//! single-file `preferences` module.

use super::*;
use iced::widget::{Column, Space, Widget, column, container, row};
use iced::{Element, Length};
use signex_widgets::{button, raw, text, text_input};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

pub(super) fn content_component_classes<'a>(
    classes: &'a [crate::fonts::ComponentClassEntry],
) -> Element<'a, PrefMsg> {
    let header = column![
        text(raw("Default Component Classes"))
            .size(TS.heading)
            .style(text_primary),
        text(raw("Seeds the class registry of newly-created libraries. \
              Per-library edits live inside each .snxlib's manifest \
              (forthcoming Library Properties pane); this list \
              controls only what new libraries inherit."))
        .size(TS.body)
        .style(text_muted),
    ]
    .spacing(6);

    let column_header = row![
        container(text(raw("Key")).size(TS.body).style(text_muted)).width(Length::FillPortion(2)),
        container(text(raw("Label")).size(TS.body).style(text_muted)).width(Length::FillPortion(3)),
        container(Space::new()).width(80),
    ]
    .spacing(SP.m)
    .padding([SP.s, 0.0]);

    let mut rows: Vec<Element<'a, PrefMsg>> = Vec::with_capacity(classes.len());
    for (idx, entry) in classes.iter().enumerate() {
        let key_input = text_input(raw("class_key"), entry.key.as_str())
            .on_input(move |s| PrefMsg::ComponentClassEditKey { index: idx, key: s })
            .padding(5)
            .size(TS.label)
            .width(Length::FillPortion(2));
        let label_input = text_input(raw("Label"), entry.label.as_str())
            .on_input(move |s| PrefMsg::ComponentClassEditLabel {
                index: idx,
                label: s,
            })
            .padding(5)
            .size(TS.label)
            .width(Length::FillPortion(3));
        let remove_btn = button(
            container(text(raw("Remove")).size(TS.body))
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
        text(raw("No classes defined. Click \"+ Add Class\" to add one or \"Reset to Defaults\" to restore the seed list."))
            .size(TS.body)
            .style(text_muted)
            .boxed()
    } else {
        Column::with_children(rows).spacing(6).boxed()
    };

    let add_btn = button(container(text(raw("+ Add Class")).size(TS.body)).padding([5.0, SP.l]))
        .on_press(PrefMsg::ComponentClassAdd)
        .style(primary_button_style);

    let reset_btn =
        button(container(text(raw("Reset to Defaults")).size(TS.body)).padding([5.0, SP.l]))
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
