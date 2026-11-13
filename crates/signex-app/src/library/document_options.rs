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

use iced::widget::{column, container, pick_list, row};
use iced::{Element, Length, Widget};
use signex_i18n::{ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{button, raw, text};

use crate::styles::{Spacing, TypeScale};
use signex_widgets::theme_ext;

use super::messages::LibraryMessage;
use super::modal_chrome::{cancel_btn, footer_strip, header_with_note, primary_btn};
use super::state::DocumentOptionsModalState;
use crate::panels::SheetColor;

pub fn view<'a>(
    state: &'a DocumentOptionsModalState,
    tokens: &'a ThemeTokens,
    i18n: &signex_i18n::I18n,
    windowed: bool,
) -> Element<'a, LibraryMessage> {
    let ts = TypeScale::default();
    let sp = Spacing::default();
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);

    let header = header_with_note(
        "Document Options",
        format!("Library: {}", state.library_name),
        windowed,
        tokens,
    );

    let row_field =
        |label: &'a str, value: Element<'a, LibraryMessage>| -> Element<'a, LibraryMessage> {
            container(
                row![
                    text(raw(label))
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
            .boxed()
        };

    let sheet_picker: Element<'a, LibraryMessage> = pick_list(
        Some(state.draft.sheet_color),
        SheetColor::ALL.to_vec(),
        |c: &SheetColor| c.to_string(),
    )
    .graphite()
    .on_select(LibraryMessage::DocumentOptionsSetSheetColor)
    .padding([sp.xs, sp.s])
    .boxed();

    let grid_check: Element<'a, LibraryMessage> =
        signex_widgets::checkbox(state.draft.grid_visible)
            .style(crate::panels::form_look::checkbox_style)
            .size(ts.heading)
            .on_toggle(|_| LibraryMessage::DocumentOptionsToggleGrid)
            .boxed();

    let grid_label = format!("{:.3} mm", state.draft.grid_size_mm);
    let grid_size_btn: Element<'a, LibraryMessage> =
        button(text(raw(grid_label)).size(ts.body).color(text_c))
            .padding([sp.xs, sp.m])
            .on_press(LibraryMessage::DocumentOptionsCycleGridSize)
            .style(crate::styles::outline_button(tokens))
            .boxed();

    let unit_label = format!("{}", state.draft.unit);
    let unit_btn: Element<'a, LibraryMessage> =
        button(text(raw(unit_label)).size(ts.body).color(text_c))
            .padding([sp.xs, sp.m])
            .on_press(LibraryMessage::DocumentOptionsCycleUnit)
            .style(crate::styles::outline_button(tokens))
            .boxed();

    let body = column![
        row_field("Sheet Color", sheet_picker),
        row_field("Grid Visible", grid_check),
        row_field("Grid Spacing", grid_size_btn),
        row_field("Unit", unit_btn),
    ]
    .spacing(sp.xs);

    let footer = footer_strip(
        Vec::new(),
        vec![
            cancel_btn(
                t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_CANCEL),
                LibraryMessage::DocumentOptionsCancel,
                tokens,
            ),
            primary_btn(
                t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_APPLY),
                LibraryMessage::DocumentOptionsApply,
                tokens,
            ),
        ],
        tokens,
    );

    container(
        column![header, container(body).padding([sp.s, 0.0]), footer].width(super::card_len(
            windowed,
            super::modal_chrome::DOCUMENT_OPTIONS_W,
        )),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .boxed()
}
