//! Rule Profiles page (#1608, mockup rows 92 and 148): the profile list with
//! its default radios and actions on the left (`list`), the selected
//! profile's rule editor on the right (`editor`). The editor is the one the
//! ERC dialog uses, shown one half at a time.

mod editor;
mod list;

use std::sync::Arc;

use super::*;
use iced::widget::{Space, Widget, column, container, row};
use iced::{Element, Length};
use signex_i18n::{I18n, ids, t};
use signex_widgets::{raw, text};

use crate::app::rule_profile_ui::{ProfileEditorTab, ProfileManager, ProfileNameAction};
use crate::rule_profiles::ProfileListing;

/// The profile column: 300 px at the 12 px body.
const LIST_W: f32 = TS.body * 25.0;
/// The gap between the list and the editor.
const COLUMN_GAP: f32 = TS.body * 4.0 / 3.0;

/// What an action on the profile library did, for the status line and the
/// follow-up work (the default choice must follow a rename or delete).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileAction {
    Created(String),
    Renamed { old: String, new: String },
    Deleted(String),
}

/// Messages of the Rule Profiles pane. Nested under `PrefMsg::Profiles`.
#[derive(Debug, Clone)]
pub enum ProfilesMsg {
    /// The library was read.
    Listed(Arc<ProfileListing>),
    /// A profile was picked in the list.
    Selected(String),
    /// The page's own view state changed.
    Page(ProfilePageMsg),
    /// A severity cell of the editor was clicked.
    SeverityChanged(signex_erc::RuleKind, signex_erc::Severity),
    /// A pin-matrix cell of the editor was clicked.
    PinCellCycled { row: u8, col: u8 },
    /// The editor's "Changed only" chip was switched; carries the new state.
    ChangedOnlyToggled(bool),
    /// The editor's "Filter rules" field changed.
    RuleFilterChanged(String),
    /// The name field changed.
    NameInputChanged(String),
    /// The default profile for new projects was chosen; `None` is automatic.
    DefaultChosen(Option<String>),
    /// New profile from the shipped rules, named by the name field.
    CreateRequested,
    /// Copy of the selected profile, named by the name field.
    DuplicateRequested,
    /// Rename the selected profile to the name field.
    RenameRequested,
    /// Ask before putting the selected profile's rules back to the shipped
    /// ones.
    ResetRequested,
    /// The user confirmed the reset.
    ResetConfirmed,
    /// The user backed out of the reset.
    ResetCancelled,
    /// Ask before deleting the selected profile.
    DeleteRequested,
    /// The user confirmed the delete.
    DeleteConfirmed,
    /// The user backed out of the delete.
    DeleteCancelled,
    /// A create / duplicate / rename / delete ended.
    ActionFinished(Arc<Result<ProfileAction, String>>),
}

/// The page's view state: the editor tab, the one open rule, the name field.
#[derive(Debug, Clone)]
pub enum ProfilePageMsg {
    /// Severity per rule or Pin Connection Matrix was picked.
    EditorTabPicked(ProfileEditorTab),
    /// A rule's name was clicked: the editor shows that rule alone.
    RuleOpened(signex_erc::RuleKind),
    /// The one-rule editor was closed.
    RuleClosed,
    /// New, Duplicate or Rename was pressed: the name field opens for it.
    NamePromptOpened(ProfileNameAction),
    /// The name field's Cancel was pressed.
    NamePromptCancelled,
    /// The name field was confirmed: its action runs.
    NamePromptConfirmed,
}

fn pane(msg: ProfilesMsg) -> PrefMsg {
    PrefMsg::Profiles(msg)
}

fn page(msg: ProfilePageMsg) -> PrefMsg {
    pane(ProfilesMsg::Page(msg))
}

pub(super) fn content_profiles<'a>(
    manager: &'a ProfileManager,
    saving: bool,
    default_profile: Option<&'a str>,
    tokens: &'a signex_types::theme::ThemeTokens,
    i18n: &'a I18n,
) -> Element<'a, PrefMsg> {
    let header = column![
        text(t!(i18n, ids::PREFERENCES_PROFILES_TITLE))
            .size(TS.heading)
            .style(text_primary),
        Space::new().height(4),
        text(t!(i18n, ids::PREFERENCES_PROFILES_HELP))
            .size(TS.body)
            .style(text_muted),
    ]
    .padding([16, 20]);

    let body = column![header].spacing(0);
    if !manager.loaded {
        return body
            .push(
                container(
                    text(t!(i18n, ids::PREFERENCES_PROFILES_READING))
                        .size(TS.body)
                        .style(text_muted),
                )
                .padding([0, 20])
                .boxed(),
            )
            .boxed();
    }

    let mut left = list::view(manager, default_profile, tokens, i18n);
    for problem in &manager.listing.problems {
        left = left.push(
            // raw: the library reader words its problems, in English (#2061).
            text(raw(problem.clone()))
                .size(TS.caption)
                .style(text_warning)
                .boxed(),
        );
    }
    if let Some((is_error, message)) = &manager.status {
        // raw: the action result is set by its handler (#2061: handler-worded state text).
        let styled = text(raw(message.clone())).size(TS.caption);
        left = left.push(if *is_error {
            styled.style(text_warning).boxed()
        } else {
            styled.style(text_muted).boxed()
        });
    }

    // "Saved" is claimed only when no edit is on its way and none failed.
    let is_saved = !saving && !manager.busy && !matches!(manager.status, Some((true, _)));
    let right: Element<'a, PrefMsg> = match manager.selected_profile() {
        Some(listed) => editor::view(manager, listed, is_saved, tokens, i18n),
        None => text(t!(i18n, ids::PREFERENCES_PROFILES_NONE_SELECTED))
            .size(TS.body)
            .style(text_muted)
            .boxed(),
    };
    body.push(
        row![
            container(left).width(LIST_W),
            container(right).width(Length::Fill),
        ]
        .spacing(COLUMN_GAP)
        .padding([0, 20])
        .boxed(),
    )
    .boxed()
}

#[cfg(test)]
mod tests {
    use signex_i18n::{Arg, I18n, LanguagePref, ids};

    fn row(i18n: &I18n) -> String {
        i18n.message_plain(
            ids::PREFERENCES_PROFILES_ROW,
            &[
                ("name", Arg::from("Signex Default")),
                ("version", Arg::from("1.2")),
            ],
        )
    }

    /// #2058: a profile row keeps its English and follows the language.
    #[test]
    fn a_profile_row_keeps_its_english_and_follows_the_language() {
        assert_eq!(row(&I18n::en_us()), "Signex Default  v1.2");
        let xa = I18n::new(&LanguagePref::Locale("en-XA".to_owned()));
        assert_ne!(row(&xa), row(&I18n::en_us()));
        assert_ne!(
            xa.message_plain(ids::PREFERENCES_PROFILES_TITLE, &[]),
            "Rule Profiles"
        );
    }
}
