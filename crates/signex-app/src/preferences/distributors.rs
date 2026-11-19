//! Library ▸ Distributor APIs page — mounts the live
//! `library::settings::distributor_apis` view under the page heading,
//! wrapping its `LibraryMessage::Settings(_)` in `PrefMsg::LibrarySettings(_)`.

use super::setting_row;
use super::{PrefMsg, PrefsView};
use iced::{Element, Widget as _};
use signex_i18n::{ids, t};

/// The Distributor APIs page: heading, intro, then the cards beside the
/// order of preference.
///
/// The library view emits `LibraryMessage::Settings(_)`; every message is
/// wrapped in `PrefMsg::LibrarySettings(_)` so the modal's outer
/// `Message::Preferences(PreferencesMsg::Inner(_))` map stays one layer. The
/// handler re-dispatches it through `Message::Library`, so the state lives on
/// `LibraryState.settings` and the async flows run in one place.
pub(super) fn content_library_distributors<'a>(v: PrefsView<'a>) -> Element<'a, PrefMsg> {
    let (i18n, tokens) = (v.i18n, v.panel_tokens);
    let heading = setting_row::page_heading(
        t!(i18n, ids::PREFERENCES_PAGE_DISTRIBUTORS),
        t!(i18n, ids::LIBRARY_DISTRIBUTORS_INTRO),
        tokens,
    );
    let pane =
        crate::library::settings::distributor_apis::view(v.distributor_settings, tokens, i18n)
            .map(wrap_library_message)
            .boxed();
    setting_row::page(vec![heading, pane])
}

/// Wrap a library-panel message for the Preferences modal. Anything that
/// is not a `Settings(_)` message is inert (`PrefMsg::Noop`) — it must never
/// dismiss the dialog.
fn wrap_library_message(lm: crate::library::messages::LibraryMessage) -> PrefMsg {
    match lm {
        crate::library::messages::LibraryMessage::Settings(s) => PrefMsg::LibrarySettings(s),
        _ => PrefMsg::Noop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::messages::LibraryMessage;

    #[test]
    fn non_settings_library_message_does_not_close_preferences() {
        let msg = wrap_library_message(LibraryMessage::OpenLibraryDialog);
        assert!(
            !matches!(msg, PrefMsg::Close),
            "unmatched library message must not close Preferences"
        );
        assert!(matches!(msg, PrefMsg::Noop));
    }
}
