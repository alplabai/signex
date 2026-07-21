//! Default Component Classes section — the seed class-registry editor
//! (add / edit / remove / reset). Moved verbatim from the former
//! single-file `preferences` module.

use super::*;
use iced::widget::{Column, Space, button, column, container, row, text, text_input};
use iced::{Element, Length};

pub(super) fn content_component_classes<'a>(
    classes: &'a [crate::fonts::ComponentClassEntry],
) -> Element<'a, PrefMsg> {
    let header = column![
        text("Default Component Classes")
            .size(15)
            .style(text_primary),
        text(
            "Seeds the class registry of newly-created libraries. \
              Per-library edits live inside each .snxlib's manifest \
              (forthcoming Library Properties pane); this list \
              controls only what new libraries inherit."
        )
        .size(11)
        .style(text_muted),
    ]
    .spacing(6);

    let column_header = row![
        container(text("Key").size(11).style(text_muted)).width(Length::FillPortion(2)),
        container(text("Label").size(11).style(text_muted)).width(Length::FillPortion(3)),
        container(Space::new()).width(80),
    ]
    .spacing(8)
    .padding([4, 0]);

    let mut rows: Vec<Element<'a, PrefMsg>> = Vec::with_capacity(classes.len());
    for (idx, entry) in classes.iter().enumerate() {
        let key_input = text_input("class_key", entry.key.as_str())
            .on_input(move |s| PrefMsg::ComponentClassEditKey { index: idx, key: s })
            .padding(5)
            .size(12)
            .width(Length::FillPortion(2));
        let label_input = text_input("Label", entry.label.as_str())
            .on_input(move |s| PrefMsg::ComponentClassEditLabel {
                index: idx,
                label: s,
            })
            .padding(5)
            .size(12)
            .width(Length::FillPortion(3));
        let remove_btn = button(
            container(text("Remove").size(11))
                .padding([4, 10])
                .center_x(Length::Fill),
        )
        .on_press(PrefMsg::ComponentClassRemove { index: idx })
        .style(danger_button_style)
        .width(80);

        rows.push(
            row![key_input, label_input, remove_btn]
                .spacing(8)
                .align_y(iced::Alignment::Center)
                .into(),
        );
    }

    let body: Element<'a, PrefMsg> = if rows.is_empty() {
        text("No classes defined. Click \"+ Add Class\" to add one or \"Reset to Defaults\" to restore the seed list.")
            .size(11)
            .style(text_muted)
            .into()
    } else {
        Column::with_children(rows).spacing(6).into()
    };

    let add_btn = button(container(text("+ Add Class").size(11)).padding([5, 12]))
        .on_press(PrefMsg::ComponentClassAdd)
        .style(primary_button_style);

    let reset_btn = button(container(text("Reset to Defaults").size(11)).padding([5, 12]))
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
        .into()
}
