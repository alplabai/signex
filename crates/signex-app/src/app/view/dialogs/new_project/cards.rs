//! The profile cards.

use iced::widget::{Space, column, container, row, svg};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, MessageId, UserText, ids, t};
use signex_types::project::{SetupProfile, UserProfile};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, button::Status, text};

use super::{SP, TS};
use crate::app::new_project::{NewProjectMsg, NewProjectState};
use crate::styles::{Radii, ti, tokens};

/// Width of a selected card's border; an unselected card has 1 px.
pub(super) const SELECTED_BORDER: f32 = 1.5;

const CHECK: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24"><path d="M5 12l5 5 9-10" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"/></svg>"#;

fn check_mark<'a>(tokens: &ThemeTokens) -> Element<'a, NewProjectMsg> {
    let ink = tokens::resolve(tokens).accent_text;
    svg(svg::Handle::from_memory(CHECK))
        .width(Length::Fixed(TS.title))
        .height(Length::Fixed(TS.title))
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        })
        .boxed()
}

/// Look of a card button: the selected one has the soft accent fill and a
/// 1.5 px accent border, the others a raised fill and a hairline.
pub(in crate::app::view) fn card_style(
    tokens: &ThemeTokens,
    selected: bool,
    status: Status,
) -> button::Style {
    let r = tokens::resolve(tokens);
    let hovered = matches!(status, Status::Hovered | Status::Pressed);
    let (fill, line, width) = if selected {
        (r.accent_soft, ti(tokens.accent), SELECTED_BORDER)
    } else if hovered {
        (r.raised, r.line_strong, 1.0)
    } else {
        (r.raised, ti(tokens.border), 1.0)
    };
    button::Style {
        background: Some(Background::Color(fill)),
        text_color: ti(tokens.text),
        border: Border {
            width,
            color: line,
            radius: Radii::GRAPHITE.card.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

/// One selectable card: title (with the check when selected), then lines.
pub(super) fn card<'a>(
    title: String,
    selected: bool,
    lines: Vec<Element<'a, NewProjectMsg>>,
    on_press: NewProjectMsg,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    let mut head = row![
        text(UserText::new(title))
            .size(TS.title)
            .color(ti(tokens.text)),
        Space::new().width(Length::Fill),
    ]
    .align_y(Alignment::Center);
    if selected {
        head = head.push(check_mark(tokens));
    }
    let body = column![head].extend(lines).spacing(SP.m);
    let tokens = *tokens;
    button(container(body).padding(SP.l).width(Length::Fill))
        .padding(0)
        .width(Length::FillPortion(1))
        .on_press(on_press)
        .style(move |_: &Theme, status| card_style(&tokens, selected, status))
        .boxed()
}

fn blurb<'a>(i18n: &I18n, id: MessageId, tokens: &ThemeTokens) -> Element<'a, NewProjectMsg> {
    text(t!(i18n, id))
        .size(TS.body)
        .color(ti(tokens.text_secondary))
        .boxed()
}

pub(super) fn mm(value: f64) -> String {
    format!("{value}")
}

/// The monospace rule summary under a profile card's blurb.
fn summary<'a>(
    state: &NewProjectState,
    profile: SetupProfile,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Vec<Element<'a, NewProjectMsg>> {
    let ink = tokens::resolve(tokens).ink3;
    let line = |text_: signex_i18n::Localized| -> Element<'a, NewProjectMsg> {
        text(text_)
            .size(TS.caption)
            .font(super::super::graphite::mono())
            .color(ink)
            .boxed()
    };
    if profile == SetupProfile::Custom {
        let layers = state.setup.custom_layers();
        return vec![
            line(
                t!(i18n, ids::DIALOG_NEW_PROJECT_SUMMARY_CUSTOM_LAYERS, "layers" => u32::from(layers)),
            ),
            line(t!(i18n, ids::DIALOG_NEW_PROJECT_SUMMARY_CUSTOM_NOTE)),
        ];
    }
    let d = profile.pcb(state.setup.pcb.copper_layers);
    vec![
        line(t!(
            i18n,
            ids::DIALOG_NEW_PROJECT_SUMMARY_CLEARANCE,
            "clearance" => mm(d.clearance_mm)
        )),
        line(t!(
            i18n,
            ids::DIALOG_NEW_PROJECT_SUMMARY_TRACKS,
            "track" => mm(d.track_width_mm),
            "via" => mm(d.via_diameter_mm),
            "drill" => mm(d.via_drill_mm)
        )),
    ]
}

fn heading_row<'a>(
    heading: signex_i18n::Localized,
    hint: signex_i18n::Localized,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    row![
        text(heading).size(TS.label).color(ti(tokens.text)),
        Space::new().width(Length::Fill),
        text(hint).size(TS.body).color(tokens::muted_text(tokens)),
    ]
    .align_y(Alignment::Center)
    .boxed()
}

/// "Start from a profile": the four built-in cards, then the user's saved
/// profiles and Save as profile.
pub(crate) fn profiles<'a>(
    state: &NewProjectState,
    users: &[UserProfile],
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    let entries = [
        (
            SetupProfile::Hobby,
            ids::DIALOG_NEW_PROJECT_PROFILE_HOBBY_NAME,
            ids::DIALOG_NEW_PROJECT_PROFILE_HOBBY_BLURB,
        ),
        (
            SetupProfile::Standard,
            ids::DIALOG_NEW_PROJECT_PROFILE_STANDARD_NAME,
            ids::DIALOG_NEW_PROJECT_PROFILE_STANDARD_BLURB,
        ),
        (
            SetupProfile::Hdi,
            ids::DIALOG_NEW_PROJECT_PROFILE_HDI_NAME,
            ids::DIALOG_NEW_PROJECT_PROFILE_HDI_BLURB,
        ),
        (
            SetupProfile::Custom,
            ids::DIALOG_NEW_PROJECT_PROFILE_CUSTOM_NAME,
            ids::DIALOG_NEW_PROJECT_PROFILE_CUSTOM_BLURB,
        ),
    ];
    let cards = entries.map(|(profile, name, text_id)| {
        let mut lines = vec![blurb(i18n, text_id, tokens)];
        lines.extend(summary(state, profile, i18n, tokens));
        card(
            t!(i18n, name).into_string(),
            state.setup.is_builtin_picked(profile),
            lines,
            NewProjectMsg::ProfilePicked(profile),
            tokens,
        )
    });
    column![
        heading_row(
            t!(i18n, ids::DIALOG_NEW_PROJECT_PROFILE_HEADING),
            t!(i18n, ids::DIALOG_NEW_PROJECT_PROFILE_HINT),
            tokens
        ),
        row(cards).spacing(SP.l).height(Length::Shrink),
        super::user_cards::section(state, users, i18n, tokens),
    ]
    .spacing(SP.m)
    .boxed()
}
