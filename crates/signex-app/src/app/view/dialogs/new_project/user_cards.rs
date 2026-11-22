//! "Your profiles" (#2232, #2536): one compact row per saved profile with
//! its menu (Edit profile..., Rename, Duplicate, Delete), the Rename field,
//! the Delete confirm, and Save as profile with its name field. Shared by
//! every setup dialog and the Project tab.

use iced::widget::{Space, column, container, row};
use iced::{Alignment, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::project::{ProfileNameProblem, UserProfile, saved_profiles};
use signex_types::schematic::{SheetOrientation, SheetSize};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, button::Status, text, text_input};

use super::card_parts::{MENU_BUTTON, icon_button, menu_row, menu_surface, more_icon};
use super::cards::mm;
use super::{SP, TS};
use crate::app::new_project::{NewProjectMsg, NewProjectState, ProfileTarget, SaveProfileProblem};
use crate::styles::{Radii, ti, tokens};

/// Height of a saved profile's row.
const UCARD_H: f32 = TS.body * 34.0 / 12.0;

/// Which values a saved profile's caption sums up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Caption {
    /// Copper layers, clearance and track: `4L · 0.15/0.2`.
    Pcb,
    /// Sheet size and designator standard: `A3 · IEC 81346-2`.
    Sheet,
}

fn caption_text(profile: &UserProfile, caption: Caption, i18n: &I18n) -> Localized {
    match caption {
        Caption::Pcb => t!(
            i18n,
            ids::SETUP_PROFILE_UCARD_PCB,
            "layers" => u32::from(profile.pcb.copper_layers),
            "clearance" => mm(profile.pcb.clearance_mm),
            "track" => mm(profile.pcb.track_width_mm)
        ),
        Caption::Sheet => t!(
            i18n,
            ids::SETUP_PROFILE_UCARD_SHEET,
            "paper" => SheetSize::parse(&profile.sheet.paper_size)
                .0
                .token(SheetOrientation::Landscape),
            "designators" => profile.designators.label()
        ),
    }
}

/// Look of a saved profile's row: panel fill and a hairline, the hover token
/// on hover; picked: the accent border over the soft accent.
pub(in crate::app::view) fn ucard_style(
    tokens: &ThemeTokens,
    picked: bool,
    status: Status,
) -> button::Style {
    let mut style = super::cards::card_style(tokens, picked, status);
    style.border.width = 1.0;
    style.border.radius = Radii::GRAPHITE.button.into();
    style
}

/// A saved profile's menu: Edit profile..., Rename, Duplicate, Delete.
fn profile_menu<'a>(
    profile: &UserProfile,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    let id = profile.id.clone();
    let copy = t!(i18n, ids::SETUP_PROFILE_COPY_NAME, "name" => profile.name.as_str());
    menu_surface(
        vec![
            menu_row(
                t!(i18n, ids::DIALOG_NEW_PROJECT_CARD_EDIT),
                NewProjectMsg::ProfileEditRequested(ProfileTarget::Saved(id.clone())),
                tokens,
            ),
            menu_row(
                t!(i18n, ids::SETUP_PROFILE_RENAME),
                NewProjectMsg::ProfileRenameRequested(id.clone()),
                tokens,
            ),
            menu_row(
                t!(i18n, ids::SETUP_PROFILE_DUPLICATE),
                NewProjectMsg::ProfileDuplicateRequested {
                    source: ProfileTarget::Saved(id.clone()),
                    name: copy.into_string(),
                },
                tokens,
            ),
            menu_row(
                t!(i18n, ids::SETUP_PROFILE_DELETE),
                NewProjectMsg::UserProfileRemoveRequested(id),
                tokens,
            ),
        ],
        tokens,
    )
}

/// One saved profile's row: name, caption and the menu button.
fn ucard<'a>(
    state: &NewProjectState,
    profile: &UserProfile,
    caption: Caption,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    let picked = state.setup.user_profile.as_deref() == Some(profile.id.as_str());
    let target = ProfileTarget::Saved(profile.id.clone());
    let open = state.profile_menu.as_ref() == Some(&target);
    let toggle = NewProjectMsg::ProfileMenuToggled((!open).then_some(target));
    let content = row![
        text(signex_i18n::UserText::new(profile.name.clone()))
            .size(TS.body)
            .color(ti(tokens.text))
            .wrapping(iced::widget::text::Wrapping::None),
        text(caption_text(profile, caption, i18n))
            .size(TS.caption)
            .font(super::super::graphite::mono())
            .color(tokens::resolve(tokens).ink4)
            .wrapping(iced::widget::text::Wrapping::None),
        icon_button(
            more_icon(tokens::resolve(tokens).ink4),
            MENU_BUTTON,
            open,
            toggle,
            tokens,
        ),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);
    let style_tokens = *tokens;
    button(content)
        .height(Length::Fixed(UCARD_H))
        .padding(iced::Padding {
            top: 0.0,
            right: SP.m + SP.xs,
            bottom: 0.0,
            left: SP.m,
        })
        .on_press(NewProjectMsg::UserProfilePicked(profile.id.clone()))
        .style(move |_: &Theme, status| ucard_style(&style_tokens, picked, status))
        .boxed()
}

