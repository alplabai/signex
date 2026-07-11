//! Library ▸ Distributor APIs section — mounts the live
//! `library::settings::distributor_apis` panel inside the Preferences
//! modal, wrapping its `LibraryMessage::Settings(_)` in
//! `PrefMsg::LibrarySettings(_)`. Moved verbatim from the former
//! single-file `preferences` module.

use super::*;
use iced::widget::{Space, column, container};
use iced::{Element, Length};

/// Mount the live Distributor APIs panel inside the Preferences modal.
///
/// The panel emits `LibraryMessage::Settings(_)`; we wrap every
/// message in `PrefMsg::LibrarySettings(_)` so the modal's outer
/// `Message::Preferences(PreferencesMsg::Inner(_))` map stays a single layer. The
/// `app/handlers/preferences.rs` handler unwraps and re-dispatches
/// via `Message::Library` so the canonical state writeback runs
/// through the same dispatcher the Tools-menu surface uses.
pub(super) fn content_library_distributors<'a>(
    settings: &'a crate::library::state::DistributorSettings,
    tokens: &'a signex_types::theme::ThemeTokens,
) -> Element<'a, PrefMsg> {
    let header: Element<'a, PrefMsg> = column![
        section_title("Library — Distributor APIs"),
        Space::new().height(8),
    ]
    .padding([16, 20])
    .into();

    // The library panel returns `Element<'a, LibraryMessage>`. We
    // map to `PrefMsg::LibrarySettings` for every Settings sub-
    // variant; non-Settings library messages are ignored (they're
    // never produced by `distributor_apis::view`).
    let pane =
        crate::library::settings::distributor_apis::view(settings, tokens).map(|lm| match lm {
            crate::library::messages::LibraryMessage::Settings(s) => PrefMsg::LibrarySettings(s),
            // distributor_apis::view never produces anything else.
            _ => PrefMsg::Close,
        });

    column![header, container(pane).padding([0, 20]).width(Length::Fill)]
        .spacing(0)
        .into()
}
