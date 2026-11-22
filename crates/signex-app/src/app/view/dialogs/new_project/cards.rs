//! The fab tier cards (#2537): Standard, Advanced, HDI and Custom in a 2 x 2
//! grid, each with its layer glyph, rule line and copper layer stepper; the
//! pencil and menu of an editable built-in (#2536); and the edit bar while a
//! profile's values are edited. Shared by New Project, New PCB and the
//! Project tab.

use iced::widget::{Space, column, container, mouse_area, row};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_types::project::{SetupProfile, UserProfile};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, button::Status, text};

use super::card_parts::{
    PENCIL, check_disc, edited_pill, glyph, icon_button, layer_stepper, menu_caption, menu_header,
    menu_row, menu_separator, menu_surface, pen_icon,
};
use super::{SP, TS};
use crate::app::new_project::{NewProjectMsg, NewProjectState, ProfileTarget, SaveProfileProblem};
use crate::styles::{Radii, ti, tokens};

/// Width of a picked card's border: the 1 px border and the half-pixel
/// inset ring of the design.
pub(super) const SELECTED_BORDER: f32 = 1.5;

/// Share of the soft accent mixed over the panel in a picked card.
const PICKED_TINT: f32 = 0.4;

/// Gap between the cards of the grid.
const GRID_GAP: f32 = SP.m + SP.xs;

/// The card catalog name and blurb of a built-in.
fn card_text(profile: SetupProfile) -> (signex_i18n::MessageId, signex_i18n::MessageId) {
    let name = crate::app::new_project::profile_name_id(profile);
    let blurb = match profile {
        SetupProfile::Standard => ids::DIALOG_NEW_PROJECT_PROFILE_STANDARD_BLURB,
        SetupProfile::Advanced => ids::DIALOG_NEW_PROJECT_PROFILE_ADVANCED_BLURB,
        SetupProfile::Hdi => ids::DIALOG_NEW_PROJECT_PROFILE_HDI_BLURB,
        SetupProfile::Custom => ids::DIALOG_NEW_PROJECT_PROFILE_CUSTOM_BLURB,
    };
    (name, blurb)
}

/// Look of a card: the panel fill and a hairline at rest, the hover token on
/// hover; a picked card has the accent border and the soft accent mixed over
/// the panel.
pub(in crate::app::view) fn card_style(
    tokens: &ThemeTokens,
    selected: bool,
    status: Status,
) -> button::Style {
    let r = tokens::resolve(tokens);
    let panel = ti(tokens.panel_bg);
    let hovered = matches!(status, Status::Hovered | Status::Pressed);
    let (fill, line, width) = if selected {
        let soft = iced::Color {
            a: r.accent_soft.a * PICKED_TINT,
            ..r.accent_soft
        };
        (
            tokens::composite_over(soft, panel),
            ti(tokens.accent),
            SELECTED_BORDER,
        )
    } else if hovered {
        (
            tokens::composite_over(ti(tokens.hover), panel),
            ti(tokens.border),
            1.0,
        )
    } else {
        (panel, ti(tokens.border), 1.0)
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

pub(super) fn mm(value: f64) -> String {
    format!("{value}")
}

/// The card's rule line: the tier's values as the user has them, or the
/// Custom note.
fn specs<'a>(
    profile: SetupProfile,
    users: &[UserProfile],
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    let line = if profile == SetupProfile::Custom {
        t!(i18n, ids::DIALOG_NEW_PROJECT_SUMMARY_CUSTOM_NOTE)
    } else {
        let d = profile.values(users);
        t!(
            i18n,
            ids::DIALOG_NEW_PROJECT_CARD_SPECS,
            "clearance" => mm(d.clearance_mm),
            "track" => mm(d.track_width_mm),
            "via" => mm(d.via_diameter_mm),
            "drill" => mm(d.via_drill_mm)
        )
    };
    text(line)
        .size(TS.caption)
        .font(super::super::graphite::mono())
        .color(ti(tokens.text_secondary))
        .boxed()
}

/// The menu of an edited built-in: Edit, Duplicate as my profile, Reset to
/// default and the override note.
fn tier_menu<'a>(
    profile: SetupProfile,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    let name = t!(i18n, card_text(profile).0).into_string();
    menu_surface(
        vec![
            menu_header(
                t!(i18n, ids::DIALOG_NEW_PROJECT_CARD_MENU_HEADER, "name" => name.as_str()),
                tokens,
            ),
            menu_row(
                t!(i18n, ids::DIALOG_NEW_PROJECT_CARD_EDIT),
                NewProjectMsg::ProfileEditRequested(ProfileTarget::BuiltIn(profile)),
                tokens,
            ),
            menu_row(
                t!(i18n, ids::DIALOG_NEW_PROJECT_CARD_DUPLICATE),
                NewProjectMsg::ProfileDuplicateRequested {
                    source: ProfileTarget::BuiltIn(profile),
                    name,
                },
                tokens,
            ),
            menu_separator(tokens),
            menu_row(
                t!(i18n, ids::DIALOG_NEW_PROJECT_CARD_RESET),
                NewProjectMsg::ProfileResetRequested(profile),
                tokens,
            ),
            menu_caption(t!(i18n, ids::DIALOG_NEW_PROJECT_CARD_OVERRIDE_NOTE), tokens),
        ],
        tokens,
    )
}

