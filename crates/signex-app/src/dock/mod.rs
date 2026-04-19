//! Panel docking system — wraps PaneGrid regions with tabbed panels.
//!
//! Signex has 3 dock regions (left, right, bottom) plus a center canvas.
//! Each region can hold multiple panels as tabs.

use std::sync::LazyLock;

use iced::widget::{Column, Space, button, canvas, column, container, mouse_area, row, svg, text};
use iced::{Color, Element, Length, Rectangle, Renderer, Theme};

use crate::panels::{self, PanelKind, PanelMsg};
use crate::styles;

// ─── SVG handles (created once via LazyLock, cloned cheaply) ──

static H_CLOSE: LazyLock<svg::Handle> =
    LazyLock::new(|| svg::Handle::from_memory(include_bytes!("../../assets/icons/close.svg")));
static H_COLLAPSE_LEFT: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!("../../assets/icons/collapse_left.svg"))
});
static H_COLLAPSE_RIGHT: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!("../../assets/icons/collapse_right.svg"))
});
static H_COLLAPSE_DOWN: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!("../../assets/icons/collapse_down.svg"))
});
static H_EXPAND_LEFT: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!("../../assets/icons/expand_left.svg"))
});
static H_EXPAND_RIGHT: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!("../../assets/icons/expand_right.svg"))
});
fn svg_icon(handle: &LazyLock<svg::Handle>) -> iced::widget::Svg<'static> {
    svg((*handle).clone()).width(10).height(10)
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
    /// Undock a panel to floating (drag tab out).
    UndockPanel(PanelPosition, usize),
    /// Mouse down on a tab — arms drag-to-undock detection.
    TabDragStart(PanelPosition, usize),
    /// Mouse released on a tab — if no undock happened, treat as click → select.
    TabClick(PanelPosition, usize),
    /// Reorder tabs within a dock region. `from` is the dragged tab's
    /// original index, `to` is the index of the tab it was released
    /// on. Currently produced by the internal TabClick handler — not
    /// emitted directly by the UI yet (left available for a future
    /// pointer-tracking drop indicator).
    #[allow(dead_code)]
    ReorderTab {
        pos: PanelPosition,
        from: usize,
        to: usize,
    },
    /// Scroll tabs left/right when they overflow the panel width.
    TabScroll(PanelPosition, i32),
    /// Move a floating panel by delta.
    #[allow(dead_code)]
    MoveFloating(usize, f32, f32),
    /// Start dragging a floating panel.
    StartDragFloating(usize),
    /// Mouse released after dragging a floating panel — try to dock at mouse pos.
    FloatingDragEnd(usize),
    /// Re-dock a floating panel (close floating → add to right dock).
    DockFloating(usize),
    /// Re-dock a floating panel to a specific region.
    DockFloatingTo(usize, PanelPosition),
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
    /// First visible tab index (for overflow scrolling).
    tab_offset: usize,
}

pub struct DockArea {
    left: DockRegion,
    right: DockRegion,
    bottom: DockRegion,
    pub floating: Vec<FloatingPanel>,
    /// Active tab drag: (region, tab index). Set on mouse-down, cleared on release or undock.
    pub tab_drag: Option<(PanelPosition, usize)>,
}