/// The Rename field in place of a saved profile's row: the name, Save and
/// Cancel, and why the last name was refused.
fn rename_field<'a>(
    state: &NewProjectState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Option<Element<'a, NewProjectMsg>> {
    let renaming = state.renaming.as_ref()?;
    let input = text_input(
        t!(i18n, ids::SETUP_PROFILE_NAME_PLACEHOLDER),
        renaming.name.clone(),
    )
    .style(crate::panels::form_look::text_input_style)
    .on_input(NewProjectMsg::ProfileRenameEdited)
    .on_submit(NewProjectMsg::ProfileRenameConfirmed)
    .size(TS.label)
    .padding(SP.s)
    .width(Length::Fixed(TS.label * 16.0));
    let save = button(text(t!(i18n, ids::SETUP_PROFILE_SAVE)).size(TS.label))
        .padding([SP.s, SP.l])
        .on_press(NewProjectMsg::ProfileRenameConfirmed)
        .style(crate::styles::outline_button(tokens));
    let cancel = button(text(t!(i18n, ids::SETUP_PROFILE_CANCEL)).size(TS.label))
        .padding([SP.s, SP.l])
        .on_press(NewProjectMsg::ProfileRenameCancelled)
        .style(crate::styles::ghost_button(tokens));
    let mut col = column![
        row![input, save, cancel]
            .spacing(SP.s)
            .align_y(Alignment::Center)
    ]
    .spacing(SP.xs);
    if let Some(problem) = renaming.problem {
        col = col.push(
            text(name_problem_text(problem, i18n))
                .size(TS.caption)
                .color(tokens::danger(tokens))
                .boxed(),
        );
    }
    Some(col.boxed())
}

/// The inline confirm of Delete: the question, then Remove (danger) and
/// Keep.
fn remove_confirm<'a>(
    profile: &UserProfile,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    let question = text(t!(
        i18n,
        ids::SETUP_PROFILE_REMOVE_CONFIRM,
        "name" => profile.name.as_str()
    ))
    .size(TS.body)
    .color(ti(tokens.text));
    let remove = button(
        text(t!(i18n, ids::SETUP_PROFILE_REMOVE))
            .size(TS.body)
            .color(tokens::danger(tokens)),
    )
    .padding([SP.xs, SP.s])
    .on_press(NewProjectMsg::UserProfileRemoveConfirmed)
    .style(crate::styles::outline_button(tokens));
    let keep = button(text(t!(i18n, ids::SETUP_PROFILE_KEEP)).size(TS.body))
        .padding([SP.xs, SP.s])
        .on_press(NewProjectMsg::UserProfileRemoveCancelled)
        .style(crate::styles::ghost_button(tokens));
    row![question, remove, keep]
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .boxed()
}

fn name_problem_text(problem: ProfileNameProblem, i18n: &I18n) -> Localized {
    match problem {
        ProfileNameProblem::Empty => t!(i18n, ids::SETUP_PROFILE_ERR_NAME_EMPTY),
        ProfileNameProblem::TooLong => t!(
            i18n,
            ids::SETUP_PROFILE_ERR_NAME_TOO_LONG,
            "max" => signex_types::project::MAX_PROFILE_NAME_CHARS as u32
        ),
        ProfileNameProblem::Taken => t!(i18n, ids::SETUP_PROFILE_ERR_NAME_TAKEN),
    }
}

/// The Save as profile control: a ghost button, or the name field with Save
/// and Cancel while it is open.
pub(crate) fn save_as<'a>(
    state: &NewProjectState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    let Some(name) = &state.save_profile_name else {
        return button(text(t!(i18n, ids::SETUP_PROFILE_SAVE_AS)).size(TS.label))
            .padding([SP.s, SP.l])
            .on_press(NewProjectMsg::SaveProfileOpened)
            .style(crate::styles::ghost_button(tokens))
            .boxed();
    };
    let input = text_input(t!(i18n, ids::SETUP_PROFILE_NAME_PLACEHOLDER), name.clone())
        .style(crate::panels::form_look::text_input_style)
        .on_input(NewProjectMsg::SaveProfileNameEdited)
        .on_submit(NewProjectMsg::SaveProfileConfirmed)
        .size(TS.label)
        .padding(SP.s)
        .width(Length::Fixed(TS.label * 16.0));
    let save = button(text(t!(i18n, ids::SETUP_PROFILE_SAVE)).size(TS.label))
        .padding([SP.s, SP.l])
        .on_press(NewProjectMsg::SaveProfileConfirmed)
        .style(crate::styles::outline_button(tokens));
    let cancel = button(text(t!(i18n, ids::SETUP_PROFILE_CANCEL)).size(TS.label))
        .padding([SP.s, SP.l])
        .on_press(NewProjectMsg::SaveProfileCancelled)
        .style(crate::styles::ghost_button(tokens));
    let mut col = column![
        row![input, save, cancel]
            .spacing(SP.s)
            .align_y(Alignment::Center)
    ]
    .spacing(SP.xs)
    .align_x(Alignment::End);
    // While a profile is edited, its own bar says why a save was refused.
    if let Some(problem) = state
        .save_profile_problem
        .filter(|_| state.editing.is_none())
    {
        let why = match problem {
            SaveProfileProblem::Rules => t!(i18n, ids::SETUP_PROFILE_ERR_RULES),
            SaveProfileProblem::Name(problem) => name_problem_text(problem, i18n),
        };
        col = col.push(
            text(why)
                .size(TS.caption)
                .color(tokens::danger(tokens))
                .boxed(),
        );
    }
    col.boxed()
}