/// The pencil of a built-in tier: shown while the card is hovered or its
/// menu is open. An edited built-in's opens its menu; otherwise it opens the
/// editor straight away. Custom has none.
fn pencil<'a>(
    state: &NewProjectState,
    profile: SetupProfile,
    edited: bool,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    let target = ProfileTarget::BuiltIn(profile);
    let open = state.profile_menu.as_ref() == Some(&target);
    let shown = profile != SetupProfile::Custom && (open || state.hovered_card == Some(profile));
    if !shown {
        return Space::new()
            .width(Length::Fixed(PENCIL))
            .height(Length::Fixed(PENCIL))
            .boxed();
    }
    let on_press = match (edited, open) {
        (true, true) => NewProjectMsg::ProfileMenuToggled(None),
        (true, false) => NewProjectMsg::ProfileMenuToggled(Some(target)),
        (false, _) => NewProjectMsg::ProfileEditRequested(target),
    };
    let ink = if open {
        ti(tokens.text)
    } else {
        tokens::resolve(tokens).ink3
    };
    icon_button(pen_icon(ink), PENCIL, open, on_press, tokens)
}

/// One built-in card.
fn tier_card<'a>(
    state: &NewProjectState,
    users: &[UserProfile],
    profile: SetupProfile,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    let picked = state.setup.is_builtin_picked(profile);
    let edited = profile.is_edited(users);
    let layers = state.setup.card_layers(profile, users);
    let (name, blurb) = card_text(profile);
    let bold = iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    };
    let mut title = row![
        text(t!(i18n, name))
            .size(TS.label)
            .font(bold)
            .color(ti(tokens.text))
            .wrapping(iced::widget::text::Wrapping::None)
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    if edited {
        title = title.push(edited_pill(i18n, tokens));
    }
    let head = row![
        title,
        Space::new().width(Length::Fill),
        pencil(state, profile, edited, tokens),
        check_disc(picked, tokens),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    let layer_row = row![
        text(t!(i18n, ids::DIALOG_NEW_PROJECT_LAYERS_LABEL))
            .size(TS.caption)
            .color(tokens::resolve(tokens).ink3),
        layer_stepper(
            layers,
            NewProjectMsg::CardLayersStepped(profile, -(signex_types::project::LAYER_STEP as i8)),
            NewProjectMsg::CardLayersStepped(profile, signex_types::project::LAYER_STEP as i8),
            tokens,
        ),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);
    let mut body = column![
        head,
        text(t!(i18n, blurb))
            .size(TS.body)
            .color(tokens::resolve(tokens).ink3),
        container(specs(profile, users, i18n, tokens)).padding([SP.xs, 0.0]),
        container(layer_row).padding(iced::Padding {
            top: SP.xs,
            ..iced::Padding::ZERO
        }),
    ]
    .spacing(SP.xs);
    if edited && state.profile_menu == Some(ProfileTarget::BuiltIn(profile)) {
        body = body.push(
            row![
                Space::new().width(Length::Fill),
                tier_menu(profile, i18n, tokens)
            ]
            .boxed(),
        );
    }
    let glyph_layers = (profile != SetupProfile::Custom).then_some(layers);
    let content = row![
        glyph(glyph_layers, picked, super::preview::copper(), tokens),
        body.width(Length::Fill),
    ]
    .spacing(SP.l)
    .align_y(Alignment::Start);
    let style_tokens = *tokens;
    let card = button(content)
        .padding([SP.l, SP.l + SP.xs])
        .width(Length::Fill)
        .on_press(NewProjectMsg::ProfilePicked(profile))
        .style(move |_: &Theme, status| card_style(&style_tokens, picked, status));
    container(
        mouse_area(card)
            .on_enter(NewProjectMsg::CardHovered {
                card: profile,
                inside: true,
            })
            .on_exit(NewProjectMsg::CardHovered {
                card: profile,
                inside: false,
            }),
    )
    .width(Length::FillPortion(1))
    .boxed()
}

/// The four built-in cards, two to a row.
pub(crate) fn tier_grid<'a>(
    state: &NewProjectState,
    users: &[UserProfile],
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    let card = |p| tier_card(state, users, p, i18n, tokens);
    column![
        row![card(SetupProfile::Standard), card(SetupProfile::Advanced)].spacing(GRID_GAP),
        row![card(SetupProfile::Hdi), card(SetupProfile::Custom)].spacing(GRID_GAP),
    ]
    .spacing(GRID_GAP)
    .boxed()
}

