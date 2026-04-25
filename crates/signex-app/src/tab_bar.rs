//! Document tab bar — tabs for open schematic sheets and PCB.

use iced::widget::{Row, container, mouse_area, row, text};
use iced::{Element, Length};
use signex_types::theme::ThemeTokens;
use signex_widgets::tab_pill::{TabPill, TabPillStyle};

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
    // Tabs sit flush against each other — Altium parity. The active
    // pill's accent underline distinguishes it from neighbours; no
    // gap or vertical divider needed.
    let mut bar = Row::new().spacing(0.0);

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
        // entire hit target.
        //
        // The pill itself is a custom widget (`signex_widgets::TabPill`)
        // that paints its own bg + 3-sided border (top + L/R only) +
        // 2 px accent strip below. Iced's stock Border can't do "top
        // and sides only", and a stacked-bg fake leaked accent through
        // the rounded corners (visible on dark themes).
        let pill_style = TabPillStyle {
            fill: pill_fill(tokens, is_active, is_dragging),
            border: styles::ti(tokens.border),
            accent: styles::ti(tokens.accent),
            is_active,
        };
        let inner = container(
            row![text(label).size(11).color(text_c)]
                .spacing(6.0)
                .align_y(iced::Alignment::Center),
        )
        .padding([4, 10]);
        let tab_el: Element<'_, TabMessage> = mouse_area(TabPill::new(inner, pill_style))
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

/// Resolve the pill bg fill for the current state. Active uses
/// `tokens.hover` at full alpha; inactive at 0.35×; dragging tints
/// with the theme accent at 22 %.
fn pill_fill(
    tokens: &ThemeTokens,
    is_active: bool,
    is_dragging: bool,
) -> iced::Color {
    let tab_active = styles::ti(tokens.hover);
    let accent = styles::ti(tokens.accent);
    if is_dragging {
        iced::Color { a: 0.22, ..accent }
    } else if is_active {
        tab_active
    } else {
        iced::Color {
            a: tab_active.a * 0.35,
            ..tab_active
        }
    }
}