/// "Your profiles": the heading with Save as profile, then one row per
/// saved profile (wrapping), its menu, the Rename field and the Delete
/// confirm. No saved profile shows a hint instead.
pub(crate) fn section<'a>(
    state: &NewProjectState,
    users: &[UserProfile],
    caption: Caption,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    let heading = row![
        text(t!(i18n, ids::SETUP_PROFILES_HEADING))
            .size(TS.label)
            .color(ti(tokens.text)),
        Space::new().width(Length::Fill),
        save_as(state, i18n, tokens),
    ]
    .align_y(Alignment::Center);
    let mut col = column![heading].spacing(SP.m);
    let saved: Vec<&UserProfile> = saved_profiles(users).collect();
    if saved.is_empty() {
        return col
            .push(
                text(t!(i18n, ids::SETUP_PROFILES_NONE))
                    .size(TS.body)
                    .color(tokens::muted_text(tokens))
                    .boxed(),
            )
            .boxed();
    }
    col = col.push(saved_rows(state, &saved, caption, i18n, tokens));
    for open in open_parts(state, &saved, i18n, tokens) {
        col = col.push(open);
    }
    col.boxed()
}

/// One row per saved profile (wrapping), or the Rename field in its place.
fn saved_rows<'a>(
    state: &NewProjectState,
    saved: &[&UserProfile],
    caption: Caption,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    let renaming = state.renaming.as_ref().map(|r| r.id.as_str());
    let mut rows: Vec<Element<'a, NewProjectMsg>> = Vec::new();
    for profile in saved {
        if renaming == Some(profile.id.as_str()) {
            if let Some(field) = rename_field(state, i18n, tokens) {
                rows.push(field);
            }
        } else {
            rows.push(ucard(state, profile, caption, i18n, tokens));
        }
    }
    row(rows).spacing(SP.m).wrap().boxed()
}

/// The open menu and the Delete confirm of the saved profiles, if any.
fn open_parts<'a>(
    state: &NewProjectState,
    saved: &[&UserProfile],
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Vec<Element<'a, NewProjectMsg>> {
    let mut parts = Vec::new();
    let menu = saved
        .iter()
        .find(|p| state.profile_menu.as_ref() == Some(&ProfileTarget::Saved(p.id.clone())));
    if let Some(profile) = menu {
        parts.push(profile_menu(profile, i18n, tokens));
    }
    let confirming = saved
        .iter()
        .find(|p| state.remove_confirm.as_deref() == Some(p.id.as_str()));
    if let Some(profile) = confirming {
        parts.push(remove_confirm(profile, i18n, tokens));
    }
    parts
}

/// Your profiles on one line for the Project tab (mockup row 37): the small
/// label, then the rows. Save as profile sits in the page header there.
pub(crate) fn compact_section<'a>(
    state: &NewProjectState,
    users: &[UserProfile],
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    let label = text(t!(i18n, ids::PROJECT_SETUP_YOUR_PROFILES))
        .size(TS.caption)
        .font(iced::Font {
            weight: iced::font::Weight::Medium,
            ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
        })
        .color(tokens::resolve(tokens).ink3);
    let saved: Vec<&UserProfile> = saved_profiles(users).collect();
    if saved.is_empty() {
        let none = text(t!(i18n, ids::SETUP_PROFILES_NONE))
            .size(TS.body)
            .color(tokens::muted_text(tokens));
        return row![label, none]
            .spacing(SP.m)
            .align_y(Alignment::Center)
            .boxed();
    }
    let rows = container(saved_rows(state, &saved, Caption::Pcb, i18n, tokens)).width(Length::Fill);
    let line = row![label, rows].spacing(SP.m).align_y(Alignment::Center);
    let mut col = column![line].spacing(SP.m);
    for open in open_parts(state, &saved, i18n, tokens) {
        col = col.push(open);
    }
    col.boxed()
}
