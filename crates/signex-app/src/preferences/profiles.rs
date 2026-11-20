//! Rule Profiles section (#1608): the profile list and default picker, the
//! New / Duplicate / Rename / Delete actions, and the rule editor for the
//! selected profile. The editor is the one the ERC dialog uses.

use std::sync::Arc;

use super::*;
use iced::widget::{Space, Widget, column, container, row};
use iced::{Border, Color, Element, Length, Theme};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_widgets::{button, raw, text, text_input};

use crate::app::rule_profile_ui::ProfileManager;
use crate::app::view::dialogs::{RuleEditor, rule_editor, shipped_rules};
use crate::rule_profiles::{ProfileKind, ProfileListing};

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

fn pane(msg: ProfilesMsg) -> PrefMsg {
    PrefMsg::Profiles(msg)
}

pub(super) fn content_profiles<'a>(
    manager: &'a ProfileManager,
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

    let mut body = column![header].spacing(0);
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

    let mut list = column![default_row(
        i18n,
        None,
        ids::PREFERENCES_PROFILES_AUTOMATIC,
        default_profile.is_none()
    )]
    .spacing(4);
    for listed in &manager.listing.profiles {
        let name = listed.profile.name.as_str();
        let selected = manager.selected.as_deref() == Some(name);
        let note = match listed.kind {
            ProfileKind::Builtin => Some(ids::PREFERENCES_PROFILES_NOTE_BUILTIN),
            ProfileKind::UserDefault => Some(ids::PREFERENCES_PROFILES_NOTE_USER_DEFAULT),
            ProfileKind::User => None,
        };
        list = list.push(profile_row(
            i18n,
            name,
            &listed.profile.version,
            note,
            selected,
            default_profile == Some(name),
        ));
    }
    body = body.push(container(list).padding([0, 20]).boxed());

    for problem in &manager.listing.problems {
        body = body.push(
            container(
                // raw: the library reader words its problems, in English (#2061).
                text(raw(problem.clone()))
                    .size(TS.caption)
                    .style(text_warning),
            )
            .padding([4, 20])
            .boxed(),
        );
    }
    body = body.push(h_sep()).push(actions_row(manager, i18n));
    if let Some((is_error, message)) = &manager.status {
        // raw: the action result is set by its handler (#2061: handler-worded state text).
        let styled = text(raw(message.clone())).size(TS.caption);
        body = body.push(
            container(if *is_error {
                styled.style(text_warning)
            } else {
                styled.style(text_muted)
            })
            .padding([4, 20])
            .boxed(),
        );
    }
    body = body.push(h_sep());

    match manager.selected_profile() {
        Some(listed) => {
            let editable = listed.kind != ProfileKind::Builtin;
            let resolved = signex_erc::project_rules::resolve(&listed.profile.erc);
            let caption = t!(
                i18n,
                if editable {
                    ids::PREFERENCES_PROFILES_EDITING
                } else {
                    ids::PREFERENCES_PROFILES_READ_ONLY
                },
                "name" => listed.profile.name.as_str()
            );
            let editor = rule_editor(RuleEditor {
                i18n,
                tokens,
                severity: &resolved.severity,
                pin_matrix: &resolved.pin_matrix,
                shipped: shipped_rules(),
                editable,
                changed_only: manager.rule_list.changed_only,
                filter: &manager.rule_list.query,
                on_severity: |rule, severity| pane(ProfilesMsg::SeverityChanged(rule, severity)),
                on_pin_cell: |row, col| pane(ProfilesMsg::PinCellCycled { row, col }),
                on_changed_only: |on| pane(ProfilesMsg::ChangedOnlyToggled(on)),
                on_filter: |query| pane(ProfilesMsg::RuleFilterChanged(query)),
            });
            let mut editing = column![text(caption).size(TS.label).style(text_primary)].spacing(8);
            if editable {
                editing = editing.push(reset_row(manager, &listed.profile.name, i18n));
            }
            body = body.push(editing.push(editor).padding([12, 20]).boxed());
        }
        None => {
            body = body.push(
                container(
                    text(t!(i18n, ids::PREFERENCES_PROFILES_NONE_SELECTED))
                        .size(TS.body)
                        .style(text_muted),
                )
                .padding([12, 20])
                .boxed(),
            );
        }
    }
    body.boxed()
}