impl DockArea {
    pub fn new() -> Self {
        Self {
            left: DockRegion {
                panels: Vec::new(),
                active: 0,
                collapsed: false,
                tab_offset: 0,
            },
            right: DockRegion {
                panels: Vec::new(),
                active: 0,
                collapsed: false,
                tab_offset: 0,
            },
            bottom: DockRegion {
                panels: Vec::new(),
                active: 0,
                collapsed: false,
                tab_offset: 0,
            },
            floating: Vec::new(),
            tab_drag: None,
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
            DockMessage::TabScroll(pos, delta) => {
                let region = match pos {
                    PanelPosition::Left => &mut self.left,
                    PanelPosition::Right => &mut self.right,
                    PanelPosition::Bottom => &mut self.bottom,
                };
                let new_off = region.tab_offset as i32 + delta;
                let max_off = region.panels.len().saturating_sub(1) as i32;
                region.tab_offset = new_off.clamp(0, max_off) as usize;
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
            DockMessage::TabDragStart(pos, idx) => {
                self.tab_drag = Some((pos, idx));
            }
            DockMessage::ReorderTab { pos, from, to } => {
                let region = match pos {
                    PanelPosition::Left => &mut self.left,
                    PanelPosition::Right => &mut self.right,
                    PanelPosition::Bottom => &mut self.bottom,
                };
                if from < region.panels.len() && to < region.panels.len() {
                    let panel = region.panels.remove(from);
                    region.panels.insert(to, panel);
                    region.active = to;
                }
            }
            DockMessage::TabClick(pos, idx) => {
                // Mouse-up on tab: if UndockPanel did not fire, treat
                // as click → select. If the drag started on a
                // different tab in the same region, reorder the
                // panels vector instead so the user can drag tabs to
                // shuffle them within the strip.
                if let Some((drag_pos, from)) = self.tab_drag.take() {
                    if drag_pos == pos && from != idx {
                        let region = match pos {
                            PanelPosition::Left => &mut self.left,
                            PanelPosition::Right => &mut self.right,
                            PanelPosition::Bottom => &mut self.bottom,
                        };
                        if from < region.panels.len() && idx < region.panels.len() {
                            let panel = region.panels.remove(from);
                            region.panels.insert(idx, panel);
                            region.active = idx;
                            region.collapsed = false;
                        }
                    } else {
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
                }
            }
            DockMessage::UndockPanel(pos, idx) => {
                self.tab_drag = None;
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
                    // Create floating panel at cursor position
                    self.floating.push(FloatingPanel {
                        kind,
                        x: 300.0,
                        y: 150.0,
                        width: 280.0,
                        height: 400.0,
                        dragging: true, // start dragging immediately
                    });
                }
            }
            DockMessage::StartDragFloating(idx) => {
                if let Some(fp) = self.floating.get_mut(idx) {
                    fp.dragging = true;
                }
            }
            DockMessage::FloatingDragEnd(idx) => {
                // Stop the drag; dock-zone detection handled by app before this.
                if let Some(fp) = self.floating.get_mut(idx) {
                    fp.dragging = false;
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
                    if !self.right.panels.contains(&fp.kind) {
                        self.right.panels.push(fp.kind);
                        self.right.active = self.right.panels.len() - 1;
                        self.right.collapsed = false;
                    }
                }
            }
            DockMessage::DockFloatingTo(idx, target) => {
                if idx < self.floating.len() {
                    let fp = self.floating.remove(idx);
                    let region = match target {
                        PanelPosition::Left => &mut self.left,
                        PanelPosition::Right => &mut self.right,
                        PanelPosition::Bottom => &mut self.bottom,
                    };
                    if !region.panels.contains(&fp.kind) {
                        region.panels.push(fp.kind);
                    }
                    region.active = region
                        .panels
                        .iter()
                        .position(|k| *k == fp.kind)
                        .unwrap_or(0);
                    region.collapsed = false;
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

    /// Check if a dock region currently contains any panels.
    pub fn has_panels(&self, position: PanelPosition) -> bool {
        match position {
            PanelPosition::Left => !self.left.panels.is_empty(),
            PanelPosition::Right => !self.right.panels.is_empty(),
            PanelPosition::Bottom => !self.bottom.panels.is_empty(),
        }
    }

    /// Panels currently docked in `position`, in display order. Used
    /// by the Panels menu to mark open panels with a ✓.
    pub fn panel_kinds(&self, position: PanelPosition) -> &[panels::PanelKind] {
        match position {
            PanelPosition::Left => &self.left.panels,
            PanelPosition::Right => &self.right.panels,
            PanelPosition::Bottom => &self.bottom.panels,
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
            return self.view_rail(position, region, ctx);
        }

        // ── Altium-style flat tabs with accent underline + overflow scroll ──
        let total_tabs = region.panels.len();
        let offset = region.tab_offset.min(total_tabs.saturating_sub(1));
        let has_overflow = total_tabs > 3;
        let can_scroll_left = offset > 0;
        let can_scroll_right = offset + 3 < total_tabs;

        let mut tab_row = row![].spacing(0.0).align_y(iced::Alignment::End);

        // Left scroll arrow (only when tabs are scrolled)
        if has_overflow {
            let arrow = button(text("<").size(10))
                .padding([4, 4])
                .style(button::text);
            tab_row = tab_row.push(if can_scroll_left {
                arrow.on_press(DockMessage::TabScroll(position, -1))
            } else {
                arrow
            });
        }

        for (i, panel) in region.panels.iter().enumerate().skip(offset) {
            let label = panel.label();
            let is_active = i == region.active;

            let text_c = if is_active {
                styles::ti(ctx.tokens.text)
            } else {
                styles::ti(ctx.tokens.text_secondary)
            };
            let line_c = if is_active {
                styles::ti(ctx.tokens.accent)
            } else {
                iced::Color::TRANSPARENT
            };

            // Visual feedback while dragging: give the source tab a
            // brighter border + tinted background so the user sees
            // which tab they grabbed.
            let is_dragging_this =
                matches!(self.tab_drag, Some((p, src)) if p == position && src == i);
            // No manual width — Iced's layout engine measures the text.
            // The accent underline is done via bottom-padding on an outer
            // container whose background is the accent color, avoiding
            // Length::Fill which would expand the tab to the panel width.
            let label_el = container(text(label).size(11).color(text_c))
                .padding([5, 10])
                .style(styles::dock_tab_container_dragging(
                    &ctx.tokens,
                    is_active,
                    is_dragging_this,
                ));
            let tab = mouse_area(
                container(label_el)
                    .padding(iced::Padding {
                        top: 0.0,
                        right: 0.0,
                        bottom: 2.0,
                        left: 0.0,
                    })
                    .style(styles::tab_underline(line_c)),
            )
            .on_press(DockMessage::TabDragStart(position, i))
            .on_release(DockMessage::TabClick(position, i));

            tab_row = tab_row.push(tab);
        }

        // Right scroll arrow (only when more tabs are hidden)
        if has_overflow {
            let arrow = button(text(">").size(10))
                .padding([4, 4])
                .style(button::text);
            tab_row = tab_row.push(if can_scroll_right {
                arrow.on_press(DockMessage::TabScroll(position, 1))
            } else {
                arrow
            });
        }

        // Collapse button (SVG icon)
        let collapse_icon = match position {
            PanelPosition::Left => svg_icon(&H_COLLAPSE_LEFT),
            PanelPosition::Right => svg_icon(&H_COLLAPSE_RIGHT),
            PanelPosition::Bottom => svg_icon(&H_COLLAPSE_DOWN),
        };
        let close_btn = button(svg_icon(&H_CLOSE))
            .padding([5, 4])
            .style(button::text)
            .on_press(DockMessage::ClosePanel(position, region.active));
        let collapse_btn = button(collapse_icon)
            .padding([5, 4])
            .style(button::text)
            .on_press(DockMessage::ToggleCollapse(position));

        // Title of the currently active panel — drawn in the top bar
        // so the user can see the panel name without scanning the tab
        // row.
        let active_title = region
            .panels
            .get(region.active)
            .map(|p| p.label().to_string())
            .unwrap_or_default();
        let title_bar = row![
            container(
                text(active_title)
                    .size(11)
                    .color(styles::ti(ctx.tokens.text)),
            )
            .padding([5, 10]),
            Space::new().width(Length::Fill),
            collapse_btn,
            close_btn,
        ]
        .spacing(0)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill);

        // Bottom tab strip — just the tab buttons + overflow arrows.
        // Collapse / close moved up to the title bar so they're always
        // visible alongside the active panel name.
        let tab_strip = row![tab_row, Space::new().width(Length::Fill),]
            .spacing(0)
            .align_y(iced::Alignment::End)
            .width(Length::Fill);

        // Panel content
        let content: Element<'_, DockMessage> =
            if let Some(panel) = region.panels.get(region.active) {
                panels::view_panel(*panel, ctx).map(DockMessage::Panel)
            } else {
                text("").into()
            };

        // Altium parity: title + collapse/close on top, tabs on the
        // bottom. Content grows between them.
        column![
            container(title_bar)
                .width(Length::Fill)
                .padding([0, 4])
                .style(styles::tab_bar_strip(&ctx.tokens)),
            container(content)
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(0),
            container(tab_strip)
                .width(Length::Fill)
                .padding([0, 4])
                .style(styles::tab_bar_strip(&ctx.tokens)),
        ]
        .into()
    }

    fn view_rail<'a>(
        &'a self,
        position: PanelPosition,
        region: &'a DockRegion,
        ctx: &'a panels::PanelContext,
    ) -> Element<'a, DockMessage> {
        // Altium-style collapsed panel: vertical tabs with full panel names
        // Each tab is a button with the panel name, stacked vertically
        let is_vertical = matches!(position, PanelPosition::Left | PanelPosition::Right);

        if is_vertical {
            let mut rail: Column<DockMessage> = Column::new().spacing(2.0);

            // Expand button with SVG arrow
            let expand_icon = match position {
                PanelPosition::Left => svg_icon(&H_EXPAND_LEFT),
                PanelPosition::Right => svg_icon(&H_EXPAND_RIGHT),
                _ => svg_icon(&H_EXPAND_LEFT),
            };
            rail = rail.push(
                button(expand_icon)
                    .padding(4)
                    .width(RAIL_CANVAS_W)
                    .style(button::text)
                    .on_press(DockMessage::ToggleCollapse(position)),
            );

            for (i, panel) in region.panels.iter().enumerate() {
                let label = panel.label().to_string();
                let is_active = i == region.active;
                let text_c = if is_active {
                    styles::ti(ctx.tokens.text)
                } else {
                    styles::ti(ctx.tokens.text_secondary)
                };

                // Canvas height = estimated text pixel width + 1× font size padding
                let tab_h = estimate_text_width(&label, RAIL_FONT_SIZE) + RAIL_FONT_SIZE;

                rail = rail.push(
                    button(
                        canvas(RotatedLabel {
                            label,
                            color: text_c,
                        })
                        .width(RAIL_CANVAS_W)
                        .height(tab_h),
                    )
                    .padding(0)
                    .on_press(DockMessage::SelectTab(position, i))
                    .style(styles::rail_tab(&ctx.tokens, is_active)),
                );
            }

            container(rail)
                .height(Length::Fill)
                .padding([3, 2])
                .style(styles::collapsed_rail(&ctx.tokens))
                .into()
        } else {
            // Bottom panel collapsed: horizontal thin strip with tab labels
            let mut rail = row![].spacing(2.0);

            for (i, panel) in region.panels.iter().enumerate() {
                let label = panel.label();
                let is_active = i == region.active;
                let text_c = if is_active {
                    styles::ti(ctx.tokens.text)
                } else {
                    styles::ti(ctx.tokens.text_secondary)
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
                .style(styles::collapsed_rail(&ctx.tokens))
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

        // Title bar with drag handle + close button
        let title_bar_content = container(
            row![
                container(text(label).size(11).color(styles::ti(ctx.tokens.text)))
                    .padding([4, 8])
                    .width(Length::Fill),
                button(svg_icon(&H_CLOSE))
                    .padding([4, 4])
                    .style(button::text)
                    .on_press(DockMessage::DockFloating(idx)),
            ]
            .spacing(2)
            .align_y(iced::Alignment::Center),
        )
        .width(fp.width)
        .style(styles::floating_title_bar(&ctx.tokens));

        let title_bar = mouse_area(title_bar_content)
            .on_press(DockMessage::StartDragFloating(idx))
            .on_release(DockMessage::FloatingDragEnd(idx));

        let content = panels::view_panel(kind, ctx).map(DockMessage::Panel);

        let panel_widget = container(
            column![
                title_bar,
                container(iced::widget::scrollable(content).width(Length::Fill))
                    .width(fp.width)
                    .height(fp.height)
                    .style(styles::floating_panel_body(&ctx.tokens)),
            ]
            .spacing(0),
        )
        .style(styles::floating_panel_shadow(&ctx.tokens));

        Some(panel_widget.into())
    }
}

/// Font size for collapsed rail labels — every other dimension derives from this.
const RAIL_FONT_SIZE: f32 = 12.0;
/// Canvas width = text line-height after 90° rotation ≈ 1.5 × font size.
const RAIL_CANVAS_W: f32 = RAIL_FONT_SIZE * 1.5;

/// Estimate the rendered pixel width of a string at `font_size`.
///
/// Ratios are calibrated for Segoe UI (Windows default / Iced default).
/// Grouped by measured glyph-width bands so every panel label gets
/// near-identical visual padding after center-alignment.
fn estimate_text_width(s: &str, font_size: f32) -> f32 {
    s.chars()
        .map(|c| {
            font_size
                * match c {
                    // ── narrowest glyphs ──
                    'i' | 'l' | '|' | '!' | '.' | ',' | ':' | ';' | '\'' => 0.28,
                    'I' | 'j' => 0.30,
                    'f' => 0.33,
                    'r' | 't' => 0.36,
                    ' ' => 0.28,
                    // ── medium-narrow ──
                    'c' | 's' | 'z' => 0.50,
                    'a' | 'e' | 'g' | 'k' | 'v' | 'x' | 'y' => 0.54,
                    // ── medium-wide ──
                    'b' | 'd' | 'h' | 'n' | 'o' | 'p' | 'q' | 'u' => 0.58,
                    // ── widest lowercase ──
                    'm' => 0.86,
                    'w' => 0.80,
                    // ── capitals ──
                    'M' | 'W' => 0.86,
                    'A'..='Z' => 0.62,
                    // ── digits & fallback ──
                    '0'..='9' => 0.58,
                    _ => 0.55,
                }
        })
        .sum()
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
        let cx = bounds.width / 2.0;
        let cy = bounds.height / 2.0;
        frame.translate(iced::Vector::new(cx, cy));
        frame.rotate(std::f32::consts::FRAC_PI_2); // 90° CW
        // After rotation, origin is at canvas center. Use center alignment
        // so the text is perfectly centered regardless of label length.
        frame.fill_text(canvas::Text {
            content: self.label.clone(),
            position: iced::Point::ORIGIN,
            color: self.color,
            size: iced::Pixels(RAIL_FONT_SIZE),
            align_x: iced::alignment::Horizontal::Center.into(),
            align_y: iced::alignment::Vertical::Center,
            ..canvas::Text::default()
        });
        vec![frame.into_geometry()]
    }
}
