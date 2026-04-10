//! Panel docking system — wraps PaneGrid regions with tabbed panels.
//!
//! Signex has 3 dock regions (left, right, bottom) plus a center canvas.
//! Each region can hold multiple panels as tabs. Panels can be collapsed
//! to a rail icon.

use iced::widget::{button, column, container, row, text, Column};
use iced::{Element, Length};

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
            DockMessage::Panel(_) => {
                // Panel messages handled by app.rs
            }
        }
    }

    pub fn view_region<'a>(&'a self, position: PanelPosition, ctx: &'a panels::PanelContext) -> Element<'a, DockMessage> {
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

        // Tab bar
        let mut tab_row = row![].spacing(0);
        for (i, panel) in region.panels.iter().enumerate() {
            let label = panel.label();
            let is_active = i == region.active;
            let btn = button(
                text(label).size(10).color(if is_active {
                    iced::Color::WHITE
                } else {
                    styles::TEXT_MUTED
                }),
            )
            .padding([3, 8])
            .on_press(DockMessage::SelectTab(position, i));
            let btn = if is_active {
                btn.style(button::primary)
            } else {
                btn.style(button::text)
            };
            tab_row = tab_row.push(btn);
        }

        // Collapse button
        let collapse_label = match position {
            PanelPosition::Left => "«",
            PanelPosition::Right => "»",
            PanelPosition::Bottom => "v",
        };
        tab_row = tab_row.push(
            button(text(collapse_label).size(11))
                .padding([3, 6])
                .style(button::text)
                .on_press(DockMessage::ToggleCollapse(position)),
        );

        // Panel content
        let content: Element<'_, DockMessage> = if let Some(panel) = region.panels.get(region.active) {
            panels::view_panel(*panel, ctx).map(DockMessage::Panel)
        } else {
            text("").into()
        };

        column![
            container(tab_row).width(Length::Fill).padding([2, 4]),
            container(content)
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(4),
        ]
        .into()
    }

    fn view_rail(&self, position: PanelPosition, region: &DockRegion) -> Element<'_, DockMessage> {
        let expand_label = match position {
            PanelPosition::Left => "»",
            PanelPosition::Right => "«",
            PanelPosition::Bottom => "^",
        };

        let mut rail: Column<DockMessage> = Column::new().spacing(2).width(28);

        rail = rail.push(
            button(text(expand_label).size(11))
                .padding([3, 6])
                .style(button::text)
                .on_press(DockMessage::ToggleCollapse(position)),
        );

        for (i, panel) in region.panels.iter().enumerate() {
            let first_char = panel
                .label()
                .chars()
                .next()
                .unwrap_or('?')
                .to_string();
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
