//! Tree view widget — Altium-style project/component browser tree.
//!
//! COSMIC composition pattern: `TreeView` struct with builder methods.
//! Matches the React reference (ProjectPanel.tsx) spacing pixel-for-pixel:
//!   - 14px indent per depth + 10px base left padding
//!   - 6px gap between elements (gap-1.5)
//!   - 5px vertical row padding
//!   - 12px body font, 10px badge
//!   - text-secondary default → text-primary on selected
//!   - hover bg, selection bg, "(empty)" indicator

use std::sync::OnceLock;

use iced::widget::{Column, Row, Space, button, container, scrollable, svg, text};
use iced::{Background, Border, Color, Element, Length, Theme};
use signex_types::theme::ThemeTokens;

// ─── SVG Chevron Icons (cached handles) ──────────────────────

const SVG_CHEVRON_RIGHT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 10 10"><path d="M3 1l5 4-5 4z" fill="currentColor"/></svg>"#;
const SVG_CHEVRON_DOWN: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 10 10"><path d="M1 3l4 5 4-5z" fill="currentColor"/></svg>"#;

fn chevron_right_handle() -> svg::Handle {
    static HANDLE: OnceLock<svg::Handle> = OnceLock::new();
    HANDLE
        .get_or_init(|| svg::Handle::from_memory(SVG_CHEVRON_RIGHT))
        .clone()
}

fn chevron_down_handle() -> svg::Handle {
    static HANDLE: OnceLock<svg::Handle> = OnceLock::new();
    HANDLE
        .get_or_init(|| svg::Handle::from_memory(SVG_CHEVRON_DOWN))
        .clone()
}

