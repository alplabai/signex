//! Overview tab — WS-E PENDING placeholder.
//!
//! The pre-refactor Overview tab edited `SchematicSide` /
//! `SharedSide` / `PcbSide` fields directly. WS-B re-shaped `Component`
//! into a binding record; WS-E owns the rebuild against the new
//! `Revision { primary_mpn, alternates, supply, datasheet, parameters,
//! pin_map_overrides, plm }` layout. WS-F here only ships the
//! Symbol/Footprint/Body3D editor surfaces, so this tab renders a
//! "WS-E pending" placeholder until the merger lands.
//!
//! TODO(merge-with-WS-E): replace this stub with the full Overview
//! form — display name, Internal PN, Manufacturer + MPN row, alternate
//! MPN list, supply / datasheet / lifecycle / parameter validation.

use iced::widget::{Space, column, container, text};
use iced::{Element, Length};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::super::messages::LibraryMessage;
use super::super::state::ComponentEditorState;

pub fn view<'a>(
    editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
    _window_id: iced::window::Id,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let text_c = theme_ext::text_primary(tokens);

    let body = column![
        text("Overview").size(13).color(text_c),
        Space::new().height(6),
        text(format!("Internal PN: {}", editor.display_internal_pn))
            .size(12)
            .color(text_c),
        text(format!("Class: {}", editor.component.class.as_str()))
            .size(11)
            .color(muted),
        Space::new().height(8),
        text("Overview tab rebuild lives in WS-E (binding-record fields:")
            .size(11)
            .color(muted),
        text("primary_mpn / alternates / supply / datasheet / lifecycle / parameters).")
            .size(11)
            .color(muted),
    ]
    .spacing(0);

    container(body)
        .padding(14)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(crate::styles::modal_card(tokens))
        .into()
}
