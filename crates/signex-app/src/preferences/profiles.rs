//! Rule Profiles page (#1608, mockup rows 92 and 148): the profile list with
//! its default radios and actions on the left (`list`), the selected
//! profile's rule editor on the right (`editor`). The editor is the one the
//! ERC dialog uses, shown one half at a time.

mod editor;
mod list;
mod page_state;
mod rule_list_filter;

pub use page_state::{ProfileEditorTab, ProfileNameAction};
pub use rule_list_filter::RuleListFilter;

use std::sync::Arc;

use super::*;
use iced::widget::{Space, Widget, column, container, row};
use iced::{Element, Length};
use signex_i18n::{I18n, ids, t};
use signex_widgets::{raw, text};

use crate::app::rule_profile_ui::{MachineRulesIssue, ProfileManager};
use crate::rule_profiles::{ProfileKind, ProfileListing};

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
    machine_issue: Option<MachineRulesIssue<'a>>,
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

    let selected = manager.selected_profile();
    let notice = selected
        .filter(|l| l.kind == ProfileKind::UserDefault)
        .and(machine_issue);
    // "Saved" is claimed only when no edit is on its way and this profile's
    // shown rules are the ones in its file.
    let is_saved = !saving
        && !manager.busy
        && notice.is_none()
        && !selected.is_some_and(|l| manager.write_failed(&l.profile.name));
    let notice = notice.map(|issue| issue.localized(i18n));
    let right: Element<'a, PrefMsg> = match selected {
        Some(listed) => editor::view(manager, listed, is_saved, notice, tokens, i18n),
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

    mod saved_pill {
        use std::sync::Arc;

        use signex_erc::{RuleKind, Severity};
        use signex_types::project::ErcRules;

        use super::super::content_profiles;
        use crate::app::{ErcMsg, Message, PreferencesMsg, Signex};
        use crate::preferences::{PrefMsg, ProfileAction, ProfilesMsg};
        use crate::rule_profiles::{self, USER_DEFAULT_PROFILE_NAME};

        fn send(app: &mut Signex, msg: ProfilesMsg) {
            let _ = app.update(Message::Preferences(PreferencesMsg::Inner(
                PrefMsg::Profiles(msg),
            )));
        }

        fn write_failed(app: &mut Signex) {
            let _ = app.update(Message::Erc(ErcMsg::ProfileWriteFinished(Arc::new(Err(
                "disk full.".to_string(),
            )))));
        }

        fn relist(app: &mut Signex, lib: &std::path::Path) {
            let listing = rule_profiles::list_library(Some(lib));
            send(app, ProfilesMsg::Listed(Arc::new(listing)));
        }

        fn page(app: &Signex) -> Vec<String> {
            let element = content_profiles(
                &app.ui_state.profile_manager,
                !app.ui_state.profile_writes.is_idle(),
                app.machine_rules_issue(),
                None,
                &app.document_view.panel_ctx.tokens,
                &app.ui_state.i18n,
            );
            crate::layout_probe::lay_out_texts(element, iced::Size::new(1000.0, 900.0))
                .into_iter()
                .map(|t| t.text)
                .collect()
        }

        fn shows_saved(app: &Signex) -> bool {
            page(app).iter().any(|t| t == "Saved")
        }

        fn app_with_library(lib: &std::path::Path) -> Signex {
            rule_profiles::write_user_default(lib, ErcRules::default()).unwrap();
            rule_profiles::save_named_profile(lib, "Company", "1.0", ErcRules::default(), false)
                .unwrap();
            let (mut app, _) = Signex::new();
            app.ui_state.erc_severity_override.clear();
            app.ui_state.pin_matrix_overrides.clear();
            relist(&mut app, lib);
            app
        }

        /// #2765: after a failed write of the machine's rules the page does
        /// not claim Saved, says why, and shows the rule ERC runs with.
        #[test]
        fn a_failed_machine_write_is_not_saved_and_the_page_agrees_with_erc() {
            let lib = tempfile::tempdir().unwrap();
            let mut app = app_with_library(lib.path());
            send(
                &mut app,
                ProfilesMsg::Selected(USER_DEFAULT_PROFILE_NAME.into()),
            );
            assert!(shows_saved(&app));

            send(
                &mut app,
                ProfilesMsg::SeverityChanged(RuleKind::UnusedPin, Severity::Off),
            );
            write_failed(&mut app);
            relist(&mut app, lib.path());

            let texts = page(&app);
            assert!(!texts.iter().any(|t| t == "Saved"), "{texts:#?}");
            assert!(
                texts
                    .iter()
                    .any(|t| t.contains("not saving") && t.contains("disk full")),
                "{texts:#?}"
            );
            let listed = app.ui_state.profile_manager.selected_profile().unwrap();
            let shown = signex_erc::project_rules::resolve(&listed.profile.erc).severity;
            assert_eq!(shown.get(&RuleKind::UnusedPin), Some(&Severity::Off));
            assert_eq!(
                shown.get(&RuleKind::UnusedPin),
                app.ui_state.erc_severity_override.get(&RuleKind::UnusedPin),
            );
        }

        /// #2765: a refused name says nothing about any file, and a failed
        /// write hides Saved on its own profile only, until the files are read.
        #[test]
        fn an_old_error_hides_saved_only_on_the_profile_it_belongs_to() {
            let lib = tempfile::tempdir().unwrap();
            let mut app = app_with_library(lib.path());
            send(&mut app, ProfilesMsg::Selected("Company".into()));
            let refused: Result<ProfileAction, String> = Err("That name is taken.".into());
            send(&mut app, ProfilesMsg::ActionFinished(Arc::new(refused)));
            assert!(shows_saved(&app), "a refused name hid Saved");

            send(
                &mut app,
                ProfilesMsg::SeverityChanged(RuleKind::UnusedPin, Severity::Off),
            );
            write_failed(&mut app);
            assert!(!shows_saved(&app), "the failed profile still claims Saved");
            send(
                &mut app,
                ProfilesMsg::Selected(USER_DEFAULT_PROFILE_NAME.into()),
            );
            assert!(shows_saved(&app), "another profile lost Saved");

            send(&mut app, ProfilesMsg::Selected("Company".into()));
            relist(&mut app, lib.path());
            assert!(shows_saved(&app), "the page now shows what the file holds");
        }
    }
}
