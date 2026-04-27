//! Edit Component Details modal — full row editor invoked by
//! double-clicking a row in the Library Browser grid.
//!
//! `view_delete_confirm` (Deliverable D) lands in this same module.

use std::collections::BTreeMap;

use iced::widget::{
    Column, Space, button, column, container, pick_list, row, scrollable, text, text_input,
};
use iced::{Border, Element, Length, Theme};
use signex_library::{ComponentClass, DatasheetRef, LifecycleState, ParamValue};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::messages::{BrowserEditMsg, LibraryMessage};
use super::state::{BUILTIN_CLASSES, EditRowModalState};

const MODAL_W: f32 = 720.0;
const MODAL_H: f32 = 640.0;

/// `pick_list` adapter for the Class dropdown.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ClassPick {
    key: String,
    label: String,
}

impl std::fmt::Display for ClassPick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

/// `pick_list` adapter for the Lifecycle State dropdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LifecyclePick(LifecycleState);

impl std::fmt::Display for LifecyclePick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

const LIFECYCLE_OPTIONS: [LifecyclePick; 5] = [
    LifecyclePick(LifecycleState::Draft),
    LifecyclePick(LifecycleState::InReview),
    LifecyclePick(LifecycleState::Released),
    LifecyclePick(LifecycleState::Deprecated),
    LifecyclePick(LifecycleState::Obsolete),
];

