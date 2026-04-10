//! Tree view widget — Altium-style project/component browser tree.
//!
//! Proper chevron indicators, colored icons, hover backgrounds,
//! selection highlighting, badges, and indentation.

use iced::widget::{button, container, mouse_area, row, scrollable, text, Column};
use iced::{Background, Border, Color, Element, Length};
use signex_types::theme::ThemeTokens;

use crate::theme_ext;

// ─── Data Model ───────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeIcon {
    Folder,
    FolderOpen,
    File,
    Schematic,
    Pcb,
    Library,
    Component,
    Sheet,
    Net,
    Pin,
}

impl TreeIcon {
    /// Clean single-char icons that render on all platforms.
    fn render(self, _expanded: bool) -> (&'static str, IconColor) {
        match self {
            TreeIcon::Folder | TreeIcon::FolderOpen => ("\u{25A0}", IconColor::Yellow), // ■
            TreeIcon::File => ("\u{25AB}", IconColor::Muted),      // ▫
            TreeIcon::Schematic => ("\u{25A3}", IconColor::Blue),   // ▣
            TreeIcon::Pcb => ("\u{25A6}", IconColor::Green),        // ▦
            TreeIcon::Library => ("\u{25C6}", IconColor::Purple),   // ◆
            TreeIcon::Component => ("\u{25C8}", IconColor::Cyan),   // ◈
            TreeIcon::Sheet => ("\u{25A1}", IconColor::Blue),       // □
            TreeIcon::Net => ("\u{223F}", IconColor::Green),        // ∿
            TreeIcon::Pin => ("\u{2022}", IconColor::Muted),        // •
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum IconColor {
    Yellow,
    Blue,
    Green,
    Purple,
    Cyan,
    Muted,
}

impl IconColor {
    fn to_iced(self, tokens: &ThemeTokens) -> Color {
        match self {
            IconColor::Yellow => theme_ext::warning_color(tokens),
            IconColor::Blue => theme_ext::accent(tokens),
            IconColor::Green => theme_ext::success_color(tokens),
            IconColor::Purple => Color::from_rgb(0.7, 0.5, 0.9),
            IconColor::Cyan => Color::from_rgb(0.4, 0.8, 0.9),
            IconColor::Muted => theme_ext::text_secondary(tokens),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TreeNode {
    pub label: String,
    pub icon: TreeIcon,
    pub expanded: bool,
    pub children: Vec<TreeNode>,
    pub badge: Option<String>,
}

impl TreeNode {
    pub fn leaf(label: impl Into<String>, icon: TreeIcon) -> Self {
        Self {
            label: label.into(),
            icon,
            expanded: false,
            children: Vec::new(),
            badge: None,
        }
    }

    pub fn branch(label: impl Into<String>, icon: TreeIcon, children: Vec<TreeNode>) -> Self {
        Self {
            label: label.into(),
            icon,
            expanded: true,
            children,
            badge: None,
        }
    }

    pub fn with_badge(mut self, badge: impl Into<String>) -> Self {
        self.badge = Some(badge.into());
        self
    }
}

// ─── Messages ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum TreeMsg {
    Toggle(Vec<usize>),
    Select(Vec<usize>),
}

// ─── View ─────────────────────────────────────────────────────

const INDENT_PX: u32 = 14;
const ROW_PADDING_V: u16 = 1;
const ROW_PADDING_H: u16 = 4;
const FONT_SIZE: f32 = 11.0;
const ICON_SIZE: f32 = 11.0;
const CHEVRON_SIZE: f32 = 9.0;
const BADGE_SIZE: f32 = 9.0;

pub fn tree_view<'a>(
    roots: &[TreeNode],
    selected: Option<&[usize]>,
    tokens: &ThemeTokens,
) -> Element<'a, TreeMsg> {
    let mut col = Column::new().spacing(0).width(Length::Fill);
    for (i, node) in roots.iter().enumerate() {
        col = render_node(col, node, 0, &[i], selected, tokens);
    }
    scrollable(col).width(Length::Fill).into()
}

fn render_node<'a>(
    mut col: Column<'a, TreeMsg>,
    node: &TreeNode,
    depth: usize,
    path: &[usize],
    selected: Option<&[usize]>,
    tokens: &ThemeTokens,
) -> Column<'a, TreeMsg> {
    let path_vec: Vec<usize> = path.to_vec();
    let is_selected = selected.is_some_and(|s| s == path);
    let has_children = !node.children.is_empty();

    // Colors
    let text_color = if is_selected {
        Color::WHITE
    } else {
        theme_ext::text_primary(tokens)
    };
    let hover_bg = theme_ext::hover_color(tokens);
    let sel_bg = theme_ext::selection_color(tokens);
    let (icon_char, icon_color_kind) = node.icon.render(node.expanded);
    let icon_color = if is_selected {
        Color::WHITE
    } else {
        icon_color_kind.to_iced(tokens)
    };

    // Chevron
    let chevron = if has_children {
        if node.expanded { "▾" } else { "▸" }
    } else {
        " "
    };
    let chevron_color = theme_ext::text_secondary(tokens);

    // Build the row content
    let indent = (depth as u32) * INDENT_PX;

    let mut row_content = row![]
        .spacing(2)
        .align_y(iced::Alignment::Center);

    // Indent spacer
    if indent > 0 {
        row_content = row_content.push(text("").width(indent));
    }

    // Chevron (fixed width for alignment)
    row_content = row_content.push(
        text(chevron).size(CHEVRON_SIZE).color(chevron_color).width(10),
    );

    // Icon (fixed width)
    row_content = row_content.push(
        container(text(icon_char.to_string()).size(ICON_SIZE).color(icon_color))
            .width(22),
    );

    // Label (single line — wrapping disabled)
    row_content = row_content.push(
        text(node.label.clone())
            .size(FONT_SIZE)
            .color(text_color)
            .wrapping(iced::widget::text::Wrapping::None),
    );

    // Badge (right-aligned, muted)
    if let Some(badge) = &node.badge {
        row_content = row_content.push(iced::widget::space::horizontal());
        row_content = row_content.push(
            text(badge.clone())
                .size(BADGE_SIZE)
                .color(theme_ext::text_secondary(tokens)),
        );
    }

    // Click message
    let msg = if has_children {
        TreeMsg::Toggle(path_vec.clone())
    } else {
        TreeMsg::Select(path_vec)
    };

    // Wrap in button for click handling
    let row_btn = button(row_content)
        .padding([ROW_PADDING_V, ROW_PADDING_H])
        .width(Length::Fill)
        .on_press(msg)
        .style(button::text);

    // Apply selection or hover background
    let styled: Element<'a, TreeMsg> = if is_selected {
        container(row_btn)
            .width(Length::Fill)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(Background::Color(sel_bg)),
                border: Border {
                    radius: 2.0.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .into()
    } else {
        // Use mouse_area for hover effect
        mouse_area(
            container(row_btn).width(Length::Fill),
        )
        .into()
    };

    col = col.push(styled);

    // Render children if expanded
    if node.expanded && has_children {
        for (i, child) in node.children.iter().enumerate() {
            let mut child_path = path.to_vec();
            child_path.push(i);
            col = render_node(col, child, depth + 1, &child_path, selected, tokens);
        }
    }

    col
}

// ─── Helpers ──────────────────────────────────────────────────

pub fn toggle(roots: &mut [TreeNode], path: &[usize]) {
    if let Some(node) = get_node_mut(roots, path) {
        node.expanded = !node.expanded;
    }
}

fn get_node_mut<'a>(roots: &'a mut [TreeNode], path: &[usize]) -> Option<&'a mut TreeNode> {
    if path.is_empty() {
        return None;
    }
    let mut current = roots.get_mut(path[0])?;
    for &idx in &path[1..] {
        current = current.children.get_mut(idx)?;
    }
    Some(current)
}
