//! Supply tab — WS-E PENDING placeholder.
//!
//! The pre-refactor Supply tab edited `SharedSide.suppliers` (a
//! `Vec<SupplierLink>`), which WS-B replaced with
//! `Revision::supply: Vec<DistributorListing>` and split the
//! manufacturer side into `primary_mpn` / `alternates`. WS-E owns the
//! full rewire — the alternate-MPN ranking UI, distributor preference
//! ordering, and "paste a URL → resolve via API" button all live
//! there.
//!
//! TODO(merge-with-WS-E): replace this stub with the alternate-MPN
//! editor + supply-listing grid + distributor settings link.

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
        text("Supply").size(13).color(text_c),
        Space::new().height(6),
        text(format!(
            "Primary MPN: {} ({})",
            editor.draft.primary_mpn.mpn, editor.draft.primary_mpn.manufacturer
        ))
        .size(12)
        .color(text_c),
        text(format!("Alternates: {}", editor.draft.alternates.len()))
            .size(11)
            .color(muted),
        text(format!("Supply listings: {}", editor.draft.supply.len()))
            .size(11)
            .color(muted),
        Space::new().height(10),
        text("Supply-tab rebuild lives in WS-E (alternate ranking, distributor")
            .size(11)
            .color(muted),
        text("listing grid, paste-URL → API resolution).")
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
