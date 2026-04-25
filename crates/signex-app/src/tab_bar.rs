//! Document tab bar — tabs for open schematic sheets and PCB.

use iced::widget::{Row, container, mouse_area, row, text};
use iced::{Element, Length};
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
        let is_dragging = dragging == Some(i);
        let text_c = if is_active { text_primary } else { text_muted };

        // No inline close-X / undock buttons. Both actions live in the
        // right-click menu now (Altium parity) — the tab itself is the
        // entire hit target, which makes drag-to-reorder and drag-to-
        // detach feel uniform across the strip.
        //
        // Visual stack (top-down):
        //   - inner pill   (top-rounded, no border, fill follows state)
        //   - 2 px gap     (bottom-padding on outer)
        //   - accent line  (outer container's bg)
        // This matches the panel-tab look: no visible bottom border,
        // accent stripe under the active tab. Shared style helpers in
        // `crate::styles` keep document and panel tabs in lockstep.
        let label_el = container(
            row![text(label).size(11).color(text_c)]
                .spacing(6.0)
                .align_y(iced::Alignment::Center),
        )
        .padding([4, 10])
        .style(styles::tab_pill(tokens, is_active, is_dragging, false));
        let tab_el: Element<'_, TabMessage> = mouse_area(
            container(label_el)
                .padding(iced::Padding {
                    top: 0.0,
                    right: 0.0,
                    bottom: 2.0,
                    left: 0.0,
                })
                .style(styles::tab_pill_underline(tokens, is_active)),
        )
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
