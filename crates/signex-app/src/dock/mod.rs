//! Panel docking system — wraps PaneGrid regions with tabbed panels.
//!
//! Signex has 3 dock regions (left, right, bottom) plus a center canvas.
//! Each region can hold multiple panels as tabs.

use iced::widget::{Column, Space, button, column, container, row, text};
use iced::{Background, Border, Color, Element, Length, Theme};

use crate::panels::{self, PanelKind, PanelMsg};
use crate::styles;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelPosition {
    Left,
    Right,
    Bottom,
}

#[derive(Debug, Clone)]
pub enum DockMessage {
    SelectTab(PanelPosition, usize),
    ToggleCollapse(PanelPosition),
    Panel(PanelMsg),
}

struct DockRegion {
    panels: Vec<PanelKind>,
    active: usize,
    collapsed: bool,
}

pub struct DockArea {
    left: DockRegion,
    right: DockRegion,
    bottom: DockRegion,
}

impl DockArea {
    pub fn new() -> Self {
        Self {
            left: DockRegion {
                panels: Vec::new(),
                active: 0,
                collapsed: false,
            },
            right: DockRegion {
                panels: Vec::new(),
                active: 0,
                collapsed: false,
            },
            bottom: DockRegion {
                panels: Vec::new(),
                active: 0,
                collapsed: false,
            },
        }
    }

    pub fn add_panel(&mut self, position: PanelPosition, kind: PanelKind) {
        let region = match position {
            PanelPosition::Left => &mut self.left,
            PanelPosition::Right => &mut self.right,
            PanelPosition::Bottom => &mut self.bottom,
        };
        region.panels.push(kind);
    }

    pub fn update(&mut self, msg: DockMessage) {
        match msg {
            DockMessage::SelectTab(pos, idx) => {
                let region = match pos {
                    PanelPosition::Left => &mut self.left,
                    PanelPosition::Right => &mut self.right,
                    PanelPosition::Bottom => &mut self.bottom,
                };
                if idx < region.panels.len() {
                    region.active = idx;
                    region.collapsed = false;
                }
            }
            DockMessage::ToggleCollapse(pos) => {
                let region = match pos {
                    PanelPosition::Left => &mut self.left,
                    PanelPosition::Right => &mut self.right,
                    PanelPosition::Bottom => &mut self.bottom,
                };
                region.collapsed = !region.collapsed;
            }
            // Panel messages are handled by app.rs before reaching here.
            // DockArea only handles tab/collapse; panel logic stays in Signex::update().
            DockMessage::Panel(_) => {}
        }
    }

    pub fn view_region<'a>(
        &'a self,
        position: PanelPosition,
        ctx: &'a panels::PanelContext,
    ) -> Element<'a, DockMessage> {
        let region = match position {
            PanelPosition::Left => &self.left,
            PanelPosition::Right => &self.right,
            PanelPosition::Bottom => &self.bottom,
        };

        if region.panels.is_empty() {
            return container(text("")).width(0).into();
        }

        if region.collapsed {
            return self.view_rail(position, region);
        }

        // ── Altium-style flat tabs with accent underline ──
        let mut tab_row = row![].spacing(0.0).align_y(iced::Alignment::End);

        for (i, panel) in region.panels.iter().enumerate() {
            let label = panel.label();
            let is_active = i == region.active;

            let text_c = if is_active {
                styles::TEXT_PRIMARY
            } else {
                styles::TEXT_MUTED
            };
            let line_c = if is_active {
                styles::ACCENT
            } else {
                Color::TRANSPARENT
            };

            // Button content: text + accent underline, with tab border
            let label_el = container(text(label).size(11).color(text_c)).padding([5, 10]);
            let underline = container(Space::new())
                .height(2.0)
                .width(Length::Fill)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(line_c)),
                    ..container::Style::default()
                });

            let border_c = styles::BORDER_SUBTLE;
            let btn = button(column![label_el, underline].spacing(0))
                .padding(0)
                .on_press(DockMessage::SelectTab(position, i))
                .style(move |_: &Theme, status: button::Status| {
                    let bg = match (is_active, status) {
                        // Active tab keeps highlighted background
                        (true, _) => Some(Background::Color(styles::TAB_ACTIVE_BG)),
                        (false, button::Status::Hovered) => {
                            Some(Background::Color(styles::TAB_ACTIVE_BG))
                        }
                        _ => None,
                    };
                    button::Style {
                        background: bg,
                        border: Border {
                            width: 1.0,
                            radius: 0.0.into(),
                            color: border_c,
                        },
                        ..button::Style::default()
                    }
                });

            tab_row = tab_row.push(btn);
        }

        // Collapse button (minimal)
        let collapse_label = match position {
            PanelPosition::Left => "\u{00AB}",   // «
            PanelPosition::Right => "\u{00BB}",  // »
            PanelPosition::Bottom => "\u{2304}", // ⌄
        };
        tab_row = tab_row.push(
            button(text(collapse_label).size(10).color(styles::TEXT_MUTED))
                .padding([5, 6])
                .style(button::text)
                .on_press(DockMessage::ToggleCollapse(position)),
        );

        // Panel content
        let content: Element<'_, DockMessage> =
            if let Some(panel) = region.panels.get(region.active) {
                panels::view_panel(*panel, ctx).map(DockMessage::Panel)
            } else {
                text("").into()
            };

        column![
            // Tab bar with bottom separator
            container(tab_row)
                .width(Length::Fill)
                .padding([0, 4])
                .style(|_: &Theme| container::Style {
                    background: Some(Background::Color(styles::TOOLBAR_BG)),
                    border: Border {
                        width: 1.0,
                        radius: 0.0.into(),
                        color: styles::BORDER_SUBTLE,
                    },
                    ..container::Style::default()
                }),
            // Panel content
            container(content)
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(4),
        ]
        .into()
    }

    fn view_rail(&self, position: PanelPosition, region: &DockRegion) -> Element<'_, DockMessage> {
        let expand_label = match position {
            PanelPosition::Left => "\u{00BB}",  // »
            PanelPosition::Right => "\u{00AB}", // «
            PanelPosition::Bottom => "^",
        };

        let mut rail: Column<DockMessage> = Column::new().spacing(2.0).width(28);

        rail = rail.push(
            button(text(expand_label).size(11))
                .padding([3, 6])
                .style(button::text)
                .on_press(DockMessage::ToggleCollapse(position)),
        );

        for (i, panel) in region.panels.iter().enumerate() {
            let first_char = panel.label().chars().next().unwrap_or('?').to_string();
            rail = rail.push(
                button(text(first_char).size(11))
                    .padding([3, 6])
                    .style(button::text)
                    .on_press(DockMessage::SelectTab(position, i)),
            );
        }

        container(rail).width(28).height(Length::Fill).into()
    }
}
