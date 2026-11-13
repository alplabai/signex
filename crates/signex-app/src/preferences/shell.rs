//! Graphite Preferences shell: 52 px header with a "Group / Page" subtitle,
//! a 236 px navigation with radius-7 rows, and an always-visible footer.
//! Cancel, Apply and OK reuse the discard, save and save-and-close actions;
//! Restore defaults has no backend yet and is drawn disabled.

use super::graphite_fields::{graphite_label, group_messages, nav_matches, nav_search};
use super::{PrefMsg, PrefNav, PrefsView, SP, TS, build_content, build_prefs_file_banner};
use crate::app::view::dialogs::{PREFERENCES_HEADER_H, graphite_header};
use crate::styles::{self, DialogButtonKind, PillTone, Radii, dialog_button, ti, tokens};
use iced::widget::{Column, Space, column, container, row, scrollable};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text};

/// Navigation width (236 px at the default base).
pub(super) const GRAPHITE_NAV_W: f32 = styles::chrome_px(236.0 * 11.0 / 12.0);
/// Card size of the in-window dialog.
pub(super) const GRAPHITE_DLG_W: f32 = 980.0;
pub(super) const GRAPHITE_DLG_H: f32 = 680.0;

fn hairline<'a>(t: &ThemeTokens) -> Element<'a, PrefMsg> {
    let line = tokens::resolve(t).line_soft;
    container(Space::new())
        .width(Length::Fill)
        .height(1)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(line)),
            ..container::Style::default()
        })
        .boxed()
}

/// The whole dialog: header (in-window only), banner, nav | content, footer.
pub(super) fn graphite_dialog<'a>(v: PrefsView<'a>, with_header: bool) -> Element<'a, PrefMsg> {
    let t = v.panel_tokens;
    let mut items: Vec<Element<'a, PrefMsg>> = Vec::new();
    if with_header {
        let subtitle = t!(
            v.i18n,
            ids::PREFERENCES_SUBTITLE,
            "group" => t!(v.i18n, group_messages(v.nav).0).into_string(),
            "page" => t!(v.i18n, graphite_label(v.nav)).into_string()
        )
        .into_string();
        items.push(graphite_header(
            "Preferences",
            Some(subtitle),
            None,
            PrefMsg::Close,
            PREFERENCES_HEADER_H,
            t,
        ));
    }
    if let Some(banner) = build_prefs_file_banner(v.prefs_load_error, v.prefs_status) {
        items.push(banner);
    }
    let divider = {
        let line = tokens::resolve(t).line_soft;
        container(Space::new())
            .width(1)
            .height(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(line)),
                ..container::Style::default()
            })
            .boxed()
    };
    items.push(
        row![
            graphite_nav(v.nav, v.nav_search, t, v.i18n),
            divider,
            build_content(v)
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed(),
    );
    items.push(hairline(t));
    items.push(graphite_footer(&v));

    container(Column::with_children(items).boxed())
        .width(Length::Fill)
        .height(Length::Fill)
        .style(styles::modal_card(t))
        .clip(true)
        .boxed()
}

fn graphite_nav<'a>(
    active: PrefNav,
    query: &'a str,
    t: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PrefMsg> {
    let mut col = column![].spacing(2).padding(SP.m).width(Length::Fill);
    col = col.push(nav_search(query, t, i18n));
    let mut last_group = "";
    for &item in PrefNav::ALL
        .iter()
        .filter(|&&n| nav_matches(i18n, n, query))
    {
        let group = item.group();
        if group != last_group {
            last_group = group;
            col = col.push(
                container(
                    text(t!(i18n, group_messages(item).1))
                        .size(TS.section)
                        .style(styles::section_label(t)),
                )
                .padding(iced::Padding {
                    top: SP.l,
                    right: SP.m,
                    bottom: SP.s,
                    left: SP.m,
                })
                .boxed(),
            );
        }
        col = col.push(nav_row(item, item == active, t, i18n));
    }
    let bg = ti(t.panel_bg);
    container(
        scrollable(col)
            .width(Length::Fill)
            .style(super::graphite_fields::scroll_style(t))
            .boxed(),
    )
    .width(Length::Fixed(GRAPHITE_NAV_W))
    .height(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(bg)),
        ..container::Style::default()
    })
    .boxed()
}

fn nav_row<'a>(item: PrefNav, active: bool, t: &ThemeTokens, i18n: &I18n) -> Element<'a, PrefMsg> {
    let (ink, selected, hover) = (
        ti(t.text),
        ti(t.selection),
        tokens::overlay(t, tokens::Overlay::Hover),
    );
    button(
        container(
            text(t!(i18n, graphite_label(item)))
                .size(TS.label)
                .color(ink),
        )
        .padding([SP.s, SP.m])
        .width(Length::Fill),
    )
    .padding(0)
    .width(Length::Fill)
    .on_press(PrefMsg::Nav(item))
    .style(move |_: &Theme, status: button::Status| button::Style {
        background: if active {
            Some(Background::Color(selected))
        } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
            Some(Background::Color(hover))
        } else {
            None
        },
        text_color: ink,
        border: Border {
            radius: Radii::GRAPHITE.control.into(),
            ..Border::default()
        },
        ..button::Style::default()
    })
    .boxed()
}

/// Footer: Restore defaults at the left; hint or unsaved pill, Cancel,
/// Apply and OK at the right. Apply needs unsaved changes; OK saves them and
/// closes, or only closes when there are none.
fn graphite_footer<'a>(v: &PrefsView<'a>) -> Element<'a, PrefMsg> {
    let t = v.panel_tokens;
    let note: Element<'a, PrefMsg> = if v.dirty {
        container(text(t!(v.i18n, ids::PREFERENCES_UNSAVED)).size(TS.caption))
            .padding([2.0, SP.m])
            .style(styles::pill(t, PillTone::Warn))
            .boxed()
    } else if v.nav == PrefNav::Appearance {
        text(t!(v.i18n, ids::PREFERENCES_THEME_PREVIEW_HINT))
            .size(TS.label)
            .color(tokens::resolve(t).ink3)
            .boxed()
    } else {
        Space::new().boxed()
    };
    let ok_msg = if v.dirty {
        PrefMsg::SaveAndClose
    } else {
        PrefMsg::Close
    };
    let buttons = row![
        note,
        dialog_button(
            t!(v.i18n, ids::DIALOG_BUTTON_CANCEL),
            DialogButtonKind::Cancel,
            Some(PrefMsg::DiscardAndClose),
            t
        ),
        dialog_button(
            t!(v.i18n, ids::DIALOG_BUTTON_APPLY),
            DialogButtonKind::Secondary,
            v.dirty.then_some(PrefMsg::Save),
            t
        ),
        dialog_button(
            t!(v.i18n, ids::DIALOG_BUTTON_OK),
            DialogButtonKind::Primary,
            Some(ok_msg),
            t
        ),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);
    container(
        row![
            dialog_button(
                t!(v.i18n, ids::PREFERENCES_RESTORE_DEFAULTS),
                DialogButtonKind::Ghost,
                None,
                t
            ),
            Space::new().width(Length::Fill).boxed(),
            buttons.boxed(),
        ]
        .align_y(Alignment::Center),
    )
    .padding([SP.m, SP.l])
    .width(Length::Fill)
    .boxed()
}
