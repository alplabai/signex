//! Panel docking system — wraps PaneGrid regions with tabbed panels.
//!
//! Signex has 3 dock regions (left, right, bottom) plus a center canvas.
//! Each region can hold multiple panels as tabs.

use iced::widget::{Column, Space, button, canvas, column, container, row, svg, text};
use iced::{Background, Border, Color, Element, Length, Rectangle, Renderer, Theme};

use crate::panels::{self, PanelKind, PanelMsg};
use crate::styles;

// SVG icons for dock buttons
const ICON_CLOSE: &[u8] = include_bytes!("../../assets/icons/close.svg");
const ICON_COLLAPSE_LEFT: &[u8] = include_bytes!("../../assets/icons/collapse_left.svg");
const ICON_COLLAPSE_RIGHT: &[u8] = include_bytes!("../../assets/icons/collapse_right.svg");
const ICON_COLLAPSE_DOWN: &[u8] = include_bytes!("../../assets/icons/collapse_down.svg");
const ICON_EXPAND_LEFT: &[u8] = include_bytes!("../../assets/icons/expand_left.svg");
const ICON_EXPAND_RIGHT: &[u8] = include_bytes!("../../assets/icons/expand_right.svg");
const ICON_UNDOCK: &[u8] = include_bytes!("../../assets/icons/undock.svg");

