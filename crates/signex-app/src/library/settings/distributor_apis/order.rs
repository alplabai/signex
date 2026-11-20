//! The order-of-preference column: a card list with a drag handle on each
//! row, the help line and the path of `distributors.toml`.

use iced::widget::{Column, Space, column, container, mouse_area, row};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, UserText, ids, t};
use signex_library::DistributorSource;
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, raw, text};

use crate::styles::{ChromeMetrics, Radii, ti, tokens::resolve};

use super::{SP, TS, caption, distributor_label, error_caption, mono, settings_msg};
use crate::library::messages::{LibraryMessage, SettingsMsg};
use crate::library::state::DistributorSettings;

/// Side of one dot of a drag handle, and the gap between dots.
const GRIP_DOT: f32 = 2.0;

/// The column: section label, the list, the help line, the file path and a
/// failed save.
pub(super) fn order_column<'a>(
    settings: &'a DistributorSettings,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let heading = text(UserText::new(i18n.upper_for_display(
        &t!(i18n, ids::LIBRARY_ORDER_OF_PREFERENCE).into_string(),
    )))
    .size(TS.section)
    .style(crate::styles::section_label(tokens));
    let mut col = column![
        heading,
        order_list(settings, tokens, i18n),
        caption(t!(i18n, ids::LIBRARY_ORDER_HELP), tokens),
    ]
    .spacing(SP.s + SP.xs)
    .width(Length::Fill);
    if let Some(path) = settings.preferred_order_path.as_deref() {
        col = col.push(
            text(UserText::new(path))
                .size(TS.caption)
                .font(mono())
                .color(resolve(tokens).ink3)
                .boxed(),
        );
    }
    // A failed save must be visible here and now — otherwise the reorder
    // looks applied and silently reverts on next launch.
    if let Some(error) = settings.preferred_order_error.as_deref() {
        col = col.push(error_caption(
            t!(i18n, ids::LIBRARY_ORDER_SAVE_FAILED, "error" => error),
            tokens,
        ));
    }
    col.boxed()
}

/// The bordered list of distributors, first tried at the top.
fn order_list<'a>(
    settings: &'a DistributorSettings,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let r = resolve(tokens);
    let (line, line_soft, ink3) = (ti(tokens.border), r.line_soft, r.ink3);
    let last = settings.preferred_order.len().saturating_sub(1);
    let rows = settings
        .preferred_order
        .iter()
        .enumerate()
        .map(|(i, &src)| {
            let name: Label<'a> = match distributor_label(src) {
                // raw: distributor names are product names, the same in every language.
                Some(name) => raw(name),
                None => t!(i18n, ids::LIBRARY_DISTRIBUTOR_OTHER).into(),
            };
            let grip = mouse_area(grip_handle(tokens))
                .on_press(settings_msg(SettingsMsg::PreferenceDragStarted(src)))
                .interaction(iced::mouse::Interaction::Grab);
            let content = row![
                grip,
                text(t!(i18n, ids::LIBRARY_ORDER_RANK, "rank" => i + 1))
                    .size(TS.caption)
                    .font(mono())
                    .color(ink3),
                text(name).size(TS.body).width(Length::Fill),
            ]
            .spacing(SP.m)
            .align_y(Alignment::Center);
            let fill = row_fill(settings, src, tokens);
            let cell = container(content)
                .padding([SP.xs, SP.m + SP.xs])
                .height(ChromeMetrics::DEFAULT.field + SP.m)
                .align_y(iced::alignment::Vertical::Center)
                .width(Length::Fill)
                .style(move |_: &Theme| container::Style {
                    background: fill.map(Background::Color),
                    border: separator(line_soft, i < last),
                    ..container::Style::default()
                });
            let mut area = mouse_area(cell);
            if settings.preferred_order_drag.is_some() {
                area = area
                    .on_enter(settings_msg(SettingsMsg::PreferenceDragHovered(src)))
                    .on_release(settings_msg(SettingsMsg::PreferenceDropped(src)));
            }
            area.boxed()
        });
    container(Column::with_children(rows))
        .width(Length::Fill)
        .clip(true)
        .style(move |_: &Theme| container::Style {
            border: Border {
                width: 1.0,
                color: line,
                radius: Radii::GRAPHITE.button.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// Fill of a row while a drag is on: the drop target takes the accent tint,
/// the dragged row a hover tint; otherwise none.
fn row_fill(
    settings: &DistributorSettings,
    src: DistributorSource,
    tokens: &ThemeTokens,
) -> Option<Color> {
    let dragged = settings.preferred_order_drag?;
    if settings.preferred_order_drop_target == Some(src) && dragged != src {
        Some(resolve(tokens).accent_soft)
    } else if dragged == src {
        Some(crate::styles::tokens::hover_overlay(tokens))
    } else {
        None
    }
}

/// A hairline under every row but the last.
fn separator(color: Color, below: bool) -> Border {
    let mut border = Border {
        color,
        ..Border::default()
    };
    if below {
        border.bottom = iced::border::Side::default().width(1.0);
    }
    border
}

/// The drag handle of a reorderable row: two columns of three dots in the
/// faint ink. Shared with the Component Classes table.
pub(crate) fn grip_handle<'a, M: 'a>(tokens: &ThemeTokens) -> Element<'a, M> {
    let ink = resolve(tokens).ink4;
    let dot = move || -> Element<'a, M> {
        container(Space::new().width(GRIP_DOT).height(GRIP_DOT))
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(ink)),
                border: Border {
                    radius: Radii::GRAPHITE.pill.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .boxed()
    };
    let dots = column![
        row![dot(), dot()].spacing(GRIP_DOT),
        row![dot(), dot()].spacing(GRIP_DOT),
        row![dot(), dot()].spacing(GRIP_DOT),
    ]
    .spacing(GRIP_DOT);
    container(dots).padding(SP.xs).boxed()
}