/// The bar over the fields while a profile is edited: what is edited, Save
/// profile and Cancel, and why a save was refused.
fn edit_bar<'a>(
    state: &NewProjectState,
    users: &[UserProfile],
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Option<Element<'a, NewProjectMsg>> {
    let target = state.editing.as_ref()?;
    let name = match target {
        ProfileTarget::BuiltIn(profile) => t!(i18n, card_text(*profile).0).into_string(),
        ProfileTarget::Saved(id) => users
            .iter()
            .find(|p| p.id == *id)
            .map(|p| p.name.clone())
            .unwrap_or_default(),
    };
    let save = button(text(t!(i18n, ids::DIALOG_NEW_PROJECT_EDIT_SAVE)).size(TS.label))
        .padding([SP.s, SP.l])
        .on_press(NewProjectMsg::ProfileEditSaved)
        .style(crate::styles::primary_button(tokens));
    let cancel = button(text(t!(i18n, ids::SETUP_PROFILE_CANCEL)).size(TS.label))
        .padding([SP.s, SP.l])
        .on_press(NewProjectMsg::ProfileEditCancelled)
        .style(crate::styles::ghost_button(tokens));
    let mut col = column![
        row![
            text(t!(i18n, ids::DIALOG_NEW_PROJECT_EDIT_BAR, "name" => name.as_str()))
                .size(TS.body)
                .color(ti(tokens.text)),
            Space::new().width(Length::Fill),
            save,
            cancel,
        ]
        .spacing(SP.s)
        .align_y(Alignment::Center)
    ]
    .spacing(SP.xs);
    if state.save_profile_problem == Some(SaveProfileProblem::Rules) {
        col = col.push(
            text(t!(i18n, ids::SETUP_PROFILE_ERR_RULES))
                .size(TS.caption)
                .color(tokens::danger(tokens))
                .boxed(),
        );
    }
    let fill = tokens::resolve(tokens).sunken;
    let line = ti(tokens.accent);
    Some(
        container(col)
            .padding([SP.m, SP.l])
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(fill.into()),
                border: Border {
                    width: 1.0,
                    color: line,
                    radius: Radii::GRAPHITE.card.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .boxed(),
    )
}

/// The cards, the edit bar and Your profiles: what "Use another profile..."
/// unfolds under the summary row of New PCB.
pub(crate) fn choices<'a>(
    state: &NewProjectState,
    users: &[UserProfile],
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    let mut col = column![tier_grid(state, users, i18n, tokens)].spacing(SP.m);
    if let Some(bar) = edit_bar(state, users, i18n, tokens) {
        col = col.push(bar);
    }
    col.push(super::user_cards::section(
        state,
        users,
        super::user_cards::Caption::Pcb,
        i18n,
        tokens,
    ))
    .boxed()
}

/// The Project tab's form of [`choices`] (mockup row 37): its page header
/// holds Save as profile, and Your profiles is one compact row.
pub(crate) fn tab_choices<'a>(
    state: &NewProjectState,
    users: &[UserProfile],
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    let mut col = column![tier_grid(state, users, i18n, tokens)].spacing(SP.m);
    if let Some(bar) = edit_bar(state, users, i18n, tokens) {
        col = col.push(bar);
    }
    col.push(super::user_cards::compact_section(
        state, users, i18n, tokens,
    ))
    .boxed()
}

/// "Start from a profile": the heading, then [`choices`].
pub(crate) fn profiles<'a>(
    state: &NewProjectState,
    users: &[UserProfile],
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    let heading = row![
        text(t!(i18n, ids::DIALOG_NEW_PROJECT_PROFILE_HEADING))
            .size(TS.label)
            .color(ti(tokens.text)),
        Space::new().width(Length::Fill),
        text(t!(i18n, ids::DIALOG_NEW_PROJECT_PROFILE_HINT))
            .size(TS.body)
            .color(tokens::muted_text(tokens)),
    ]
    .align_y(Alignment::Center);
    column![heading, choices(state, users, i18n, tokens)]
        .spacing(SP.m)
        .boxed()
}
