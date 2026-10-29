//! The parameters table of the Edit Component Details modal.

use iced::widget::{Column, Space, button, column, container, row, text, text_input};
use iced::{Border, Element, Length, Theme};
use signex_library::{
    ParamKind, ParamSlot, ParamValue, ParameterTemplate, slot_problem, slot_unit,
};
use signex_types::quantity::{SiPrefix, Unit};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::messages::{BrowserEditMsg, LibraryMessage};
use super::prefix_picker::prefix_picker;
use super::state::EditRowModalState;
use super::unit_picker::{unit_of_token, unit_picker};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

pub(super) fn view<'a>(
    edit: &'a EditRowModalState,
    template: Option<&'a ParameterTemplate>,
    tokens: &'a ThemeTokens,
    send: impl Fn(BrowserEditMsg) -> LibraryMessage + Clone + 'a,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    let header = row![
        text("Parameters")
            .size(TS.body)
            .color(muted)
            .width(Length::Fill),
        button(
            container(text("+ Add Parameter").size(TS.body).color(text_c)).padding([SP.xs, SP.l])
        )
        .on_press(send.clone()(BrowserEditMsg::AddParam))
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.04,
            ))),
            text_color: text_c,
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
            },
            ..iced::widget::button::Style::default()
        }),
    ]
    .align_y(iced::Alignment::Center);

    let mut col: Column<'a, LibraryMessage> = column![header, Space::new().height(SP.s)].spacing(0);

    let header_row = container(
        row![
            text("Key")
                .size(TS.caption)
                .color(muted)
                .width(Length::FillPortion(3)),
            text("Value")
                .size(TS.caption)
                .color(muted)
                .width(Length::FillPortion(3)),
            text("Unit")
                .size(TS.caption)
                .color(muted)
                .width(Length::FillPortion(2)),
            text("").size(TS.caption).width(Length::Fixed(40.0)),
        ]
        .padding([SP.xs, 6.0]),
    )
    .style(move |_: &Theme| iced::widget::container::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            1.0, 1.0, 1.0, 0.04,
        ))),
        border: Border {
            width: 0.0,
            radius: 2.0.into(),
            color: border,
        },
        ..Default::default()
    });
    col = col.push(header_row);

    // Stage 18: `tags` has its own dedicated input above this section,
    // so we hide it from the parameters table to avoid a duplicate row.
    let visible_params: Vec<(&String, &ParamValue)> = edit
        .draft
        .parameters
        .iter()
        .filter(|(k, _)| k.as_str() != "tags")
        .collect();
    if visible_params.is_empty() {
        col = col.push(
            container(text("No parameters yet.").size(TS.body).color(muted)).padding([SP.m, 6.0]),
        );
    }
    // BTreeMap key order keeps the layout stable.
    for (key, value) in visible_params {
        let slot = template.and_then(|t| t.slot(key));
        col = col.push(param_row(edit, key, value, slot, tokens, &send));
    }

    container(col)
        .padding([0, 0])
        .style(move |_: &Theme| iced::widget::container::Style {
            background: None,
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
            },
            ..Default::default()
        })
        .into()
}