pub fn view<'a>(
    library_path: &'a std::path::Path,
    edit: &'a EditRowModalState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    let title = format!("Edit Component — {}", edit.draft.internal_pn.as_str());
    let header = container(
        row![
            text(title).size(14).color(text_c),
            Space::new().width(Length::Fill),
            close_x(library_path, tokens),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 14])
    .style(crate::styles::modal_header_strip(tokens));

    let lib_for_msg = library_path.to_path_buf();
    let send = move |m: BrowserEditMsg| LibraryMessage::BrowserEdit {
        library_path: lib_for_msg.clone(),
        msg: m,
    };

    let pn_input = text_input("Internal PN", edit.draft.internal_pn.as_str())
        .on_input({
            let send = send.clone();
            move |s| send(BrowserEditMsg::SetInternalPn(s))
        })
        .padding(6)
        .size(12);

    let class_picks: Vec<ClassPick> = BUILTIN_CLASSES
        .iter()
        .map(|(key, label)| ClassPick {
            key: (*key).to_string(),
            label: (*label).to_string(),
        })
        .collect();
    let selected_class = class_picks
        .iter()
        .find(|p| p.key == edit.draft.class.as_str())
        .cloned();
    let class_picker = pick_list(class_picks, selected_class, {
        let send = send.clone();
        move |pick: ClassPick| send(BrowserEditMsg::SetClass(ComponentClass::new(pick.key)))
    })
    .placeholder("Select class…")
    .padding(6)
    .text_size(12);

    let selected_state = LIFECYCLE_OPTIONS
        .iter()
        .find(|p| p.0 == edit.draft.state)
        .copied();
    let state_picker = pick_list(LIFECYCLE_OPTIONS.to_vec(), selected_state, {
        let send = send.clone();
        move |pick: LifecyclePick| send(BrowserEditMsg::SetState(pick.0))
    })
    .placeholder("Select lifecycle…")
    .padding(6)
    .text_size(12);

    let datasheet_url: &str = match &edit.draft.datasheet {
        DatasheetRef::Url { url } => url.as_str(),
        DatasheetRef::HashPinned { filename, .. } => filename.as_str(),
    };
    let datasheet_input = text_input("https://example.com/datasheet.pdf", datasheet_url)
        .on_input({
            let send = send.clone();
            move |s| send(BrowserEditMsg::SetDatasheetUrl(s))
        })
        .padding(6)
        .size(12);

    let mfr_input = text_input("Manufacturer", &edit.draft.primary_mpn.manufacturer)
        .on_input({
            let send = send.clone();
            move |s| send(BrowserEditMsg::SetManufacturer(s))
        })
        .padding(6)
        .size(12);

    let mpn_input = text_input("MPN", &edit.draft.primary_mpn.mpn)
        .on_input({
            let send = send.clone();
            move |s| send(BrowserEditMsg::SetMpn(s))
        })
        .padding(6)
        .size(12);

    // Symbol / Footprint ref rows.
    let symbol_short = short_uuid(&edit.draft.symbol_ref.uuid);
    let symbol_label = if edit.draft.symbol_ref.uuid.is_nil() {
        "Unbound — pick a symbol".to_string()
    } else {
        format!("symbol uuid {}", symbol_short)
    };
    let symbol_btn =
        button(container(text("Pick Symbol…").size(11).color(text_c)).padding([4, 12]))
            .on_press(send.clone()(BrowserEditMsg::OpenSymbolPicker))
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
            });

    let footprint_label = match edit.draft.footprint_ref.as_ref() {
        Some(r) => format!("footprint uuid {}", short_uuid(&r.uuid)),
        None => "Unbound (optional)".to_string(),
    };
    let footprint_btn =
        button(container(text("Pick Footprint…").size(11).color(text_c)).padding([4, 12]))
            .on_press(send.clone()(BrowserEditMsg::OpenFootprintPicker))
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
            });

    let labelled =
        |lbl: &'a str, body: Element<'a, LibraryMessage>| -> Element<'a, LibraryMessage> {
            column![
                text(lbl).size(10).color(muted),
                container(body).padding([2, 0])
            ]
            .spacing(2)
            .into()
        };

    let pn_class_state_row = row![
        labelled("Internal PN", pn_input.into()),
        Space::new().width(8),
        labelled("Class", class_picker.into()),
        Space::new().width(8),
        labelled("State", state_picker.into()),
    ]
    .spacing(0);

    let mfr_mpn_row = row![
        labelled("Manufacturer", mfr_input.into()),
        Space::new().width(8),
        labelled("MPN", mpn_input.into()),
    ]
    .spacing(0);

    let symbol_row = row![
        text(symbol_label)
            .size(11)
            .color(text_c)
            .width(Length::Fill),
        symbol_btn,
    ]
    .align_y(iced::Alignment::Center)
    .padding([4, 0]);

    let footprint_row = row![
        text(footprint_label)
            .size(11)
            .color(text_c)
            .width(Length::Fill),
        footprint_btn,
    ]
    .align_y(iced::Alignment::Center)
    .padding([4, 0]);

    // Parameters table.
    let params_section = view_params_section(edit, tokens, send.clone());

    let form_body = column![
        pn_class_state_row,
        Space::new().height(8),
        labelled("Datasheet URL", datasheet_input.into()),
        Space::new().height(8),
        mfr_mpn_row,
        Space::new().height(12),
        text("Symbol ref").size(10).color(muted),
        symbol_row,
        Space::new().height(6),
        text("Footprint ref").size(10).color(muted),
        footprint_row,
        Space::new().height(12),
        params_section,
    ]
    .spacing(0)
    .padding([14, 16]);

    let body = container(
        scrollable(form_body)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill);

    let error_row: Element<'_, LibraryMessage> = if let Some(err) = edit.error.as_ref() {
        container(
            text(err.clone())
                .size(11)
                .color(iced::Color::from_rgb(0.85, 0.3, 0.3)),
        )
        .padding([4, 16])
        .into()
    } else {
        Space::new().height(0).into()
    };

    let cancel_btn = button(container(text("Cancel").size(11).color(text_c)).padding([4, 14]))
        .on_press(send.clone()(BrowserEditMsg::Cancel))
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
        });

    let save_btn =
        button(container(text("Save").size(11).color(iced::Color::WHITE)).padding([4, 14]))
            .on_press(send(BrowserEditMsg::Save))
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgb(
                    0.00, 0.47, 0.84,
                ))),
                text_color: iced::Color::WHITE,
                border: Border {
                    width: 0.0,
                    radius: 3.0.into(),
                    ..Border::default()
                },
                ..iced::widget::button::Style::default()
            });

    let footer = container(
        row![
            Space::new().width(Length::Fill),
            cancel_btn,
            Space::new().width(8),
            save_btn,
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 14])
    .style(crate::styles::modal_footer_strip(tokens));

    container(
        column![header, body, error_row, footer]
            .width(Length::Fixed(MODAL_W))
            .height(Length::Fixed(MODAL_H)),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .into()
}

