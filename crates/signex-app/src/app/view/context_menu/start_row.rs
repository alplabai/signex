//! Right-click menu of a Start screen project row (#2305): open the
//! project, and pin or unpin it; a pinned project missing on disk offers
//! Remove instead.

use super::*;

use std::path::Path;

use super::items::dd_msg;
use crate::pinned_projects::PinnedProjects;
use crate::start_screen::{StartScreenMsg, message};
use signex_i18n::{I18n, ids};
use signex_widgets::active_bar_dropdown::DropdownEntry;

/// Menu rows for `path`: pure data, so the labels and messages are tested
/// without a renderer.
pub(super) fn start_row_menu_entries(
    i18n: &I18n,
    path: &Path,
    pinned: &PinnedProjects,
) -> Vec<DropdownEntry<Message>> {
    let label = |id| i18n.message(id, &[]).into_string();
    let owned = path.to_path_buf();
    if !pinned.contains(path) {
        return vec![
            dd_msg(
                None,
                &label(ids::START_OPEN_PROJECT),
                "",
                message(StartScreenMsg::RecentProjectClicked(owned.clone())),
            ),
            dd_msg(
                None,
                &label(ids::START_PIN),
                "",
                message(StartScreenMsg::ProjectPinned(owned)),
            ),
        ];
    }
    let unpin = message(StartScreenMsg::ProjectUnpinned(owned.clone()));
    if pinned.is_missing(path) {
        return vec![dd_msg(None, &label(ids::START_REMOVE), "", unpin)];
    }
    vec![
        dd_msg(
            None,
            &label(ids::START_OPEN_PROJECT),
            "",
            message(StartScreenMsg::PinnedProjectClicked(owned)),
        ),
        dd_msg(None, &label(ids::START_UNPIN), "", unpin),
    ]
}

impl Signex {
    /// The open row menu with its click-outside backdrop; empty when no
    /// menu is open. The Start screen shows in the main window only.
    pub(in crate::app::view) fn start_row_context_menu_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Vec<Element<'_, Message>> {
        let Some(menu_ctx) = self.interaction_state.start_row_menu.as_ref() else {
            return Vec::new();
        };
        if !self.overlay_belongs_to(None, window_id) {
            return Vec::new();
        }
        let panel_ctx = &self.document_state.panel_ctx;
        let menu = super::placement::menu_card(
            start_row_menu_entries(
                &self.ui_state.i18n,
                &menu_ctx.path,
                &self.ui_state.pinned_projects,
            ),
            &panel_ctx.tokens,
            &panel_ctx.ui_font_name,
        );
        Self::menu_overlay(
            message(StartScreenMsg::RowMenuDismissed),
            menu.element,
            (menu_ctx.x, menu_ctx.y),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn rows(path: &Path, pinned: &PinnedProjects) -> Vec<(String, Message)> {
        start_row_menu_entries(&I18n::en_us(), path, pinned)
            .into_iter()
            .filter_map(|e| match e {
                DropdownEntry::Item(item) => Some((item.label, item.on_press?)),
                _ => None,
            })
            .collect()
    }

    fn labels(rows: &[(String, Message)]) -> Vec<&str> {
        rows.iter().map(|(label, _)| label.as_str()).collect()
    }

    #[test]
    fn a_recent_row_offers_open_and_pin() {
        let path = PathBuf::from("a.snxprj");
        let rows = rows(&path, &PinnedProjects::default());
        assert_eq!(labels(&rows), ["Open", "Pin"]);
        // Open goes through the Start screen so the menu closes first.
        assert!(matches!(
            &rows[0].1,
            Message::Project(crate::app::ProjectMsg::StartScreen(StartScreenMsg::RecentProjectClicked(p))) if *p == path
        ));
        assert!(matches!(
            &rows[1].1,
            Message::Project(crate::app::ProjectMsg::StartScreen(StartScreenMsg::ProjectPinned(p))) if *p == path
        ));
    }

    #[test]
    fn a_pinned_row_offers_open_and_unpin_and_a_missing_one_remove() {
        let path = PathBuf::from("a.snxprj");
        let mut pinned = PinnedProjects::default();
        pinned.pin(&path);
        let present = rows(&path, &pinned);
        assert_eq!(labels(&present), ["Open", "Unpin"]);
        assert!(matches!(
            &present[0].1,
            Message::Project(crate::app::ProjectMsg::StartScreen(StartScreenMsg::PinnedProjectClicked(p))) if *p == path
        ));

        pinned.set_missing(&path, true);
        let missing = rows(&path, &pinned);
        assert_eq!(labels(&missing), ["Remove"]);
        assert!(matches!(
            &missing[0].1,
            Message::Project(crate::app::ProjectMsg::StartScreen(StartScreenMsg::ProjectUnpinned(p))) if *p == path
        ));
    }
}
