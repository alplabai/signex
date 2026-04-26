//! "New Component" modal — opened from File ▸ Library ▸ New
//! Component… Phase 1 collects an `internal_pn` + which open
//! library to add the new draft to. On Submit the dispatcher
//! creates a fresh `Component` with one `LifecycleState::Draft`
//! revision, persists via `adapter.save_revision`, and opens the
//! Component Editor on the new component.
//!
//! Shape:
//!
//! ```text
//! ┌─[New Component ─────────────────────────────────────── X]─┐
//! │ Internal PN  [______________________________________]    │
//! │ Library      [▾ MyComponents                          ]   │
//! │                                                          │
//! │ <error string, if any>                                   │
//! ├──────────────────────────────────────────────────────────┤
//! │                                  [ Cancel ] [ Create ]   │
//! └──────────────────────────────────────────────────────────┘
//! ```
//!
//! Reuses the picker modal's chrome (modal_card / modal_header_strip
//! / modal_footer_strip) for visual parity with the rest of the
//! Library subsystem.

use iced::widget::{Space, button, column, container, pick_list, row, text, text_input};
use iced::{Border, Element, Length, Theme};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::messages::LibraryMessage;
use super::state::{LibraryState, NewComponentState};

const MODAL_W: f32 = 520.0;
const MODAL_H: f32 = 320.0;

/// `pick_list` adapter — wraps the index so we can derive a printable
/// string from the open-library list for the dropdown rendering, but
/// emit the index when the user picks.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LibraryPick {
    idx: usize,
    label: String,
}

impl std::fmt::Display for LibraryPick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

pub fn view<'a>(
    state: &'a LibraryState,
    nc: &'a NewComponentState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    let header = container(
        row![
            text("New Component").size(14).color(text_c),
            Space::new().width(Length::Fill),
            close_x(LibraryMessage::CloseNewComponent, tokens),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 14])
    .style(crate::styles::modal_header_strip(tokens));

    let pn_input = text_input("e.g. R0805_10k", &nc.internal_pn)
        .on_input(LibraryMessage::NewComponentSetInternalPn)
        .padding(6)
        .size(12);

    // Library dropdown — every open library shows up by display name.
    // We map the picked variant back to its `idx` and emit
    // `NewComponentSetLibrary(idx)`. The dropdown is empty (and
    // submit disabled) when no libraries are open.
    let lib_picks: Vec<LibraryPick> = state
        .open_libraries
        .iter()
        .enumerate()
        .map(|(i, lib)| LibraryPick {
            idx: i,
            label: lib.display_name.clone(),
        })
        .collect();
    let selected_pick = nc
        .library_idx
        .and_then(|i| lib_picks.iter().find(|p| p.idx == i).cloned());

    let lib_picker: Element<'_, LibraryMessage> = if lib_picks.is_empty() {
        text("No open libraries — open one via File ▸ Library ▸ Open Library… first.")
            .size(11)
            .color(muted)
            .into()
    } else {
        pick_list(lib_picks.clone(), selected_pick, |pick: LibraryPick| {
            LibraryMessage::NewComponentSetLibrary(pick.idx)
        })
        .placeholder("Select library…")
        .padding(6)
        .text_size(12)
        .into()
    };

    let pn_label: Element<'_, LibraryMessage> =
        text("Internal PN").size(11).color(muted).into();
    let lib_label: Element<'_, LibraryMessage> = text("Library").size(11).color(muted).into();

    let form = column![
        column![pn_label, container(pn_input).padding([2, 0])].spacing(4),
        Space::new().height(8),
        column![lib_label, container(lib_picker).padding([2, 0])].spacing(4),
    ]
    .spacing(0)
    .padding([16, 16]);

    let error_row: Element<'_, LibraryMessage> = if let Some(err) = nc.error.as_ref() {
        container(
            text(err.clone())
                .size(11)
                .color(iced::Color::from_rgb(0.85, 0.3, 0.3)),
        )
        .padding([0, 16])
        .into()
    } else {
        Space::new().height(0).into()
    };

    let submit_enabled = !nc.internal_pn.trim().is_empty() && nc.library_idx.is_some();
    let submit_bg = if submit_enabled {
        iced::Color::from_rgb(0.00, 0.47, 0.84)
    } else {
        iced::Color::from_rgba(1.0, 1.0, 1.0, 0.04)
    };
    let submit_fg = if submit_enabled {
        iced::Color::WHITE
    } else {
        iced::Color::from_rgba(1.0, 1.0, 1.0, 0.4)
    };
    let mut submit_btn = button(
        container(text("Create").size(11).color(submit_fg)).padding([4, 14]),
    )
    .style(move |_: &Theme, _| iced::widget::button::Style {
        background: Some(iced::Background::Color(submit_bg)),
        text_color: submit_fg,
        border: Border {
            width: 0.0,
            radius: 3.0.into(),
            ..Border::default()
        },
        ..iced::widget::button::Style::default()
    });
    if submit_enabled {
        submit_btn = submit_btn.on_press(LibraryMessage::NewComponentSubmit);
    }

    let footer = container(
        row![
            Space::new().width(Length::Fill),
            button(container(text("Cancel").size(11).color(text_c)).padding([4, 14]))
                .on_press(LibraryMessage::CloseNewComponent)
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
            Space::new().width(8),
            submit_btn,
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 14])
    .style(crate::styles::modal_footer_strip(tokens));

    container(
        column![header, form, error_row, Space::new().height(Length::Fill), footer]
            .width(Length::Fixed(MODAL_W))
            .height(Length::Fixed(MODAL_H)),
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