fn view_params_section<'a>(
    edit: &'a EditRowModalState,
    tokens: &'a ThemeTokens,
    send: impl Fn(BrowserEditMsg) -> LibraryMessage + Clone + 'a,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    let header = row![
        text("Parameters").size(11).color(muted).width(Length::Fill),
        button(container(text("+ Add Parameter").size(11).color(text_c)).padding([3, 10]))
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

    let mut col: Column<'a, LibraryMessage> = column![header, Space::new().height(4)].spacing(0);

    let header_row = container(
        row![
            text("Key")
                .size(10)
                .color(muted)
                .width(Length::FillPortion(3)),
            text("Value")
                .size(10)
                .color(muted)
                .width(Length::FillPortion(3)),
            text("Unit")
                .size(10)
                .color(muted)
                .width(Length::FillPortion(2)),
            text("").size(10).width(Length::Fixed(40.0)),
        ]
        .padding([2, 6]),
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

    if edit.draft.parameters.is_empty() {
        col = col.push(container(text("No parameters yet.").size(11).color(muted)).padding([8, 6]));
    } else {
        // Iterate in BTreeMap key order for stable layout.
        let entries: BTreeMap<&String, &ParamValue> = edit.draft.parameters.iter().collect();
        for (key, value) in entries {
            let key_owned = key.clone();
            let (cur_value, cur_unit) = match edit.param_buf.get(&key_owned) {
                Some((v, u)) => (v.clone(), u.clone()),
                None => match value {
                    ParamValue::Text(s) => (s.clone(), String::new()),
                    ParamValue::Number(n) => (n.to_string(), String::new()),
                    ParamValue::Bool(b) => (b.to_string(), String::new()),
                    ParamValue::Measurement { value, unit } => (value.to_string(), unit.clone()),
                },
            };

            let send_v = send.clone();
            let key_for_v = key_owned.clone();
            let val_input = text_input("value", &cur_value)
                .on_input(move |s| {
                    send_v(BrowserEditMsg::SetParamValue {
                        key: key_for_v.clone(),
                        value: s,
                    })
                })
                .on_submit({
                    let send_c = send.clone();
                    let key_c = key_owned.clone();
                    send_c(BrowserEditMsg::CommitParam { key: key_c })
                })
                .padding(4)
                .size(11);

            let send_u = send.clone();
            let key_for_u = key_owned.clone();
            let unit_input = text_input("unit", &cur_unit)
                .on_input(move |s| {
                    send_u(BrowserEditMsg::SetParamUnit {
                        key: key_for_u.clone(),
                        unit: s,
                    })
                })
                .on_submit({
                    let send_c = send.clone();
                    let key_c = key_owned.clone();
                    send_c(BrowserEditMsg::CommitParam { key: key_c })
                })
                .padding(4)
                .size(11);

            let send_del = send.clone();
            let key_for_del = key_owned.clone();
            let delete_btn = button(container(text("✕").size(11).color(muted)).padding([2, 8]))
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

            col = col.push(
                row![
                    container(text(key_owned.clone()).size(11).color(text_c))
                        .padding([2, 6])
                        .width(Length::FillPortion(3)),
                    container(val_input)
                        .padding([2, 4])
                        .width(Length::FillPortion(3)),
                    container(unit_input)
                        .padding([2, 4])
                        .width(Length::FillPortion(2)),
                    container(delete_btn)
                        .padding([2, 6])
                        .width(Length::Fixed(40.0)),
                ]
                .align_y(iced::Alignment::Center),
            );
        }
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

fn close_x<'a>(
    library_path: &'a std::path::Path,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);
    let lib_owned = library_path.to_path_buf();
    button(container(text("\u{00D7}".to_string()).size(14).color(text_c)).padding([0, 6]))
        .on_press(LibraryMessage::BrowserEdit {
            library_path: lib_owned,
            msg: BrowserEditMsg::Cancel,
        })
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let bg = match status {
                iced::widget::button::Status::Hovered => Some(iced::Background::Color(
                    iced::Color::from_rgba(1.0, 1.0, 1.0, 0.10),
                )),
                _ => Some(iced::Background::Color(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, 0.03,
                ))),
            };
            iced::widget::button::Style {
                background: bg,
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border,
                },
                text_color: text_c,
                ..iced::widget::button::Style::default()
            }
        })
        .into()
}

pub(crate) fn short_uuid(u: &uuid::Uuid) -> String {
    let s = u.simple().to_string();
    if s.len() >= 8 { s[..8].to_string() } else { s }
}
