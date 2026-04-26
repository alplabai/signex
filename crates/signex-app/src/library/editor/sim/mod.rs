//! Sim tab — WS-E PENDING placeholder.
//!
//! The pre-refactor Sim tab held a `SimTabState` carrying the
//! `text_editor::Content` for the SPICE body and the pin-number cache.
//! WS-B replaced inline `SpiceModel` with the typed `SimModel`
//! primitive bound by `Revision::sim_ref`, mirroring Symbol /
//! Footprint. WS-E owns the rebuild — typed pin-to-node grid,
//! template helpers, primitive resolution / save through `LibrarySet`.
//!
//! TODO(merge-with-WS-E): replace this stub with the multi-line SPICE
//! editor + pin-to-node grid backed by the new `SimModel` primitive.

pub mod state;

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

    let has_sim = editor.draft.sim_ref.is_some();
    let body = column![
        text("Sim").size(13).color(text_c),
        Space::new().height(6),
        text(format!(
            "SPICE model bound: {}",
            if has_sim { "yes" } else { "no" }
        ))
        .size(11)
        .color(muted),
        Space::new().height(10),
        text("Sim-tab rebuild lives in WS-E (SimModel primitive editor + pin-to-node grid).")
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
