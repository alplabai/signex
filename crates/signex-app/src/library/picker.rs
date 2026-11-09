//! Component picker modal.
//!
//! Opened from File ▸ Library ▸ Place Component… (and, eventually,
//! the `P` shortcut once Phase 2 wires the placement flow).
//!
//! Shape:
//!
//! ```text
//! ┌─[Place Component ─────────────────────────────────── X]─┐
//! │ [Search internal_pn / mpn / description…]              │
//! ├────────────────────────────────────────────────────────┤
//! │ ► R0805_10k    1.2  Released   Yageo  RC0805FR-…      │
//! │   C0805_100n   1.0  Released   Murata GRM21BR…        │
//! │   …                                                    │
//! ├────────────────────────────────────────────────────────┤
//! │                                  [ Cancel ] [ Place ]  │
//! └────────────────────────────────────────────────────────┘
//! ```

use iced::widget::{column, container, row, scrollable};
use iced::{Border, Element, Length, Theme, Widget};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;
use signex_widgets::{button, raw, text, text_input};

use super::messages::{LibraryMessage, PickerMsg, PlacedPartMsg};
use super::modal_chrome::{cancel_btn, footer_strip, header, primary_btn_if};
use super::placed_part::{change_preview_view, datasheet_choice_view};
use super::range_filter::range_filter_bar;
use super::state::{PickerMode, PickerState};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

pub(crate) const TITLE_CHANGE_COMPONENT: &str = "Change Component";
pub(crate) const TITLE_OPEN_DATASHEET: &str = "Open Datasheet";

/// Title of the picker modal's current stage; the native window title of
/// a detached picker reads the same.
pub(crate) fn title(mode: &PickerMode) -> &'static str {
    match mode {
        PickerMode::Place => "Place Component",
        PickerMode::ChangeComponent(_) | PickerMode::ChangePreview(_) => TITLE_CHANGE_COMPONENT,
        PickerMode::Datasheets(_) => TITLE_OPEN_DATASHEET,
    }
}

const PICKER_W: f32 = 720.0;
const PICKER_H: f32 = 480.0;

/// Draws the list `LibraryState::sync_view_caches` last built for the
/// picker; `view` itself filters and clones nothing.
pub fn view<'a>(
    picker: &'a PickerState,
    tokens: &'a ThemeTokens,
    windowed: bool,
) -> Element<'a, LibraryMessage> {
    // Change Component and Open Datasheet reuse this modal: their later
    // stages draw their own card.
    match &picker.mode {
        PickerMode::ChangePreview(change) => return change_preview_view(change, tokens, windowed),
        PickerMode::Datasheets(choice) => return datasheet_choice_view(choice, tokens, windowed),
        PickerMode::Place | PickerMode::ChangeComponent(_) => {}
    }
    let changing = matches!(picker.mode, PickerMode::ChangeComponent(_));
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);

    let header = header(
        title(&picker.mode),
        LibraryMessage::ClosePicker,
        windowed,
        tokens,
    );

    let search = text_input(
        raw("Search internal PN / MPN / description…"),
        &picker.filter,
    )
    .style(crate::panels::form_look::text_input_style)
    .on_input(|s| LibraryMessage::Picker(PickerMsg::FilterChanged(s)))
    .padding(6)
    .size(TS.label);

    let rows = &picker.rows;

    let mut list_col = column![].spacing(0);
    if rows.is_empty() {
        list_col = list_col.push(
            container(
                text(raw(
                    "No components match. Open a library or refine the filter.",
                ))
                .size(TS.body)
                .color(muted),
            )
            .padding([14, 14])
            .boxed(),
        );
    } else {
        for entry in rows.visible() {
            let summary = &entry.summary;
            let is_selected = picker
                .selected
                .as_ref()
                .map(|(p, c)| p.as_path() == &*entry.path && c.row_id == summary.row_id)
                .unwrap_or(false);
            let row_bg = if is_selected {
                Some(crate::styles::ti(tokens.hover))
            } else {
                None
            };
            let row_widget = button(
                row![
                    text(raw(summary.internal_pn.as_str()))
                        .size(TS.body)
                        .color(text_c)
                        .width(Length::FillPortion(2)),
                    text(raw(entry.state_label.as_str()))
                        .size(TS.body)
                        .color(muted)
                        .width(Length::Fixed(96.0)),
                    text(raw(summary.mpn.as_str()))
                        .size(TS.body)
                        .color(text_c)
                        .width(Length::FillPortion(3)),
                    text(raw(summary.description.as_str()))
                        .size(TS.body)
                        .color(muted)
                        .width(Length::FillPortion(4)),
                ]
                .spacing(SP.m)
                .padding([3.0, SP.m]),
            )
            .padding(0)
            .width(Length::Fill)
            .on_press(LibraryMessage::Picker(PickerMsg::SelectComponent(
                summary.row_id,
            )))
            .style(move |_: &Theme, status: iced::widget::button::Status| {
                let hover_bg = crate::styles::ti(tokens.hover);
                let bg = match (row_bg, status) {
                    (Some(bg), _) => Some(iced::Background::Color(bg)),
                    (None, iced::widget::button::Status::Hovered) => {
                        Some(iced::Background::Color(hover_bg))
                    }
                    _ => None,
                };
                iced::widget::button::Style {
                    background: bg,
                    border: Border::default(),
                    text_color: text_c,
                    ..iced::widget::button::Style::default()
                }
            });
            list_col = list_col.push(row_widget.boxed());
        }
    }

    let body = container(
        scrollable(list_col)
            .height(Length::Fill)
            .width(Length::Fill),
    )
    .padding([6, 0])
    .height(Length::Fill);

    let place_enabled = picker.selected.is_some();
    let footer = footer_strip(
        Vec::new(),
        vec![
            cancel_btn("Cancel", LibraryMessage::ClosePicker, tokens),
            primary_btn_if(
                if changing { "Next" } else { "Place" },
                if changing {
                    LibraryMessage::PlacedPart(PlacedPartMsg::ChangePartPicked)
                } else {
                    LibraryMessage::Picker(PickerMsg::PlaceSelected)
                },
                place_enabled,
                tokens,
            ),
        ],
        tokens,
    );

    // Range filters on the measurement columns (#1572, phase 4).
    let range_bar = range_filter_bar(
        &picker.range_filters,
        picker.rows.filter_columns(),
        picker.rows.range_excluded(),
        tokens,
        |msg| LibraryMessage::Picker(PickerMsg::RangeFilter(msg)),
    );
    let mut top = column![container(search).padding([6.0, SP.l])].spacing(0);
    if let Some(bar) = range_bar {
        top = top.push(container(bar).padding([0.0, SP.l]).boxed());
    }
    let body_pad = container(column![top, body].spacing(0))
        .width(Length::Fill)
        .height(Length::Fill);

    container(
        column![header, body_pad, footer]
            .width(super::card_len(windowed, PICKER_W))
            .height(super::card_len(windowed, PICKER_H)),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .boxed()
}