/// One selectable profile: its name opens it in the editor, the second button
/// makes it the default for new projects.
fn profile_row<'a>(
    i18n: &I18n,
    name: &'a str,
    version: &'a str,
    note: Option<MessageId>,
    selected: bool,
    is_default: bool,
) -> Element<'a, PrefMsg> {
    // The name and version are the user's; the kind note is ours.
    let label = match note {
        None => t!(
            i18n,
            ids::PREFERENCES_PROFILES_ROW,
            "name" => name,
            "version" => version
        ),
        Some(note) => t!(
            i18n,
            ids::PREFERENCES_PROFILES_ROW_NOTE,
            "name" => name,
            "version" => version,
            "note" => t!(i18n, note).into_string()
        ),
    };
    row![
        button(text(label).size(TS.label))
            .padding([4, 10])
            .width(Length::Fill)
            .on_press(pane(ProfilesMsg::Selected(name.to_string())))
            .style(move |theme: &Theme, status| row_style(theme, status, selected)),
        default_button(i18n, Some(name), is_default),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center)
    .boxed()
}

fn default_row<'a>(
    i18n: &I18n,
    name: Option<&'a str>,
    label: MessageId,
    is_default: bool,
) -> Element<'a, PrefMsg> {
    row![
        text(t!(i18n, label))
            .size(TS.body)
            .style(text_muted)
            .width(Length::Fill),
        default_button(i18n, name, is_default),
    ]
    .spacing(8)
    .padding([0, 10])
    .align_y(iced::Alignment::Center)
    .boxed()
}

fn default_button<'a>(
    i18n: &I18n,
    name: Option<&'a str>,
    is_default: bool,
) -> Element<'a, PrefMsg> {
    if is_default {
        return text(t!(i18n, ids::PREFERENCES_PROFILES_IS_DEFAULT))
            .size(TS.body)
            .style(text_primary)
            .boxed();
    }
    button(text(t!(i18n, ids::PREFERENCES_PROFILES_USE_AS_DEFAULT)).size(TS.body))
        .padding([4, 10])
        .on_press(pane(ProfilesMsg::DefaultChosen(name.map(str::to_string))))
        .style(secondary_button_style)
        .boxed()
}

fn row_style(theme: &Theme, status: button::Status, selected: bool) -> button::Style {
    let palette = theme.palette();
    let background = if selected {
        palette.primary.weak.color
    } else if matches!(status, button::Status::Hovered) {
        palette.background.strong.color
    } else {
        palette.background.weak.color
    };
    button::Style {
        background: Some(iced::Background::Color(background)),
        text_color: if selected {
            palette.primary.weak.text
        } else {
            palette.background.base.text
        },
        border: Border {
            width: 1.0,
            radius: 3.0.into(),
            color: if selected {
                palette.primary.base.color
            } else {
                Color::TRANSPARENT
            },
            ..Border::default()
        },
        ..button::Style::default()
    }
}

