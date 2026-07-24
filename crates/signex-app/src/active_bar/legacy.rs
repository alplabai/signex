//! Legacy Active Bar chrome — bespoke button + separator builders,
//! superseded by `signex_widgets::active_bar::ActiveBarButton` /
//! `ActiveBarItem::Separator`. Kept (dead) for one migration cycle so a
//! follow-up can lift the chevron / mouse_area / tooltip details if the
//! generic widget needs them. Remove when the migration is fully bedded in.

#![allow(dead_code)]

use iced::widget::{Space, button, container, svg, text};
use iced::{Background, Border, Color, Element, Theme};
use signex_types::theme::ThemeId;

use crate::icons as ic;

use super::{ActiveBarMsg, DISABLED_TEXT, action_enabled};

/// Active Bar button: left-click activates tool, right-click opens dropdown.
/// Shows a small 45° chevron at bottom-right if button has a dropdown.
/// Legacy bespoke button builder — superseded by
/// `signex_widgets::active_bar::ActiveBarButton`. Kept here so a
/// follow-up patch can lift the chevron / mouse_area details if
/// the generic widget needs them; remove when the migration is
/// fully bedded in.
#[allow(dead_code)]
fn ab_icon_btn(
    icon: svg::Handle,
    active: bool,
    left_click: ActiveBarMsg,
    right_click: Option<ActiveBarMsg>,
    tooltip_text: &'static str,
    tid: ThemeId,
) -> Element<'static, ActiveBarMsg> {
    let handle = icon;
    let has_dropdown = right_click.is_some();
    // Pre-compute the gating decision so both the icon tint and the
    // `on_press` wiring see the same answer.
    let left_enabled = match &left_click {
        ActiveBarMsg::Action(action) => action_enabled(action),
        _ => true,
    };
    let icon_widget = {
        let s = svg(handle).width(20).height(20);
        if left_enabled {
            s
        } else {
            s.style(|_: &Theme, _| iced::widget::svg::Style {
                color: Some(DISABLED_TEXT),
            })
        }
    };

    // Icon with optional chevron indicator
    let icon_content: Element<'static, ActiveBarMsg> = if has_dropdown {
        let chevron = svg(ic::icon_chevron_45(tid)).width(14).height(14);
        iced::widget::Stack::new()
            .push(
                container(icon_widget)
                    .width(26)
                    .height(26)
                    .align_x(iced::alignment::Horizontal::Center)
                    .align_y(iced::alignment::Vertical::Center),
            )
            .push(
                container(chevron)
                    .width(26)
                    .height(26)
                    .align_x(iced::alignment::Horizontal::Right)
                    .align_y(iced::alignment::Vertical::Bottom),
            )
            .into()
    } else {
        container(icon_widget)
            .width(26)
            .height(26)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .into()
    };

    // Use a button for left-click (reliable event delivery) and wrap
    // with mouse_area for right-click (dropdown toggle). When the
    // left-click is `Action(a)` for a selection-dependent action and
    // the canvas selection is empty, skip `on_press` so iced renders
    // the button in its `Disabled` state. The right-click (dropdown
    // toggle) still works via the surrounding `mouse_area`, so the
    // user can open the menu and discover what's greyed out.
    let left_msg = left_click;
    let mut btn =
        button(icon_content)
            .padding(0)
            .style(move |_: &Theme, status: button::Status| {
                let bg = match status {
                    button::Status::Hovered => Color::from_rgb(0.26, 0.27, 0.34),
                    _ if active => Color::from_rgb(0.22, 0.23, 0.30),
                    _ => Color::TRANSPARENT,
                };
                button::Style {
                    background: Some(Background::Color(bg)),
                    border: Border {
                        width: 0.0,
                        radius: 3.0.into(),
                        color: Color::TRANSPARENT,
                    },
                    ..button::Style::default()
                }
            });
    if left_enabled {
        btn = btn.on_press(left_msg);
    }

    let widget: Element<'static, ActiveBarMsg> = if let Some(rc) = right_click {
        iced::widget::mouse_area(btn).on_right_press(rc).into()
    } else {
        btn.into()
    };

    let tip = container(
        text(tooltip_text)
            .size(11)
            .color(Color::from_rgb(0.85, 0.85, 0.88)),
    )
    .padding([4, 8])
    .style(|_: &Theme| container::Style {
        background: Some(Color::from_rgb(0.14, 0.14, 0.18).into()),
        border: Border {
            width: 1.0,
            radius: 4.0.into(),
            color: Color::from_rgb(0.24, 0.25, 0.30),
        },
        ..container::Style::default()
    });

    iced::widget::tooltip(widget, tip, iced::widget::tooltip::Position::Bottom)
        .gap(4)
        .into()
}

/// Legacy separator builder — superseded by
/// `ActiveBarItem::Separator`. See `ab_icon_btn` for the rationale
/// to keep this around for one more cycle.
#[allow(dead_code)]
fn sep(sep_c: Color) -> Element<'static, ActiveBarMsg> {
    container(Space::new())
        .width(1)
        .height(22)
        .style(move |_: &Theme| container::Style {
            background: Some(sep_c.into()),
            ..container::Style::default()
        })
        .into()
}
