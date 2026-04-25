//! Document tab bar — tabs for open schematic sheets and PCB.

use iced::widget::{Row, container, mouse_area, row, text};
use iced::{Background, Border, Color, Element, Length, Theme};
use signex_types::theme::ThemeTokens;

use crate::app::TabInfo;
use crate::styles;

#[derive(Debug, Clone)]
pub enum TabMessage {
    Select(usize),
    /// User pressed the mouse on this tab at (x, y). Arms the tab for
    /// drag-to-undock — if the cursor later leaves the main window, the
    /// tab auto-detaches.
    StartDrag(usize, f32, f32),
    /// Right-click on this tab — the app opens the per-tab context
    /// menu (Close [filename] / Close All Others / Close All / Open
    /// In New Window). Replaces the inline close-X / undock buttons
    /// that used to live on every tab.
    ContextMenu(usize),
}

pub fn view<'a>(
    tabs: &[TabInfo],
    active: usize,
    dragging: Option<usize>,
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

        // Tab title — no dirty indicator here. The unsaved-changes
        // marker now lives on the corresponding row in the Projects
        // tree (red dot to the right of the file name) so the user
        // sees dirty state without it competing with tab close-X.
        let label = tab.title.clone();

        let is_active = i == active;
        let text_c = if is_active { text_primary } else { text_muted };

        // No inline close-X / undock buttons. Both actions live in the
        // right-click menu now (Altium parity) — the tab itself is the
        // entire hit target, which makes drag-to-reorder and drag-to-
        // detach feel uniform across the strip.
        let is_dragging = dragging == Some(i);
        let accent = Color::from_rgb(0.00, 0.47, 0.84);
        let drag_bg = Color::from_rgba(0.0, 0.47, 0.84, 0.18);
        let bg = if is_dragging {
            Some(Background::Color(drag_bg))
        } else if is_active {
            Some(Background::Color(tab_active_bg))
        } else {
            None
        };
        let border_w = if is_dragging { 2.0 } else { 1.0 };
        let border_c = if is_dragging { accent } else { border };
        let tab_body = container(
            row![text(label).size(11).color(text_c)]
                .spacing(6.0)
                .align_y(iced::Alignment::Center),
        )
        .padding([4, 10])
        .style(move |_: &Theme| container::Style {
            background: bg,
            border: Border {
                width: border_w,
                radius: 0.0.into(),
                color: border_c,
            },
            ..container::Style::default()
        });

        let tab_el: Element<'_, TabMessage> = mouse_area(tab_body)
            .on_press(TabMessage::StartDrag(i, 0.0, 0.0))
            .on_release(TabMessage::Select(i))
            .on_right_press(TabMessage::ContextMenu(i))
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