/// One parameter row: key, number field with its prefix picker, unit,
/// delete. A class-template measurement slot fixes the unit (a label, no
/// picker) and its sign rules apply; any other parameter picks its unit.
fn param_row<'a>(
    edit: &'a EditRowModalState,
    key: &str,
    value: &ParamValue,
    slot: Option<&ParamSlot>,
    tokens: &'a ThemeTokens,
    send: &(impl Fn(BrowserEditMsg) -> LibraryMessage + Clone + 'a),
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);
    let key_owned = key.to_string();

    let fixed_unit = slot
        .filter(|s| s.kind == ParamKind::Measurement)
        .and_then(slot_unit);
    let (cur_value, buf_unit) = match edit.param_buf.get(key) {
        Some((v, u)) => (v.clone(), u.clone()),
        None => EditRowModalState::param_buf_entry(value),
    };
    // The token the row edits in: the slot's, else the buffer's.
    let cur_unit = fixed_unit
        .as_ref()
        .map(|u| u.token().to_string())
        .unwrap_or(buf_unit);

    let send_v = send.clone();
    let key_for_v = key_owned.clone();
    let send_c = send.clone();
    let key_c = key_owned.clone();
    let val_input = text_input("value", &cur_value)
        .on_input(move |s| {
            send_v(BrowserEditMsg::SetParamValue {
                key: key_for_v.clone(),
                value: s,
            })
        })
        .on_submit(send_c(BrowserEditMsg::CommitParam { key: key_c }))
        .padding(SP.s)
        .size(TS.body);

    // A known unit takes the number in a picked SI prefix; the number
    // field itself never takes a prefix or unit. Hidden for units that
    // take no prefix (`%`, `ppm/C`).
    let prefix_pick = Unit::from_alias(cur_unit.trim()).and_then(|u| {
        let selected = edit.param_prefix.get(key).copied().unwrap_or(SiPrefix::One);
        let send_p = send.clone();
        let key_p = key_owned.clone();
        prefix_picker(&u, selected, TS.body, move |prefix| {
            send_p(BrowserEditMsg::ParamPrefixPicked {
                key: key_p.clone(),
                prefix,
            })
        })
    });
    let field: Element<'a, LibraryMessage> = match prefix_pick {
        Some(pick) => row![val_input, pick]
            .spacing(SP.s)
            .align_y(iced::Alignment::Center)
            .into(),
        None => val_input.into(),
    };
    // A stored value that already breaks its slot stays as it is until it
    // is edited; say so next to it.
    let problem = slot.and_then(|s| slot_problem(s, value));
    let value_cell: Element<'a, LibraryMessage> = match problem {
        Some(why) => column![
            field,
            text(format!("\u{2717} {why}"))
                .size(TS.caption)
                .color(theme_ext::error_color(tokens))
        ]
        .spacing(SP.xs)
        .into(),
        None => field,
    };

    // The unit is fixed by the slot, or picked, never typed. A unit string
    // the build does not know is shown as it is and kept untouched.
    let unit_cell: Element<'a, LibraryMessage> = match (&fixed_unit, unit_of_token(&cur_unit)) {
        (Some(u), _) => text(u.glyph().to_string())
            .size(TS.body)
            .color(muted)
            .into(),
        (None, Ok(selected)) => {
            let send_u = send.clone();
            let key_for_u = key_owned.clone();
            unit_picker(selected, true, TS.body, move |unit| {
                send_u(BrowserEditMsg::SetParamUnit {
                    key: key_for_u.clone(),
                    unit,
                })
            })
        }
        (None, Err(raw)) => text(raw.to_string()).size(TS.body).color(muted).into(),
    };

    let send_del = send.clone();
    let key_for_del = key_owned.clone();
    let delete_btn = button(container(text("✕").size(TS.body).color(muted)).padding([SP.xs, SP.m]))
        .on_press_with(move || {
            send_del(BrowserEditMsg::DeleteParam {
                key: key_for_del.clone(),
            })
        })
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.04,
            ))),
            text_color: muted,
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
            },
            ..iced::widget::button::Style::default()
        });

    row![
        container(text(key_owned).size(TS.body).color(text_c))
            .padding([SP.xs, 6.0])
            .width(Length::FillPortion(3)),
        container(value_cell)
            .padding([SP.xs, SP.s])
            .width(Length::FillPortion(3)),
        container(unit_cell)
            .padding([SP.xs, SP.s])
            .width(Length::FillPortion(2)),
        container(delete_btn)
            .padding([SP.xs, 6.0])
            .width(Length::Fixed(40.0)),
    ]
    .align_y(iced::Alignment::Center)
    .into()
}
