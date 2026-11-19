//! Right-click menu of a Start screen project row (#2305, mock m3): open
//! the project, and pin or unpin it; a pinned project missing on disk
//! offers Remove instead. Every row has its glyph in the icon column (Open
//! a folder, Pin and Unpin a pin, Remove a cross in the neutral ink).

use super::*;

use std::path::Path;

use super::items::dd_msg;
use crate::pinned_projects::PinnedProjects;
use crate::start_screen::{StartScreenMsg, message};
use signex_i18n::{I18n, ids};
use signex_types::theme::ThemeId;
use signex_widgets::active_bar_dropdown::DropdownEntry;

/// Menu rows for `path`: pure data, so the labels, glyphs and messages are
/// tested without a renderer.
pub(super) fn start_row_menu_entries(
    i18n: &I18n,
    tid: ThemeId,
    path: &Path,
    pinned: &PinnedProjects,
) -> Vec<DropdownEntry<Message>> {
    let label = |id| i18n.message(id, &[]).into_string();
    let owned = path.to_path_buf();
    let open = |event| {
        dd_msg(
            Some(crate::icons::icon_start_folder(tid)),
            &label(ids::START_OPEN_PROJECT),
            "",
            message(event),
        )
    };
    let pin_row = |id, event| {
        dd_msg(
            Some(crate::icons::icon_start_pin(tid)),
            &label(id),
            "",
            message(event),
        )
    };
    if !pinned.contains(path) {
        return vec![
            open(StartScreenMsg::RecentProjectClicked(owned.clone())),
            pin_row(ids::START_PIN, StartScreenMsg::ProjectPinned(owned)),
        ];
    }
    let unpin = StartScreenMsg::ProjectUnpinned(owned.clone());
    if pinned.is_missing(path) {
        return vec![dd_msg(
            Some(crate::icons::icon_start_remove(tid)),
            &label(ids::START_REMOVE),
            "",
            message(unpin),
        )];
    }
    vec![
        open(StartScreenMsg::PinnedProjectClicked(owned)),
        pin_row(ids::START_UNPIN, unpin),
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
        // The menu belongs to the Start screen; it never paints over a
        // project or tab opened while it was up.
        if !self.start_screen_shown() {
            return Vec::new();
        }
        if !self.overlay_belongs_to(None, window_id) {
            return Vec::new();
        }
        let panel_ctx = &self.document_state.panel_ctx;
        let menu = super::placement::menu_card(
            start_row_menu_entries(
                &self.ui_state.i18n,
                self.ui_state.effective_theme_id(),
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

    const TID: ThemeId = ThemeId::GraphiteLight;

    fn items(path: &Path, pinned: &PinnedProjects) -> Vec<DropdownItemFacts> {
        start_row_menu_entries(&I18n::en_us(), TID, path, pinned)
            .into_iter()
            .filter_map(|e| match e {
                DropdownEntry::Item(item) => Some(DropdownItemFacts {
                    label: item.label,
                    icon: item.icon,
                    message: item.on_press?,
                }),
                _ => None,
            })
            .collect()
    }

    struct DropdownItemFacts {
        label: String,
        icon: Option<iced::widget::svg::Handle>,
        message: Message,
    }

    fn labels(rows: &[DropdownItemFacts]) -> Vec<&str> {
        rows.iter().map(|row| row.label.as_str()).collect()
    }

    fn icons(rows: &[DropdownItemFacts]) -> Vec<Option<iced::widget::svg::Handle>> {
        rows.iter().map(|row| row.icon.clone()).collect()
    }

    #[test]
    fn a_recent_row_offers_open_and_pin() {
        let path = PathBuf::from("a.snxprj");
        let rows = items(&path, &PinnedProjects::default());
        assert_eq!(labels(&rows), ["Open", "Pin"]);
        // Open goes through the Start screen so the menu closes first.
        assert!(matches!(
            &rows[0].message,
            Message::Project(crate::app::ProjectMsg::StartScreen(StartScreenMsg::RecentProjectClicked(p))) if *p == path
        ));
        assert!(matches!(
            &rows[1].message,
            Message::Project(crate::app::ProjectMsg::StartScreen(StartScreenMsg::ProjectPinned(p))) if *p == path
        ));
    }

    #[test]
    fn a_pinned_row_offers_open_and_unpin_and_a_missing_one_remove() {
        let path = PathBuf::from("a.snxprj");
        let mut pinned = PinnedProjects::default();
        pinned.pin(&path);
        let present = items(&path, &pinned);
        assert_eq!(labels(&present), ["Open", "Unpin"]);
        assert!(matches!(
            &present[0].message,
            Message::Project(crate::app::ProjectMsg::StartScreen(StartScreenMsg::PinnedProjectClicked(p))) if *p == path
        ));

        pinned.set_missing(&path, true);
        let missing = items(&path, &pinned);
        assert_eq!(labels(&missing), ["Remove"]);
        assert!(matches!(
            &missing[0].message,
            Message::Project(crate::app::ProjectMsg::StartScreen(StartScreenMsg::ProjectUnpinned(p))) if *p == path
        ));
    }

    /// m3: every row has its glyph in the icon column, so the labels line
    /// up: Open a folder, Pin and Unpin a pin, Remove a cross.
    #[test]
    fn every_menu_row_has_its_glyph() {
        let path = PathBuf::from("a.snxprj");
        let folder = Some(crate::icons::icon_start_folder(TID));
        let pin = Some(crate::icons::icon_start_pin(TID));
        let mut pinned = PinnedProjects::default();
        assert_eq!(icons(&items(&path, &pinned)), [folder.clone(), pin.clone()]);
        pinned.pin(&path);
        assert_eq!(icons(&items(&path, &pinned)), [folder, pin]);
        pinned.set_missing(&path, true);
        assert_eq!(
            icons(&items(&path, &pinned)),
            [Some(crate::icons::icon_start_remove(TID))]
        );
    }
}
