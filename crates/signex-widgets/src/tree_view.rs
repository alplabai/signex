//! Tree view widget for project panels and component browsers.
//!
//! Built on stock Iced 0.14 primitives — Column of Rows with indentation,
//! expand/collapse, and click selection. Fully themed via `ThemeTokens`.

use iced::widget::{button, container, row, scrollable, space, text, Column};
use iced::{Border, Element, Length};
use signex_types::theme::ThemeTokens;

use crate::theme_ext;

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

/// Type of icon displayed next to a tree node label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeIcon {
    Folder,
    File,
    Schematic,
    Pcb,
    Library,
    Component,
    Sheet,
}

impl TreeIcon {
    /// Return a short text representation for display.
    fn label(self) -> &'static str {
        match self {
            TreeIcon::Folder => "[D]",
            TreeIcon::File => "[F]",
            TreeIcon::Schematic => "[S]",
            TreeIcon::Pcb => "[P]",
            TreeIcon::Library => "[L]",
            TreeIcon::Component => "[C]",
            TreeIcon::Sheet => "[H]",
        }
    }
}

/// A single node in the tree hierarchy.
#[derive(Debug, Clone)]
pub struct TreeNode {
    pub label: String,
    pub icon: TreeIcon,
    pub expanded: bool,
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    /// Create a leaf node (no children).
    pub fn leaf(label: impl Into<String>, icon: TreeIcon) -> Self {
        Self {
            label: label.into(),
            icon,
            expanded: false,
            children: Vec::new(),
        }
    }

    /// Create a branch node (with children, expanded by default).
    pub fn branch(label: impl Into<String>, icon: TreeIcon, children: Vec<TreeNode>) -> Self {
        Self {
            label: label.into(),
            icon,
            expanded: true,
            children,
        }
    }
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

/// Messages emitted by the tree view.
#[derive(Debug, Clone)]
pub enum TreeMsg {
    /// A branch node was toggled (expand/collapse). Payload is the index path.
    Toggle(Vec<usize>),
    /// A node was selected (clicked). Payload is the index path.
    Select(Vec<usize>),
}

// ---------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------

/// Render a scrollable tree view from root nodes.
///
/// * `roots`    — top-level tree nodes to display.
/// * `selected` — currently selected path (if any), to highlight.
/// * `tokens`   — theme tokens for all colors.
pub fn tree_view<'a>(
    roots: &[TreeNode],
    selected: Option<&[usize]>,
    tokens: &ThemeTokens,
) -> Element<'a, TreeMsg> {
    let mut col = Column::new().spacing(1);
    for (i, node) in roots.iter().enumerate() {
        col = render_node(col, node, 0, &[i], selected, tokens);
    }
    scrollable(col.width(Length::Fill)).into()
}

fn render_node<'a>(
    mut col: Column<'a, TreeMsg>,
    node: &TreeNode,
    depth: usize,
    path: &[usize],
    selected: Option<&[usize]>,
    tokens: &ThemeTokens,
) -> Column<'a, TreeMsg> {
    let indent = (depth * 16) as f32;
    let path_vec: Vec<usize> = path.to_vec();

    let is_selected = selected.is_some_and(|s| s == path);

    // Expand / collapse indicator
    let expand_icon = if node.children.is_empty() {
        "  "
    } else if node.expanded {
        "v "
    } else {
        "> "
    };

    let text_color = if is_selected {
        theme_ext::text_primary(tokens)
    } else {
        theme_ext::text_secondary(tokens)
    };
    let icon_color = theme_ext::text_secondary(tokens);

    let label_row = row![
        space::horizontal().width(indent),
        text(expand_icon).size(11).color(icon_color),
        text(node.icon.label()).size(10).color(icon_color),
        text(" ").size(11),
        text(node.label.clone()).size(11).color(text_color),
    ]
    .align_y(iced::Alignment::Center);

    let msg = if node.children.is_empty() {
        TreeMsg::Select(path_vec.clone())
    } else {
        TreeMsg::Toggle(path_vec)
    };

    // Wrap the row in a button for click handling
    let row_btn = button(label_row)
        .padding([2, 4])
        .width(Length::Fill)
        .on_press(msg);

    // Apply styling based on selection state
    let styled_row: Element<'a, TreeMsg> = if is_selected {
        let accent = theme_ext::accent(tokens);
        let sel_bg = theme_ext::selection_color(tokens);
        container(row_btn.style(button::text))
            .width(Length::Fill)
            .style(move |_theme: &iced::Theme| container::Style {
                background: Some(sel_bg.into()),
                text_color: Some(accent),
                border: Border::default(),
                ..container::Style::default()
            })
            .into()
    } else {
        row_btn.style(button::text).into()
    };

    col = col.push(styled_row);

    if node.expanded {
        for (i, child) in node.children.iter().enumerate() {
            let mut child_path = path.to_vec();
            child_path.push(i);
            col = render_node(col, child, depth + 1, &child_path, selected, tokens);
        }
    }

    col
}

// ---------------------------------------------------------------------------
// Tree manipulation helpers
// ---------------------------------------------------------------------------

/// Toggle expand/collapse at the given path.
pub fn toggle(roots: &mut [TreeNode], path: &[usize]) {
    if let Some(node) = get_node_mut(roots, path) {
        node.expanded = !node.expanded;
    }
}

/// Get a mutable reference to the node at the given path.
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
