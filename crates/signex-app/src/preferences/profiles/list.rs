//! The left column of the Rule Profiles page (mockup row 148): the radio list
//! of profiles, New / Duplicate / Rename / Delete under it, and the name field
//! those actions open.

use iced::widget::{Column, Row, column, container, hover, row, svg};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget};
use signex_i18n::{I18n, Localized, MessageId, ids, t};
use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::{button, text, text_input};

use super::super::theme_cards::radio;
use super::super::{PrefMsg, SP, TS, danger_button_style, text_muted, text_warning};
use super::{ProfileNameAction, ProfilePageMsg, ProfilesMsg, page, pane};
use crate::app::rule_profile_ui::ProfileManager;
use crate::app::view::dialogs::section_heading;
use crate::rule_profiles::ProfileKind;
use crate::styles::{Radii, ti, tokens::resolve};

/// The lock and plus glyphs.
const GLYPH: f32 = TS.body;

/// Heading, framed list, actions, the open name field and the hint. The page
/// adds its problem and status lines under them.
pub(super) fn view<'a>(
    manager: &'a ProfileManager,
    default_profile: Option<&'a str>,
    tokens: &'a ThemeTokens,
    i18n: &'a I18n,
) -> Column<Element<'a, PrefMsg>> {
    let mut rows: Column<Element<'a, PrefMsg>> = Column::new().width(Length::Fill).push(list_row(
        ListRow {
            title: t!(i18n, ids::PREFERENCES_PROFILES_AUTOMATIC_NAME),
            note: Some(ids::PREFERENCES_PROFILES_AUTOMATIC_NOTE),
            is_locked: false,
            is_default: default_profile.is_none(),
            is_selected: false,
            choose: None,
            select: None,
        },
        tokens,
        i18n,
    ));
    for listed in &manager.listing.profiles {
        let name = listed.profile.name.as_str();
        let is_default = default_profile == Some(name);
        let note = match listed.kind {
            ProfileKind::Builtin => Some(ids::PREFERENCES_PROFILES_NOTE_BUILTIN),
            ProfileKind::UserDefault => Some(ids::PREFERENCES_PROFILES_NOTE_USER_DEFAULT),
            ProfileKind::User => is_default.then_some(ids::PREFERENCES_PROFILES_IS_DEFAULT),
        };
        rows = rows.push(list_row(
            ListRow {
                title: t!(
                    i18n,
                    ids::PREFERENCES_PROFILES_ROW,
                    "name" => name,
                    "version" => listed.profile.version.as_str()
                ),
                note,
                is_locked: listed.kind == ProfileKind::Builtin,
                is_default,
                is_selected: manager.selected.as_deref() == Some(name),
                choose: Some(name.to_string()),
                select: Some(pane(ProfilesMsg::Selected(name.to_string()))),
            },
            tokens,
            i18n,
        ));
    }
    let edge = resolve(tokens).line_strong;
    let frame = container(rows)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            border: Border {
                width: 1.0,
                color: edge,
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    let mut out = column![
        section_heading(i18n, ids::PREFERENCES_PROFILES_LIST_HEADING, tokens),
        frame,
        actions(manager, tokens, i18n),
    ]
    .spacing(SP.s)
    .width(Length::Fill);
    if let Some(action) = manager.name_prompt {
        out = out.push(name_prompt(manager, action, tokens, i18n));
    }
    if let Some(name) = &manager.confirm_delete {
        out = out.push(delete_confirm(name, tokens, i18n));
    }
    out.push(
        text(t!(i18n, ids::PREFERENCES_PROFILES_HINT))
            .size(TS.caption)
            .style(text_muted)
            .boxed(),
    )
}

/// One row of the list.
struct ListRow {
    title: Localized,
    note: Option<MessageId>,
    is_locked: bool,
    is_default: bool,
    is_selected: bool,
    /// The profile the radio makes the default; `None` is automatic.
    choose: Option<String>,
    /// What a click on the row does; the Automatic row opens nothing.
    select: Option<PrefMsg>,
}

