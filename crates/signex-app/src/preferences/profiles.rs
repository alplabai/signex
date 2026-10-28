//! Rule Profiles section (#1608): the profile list and default picker, the
//! New / Duplicate / Rename / Delete actions, and the rule editor for the
//! selected profile. The editor is the one the ERC dialog uses.

use std::sync::Arc;

use super::*;
use iced::widget::{Space, button, column, container, row, text, text_input};
use iced::{Border, Color, Element, Length, Theme};

use crate::app::rule_profile_ui::ProfileManager;
use crate::app::view::dialogs::{RuleEditor, rule_editor};
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
) -> Element<'a, PrefMsg> {
    let header = column![
        text("Rule Profiles").size(15).style(text_primary),
        Space::new().height(4),
        text(
            "A profile is a named set of ERC rules. A new project starts with a copy of the \
             default profile. Changes here are saved to the profile's file at once."
        )
        .size(11)
        .style(text_muted),
    ]
    .padding([16, 20]);

    let mut body = column![header].spacing(0);
    if !manager.loaded {
        return body
            .push(
                container(
                    text("Reading the profile library...")
                        .size(11)
                        .style(text_muted),
                )
                .padding([0, 20]),
            )
            .into();
    }

    let mut list = column![default_row(
        None,
        "Automatic (User Default if you have one, else Signex Default)",
        default_profile.is_none()
    )]
    .spacing(4);
    for listed in &manager.listing.profiles {
        let name = listed.profile.name.as_str();
        let selected = manager.selected.as_deref() == Some(name);
        let note = match listed.kind {
            ProfileKind::Builtin => "shipped, read-only",
            ProfileKind::UserDefault => "this machine's rules",
            ProfileKind::User => "",
        };
        list = list.push(profile_row(
            name,
            &listed.profile.version,
            note,
            selected,
            default_profile == Some(name),
        ));
    }
    body = body.push(container(list).padding([0, 20]));

    for problem in &manager.listing.problems {
        body = body
            .push(container(text(problem.clone()).size(10).style(text_warning)).padding([4, 20]));
    }
    body = body.push(h_sep()).push(actions_row(manager));
    if let Some((is_error, message)) = &manager.status {
        let styled = text(message.clone()).size(10);
        body = body.push(
            container(if *is_error {
                styled.style(text_warning)
            } else {
                styled.style(text_muted)
            })
            .padding([4, 20]),
        );
    }
    body = body.push(h_sep());

    match manager.selected_profile() {
        Some(listed) => {
            let editable = listed.kind != ProfileKind::Builtin;
            let resolved = signex_erc::project_rules::resolve(&listed.profile.erc);
            let caption = if editable {
                format!("Editing \"{}\"", listed.profile.name)
            } else {
                format!(
                    "\"{}\" is read-only. Duplicate it to change the rules.",
                    listed.profile.name
                )
            };
            let editor = rule_editor(RuleEditor {
                tokens,
                severity: &resolved.severity,
                pin_matrix: &resolved.pin_matrix,
                editable,
                on_severity: |rule, severity| pane(ProfilesMsg::SeverityChanged(rule, severity)),
                on_pin_cell: |row, col| pane(ProfilesMsg::PinCellCycled { row, col }),
            });
            body = body.push(
                column![text(caption).size(12).style(text_primary), editor]
                    .spacing(8)
                    .padding([12, 20]),
            );
        }
        None => {
            body = body.push(
                container(text("No profile is selected.").size(11).style(text_muted))
                    .padding([12, 20]),
            );
        }
    }
    body.into()
}

/// One selectable profile: its name opens it in the editor, the second button
/// makes it the default for new projects.
fn profile_row<'a>(
    name: &'a str,
    version: &'a str,
    note: &'a str,
    selected: bool,
    is_default: bool,
) -> Element<'a, PrefMsg> {
    let label = if note.is_empty() {
        format!("{name}  v{version}")
    } else {
        format!("{name}  v{version}  ({note})")
    };
    row![
        button(text(label).size(12))
            .padding([4, 10])
            .width(Length::Fill)
            .on_press(pane(ProfilesMsg::Selected(name.to_string())))
            .style(move |theme: &Theme, status| row_style(theme, status, selected)),
        default_button(Some(name), is_default),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center)
    .into()
}

fn default_row<'a>(
    name: Option<&'a str>,
    label: &'a str,
    is_default: bool,
) -> Element<'a, PrefMsg> {
    row![
        text(label).size(11).style(text_muted).width(Length::Fill),
        default_button(name, is_default),
    ]
    .spacing(8)
    .padding([0, 10])
    .align_y(iced::Alignment::Center)
    .into()
}

fn default_button<'a>(name: Option<&'a str>, is_default: bool) -> Element<'a, PrefMsg> {
    if is_default {
        return text("Default for new projects")
            .size(11)
            .style(text_primary)
            .into();
    }
    button(text("Use as default").size(11))
        .padding([4, 10])
        .on_press(pane(ProfilesMsg::DefaultChosen(name.map(str::to_string))))
        .style(secondary_button_style)
        .into()
}

fn row_style(theme: &Theme, status: button::Status, selected: bool) -> button::Style {
    let palette = theme.extended_palette();
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
        },
        ..button::Style::default()
    }
}

fn actions_row(manager: &ProfileManager) -> Element<'_, PrefMsg> {
    let selected_kind = manager.selected_profile().map(|l| l.kind);
    let named = !manager.name_input.trim().is_empty();
    let action = |label: &'static str, enabled: bool, msg: ProfilesMsg| {
        button(text(label).size(11))
            .padding([5, 12])
            .on_press_maybe(enabled.then(|| pane(msg)))
            .style(secondary_button_style)
    };
    let mut buttons = row![
        action("New", named, ProfilesMsg::CreateRequested),
        action(
            "Duplicate",
            named && selected_kind.is_some(),
            ProfilesMsg::DuplicateRequested
        ),
        action(
            "Rename",
            named && selected_kind == Some(ProfileKind::User),
            ProfilesMsg::RenameRequested
        ),
    ]
    .spacing(8);
    buttons = match &manager.confirm_delete {
        Some(name) => buttons
            .push(
                text(format!("Delete \"{name}\"?"))
                    .size(11)
                    .style(text_warning),
            )
            .push(
                button(text("Delete").size(11))
                    .padding([5, 12])
                    .on_press(pane(ProfilesMsg::DeleteConfirmed))
                    .style(danger_button_style),
            )
            .push(
                button(text("Keep").size(11))
                    .padding([5, 12])
                    .on_press(pane(ProfilesMsg::DeleteCancelled))
                    .style(secondary_button_style),
            ),
        None => buttons.push(action(
            "Delete",
            selected_kind == Some(ProfileKind::User),
            ProfilesMsg::DeleteRequested,
        )),
    };
    container(
        column![
            row![
                text("Name").size(11).style(text_muted),
                text_input("Profile name", &manager.name_input)
                    .size(12)
                    .on_input(|v| pane(ProfilesMsg::NameInputChanged(v))),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
            buttons,
        ]
        .spacing(8),
    )
    .padding([8, 20])
    .into()
}