use crate::PushIf;
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
    fn glyph(self) -> &'static str {
        match self {
            Self::Folder | Self::FolderOpen => "\u{25A0}", // ■
            Self::File => "\u{25AB}",                      // ▫
            Self::Schematic => "\u{25A3}",                 // ▣
            Self::Pcb => "\u{25A6}",                       // ▦
            Self::Library => "\u{25C6}",                   // ◆
            Self::Component => "\u{25C8}",                 // ◈
            Self::Sheet => "\u{25A1}",                     // □
            Self::Net => "\u{223F}",                       // ∿
            Self::Pin => "\u{2022}",                       // •
        }
    }

    fn color(self, tokens: &ThemeTokens) -> Color {
        match self {
            Self::Folder | Self::FolderOpen => theme_ext::warning_color(tokens),
            Self::File => theme_ext::text_secondary(tokens),
            Self::Schematic => theme_ext::accent(tokens),
            Self::Pcb => theme_ext::success_color(tokens),
            Self::Library => Color::from_rgb(0.70, 0.50, 0.90),
            Self::Component => Color::from_rgb(0.40, 0.80, 0.90),
            Self::Sheet => theme_ext::accent(tokens),
            Self::Net => theme_ext::success_color(tokens),
            Self::Pin => theme_ext::text_secondary(tokens),
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
    /// Node represents a folder that can hold children (show expand even if empty).
    pub is_folder: bool,
}

impl TreeNode {
    pub fn leaf(label: impl Into<String>, icon: TreeIcon) -> Self {
        Self {
            label: label.into(),
            icon,
            expanded: false,
            children: Vec::new(),
            badge: None,
            is_folder: false,
        }
    }

    pub fn branch(label: impl Into<String>, icon: TreeIcon, children: Vec<TreeNode>) -> Self {
        Self {
            label: label.into(),
            icon,
            expanded: true,
            children,
            badge: None,
            is_folder: true,
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

// ─── Layout Constants (matched to Altium Designer) ────────────

const INDENT_PER_DEPTH: f32 = 16.0; // Altium: ~16px per depth
const BASE_PAD_LEFT: f32 = 4.0; // minimal base indent
const ELEM_GAP: f32 = 2.0; // Altium: very tight gaps
const CHEVRON_W: f32 = 10.0; // triangle column
const ICON_SZ: f32 = 13.0; // Altium: small colored icons
const FONT_SZ: f32 = 12.0; // body text
const BADGE_SZ: f32 = 10.0; // muted counts
const PAD_V: u16 = 2; // Altium: compact rows (~20px total)
const PAD_R: u16 = 4; // minimal right margin

// ─── TreeView (COSMIC composition struct) ─────────────────────

/// Composable tree view widget.
///
/// ```rust,ignore
/// TreeView::new(&roots, &tokens)
///     .selected(&path)
///     .view()
///     .map(PanelMsg::Tree)
/// ```
pub struct TreeView<'a> {
    roots: &'a [TreeNode],
    selected: Option<&'a [usize]>,
    tokens: &'a ThemeTokens,
}

impl<'a> TreeView<'a> {
    pub fn new(roots: &'a [TreeNode], tokens: &'a ThemeTokens) -> Self {
        Self {
            roots,
            selected: None,
            tokens,
        }
    }

    pub fn selected(mut self, path: &'a [usize]) -> Self {
        self.selected = Some(path);
        self
    }

    /// Build the tree into a scrollable Element.
    pub fn view(self) -> Element<'static, TreeMsg> {
        let mut col: Column<'static, TreeMsg> = Column::new().spacing(0.0).width(Length::Fill);
        for (i, node) in self.roots.iter().enumerate() {
            col = render_node(col, node, 0, &[i], self.selected, self.tokens);
        }
        scrollable(col).width(Length::Fill).into()
    }
}

// ─── Row Rendering ────────────────────────────────────────────

fn render_node(
    mut col: Column<'static, TreeMsg>,
    node: &TreeNode,
    depth: usize,
    path: &[usize],
    selected: Option<&[usize]>,
    tokens: &ThemeTokens,
) -> Column<'static, TreeMsg> {
    let path_vec: Vec<usize> = path.to_vec();
    let is_sel = selected.is_some_and(|s| s == path);
    let has_kids = !node.children.is_empty();
    let is_expandable = has_kids || node.is_folder;

    // --- Colors ---
    let txt_c = if is_sel {
        Color::WHITE
    } else {
        theme_ext::text_secondary(tokens) // React: text-text-secondary by default
    };
    let icon_c = if is_sel {
        Color::WHITE
    } else {
        node.icon.color(tokens)
    };
    let badge_c = {
        let base = theme_ext::text_secondary(tokens);
        Color::from_rgba(base.r, base.g, base.b, 0.5)
    };
    let sel_bg = theme_ext::selection_color(tokens);
    let hov_bg = theme_ext::hover_color(tokens);

    // --- Assemble row ---
    let pad_left = (depth as f32) * INDENT_PER_DEPTH + BASE_PAD_LEFT;

    let mut r: Row<'static, TreeMsg> = Row::new()
        .spacing(ELEM_GAP)
        .align_y(iced::Alignment::Center);

    // Left indent
    r = r.push_if(pad_left > 0.0, || Space::new().width(pad_left));

    // Chevron — SVG icon (Unicode triangles render as colored emoji on Windows)
    if is_expandable {
        let handle = if node.expanded {
            chevron_down_handle()
        } else {
            chevron_right_handle()
        };
        r = r.push(
            svg(handle)
                .width(CHEVRON_W)
                .height(CHEVRON_W)
                .style(|_: &Theme, _| svg::Style {
                    color: Some(Color::WHITE),
                }),
        );
    } else {
        r = r.push(Space::new().width(CHEVRON_W));
    }

    // Icon
    r = r.push(text(node.icon.glyph()).size(ICON_SZ).color(icon_c));

    // Label (flex, no wrap — truncation handled by scrollable parent)
    r = r.push(
        text(node.label.clone())
            .size(FONT_SZ)
            .color(txt_c)
            .wrapping(iced::widget::text::Wrapping::None),
    );

    // Badge (right-aligned, very muted, single line)
    if let Some(badge) = &node.badge {
        r = r.push(iced::widget::space::horizontal());
        r = r.push(
            text(badge.clone())
                .size(BADGE_SZ)
                .color(badge_c)
                .wrapping(iced::widget::text::Wrapping::None),
        );
    }

    // Click action
    let msg = if is_expandable {
        TreeMsg::Toggle(path_vec.clone())
    } else {
        TreeMsg::Select(path_vec)
    };

    // Button with hover/selection styling
    let row_btn = button(r)
        .padding([PAD_V, PAD_R])
        .width(Length::Fill)
        .on_press(msg)
        .style(move |_theme: &Theme, status: button::Status| {
            let bg = match (is_sel, status) {
                (true, _) => Some(Background::Color(sel_bg)),
                (false, button::Status::Hovered | button::Status::Pressed) => {
                    Some(Background::Color(hov_bg))
                }
                _ => None,
            };
            button::Style {
                background: bg,
                text_color: txt_c,
                border: Border {
                    radius: 2.0.into(),
                    ..Border::default()
                },
                ..button::Style::default()
            }
        });

    col = col.push(row_btn);

    // Expanded children
    if is_expandable && node.expanded {
        if has_kids {
            for (i, child) in node.children.iter().enumerate() {
                let mut child_path = path.to_vec();
                child_path.push(i);
                col = render_node(col, child, depth + 1, &child_path, selected, tokens);
            }
        } else {
            // "(empty)" indicator — matches React's italic muted placeholder
            let empty_pad =
                ((depth + 1) as f32) * INDENT_PER_DEPTH + BASE_PAD_LEFT + CHEVRON_W + ELEM_GAP;
            let muted = theme_ext::text_secondary(tokens);
            let empty_c = Color::from_rgba(muted.r, muted.g, muted.b, 0.3);
            col = col.push(
                container(
                    text("(empty)")
                        .size(11.0)
                        .color(empty_c)
                        .wrapping(iced::widget::text::Wrapping::None),
                )
                .padding(iced::Padding {
                    top: 2.0,
                    right: 0.0,
                    bottom: 2.0,
                    left: empty_pad,
                }),
            );
        }
    }

    col
}

// ─── State Helpers ────────────────────────────────────────────

/// Get a node by path (immutable).
pub fn get_node<'a>(roots: &'a [TreeNode], path: &[usize]) -> Option<&'a TreeNode> {
    if path.is_empty() {
        return None;
    }
    let mut current = roots.get(path[0])?;
    for &idx in &path[1..] {
        current = current.children.get(idx)?;
    }
    Some(current)
}

/// Toggle expand/collapse at the given path.
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