/// A radio for the default, the row's name and note, and "Use as default"
/// while the pointer is over a row that is not the default.
fn list_row<'a>(item: ListRow, tokens: &'a ThemeTokens, i18n: &I18n) -> Element<'a, PrefMsg> {
    let r = resolve(tokens);
    let choose = pane(ProfilesMsg::DefaultChosen(item.choose));
    let mark = button(radio(item.is_default, tokens))
        .padding(0)
        .on_press(choose.clone())
        .style(crate::styles::ghost_button(tokens));
    let weight = if item.is_selected {
        iced::font::Weight::Semibold
    } else {
        iced::font::Weight::Normal
    };
    let mut label: Column<Element<'a, PrefMsg>> = Column::new().push(
        text(item.title)
            .size(TS.label)
            .font(signex_widgets::ui_font::at(weight))
            .color(ti(tokens.text))
            .boxed(),
    );
    if let Some(note) = item.note {
        let ink = r.ink3;
        let mut line: Row<Element<'a, PrefMsg>> =
            Row::new().spacing(SP.xs).align_y(Alignment::Center);
        if item.is_locked {
            line = line.push(
                svg(crate::icons::icon_chrome_lock(ThemeId::GraphiteDark))
                    .width(GLYPH)
                    .height(GLYPH)
                    .style(move |_: &Theme, _| svg::Style {
                        color: Some(ink),
                        ..Default::default()
                    })
                    .boxed(),
            );
        }
        let line = line.push(text(t!(i18n, note)).size(TS.caption).color(ink).boxed());
        label = label.push(line.boxed());
    }
    let open = button(label)
        .padding(0)
        .width(Length::Fill)
        .on_press_maybe(item.select)
        .style(crate::styles::ghost_button(tokens));
    let fill = item.is_selected.then_some(r.accent_soft);
    let base = container(
        row![mark, open]
            .spacing(SP.m)
            .align_y(Alignment::Center)
            .padding([SP.xs, SP.m]),
    )
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: fill.map(Background::Color),
        ..container::Style::default()
    });
    if item.is_default {
        return base.boxed();
    }
    let use_it = button(text(t!(i18n, ids::PREFERENCES_PROFILES_USE_AS_DEFAULT)).size(TS.caption))
        .padding([SP.xs, SP.s])
        .on_press(choose)
        .style(crate::styles::outline_button(tokens));
    hover(
        base,
        container(use_it)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding([0.0, SP.m])
            .align_x(iced::alignment::Horizontal::Right)
            .align_y(iced::alignment::Vertical::Center),
    )
    .boxed()
}

/// New, Duplicate, Rename and Delete; each opens a field or a question.
fn actions<'a>(
    manager: &'a ProfileManager,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PrefMsg> {
    let kind = manager.selected_profile().map(|l| l.kind);
    let ink = ti(tokens.text);
    let action = |label: MessageId, enabled: bool, msg: PrefMsg| {
        button(text(t!(i18n, label)).size(TS.body))
            .padding([SP.xs, SP.m])
            .on_press_maybe(enabled.then_some(msg))
            .style(crate::styles::outline_button(tokens))
    };
    let new = button(
        row![
            svg(crate::icons::icon_plus(ThemeId::GraphiteDark))
                .width(GLYPH)
                .height(GLYPH)
                .style(move |_: &Theme, _| svg::Style {
                    color: Some(ink),
                    ..Default::default()
                }),
            text(t!(i18n, ids::PREFERENCES_PROFILES_NEW)).size(TS.body),
        ]
        .spacing(SP.xs)
        .align_y(Alignment::Center),
    )
    .padding([SP.xs, SP.m])
    .on_press(page(ProfilePageMsg::NamePromptOpened(
        ProfileNameAction::New,
    )))
    .style(crate::styles::outline_button(tokens));
    let can_delete = kind == Some(ProfileKind::User);
    let delete_ink = if can_delete {
        ti(tokens.error)
    } else {
        resolve(tokens).ink4
    };
    let delete = button(
        text(t!(i18n, ids::DIALOG_BUTTON_DELETE))
            .size(TS.body)
            .color(delete_ink),
    )
    .padding([SP.xs, SP.m])
    .on_press_maybe(can_delete.then_some(pane(ProfilesMsg::DeleteRequested)))
    .style(crate::styles::outline_button(tokens));
    let bar: Row<Element<'a, PrefMsg>> = Row::new()
        .spacing(SP.xs)
        .push(new.boxed())
        .push(
            action(
                ids::PREFERENCES_PROFILES_DUPLICATE,
                kind.is_some(),
                page(ProfilePageMsg::NamePromptOpened(
                    ProfileNameAction::Duplicate,
                )),
            )
            .boxed(),
        )
        .push(
            action(
                ids::PREFERENCES_PROFILES_RENAME,
                kind == Some(ProfileKind::User),
                page(ProfilePageMsg::NamePromptOpened(ProfileNameAction::Rename)),
            )
            .boxed(),
        )
        .push(delete.boxed());
    bar.wrap().vertical_spacing(SP.xs).boxed()
}

