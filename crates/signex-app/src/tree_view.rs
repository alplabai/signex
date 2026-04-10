//! Custom tree view widget for Projects panel and component browser.
//!
//! Iced has no built-in tree view. This implements one using Column + Row
//! with indentation, expand/collapse, and click selection.

use iced::widget::{button, row, text, Column};
use iced::Element;

#[derive(Debug, Clone)]
pub struct TreeNode {
    pub label: String,
    pub expanded: bool,
    pub children: Vec<TreeNode>,
}

#[derive(Debug, Clone)]
pub enum TreeMessage {
    Toggle(Vec<usize>),
    Select(Vec<usize>),
}

impl TreeNode {
    pub fn leaf(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            expanded: false,
            children: Vec::new(),
        }
    }

    pub fn branch(label: impl Into<String>, children: Vec<TreeNode>) -> Self {
        Self {
            label: label.into(),
            expanded: true,
            children,
        }
    }
}

/// Render a tree from a list of root nodes.
pub fn view<'a>(roots: &'a [TreeNode]) -> Element<'a, TreeMessage> {
    let mut col = Column::new().spacing(1);
    for (i, node) in roots.iter().enumerate() {
        col = render_node(col, node, 0, &[i]);
    }
    col.into()
}

fn render_node<'a>(
    mut col: Column<'a, TreeMessage>,
    node: &'a TreeNode,
    depth: usize,
    path: &[usize],
) -> Column<'a, TreeMessage> {
    let indent = depth * 16;
    let path_vec: Vec<usize> = path.to_vec();

    let icon = if node.children.is_empty() {
        "  "
    } else if node.expanded {
        "v "
    } else {
        "> "
    };

    let label_row = row![
        text("").width(indent as u32),
        button(
            row![text(icon).size(11), text(&node.label).size(12)]
                .align_y(iced::Alignment::Center),
        )
        .padding([2, 4])
        .style(button::text)
        .on_press(if node.children.is_empty() {
            TreeMessage::Select(path_vec.clone())
        } else {
            TreeMessage::Toggle(path_vec)
        }),
    ];

    col = col.push(label_row);

    if node.expanded {
        for (i, child) in node.children.iter().enumerate() {
            let mut child_path = path.to_vec();
            child_path.push(i);
            col = render_node(col, child, depth + 1, &child_path);
        }
    }

    col
}

/// Toggle expand/collapse at the given path.
pub fn toggle(roots: &mut [TreeNode], path: &[usize]) {
    if path.is_empty() {
        return;
    }
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
