//! The user's saved setup profiles (#2232, 2026-11-12): their cards with
//! Remove, and Save as profile with its name field. Shared by every setup
//! dialog and the Project tab.

use iced::widget::{Space, column, row};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::project::{ProfileNameProblem, UserProfile};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text, text_input};

use super::cards::{card, mm};
use super::{SP, TS};
use crate::app::new_project::{NewProjectMsg, NewProjectState, SaveProfileProblem};
use crate::styles::{ti, tokens};

/// Cards per row; a longer list wraps onto the next row.
const PER_ROW: usize = 4;

fn caption<'a>(line: Localized, tokens: &ThemeTokens) -> Element<'a, NewProjectMsg> {
    text(line)
        .size(TS.caption)
        .font(super::super::graphite::mono())
        .color(tokens::resolve(tokens).ink3)
        .boxed()
}

/// One saved profile: its card (rules summary) and Remove under it.
fn profile_card<'a>(
    state: &NewProjectState,
    profile: &UserProfile,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    let pcb = &profile.pcb;
    let lines = vec![
        caption(
            t!(
                i18n,
                ids::DIALOG_NEW_PROJECT_SUMMARY_CUSTOM_LAYERS,
                "layers" => u32::from(pcb.copper_layers)
            ),
            tokens,
        ),
        caption(
            t!(
                i18n,
                ids::DIALOG_NEW_PROJECT_SUMMARY_CLEARANCE,
                "clearance" => mm(pcb.clearance_mm)
            ),
            tokens,
        ),
        caption(
            t!(
                i18n,
                ids::DIALOG_NEW_PROJECT_SUMMARY_TRACKS,
                "track" => mm(pcb.track_width_mm),
                "via" => mm(pcb.via_diameter_mm),
                "drill" => mm(pcb.via_drill_mm)
            ),
            tokens,
        ),
    ];
    let selected = state.setup.user_profile.as_deref() == Some(profile.id.as_str());
    let confirming = state.remove_confirm.as_deref() == Some(profile.id.as_str());
    let remove = if confirming {
        remove_confirm(profile, i18n, tokens)
    } else {
        button(
            text(t!(i18n, ids::SETUP_PROFILE_REMOVE))
                .size(TS.body)
                .color(tokens::muted_text(tokens)),
        )
        .padding([SP.xs, SP.s])
        .on_press(NewProjectMsg::UserProfileRemoveRequested(
            profile.id.clone(),
        ))
        .style(crate::styles::ghost_button(tokens))
        .boxed()
    };
    column![
        card(
            profile.name.clone(),
            selected,
            lines,
            NewProjectMsg::UserProfilePicked(profile.id.clone()),
            tokens,
        ),
        remove,
    ]
    .spacing(SP.xs)
    .width(Length::FillPortion(1))
    .boxed()
}

/// The inline confirm under a card whose Remove was pressed: the question,
/// then Remove (danger) and Keep.
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
    column![
        question,
        row![remove, keep].spacing(SP.s).align_y(Alignment::Center)
    ]
    .spacing(SP.xs)
    .boxed()
}

/// The Save as profile control: a button, or the name field with Save and
/// Cancel while it is open.
fn save_as<'a>(
    state: &NewProjectState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    let Some(name) = &state.save_profile_name else {
        return button(text(t!(i18n, ids::SETUP_PROFILE_SAVE_AS)).size(TS.label))
            .padding([SP.s, SP.l])
            .on_press(NewProjectMsg::SaveProfileOpened)
            .style(crate::styles::outline_button(tokens))
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
    if let Some(problem) = state.save_profile_problem {
        let why = match problem {
            SaveProfileProblem::Rules => t!(i18n, ids::SETUP_PROFILE_ERR_RULES),
            SaveProfileProblem::Name(ProfileNameProblem::Empty) => {
                t!(i18n, ids::SETUP_PROFILE_ERR_NAME_EMPTY)
            }
            SaveProfileProblem::Name(ProfileNameProblem::TooLong) => t!(
                i18n,
                ids::SETUP_PROFILE_ERR_NAME_TOO_LONG,
                "max" => signex_types::project::MAX_PROFILE_NAME_CHARS as u32
            ),
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

/// "Your profiles": heading with Save as profile, then the saved cards
/// (none shows only the heading row).
pub(crate) fn section<'a>(
    state: &NewProjectState,
    users: &[UserProfile],
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
    if users.is_empty() {
        col = col.push(
            text(t!(i18n, ids::SETUP_PROFILES_NONE))
                .size(TS.body)
                .color(tokens::muted_text(tokens))
                .boxed(),
        );
    }
    for chunk in users.chunks(PER_ROW) {
        let mut cards: Vec<Element<'a, NewProjectMsg>> = chunk
            .iter()
            .map(|p| profile_card(state, p, i18n, tokens))
            .collect();
        // Short rows keep the card width of a full one.
        for _ in chunk.len()..PER_ROW {
            cards.push(Space::new().width(Length::FillPortion(1)).boxed());
        }
        col = col.push(row(cards).spacing(SP.l).boxed());
    }
    col.boxed()
}