/// The name field of the open action, with its confirm and Cancel.
fn name_prompt<'a>(
    manager: &'a ProfileManager,
    action: ProfileNameAction,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PrefMsg> {
    let confirm = match action {
        ProfileNameAction::New => ids::PREFERENCES_PROFILES_NEW,
        ProfileNameAction::Duplicate => ids::PREFERENCES_PROFILES_DUPLICATE,
        ProfileNameAction::Rename => ids::PREFERENCES_PROFILES_RENAME,
    };
    let is_named = !manager.name_input.trim().is_empty();
    column![
        row![
            text(t!(i18n, ids::PREFERENCES_PROFILES_NAME))
                .size(TS.body)
                .style(text_muted),
            text_input(t!(i18n, ids::PLACEHOLDER_PROFILE_NAME), &manager.name_input)
                .size(TS.label)
                .on_input(|v| pane(ProfilesMsg::NameInputChanged(v)))
                .on_submit(page(ProfilePageMsg::NamePromptConfirmed)),
        ]
        .spacing(SP.s)
        .align_y(Alignment::Center),
        row![
            button(text(t!(i18n, confirm)).size(TS.body))
                .padding([SP.xs, SP.m])
                .on_press_maybe(is_named.then_some(page(ProfilePageMsg::NamePromptConfirmed)))
                .style(crate::styles::outline_button(tokens)),
            button(text(t!(i18n, ids::DIALOG_BUTTON_CANCEL)).size(TS.body))
                .padding([SP.xs, SP.m])
                .on_press(page(ProfilePageMsg::NamePromptCancelled))
                .style(crate::styles::outline_button(tokens)),
        ]
        .spacing(SP.xs),
    ]
    .spacing(SP.xs)
    .boxed()
}

/// The question before a delete.
fn delete_confirm<'a>(name: &'a str, tokens: &'a ThemeTokens, i18n: &I18n) -> Element<'a, PrefMsg> {
    column![
        text(t!(
            i18n,
            ids::PREFERENCES_PROFILES_DELETE_CONFIRM,
            "name" => name
        ))
        .size(TS.body)
        .style(text_warning),
        row![
            button(text(t!(i18n, ids::DIALOG_BUTTON_DELETE)).size(TS.body))
                .padding([SP.xs, SP.m])
                .on_press(pane(ProfilesMsg::DeleteConfirmed))
                .style(danger_button_style),
            button(text(t!(i18n, ids::PREFERENCES_PROFILES_KEEP)).size(TS.body))
                .padding([SP.xs, SP.m])
                .on_press(pane(ProfilesMsg::DeleteCancelled))
                .style(crate::styles::outline_button(tokens)),
        ]
        .spacing(SP.xs),
    ]
    .spacing(SP.xs)
    .boxed()
}