fn svg_icon(bytes: &'static [u8]) -> iced::widget::Svg<'static> {
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
    /// Undock a panel to floating (double-click on tab).
    UndockPanel(PanelPosition, usize),
    /// Move a floating panel by delta.
    MoveFloating(usize, f32, f32),
    /// Start dragging a floating panel.
    StartDragFloating(usize),
    /// Re-dock a floating panel (close floating → add to right dock).
    DockFloating(usize),
    Panel(PanelMsg),
}

/// A panel floating as an overlay window.
#[derive(Debug, Clone)]
pub struct FloatingPanel {
    pub kind: PanelKind,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub dragging: bool,
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
    pub floating: Vec<FloatingPanel>,
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
            floating: Vec::new(),
        }
    }

    pub fn add_panel(&mut self, position: PanelPosition, kind: PanelKind) {
        let region = match position {
            PanelPosition::Left => &mut self.left,
            PanelPosition::Right => &mut self.right,
            PanelPosition::Bottom => &mut self.bottom,
        };
        if region.panels.contains(&kind) {
            return;
        }
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
            DockMessage::UndockPanel(pos, idx) => {
                let region = match pos {
                    PanelPosition::Left => &mut self.left,
                    PanelPosition::Right => &mut self.right,
                    PanelPosition::Bottom => &mut self.bottom,
                };
                if idx < region.panels.len() {
                    let kind = region.panels.remove(idx);
                    if region.active >= region.panels.len() && region.active > 0 {
                        region.active -= 1;
                    }
                    // Create floating panel at center of screen
                    self.floating.push(FloatingPanel {
                        kind,
                        x: 300.0,
                        y: 150.0,
                        width: 280.0,
                        height: 400.0,
                        dragging: false,
                    });
                }
            }
            DockMessage::StartDragFloating(idx) => {
                if let Some(fp) = self.floating.get_mut(idx) {
                    fp.dragging = true;
                }
            }
            DockMessage::MoveFloating(idx, dx, dy) => {
                if let Some(fp) = self.floating.get_mut(idx) {
                    fp.x += dx;
                    fp.y += dy;
                }
            }
            DockMessage::DockFloating(idx) => {
                if idx < self.floating.len() {
                    let fp = self.floating.remove(idx);
                    // Re-dock to right panel
                    if !self.right.panels.contains(&fp.kind) {
                        self.right.panels.push(fp.kind);
                        self.right.active = self.right.panels.len() - 1;
                        self.right.collapsed = false;
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

        // Undock button (float panel)
        tab_row = tab_row.push(
            button(svg_icon(ICON_UNDOCK))
                .padding([5, 4])
                .style(button::text)
                .on_press(DockMessage::UndockPanel(position, region.active)),
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
                let label = panel.label().to_string();
                let is_active = i == region.active;
                let border_c = styles::BORDER_COLOR;
                let text_c = if is_active {
                    styles::TEXT_PRIMARY
                } else {
                    styles::TEXT_MUTED
                };

                // Rotated text via canvas (Altium-style sideways tabs)
                let tab_h = (label.len() as f32 * 7.5 + 16.0).max(60.0);

                rail = rail.push(
                    button(
                        canvas(RotatedLabel {
                            label,
                            color: text_c,
                        })
                        .width(24)
                        .height(tab_h),
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

    /// Render a single floating panel as an overlay element.
    pub fn view_floating_panel<'a>(
        &'a self,
        idx: usize,
        ctx: &'a panels::PanelContext,
    ) -> Option<Element<'a, DockMessage>> {
        let fp = self.floating.get(idx)?;
        let kind = fp.kind;
        let label = kind.label();

        // Title bar with drag handle + dock/close buttons
        let title_bar = container(
            row![
                // Drag handle area — use mouse_area for drag
                iced::widget::mouse_area(
                    container(
                        text(label).size(11).color(styles::TEXT_PRIMARY),
                    )
                    .padding([4, 8])
                    .width(Length::Fill),
                )
                .on_press(DockMessage::StartDragFloating(idx)),
                // Dock button (re-dock to panel area)
                button(svg_icon(ICON_UNDOCK))
                    .padding([4, 4])
                    .style(button::text)
                    .on_press(DockMessage::DockFloating(idx)),
                // Close (same as dock for now)
                button(svg_icon(ICON_CLOSE))
                    .padding([4, 4])
                    .style(button::text)
                    .on_press(DockMessage::DockFloating(idx)),
            ]
            .spacing(2)
            .align_y(iced::Alignment::Center),
        )
        .width(fp.width)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(Color::from_rgb(0.14, 0.14, 0.16))),
            border: Border {
                width: 1.0,
                radius: 6.0.into(),
                color: Color::from_rgb(0.24, 0.25, 0.33),
            },
            ..container::Style::default()
        });

        // Panel content
        let content = panels::view_panel(kind, ctx).map(DockMessage::Panel);

        let panel_widget = container(
            column![
                title_bar,
                container(
                    iced::widget::scrollable(content).width(Length::Fill),
                )
                .width(fp.width)
                .height(fp.height)
                .style(|_: &Theme| container::Style {
                    background: Some(Background::Color(styles::PANEL_BG)),
                    border: Border {
                        width: 1.0,
                        radius: 6.0.into(),
                        color: Color::from_rgb(0.24, 0.25, 0.33),
                    },
                    ..container::Style::default()
                }),
            ]
            .spacing(0),
        )
        .style(|_: &Theme| container::Style {
            shadow: iced::Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                offset: iced::Vector::new(0.0, 4.0),
                blur_radius: 12.0,
            },
            ..container::Style::default()
        });

        Some(panel_widget.into())
    }
}

/// Canvas program that draws rotated text (90° CW) for collapsed panel tabs.
struct RotatedLabel {
    label: String,
    color: Color,
}

impl canvas::Program<DockMessage> for RotatedLabel {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        // Rotate 90° clockwise: translate to center, rotate, draw text
        let cx = bounds.width / 2.0;
        let cy = bounds.height / 2.0;
        frame.translate(iced::Vector::new(cx, cy));
        frame.rotate(std::f32::consts::FRAC_PI_2); // 90° CW
        frame.fill_text(canvas::Text {
            content: self.label.clone(),
            position: iced::Point::new(-cy + 8.0, -5.0),
            color: self.color,
            size: iced::Pixels(11.0),
            ..canvas::Text::default()
        });
        vec![frame.into_geometry()]
    }
}
