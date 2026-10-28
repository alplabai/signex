//! Tools ▸ Document Options modal — Altium SchLib parity.
//!
//! Per-`.snxlib` view settings: sheet color, grid spacing, grid
//! visibility, coordinate display unit. Edits go to a working
//! [`crate::library::state::DocumentOptionsModalState::draft`];
//! Save commits to `OpenLibrary.display`; Cancel discards.
//!
//! Mounted as a full-screen overlay backdrop via
//! `app/view/mod.rs::collect_overlays` when
//! `LibraryState::document_options.is_some()`.

use iced::widget::{Space, button, column, container, pick_list, row, text};
use iced::{Element, Length};
use signex_types::theme::ThemeTokens;

use crate::styles::{Spacing, TypeScale};
use signex_widgets::theme_ext;

use super::messages::LibraryMessage;
use super::state::DocumentOptionsModalState;
use crate::panels::SheetColor;

pub fn view<'a>(
    state: &'a DocumentOptionsModalState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let ts = TypeScale::default();
    let sp = Spacing::default();
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);

    let header = container(
        row![
            text("Document Options").size(ts.heading).color(text_c),
            Space::new().width(Length::Fill),
            text(format!("Library: {}", state.library_name))
                .size(ts.body)
                .color(muted),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([sp.m, sp.l])
    .style(crate::styles::modal_header_strip(tokens));

    let row_field =
        |label: &'a str, value: Element<'a, LibraryMessage>| -> Element<'a, LibraryMessage> {
            container(
                row![
                    text(label)
                        .size(ts.body)
                        .color(muted)
                        .width(Length::FillPortion(2)),
                    container(value).width(Length::FillPortion(3)),
                ]
                .spacing(sp.s)
                .align_y(iced::Alignment::Center),
            )
            .padding([sp.s, sp.l])
            .width(Length::Fill)
            .into()
        };

    let sheet_picker: Element<'a, LibraryMessage> = pick_list(
        SheetColor::ALL.to_vec(),
        Some(state.draft.sheet_color),
        LibraryMessage::DocumentOptionsSetSheetColor,
    )
    .padding([sp.xs, sp.s])
    .text_size(ts.body)
    .into();

    let grid_check: Element<'a, LibraryMessage> = iced::widget::checkbox(state.draft.grid_visible)
        .size(ts.heading)
        .on_toggle(|_| LibraryMessage::DocumentOptionsToggleGrid)
        .into();

    let grid_label = format!("{:.3} mm", state.draft.grid_size_mm);
    let grid_size_btn: Element<'a, LibraryMessage> =
        button(text(grid_label).size(ts.body).color(text_c))
            .padding([sp.xs, sp.m])
            .on_press(LibraryMessage::DocumentOptionsCycleGridSize)
            .style(crate::styles::outline_button(tokens))
            .into();

    let unit_label = format!("{}", state.draft.unit);
    let unit_btn: Element<'a, LibraryMessage> =
        button(text(unit_label).size(ts.body).color(text_c))
            .padding([sp.xs, sp.m])
            .on_press(LibraryMessage::DocumentOptionsCycleUnit)
            .style(crate::styles::outline_button(tokens))
            .into();

    let body = column![
        row_field("Sheet Color", sheet_picker),
        row_field("Grid Visible", grid_check),
        row_field("Grid Spacing", grid_size_btn),
        row_field("Unit", unit_btn),
    ]
    .spacing(sp.xs);

    let cancel = button(text("Cancel").size(ts.body).color(text_c))
        .padding([sp.s, sp.l])
        .on_press(LibraryMessage::DocumentOptionsCancel)
        .style(crate::styles::outline_button(tokens));
    let apply = button(
        text("Apply")
            .size(ts.body)
            .color(crate::styles::tokens::on_accent(tokens)),
    )
    .padding([sp.s, sp.l])
    .on_press(LibraryMessage::DocumentOptionsApply)
    .style(crate::styles::accent_button(tokens));

    let footer = container(
        row![
            Space::new().width(Length::Fill),
            cancel,
            Space::new().width(sp.m),
            apply
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([sp.m, sp.l])
    .style(crate::styles::modal_footer_strip(tokens));

    container(
        column![header, container(body).padding([sp.s, 0.0]), footer]
            .width(Length::Fixed(super::modal_chrome::DOCUMENT_OPTIONS_W)),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .into()
}
