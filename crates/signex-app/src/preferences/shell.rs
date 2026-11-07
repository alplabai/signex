//! Graphite Preferences shell: 52 px header with a "Group / Page" subtitle,
//! a 236 px navigation with radius-7 rows, and an always-visible footer.
//! Cancel, Apply and OK reuse the discard, save and save-and-close actions;
//! Restore defaults has no backend yet and is drawn disabled.

use super::{PrefMsg, PrefNav, PrefsView, SP, TS, build_content, build_prefs_file_banner};
use crate::app::view::dialogs::{PREFERENCES_HEADER_H, graphite_header};
use crate::styles::{
    self, DialogButtonKind, PillTone, Radii, dialog_button, is_graphite, ti, tokens,
};
use iced::widget::{Column, Space, button, column, container, row, scrollable, text};
use iced::{Alignment, Background, Border, Element, Length, Theme};
use signex_types::theme::ThemeTokens;

/// Navigation width (236 px at the default base).
pub(super) const GRAPHITE_NAV_W: f32 = styles::chrome_px(236.0 * 11.0 / 12.0);
/// Card size of the in-window dialog.
pub(super) const GRAPHITE_DLG_W: f32 = 980.0;
pub(super) const GRAPHITE_DLG_H: f32 = 680.0;

/// Whether the dialog draws the Graphite shell.
pub(super) fn is_graphite_shell(v: &PrefsView<'_>) -> bool {
    is_graphite(v.panel_tokens)
}

fn hairline<'a>(t: &ThemeTokens) -> Element<'a, PrefMsg> {
    let line = tokens::resolve(t).line_soft;
    container(Space::new())
        .width(Length::Fill)
        .height(1)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(line)),
            ..container::Style::default()
        })
        .into()
}

/// The whole dialog: header (in-window only), banner, nav | content, footer.
pub(super) fn graphite_dialog<'a>(v: PrefsView<'a>, with_header: bool) -> Element<'a, PrefMsg> {
    let t = v.panel_tokens;
    let mut items: Vec<Element<'a, PrefMsg>> = Vec::new();
    if with_header {
        let subtitle = format!("{} / {}", v.nav.group(), v.nav.label());
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
    };
    items.push(
        row![graphite_nav(v.nav, t), divider, build_content(v)]
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),
    );
    items.push(hairline(t));
    items.push(graphite_footer(&v));

    container(Column::with_children(items))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(styles::modal_card(t))
        .clip(true)
        .into()
}

fn graphite_nav<'a>(active: PrefNav, t: &ThemeTokens) -> Element<'a, PrefMsg> {
    let mut col = column![].spacing(2).padding(SP.m).width(Length::Fill);
    let mut last_group = "";
    for &item in PrefNav::ALL {
        let group = item.group();
        if group != last_group {
            last_group = group;
            col = col.push(
                container(
                    text(group.to_uppercase())
                        .size(TS.section)
                        .style(styles::section_label(t)),
                )
                .padding(iced::Padding {
                    top: SP.l,
                    right: SP.m,
                    bottom: SP.s,
                    left: SP.m,
                }),
            );
        }
        col = col.push(nav_row(item, item == active, t));
    }
    let bg = ti(t.panel_bg);
    container(scrollable(col).width(Length::Fill))
        .width(Length::Fixed(GRAPHITE_NAV_W))
        .height(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(bg)),
            ..container::Style::default()
        })
        .into()
}

fn nav_row<'a>(item: PrefNav, active: bool, t: &ThemeTokens) -> Element<'a, PrefMsg> {
    let (ink, selected, hover) = (
        ti(t.text),
        ti(t.selection),
        tokens::overlay(t, tokens::Overlay::Hover),
    );
    button(
        container(text(item.label()).size(TS.label).color(ink))
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
    .into()
}

/// Footer: Restore defaults at the left; hint or unsaved pill, Cancel,
/// Apply and OK at the right. Apply needs unsaved changes; OK saves them and
/// closes, or only closes when there are none.
fn graphite_footer<'a>(v: &PrefsView<'a>) -> Element<'a, PrefMsg> {
    let t = v.panel_tokens;
    let note: Element<'a, PrefMsg> = if v.dirty {
        container(text("Unsaved changes").size(TS.caption))
            .padding([2.0, SP.m])
            .style(styles::pill(t, PillTone::Warn))
            .into()
    } else if v.nav == PrefNav::Appearance {
        text("Theme changes preview live")
            .size(TS.label)
            .color(tokens::resolve(t).ink3)
            .into()
    } else {
        Space::new().into()
    };
    let ok_msg = if v.dirty {
        PrefMsg::SaveAndClose
    } else {
        PrefMsg::Close
    };
    let buttons = row![
        note,
        dialog_button(
            "Cancel",
            DialogButtonKind::Secondary,
            Some(PrefMsg::DiscardAndClose),
            t
        ),
        dialog_button(
            "Apply",
            DialogButtonKind::Secondary,
            v.dirty.then_some(PrefMsg::Save),
            t
        ),
        dialog_button("OK", DialogButtonKind::Primary, Some(ok_msg), t),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);
    container(
        row![
            dialog_button("Restore defaults", DialogButtonKind::Ghost, None, t),
            Space::new().width(Length::Fill),
            buttons,
        ]
        .align_y(Alignment::Center),
    )
    .padding([SP.m, SP.l])
    .width(Length::Fill)
    .into()
}
