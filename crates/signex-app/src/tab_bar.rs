//! Document tab bar — tabs for open schematic sheets and PCB.

use iced::widget::{Row, button, container, mouse_area, row, text};
use iced::{Background, Border, Color, Element, Length, Theme};
use signex_types::theme::ThemeTokens;

use crate::app::TabInfo;
use crate::styles;

#[derive(Debug, Clone)]
pub enum TabMessage {
    Select(usize),
    Close(usize),
    /// Pop this tab into its own OS window (Altium-style undock). The
    /// tab stays in `document_state.tabs` while the window lives; closing
    /// the window reattaches it in place.
    Undock(usize),
    /// User pressed the mouse on this tab at (x, y). Arms the tab for
    /// drag-to-undock — if the cursor later leaves the main window, the
    /// tab auto-detaches.
    StartDrag(usize, f32, f32),
}

pub fn view<'a>(
    tabs: &[TabInfo],
    active: usize,
    visible_paths: &std::collections::HashSet<std::path::PathBuf>,
    tokens: &ThemeTokens,
) -> Element<'a, TabMessage> {
    let mut bar = Row::new().spacing(2.0);

    let text_primary = styles::ti(tokens.text);
    let text_muted = styles::ti(tokens.text_secondary);
    let tab_active_bg = styles::ti(tokens.hover);
    let border = styles::ti(tokens.border);

    for (i, tab) in tabs.iter().enumerate() {
        // Only show tabs that belong to the window being rendered. Main
        // gets all tabs except those owned by undocked windows; undocked
        // windows get only their owned tab.
        if !visible_paths.contains(&tab.path) {
            continue;
        }

        let label = if tab.dirty {
            format!("{} \u{2022}", tab.title) // bullet for dirty
        } else {
            tab.title.clone()
        };

        let is_active = i == active;
        let text_c = if is_active { text_primary } else { text_muted };

        // Close button — visible "×" with hover highlight
        let hover_close = Color::from_rgb(0.35, 0.35, 0.38);
        let close_btn = button(text("\u{00D7}").size(14).color(text_muted))
            .padding([0, 4])
            .style(move |_: &Theme, status: button::Status| {
                let bg = match status {
                    button::Status::Hovered => Some(Background::Color(hover_close)),
                    _ => None,
                };
                button::Style {
                    background: bg,
                    border: Border {
                        radius: 2.0.into(),
                        ..Border::default()
                    },
                    ..button::Style::default()
                }
            })
            .on_press(TabMessage::Close(i));

        // Undock button — ↗ arrow. Pops the tab into its own OS window.
        let undock_btn = button(text("\u{2197}").size(12).color(text_muted))
            .padding([0, 4])
            .style(move |_: &Theme, status: button::Status| {
                let bg = match status {
                    button::Status::Hovered => Some(Background::Color(hover_close)),
                    _ => None,
                };
                button::Style {
                    background: bg,
                    border: Border {
                        radius: 2.0.into(),
                        ..Border::default()
                    },
                    ..button::Style::default()
                }
            })
            .on_press(TabMessage::Undock(i));

        // Use a non-capturing container for the tab body so the outer
        // mouse_area actually sees ButtonPressed — iced's mouse_area
        // bails out when the inner widget captures the event, and
        // button does capture on press. The undock / close buttons
        // still capture their own presses so those clicks behave
        // normally; presses on the text area fall through to the
        // mouse_area's on_press (StartDrag).
        let tab_body = container(
            row![text(label).size(11).color(text_c), undock_btn, close_btn,]
                .spacing(6.0)
                .align_y(iced::Alignment::Center),
        )
        .padding([4, 10])
        .style(move |_: &Theme| container::Style {
            background: if is_active {
                Some(Background::Color(tab_active_bg))
            } else {
                None
            },
            border: Border {
                width: 1.0,
                radius: 0.0.into(),
                color: border,
            },
            ..container::Style::default()
        });

        let tab_el: Element<'_, TabMessage> = mouse_area(tab_body)
            .on_press(TabMessage::StartDrag(i, 0.0, 0.0))
            .on_release(TabMessage::Select(i))
            // Grab cursor advertises that the tab is draggable —
            // discoverability for the Altium-style drag-to-undock
            // behaviour.
            .interaction(iced::mouse::Interaction::Grab)
            .into();
        bar = bar.push(tab_el);
    }

    container(bar)
        .width(Length::Fill)
        .padding([2, 6])
        .style(styles::toolbar_strip(tokens))
        .into()
}
