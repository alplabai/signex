//! Primitive picker modal — Pick Symbol / Pick Footprint.
//!
//! Listing source: every mounted library's primitives surfaced via
//! `LibraryAdapter::list_symbols` (resp. `list_footprints`). Filtering
//! is case-insensitive across the primitive name.
//!
//! Shape:
//!
//! ```text
//! ┌─[ Pick Symbol ]────────────────────────────────────┐
//! │ Filter: [_______________]                          │
//! │                                                    │
//! │ ▼ Loratis-SN-lib-2.snxlib                          │
//! │     ESP32-WROOM-32.snxsym       (uuid abc12345…)  │
//! │     LM7805.snxsym               (uuid bd45ee01…)  │
//! │ ▼ Loratis-SN-lib-3.snxlib                          │
//! │     74HC595.snxsym                                 │
//! │                                                    │
//! │ [ Browse filesystem… ]    [ Cancel ]   [ Pick ]    │
//! └────────────────────────────────────────────────────┘
//! ```

use iced::widget::{Space, button, column, container, row, scrollable, text, text_input};
use iced::{Border, Element, Length, Theme};
use signex_library::{PrimitiveKind, PrimitiveRef, PrimitiveSummary};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::messages::{LibraryMessage, PrimitivePickerMsg};
use super::state::{LibraryState, PrimitivePickerState};

const PICKER_W: f32 = 640.0;
const PICKER_H: f32 = 520.0;

pub fn view<'a>(
    state: &'a LibraryState,
    picker: &'a PrimitivePickerState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    let title = match picker.kind {
        PrimitiveKind::Symbol => "Pick Symbol",
        PrimitiveKind::Footprint => "Pick Footprint",
        PrimitiveKind::Sim => "Pick Sim Model",
        _ => "Pick Primitive",
    };

    let header = container(
        row![
            text(title).size(14).color(text_c),
            Space::new().width(Length::Fill),
            close_x(
                LibraryMessage::PrimitivePicker(PrimitivePickerMsg::Cancel),
                tokens
            ),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 14])
    .style(crate::styles::modal_header_strip(tokens));

    let search = text_input("Filter…", &picker.filter)
        .on_input(|s| LibraryMessage::PrimitivePicker(PrimitivePickerMsg::SetFilter(s)))
        .padding(6)
        .size(12);

    let needle = picker.filter.trim().to_lowercase();

    let mut list_col = column![].spacing(0);
    let mut total_visible = 0usize;
    for lib in &state.open_libraries {
        let Some(adapter) = state.set.get(lib.library_id) else {
            continue;
        };
        let summaries: Vec<PrimitiveSummary> = match picker.kind {
            PrimitiveKind::Symbol => adapter.list_symbols().unwrap_or_default(),
            PrimitiveKind::Footprint => adapter.list_footprints().unwrap_or_default(),
            PrimitiveKind::Sim => adapter.list_sims().unwrap_or_default(),
            _ => Vec::new(),
        };
        let filtered: Vec<&PrimitiveSummary> = summaries
            .iter()
            .filter(|s| {
                if needle.is_empty() {
                    return true;
                }
                s.name.to_lowercase().contains(&needle)
                    || s.uuid.simple().to_string().contains(&needle)
            })
            .collect();
        if filtered.is_empty() {
            continue;
        }
        // Library section header.
        list_col = list_col.push(
            container(
                text(format!(
                    "▼ {}.snxlib  ({})",
                    lib.display_name,
                    filtered.len()
                ))
                .size(11)
                .color(muted),
            )
            .padding([6, 8]),
        );
        for summary in filtered {
            total_visible += 1;
            let primitive_ref = PrimitiveRef::new(lib.library_id, summary.uuid);
            let short_uuid = {
                let s = summary.uuid.simple().to_string();
                if s.len() >= 8 { s[..8].to_string() } else { s }
            };
            let ext = match picker.kind {
                PrimitiveKind::Symbol => "snxsym",
                PrimitiveKind::Footprint => "snxfpt",
                PrimitiveKind::Sim => "snxsim",
                _ => "snx",
            };
            let label = format!("{}.{}", summary.name, ext);
            let row_widget = button(
                row![
                    Space::new().width(20),
                    text(label)
                        .size(11)
                        .color(text_c)
                        .width(Length::FillPortion(3)),
                    text(format!("(uuid {}…)", short_uuid))
                        .size(10)
                        .color(muted)
                        .width(Length::FillPortion(2)),
                ]
                .spacing(0)
                .padding([3, 8]),
            )
            .padding(0)
            .width(Length::Fill)
            .on_press(LibraryMessage::PrimitivePicker(PrimitivePickerMsg::Pick(
                primitive_ref,
            )))
            .style(move |_: &Theme, status: iced::widget::button::Status| {
                let bg = match status {
                    iced::widget::button::Status::Hovered => Some(iced::Background::Color(
                        iced::Color::from_rgba(1.0, 1.0, 1.0, 0.06),
                    )),
                    _ => None,
                };
                iced::widget::button::Style {
                    background: bg,
                    border: Border::default(),
                    text_color: text_c,
                    ..iced::widget::button::Style::default()
                }
            });
            list_col = list_col.push(row_widget);
        }
    }

    if total_visible == 0 {
        list_col = list_col.push(
            container(
                text(if state.open_libraries.is_empty() {
                    "No libraries open. Use Browse filesystem… to pick a primitive directly."
                } else {
                    "No primitives match. Open more libraries or use Browse filesystem…"
                })
                .size(11)
                .color(muted),
            )
            .padding([14, 14]),
        );
    }

    let body = container(
        scrollable(list_col)
            .height(Length::Fill)
            .width(Length::Fill),
    )
    .padding([6, 0])
    .height(Length::Fill);

    let error_row: Element<'_, LibraryMessage> = if let Some(err) = picker.error.as_ref() {
        container(
            text(err.clone())
                .size(11)
                .color(iced::Color::from_rgb(0.85, 0.3, 0.3)),
        )
        .padding([4, 14])
        .into()
    } else {
        Space::new().height(0).into()
    };

    let browse_btn =
        button(container(text("Browse filesystem…").size(11).color(text_c)).padding([4, 14]))
            .on_press(LibraryMessage::PrimitivePicker(PrimitivePickerMsg::Browse))
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

    let footer = container(
        row![
            browse_btn,
            Space::new().width(Length::Fill),
            button(container(text("Cancel").size(11).color(text_c)).padding([4, 14]))
                .on_press(LibraryMessage::PrimitivePicker(PrimitivePickerMsg::Cancel))
                .style(move |_: &Theme, _| iced::widget::button::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgba(
                        1.0, 1.0, 1.0, 0.04
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
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 14])
    .style(crate::styles::modal_footer_strip(tokens));

    let body_pad =
        container(column![container(search).padding([6, 12]), body, error_row].spacing(0))
            .width(Length::Fill)
            .height(Length::Fill);

    container(
        column![header, body_pad, footer]
            .width(Length::Fixed(PICKER_W))
            .height(Length::Fixed(PICKER_H)),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .into()
}

fn close_x<'a>(message: LibraryMessage, tokens: &ThemeTokens) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);
    button(container(text("\u{00D7}".to_string()).size(14).color(text_c)).padding([0, 6]))
        .on_press(message)
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
