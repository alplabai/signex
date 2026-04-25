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

    // Collect the indices of tabs that should render in this window —
    // we need the count up front so each pill knows whether it's the
    // last one (which draws the right edge; the rest skip it so
    // adjacent tabs share their L/R divider as a single 1 px line).
    let visible_indices: Vec<usize> = tabs
        .iter()
        .enumerate()
        .filter_map(|(i, tab)| visible_paths.contains(&tab.path).then_some(i))
        .collect();
    let last_idx = visible_indices.last().copied();

    for &i in &visible_indices {
        let tab = &tabs[i];
        // Tab title — no dirty indicator here. The unsaved-changes
        // marker now lives on the corresponding row in the Projects
        // tree (red dot to the right of the file name) so the user
        // sees dirty state without it competing with tab close-X.
        let label = tab.title.clone();

        let is_active = i == active;
        let is_dragging = dragging == Some(i);
        let is_last = Some(i) == last_idx;
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
            // Theme border colour — reads as a subtle divider on
            // each theme's chrome surface. Tried pure black; it
            // looked harsh on every theme.
            border: styles::ti(tokens.border),
            accent: styles::ti(tokens.accent),
            is_active,
            is_last,
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

    // Strip baseline — 1 px theme-border line at the bottom of the
    // tab bar. Inactive tabs sit "on" the line; the active tab's
    // accent stripe overlays it at its x-range, visually punching
    // through. Color follows the theme's border token so it reads
    // as part of the chrome rather than a hard black edge.
    let baseline_color = styles::ti(tokens.border);
    let baseline = container(iced::widget::Space::new())
        .width(Length::Fill)
        .height(1)
        .style(move |_: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(baseline_color)),
            ..iced::widget::container::Style::default()
        });
    container(iced::widget::column![
        container(bar)
            .width(Length::Fill)
            .padding(iced::Padding {
                top: 2.0,
                right: 6.0,
                bottom: 0.0,
                left: 6.0,
            }),
        baseline,
    ])
    .width(Length::Fill)
    .style(styles::toolbar_strip(tokens))
    .into()
}

/// Resolve the pill bg fill for the current state. Altium parity:
/// inactive tabs are dimmer than active so they read as "off" but
/// still show their tab body (transparent inactive made them look
/// like floating labels — the strip showed through). Active uses
/// `tokens.hover` at full alpha; inactive 0.35× the same; drag
/// tints with theme accent at 22 %.
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