fn actions_row<'a>(manager: &'a ProfileManager, i18n: &I18n) -> Element<'a, PrefMsg> {
    let selected_kind = manager.selected_profile().map(|l| l.kind);
    let named = !manager.name_input.trim().is_empty();
    let action = |label: MessageId, enabled: bool, msg: ProfilesMsg| {
        button(text(t!(i18n, label)).size(TS.body))
            .padding([5, 12])
            .on_press_maybe(enabled.then(|| pane(msg)))
            .style(secondary_button_style)
    };
    let mut buttons = row![
        action(
            ids::PREFERENCES_PROFILES_NEW,
            named,
            ProfilesMsg::CreateRequested
        ),
        action(
            ids::PREFERENCES_PROFILES_DUPLICATE,
            named && selected_kind.is_some(),
            ProfilesMsg::DuplicateRequested
        ),
        action(
            ids::PREFERENCES_PROFILES_RENAME,
            named && selected_kind == Some(ProfileKind::User),
            ProfilesMsg::RenameRequested
        ),
    ]
    .spacing(8);
    buttons = match &manager.confirm_delete {
        Some(name) => buttons
            .push(
                text(t!(
                    i18n,
                    ids::PREFERENCES_PROFILES_DELETE_CONFIRM,
                    "name" => name.as_str()
                ))
                .size(TS.body)
                .style(text_warning)
                .boxed(),
            )
            .push(
                button(text(t!(i18n, ids::DIALOG_BUTTON_DELETE)).size(TS.body))
                    .padding([5, 12])
                    .on_press(pane(ProfilesMsg::DeleteConfirmed))
                    .style(danger_button_style)
                    .boxed(),
            )
            .push(
                button(text(t!(i18n, ids::PREFERENCES_PROFILES_KEEP)).size(TS.body))
                    .padding([5, 12])
                    .on_press(pane(ProfilesMsg::DeleteCancelled))
                    .style(secondary_button_style)
                    .boxed(),
            ),
        None => buttons.push(
            action(
                ids::DIALOG_BUTTON_DELETE,
                selected_kind == Some(ProfileKind::User),
                ProfilesMsg::DeleteRequested,
            )
            .boxed(),
        ),
    };
    container(
        column![
            row![
                text(t!(i18n, ids::PREFERENCES_PROFILES_NAME))
                    .size(TS.body)
                    .style(text_muted),
                text_input(t!(i18n, ids::PLACEHOLDER_PROFILE_NAME), &manager.name_input)
                    .size(TS.label)
                    .on_input(|v| pane(ProfilesMsg::NameInputChanged(v))),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
            buttons,
        ]
        .spacing(8),
    )
    .padding([8, 20])
    .boxed()
}

/// "Reset to Signex Default" for an editable profile, with its confirmation.
fn reset_row<'a>(manager: &'a ProfileManager, name: &'a str, i18n: &I18n) -> Element<'a, PrefMsg> {
    if manager.confirm_reset.as_deref() == Some(name) {
        return row![
            text(t!(
                i18n,
                ids::PREFERENCES_PROFILES_RESET_CONFIRM,
                "name" => name
            ))
            .size(TS.body)
            .style(text_warning),
            button(text(t!(i18n, ids::PREFERENCES_PROFILES_RESET)).size(TS.body))
                .padding([5, 12])
                .on_press(pane(ProfilesMsg::ResetConfirmed))
                .style(danger_button_style),
            button(text(t!(i18n, ids::PREFERENCES_PROFILES_KEEP)).size(TS.body))
                .padding([5, 12])
                .on_press(pane(ProfilesMsg::ResetCancelled))
                .style(secondary_button_style),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .boxed();
    }
    button(text(t!(i18n, ids::PREFERENCES_PROFILES_RESET_TO_DEFAULT)).size(TS.body))
        .padding([5, 12])
        .on_press_maybe((!manager.busy).then(|| pane(ProfilesMsg::ResetRequested)))
        .style(secondary_button_style)
        .boxed()
}

#[cfg(test)]
mod tests {
    use signex_i18n::{Arg, I18n, LanguagePref, ids};

    fn row(i18n: &I18n) -> String {
        i18n.message_plain(
            ids::PREFERENCES_PROFILES_ROW_NOTE,
            &[
                ("name", Arg::from("Signex Default")),
                ("version", Arg::from("1.2")),
                ("note", Arg::from("shipped, read-only")),
            ],
        )
    }

    /// #2058: a profile row keeps its English and follows the language.
    #[test]
    fn a_profile_row_keeps_its_english_and_follows_the_language() {
        assert_eq!(
            row(&I18n::en_us()),
            "Signex Default  v1.2  (shipped, read-only)"
        );
        let xa = I18n::new(&LanguagePref::Locale("en-XA".to_owned()));
        assert_ne!(row(&xa), row(&I18n::en_us()));
        assert_ne!(
            xa.message_plain(ids::PREFERENCES_PROFILES_TITLE, &[]),
            "Rule Profiles"
        );
    }
}
