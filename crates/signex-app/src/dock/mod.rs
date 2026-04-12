//! Panel docking system — wraps PaneGrid regions with tabbed panels.
//!
//! Signex has 3 dock regions (left, right, bottom) plus a center canvas.
//! Each region can hold multiple panels as tabs.

use iced::widget::{Column, Space, button, column, container, row, svg, text};
use iced::{Background, Border, Color, Element, Length, Theme};
use std::sync::OnceLock;

use crate::panels::{self, PanelKind, PanelMsg};
use crate::styles;

// SVG icons for dock buttons
const ICON_CLOSE: &[u8] = include_bytes!("../../assets/icons/close.svg");
const ICON_COLLAPSE_LEFT: &[u8] = include_bytes!("../../assets/icons/collapse_left.svg");
const ICON_COLLAPSE_RIGHT: &[u8] = include_bytes!("../../assets/icons/collapse_right.svg");
const ICON_COLLAPSE_DOWN: &[u8] = include_bytes!("../../assets/icons/collapse_down.svg");
const ICON_EXPAND_LEFT: &[u8] = include_bytes!("../../assets/icons/expand_left.svg");
const ICON_EXPAND_RIGHT: &[u8] = include_bytes!("../../assets/icons/expand_right.svg");

fn svg_icon(bytes: &'static [u8]) -> iced::widget::Svg {
    svg(svg::Handle::from_memory(bytes)).width(10).height(10)
}

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
    ClosePanel(PanelPosition, usize),
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
            DockMessage::ClosePanel(pos, idx) => {
                let region = match pos {
                    PanelPosition::Left => &mut self.left,
                    PanelPosition::Right => &mut self.right,
                    PanelPosition::Bottom => &mut self.bottom,
                };
                if idx < region.panels.len() {
                    region.panels.remove(idx);
                    if region.active >= region.panels.len() && region.active > 0 {
                        region.active -= 1;
                    }
                }
            }
            // Panel messages are handled by app.rs before reaching here.
            DockMessage::Panel(_) => {}
        }
    }

    /// Check if a dock region is collapsed.
    pub fn is_collapsed(&self, position: PanelPosition) -> bool {
        match position {
            PanelPosition::Left => self.left.collapsed,
            PanelPosition::Right => self.right.collapsed,
            PanelPosition::Bottom => self.bottom.collapsed,
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

        // Spacer to push buttons right
        tab_row = tab_row.push(Space::new().width(Length::Fill));

        // Collapse button (SVG icon)
        let collapse_icon = match position {
            PanelPosition::Left => svg_icon(ICON_COLLAPSE_LEFT),
            PanelPosition::Right => svg_icon(ICON_COLLAPSE_RIGHT),
            PanelPosition::Bottom => svg_icon(ICON_COLLAPSE_DOWN),
        };
        tab_row = tab_row.push(
            button(collapse_icon)
                .padding([5, 4])
                .style(button::text)
                .on_press(DockMessage::ToggleCollapse(position)),
        );

        // Close button (X) for active panel
        tab_row = tab_row.push(
            button(svg_icon(ICON_CLOSE))
                .padding([5, 4])
                .style(button::text)
                .on_press(DockMessage::ClosePanel(position, region.active)),
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
        // Altium-style collapsed panel: vertical tabs with full panel names
        // Each tab is a button with the panel name, stacked vertically
        let is_vertical = matches!(position, PanelPosition::Left | PanelPosition::Right);

        if is_vertical {
            let mut rail: Column<DockMessage> = Column::new().spacing(2.0);

            // Expand button with SVG arrow
            let expand_icon = match position {
                PanelPosition::Left => svg_icon(ICON_EXPAND_LEFT),
                PanelPosition::Right => svg_icon(ICON_EXPAND_RIGHT),
                _ => svg_icon(ICON_EXPAND_LEFT),
            };
            rail = rail.push(
                button(expand_icon)
                    .padding([4, 6])
                    .style(button::text)
                    .on_press(DockMessage::ToggleCollapse(position)),
            );

            for (i, panel) in region.panels.iter().enumerate() {
                let label = panel.label();
                let is_active = i == region.active;
                let text_c = if is_active {
                    styles::TEXT_PRIMARY
                } else {
                    styles::TEXT_MUTED
                };
                let border_c = styles::BORDER_COLOR;

                // Vertical text: one char per line to simulate rotation
                let vertical: String = label.chars().map(|c| format!("{c}\n")).collect();
                let vertical = vertical.trim_end().to_string();

                rail = rail.push(
                    button(
                        container(
                            text(vertical)
                                .size(10)
                                .color(text_c)
                                .align_x(iced::alignment::Horizontal::Center)
                                .line_height(iced::widget::text::LineHeight::Absolute(
                                    iced::Pixels(11.0),
                                )),
                        )
                        .width(22)
                        .padding([6, 2])
                        .align_x(iced::alignment::Horizontal::Center),
                    )
                    .padding(0)
                    .on_press(DockMessage::SelectTab(position, i))
                    .style(move |_: &Theme, status: button::Status| {
                        let bg = match (is_active, status) {
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
                                radius: 3.0.into(),
                                color: border_c,
                            },
                            ..button::Style::default()
                        }
                    }),
                );
            }

            container(rail)
                .width(28)
                .height(Length::Fill)
                .padding([4, 2])
                .style(|_: &Theme| container::Style {
                    background: Some(Background::Color(styles::PANEL_BG)),
                    border: Border {
                        width: 1.0,
                        radius: 0.0.into(),
                        color: styles::BORDER_SUBTLE,
                    },
                    ..container::Style::default()
                })
                .into()
        } else {
            // Bottom panel collapsed: horizontal thin strip with tab labels
            let mut rail = row![].spacing(2.0);

            for (i, panel) in region.panels.iter().enumerate() {
                let label = panel.label();
                let is_active = i == region.active;
                let text_c = if is_active {
                    styles::TEXT_PRIMARY
                } else {
                    styles::TEXT_MUTED
                };

                rail = rail.push(
                    button(text(label).size(10).color(text_c))
                        .padding([2, 8])
                        .on_press(DockMessage::SelectTab(position, i))
                        .style(button::text),
                );
            }

            container(rail)
                .width(Length::Fill)
                .height(28)
                .padding([2, 4])
                .style(|_: &Theme| container::Style {
                    background: Some(Background::Color(styles::PANEL_BG)),
                    border: Border {
                        width: 1.0,
                        radius: 0.0.into(),
                        color: styles::BORDER_SUBTLE,
                    },
                    ..container::Style::default()
                })
                .into()
        }
    }
}
