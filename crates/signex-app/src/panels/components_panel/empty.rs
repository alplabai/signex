//! The Components panel with no library mounted: a card with Add
//! Library… and Open Project… (#2535).

use iced::widget::container;
use iced::{Element, Length, Widget};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeTokens;

use super::{SP, look};
use crate::library::messages::LibraryMessage;
use crate::panels::empty_state::EmptyStateAction;

/// The two actions of the "No libraries mounted" card: Add Library… opens
/// the "+ Add" menu, Open Project… runs File > Open Project.
pub(super) fn nothing_mounted_actions(i18n: &I18n) -> Vec<EmptyStateAction<LibraryMessage>> {
    vec![
        EmptyStateAction {
            label: t!(i18n, ids::PANELS_COMPONENTS_ADD_LIBRARY),
            on_press: LibraryMessage::ComponentsPanelAddMenuToggled,
            primary: true,
        },
        EmptyStateAction {
            label: t!(i18n, ids::PANELS_COMPONENTS_OPEN_PROJECT),
            on_press: LibraryMessage::ComponentsPanelOpenProjectClicked,
            primary: false,
        },
    ]
}

/// "No libraries mounted" as a card with its two buttons; its caption says
/// the bundled stock library is missing when `stock_missing`.
pub(super) fn nothing_mounted<'a>(
    tokens: &ThemeTokens,
    i18n: &I18n,
    stock_missing: bool,
) -> Element<'a, LibraryMessage> {
    container(crate::panels::empty_state::empty_state_card_with_actions(
        tokens,
        t!(i18n, ids::PANELS_COMPONENTS_NOTHING_MOUNTED),
        t!(i18n, ids::PANELS_COMPONENTS_NOTHING_MOUNTED_BODY),
        stock_missing.then(|| t!(i18n, ids::PANELS_COMPONENTS_EMPTY_STOCK)),
        nothing_mounted_actions(i18n),
    ))
    .padding([SP.l, look::EDGE])
    .width(Length::Fill)
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The empty card offers Add Library… (primary, the "+ Add" menu) and
    /// Open Project… (File > Open Project).
    #[test]
    fn the_nothing_mounted_card_offers_add_library_and_open_project() {
        let en = I18n::en_us();
        let actions = nothing_mounted_actions(&en);
        let labels: Vec<String> = actions
            .iter()
            .map(|a| a.label.clone().into_string())
            .collect();
        assert_eq!(labels, ["Add Library…", "Open Project…"]);
        assert!(actions[0].primary && !actions[1].primary);
        assert!(matches!(
            actions[0].on_press,
            LibraryMessage::ComponentsPanelAddMenuToggled
        ));
        assert!(matches!(
            actions[1].on_press,
            LibraryMessage::ComponentsPanelOpenProjectClicked
        ));
    }

    /// With nothing mounted, the card's caption names a missing bundled
    /// stock library, and says nothing of it otherwise.
    #[test]
    fn the_card_names_a_missing_stock_library_only_when_it_is_missing() {
        use signex_types::theme::{ThemeId, theme_tokens};
        let en = I18n::en_us();
        let tokens = theme_tokens(ThemeId::GraphiteLight);
        let size = iced::Size::new(600.0, 400.0);
        let shows = |missing: bool| {
            crate::layout_probe::lay_out_texts(nothing_mounted(&tokens, &en, missing), size)
                .into_iter()
                .any(|t| t.text.starts_with("Stock library not found."))
        };
        assert!(shows(true));
        assert!(!shows(false));
    }
}
