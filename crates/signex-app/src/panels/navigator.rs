//! Navigator panel model — the project's sheet hierarchy as a [`TreeView`]
//! tree, plus the click state that tree needs.
//!
//! The hierarchy itself ([`SheetNode`]) is built once from the assembled
//! project sheet set; this module only turns it into rows and remembers which
//! row is highlighted, so the panel keeps showing the whole project whichever
//! sheet is focused.
//!
//! [`TreeView`]: signex_widgets::tree_view::TreeView

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use signex_widgets::tree_view::{TreeIcon, TreeNode};

use crate::app::sheet_hierarchy::{SheetNode, SheetNodeKind};

/// Two clicks on the same row within this window are a double-click — the same
/// 500 ms the Projects tree uses.
const DOUBLE_CLICK_WINDOW: Duration = Duration::from_millis(500);

/// State behind the Navigator panel.
#[derive(Debug, Default)]
pub struct NavigatorModel {
    /// What the hierarchy was built from; unchanged means the cached
    /// `hierarchy` is still current and the disk is not read again.
    pub fingerprint: String,
    /// The occurrence tree; `None` when no project is in view.
    pub hierarchy: Option<SheetNode>,
    /// Rows derived from `hierarchy` (one root). Holds expand/collapse state.
    pub tree: Vec<TreeNode>,
    /// Highlighted row (single click).
    pub selected: Option<Vec<usize>>,
    /// Previous click, for double-click detection.
    pub last_click: Option<(Vec<usize>, Instant)>,
}

impl NavigatorModel {
    /// The hierarchy node a tree row path stands for.
    pub fn node_at(&self, tree_path: &[usize]) -> Option<&SheetNode> {
        let (&0, rest) = tree_path.split_first()? else {
            return None;
        };
        self.hierarchy.as_ref()?.descend(rest)
    }

    /// Re-derive the rows from the cached hierarchy, keeping the user's
    /// collapsed branches, and mark the active and open sheets.
    pub fn rebuild_tree(&mut self, active: Option<&Path>, open: &[PathBuf]) {
        let collapsed = collapsed_paths(&self.tree);
        self.tree = self
            .hierarchy
            .as_ref()
            .map(|root| vec![tree_node(root, active, open)])
            .unwrap_or_default();
        for path in &collapsed {
            if let Some(node) = node_mut(&mut self.tree, path)
                && !node.children.is_empty()
            {
                node.expanded = false;
            }
        }
    }

    /// Record a click on `path`; `true` when it completes a double-click.
    pub fn register_click(&mut self, path: &[usize], now: Instant) -> bool {
        let is_double = matches!(
            &self.last_click,
            Some((prev, at)) if prev == path && now.duration_since(*at) <= DOUBLE_CLICK_WINDOW
        );
        self.last_click = if is_double {
            None
        } else {
            Some((path.to_vec(), now))
        };
        is_double
    }
}

fn tree_node(node: &SheetNode, active: Option<&Path>, open: &[PathBuf]) -> TreeNode {
    let label = match node.kind {
        SheetNodeKind::Present => node.name.clone(),
        SheetNodeKind::Missing => format!("{} (missing)", node.name),
        SheetNodeKind::Cycle => format!("{} (cycle)", node.name),
        SheetNodeKind::Truncated => format!("{} (not expanded)", node.name),
    };
    let children: Vec<TreeNode> = node
        .children
        .iter()
        .map(|child| tree_node(child, active, open))
        .collect();
    let row = if children.is_empty() {
        TreeNode::leaf(label, TreeIcon::Sheet)
    } else {
        TreeNode::branch(label, TreeIcon::Sheet, children)
    };
    // The file name rides along as the secondary text, the same pair
    // (name, file) the Properties panel shows for a selected sheet.
    row.with_badge(node.filename.clone())
        .with_active(active.is_some_and(|p| node.is_at(p)))
        .with_open(open.iter().any(|p| node.is_at(p)))
}

fn collapsed_paths(roots: &[TreeNode]) -> Vec<Vec<usize>> {
    fn walk(nodes: &[TreeNode], prefix: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        for (i, node) in nodes.iter().enumerate() {
            prefix.push(i);
            if !node.children.is_empty() && !node.expanded {
                out.push(prefix.clone());
            }
            walk(&node.children, prefix, out);
            prefix.pop();
        }
    }
    let mut out = Vec::new();
    walk(roots, &mut Vec::new(), &mut out);
    out
}

fn node_mut<'a>(roots: &'a mut [TreeNode], path: &[usize]) -> Option<&'a mut TreeNode> {
    let (&first, rest) = path.split_first()?;
    let mut node = roots.get_mut(first)?;
    for &i in rest {
        node = node.children.get_mut(i)?;
    }
    Some(node)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(name: &str, file: &str, children: Vec<SheetNode>) -> SheetNode {
        SheetNode {
            name: name.to_string(),
            filename: file.to_string(),
            path: PathBuf::from("/proj").join(file),
            kind: SheetNodeKind::Present,
            children,
        }
    }

    fn model() -> NavigatorModel {
        let mut m = NavigatorModel {
            hierarchy: Some(node(
                "root",
                "root.snxsch",
                vec![node(
                    "MCU",
                    "mcu.snxsch",
                    vec![node("ADC", "adc.snxsch", vec![])],
                )],
            )),
            ..NavigatorModel::default()
        };
        m.rebuild_tree(None, &[]);
        m
    }

    #[test]
    fn rows_nest_with_the_file_name_as_secondary_text() {
        let m = model();
        let adc = &m.tree[0].children[0].children[0];
        assert_eq!(adc.label, "ADC");
        assert_eq!(adc.badge.as_deref(), Some("adc.snxsch"));
    }

    #[test]
    fn the_active_sheet_is_highlighted_wherever_it_sits() {
        let mut m = model();
        m.rebuild_tree(Some(Path::new("/proj/adc.snxsch")), &[]);
        assert!(m.tree[0].children[0].children[0].is_active);
        assert!(!m.tree[0].is_active);
    }

    #[test]
    fn rebuild_keeps_collapsed_branches() {
        let mut m = model();
        m.tree[0].children[0].expanded = false;
        m.rebuild_tree(None, &[]);
        assert!(!m.tree[0].children[0].expanded);
        assert!(m.tree[0].expanded);
    }

    #[test]
    fn missing_and_cycle_rows_carry_a_marker() {
        let mut m = model();
        let h = m.hierarchy.as_mut().expect("hierarchy");
        h.children[0].children[0].kind = SheetNodeKind::Missing;
        h.children.push(SheetNode {
            kind: SheetNodeKind::Cycle,
            ..node("Loop", "root.snxsch", vec![])
        });
        m.rebuild_tree(None, &[]);
        assert_eq!(m.tree[0].children[0].children[0].label, "ADC (missing)");
        assert_eq!(m.tree[0].children[1].label, "Loop (cycle)");
    }

    #[test]
    fn tree_paths_map_to_hierarchy_nodes() {
        let m = model();
        assert_eq!(m.node_at(&[0]).map(|n| n.name.as_str()), Some("root"));
        assert_eq!(m.node_at(&[0, 0, 0]).map(|n| n.name.as_str()), Some("ADC"));
        assert!(m.node_at(&[1]).is_none());
        assert!(m.node_at(&[]).is_none());
    }

    #[test]
    fn double_click_needs_the_same_row_inside_the_window() {
        let mut m = model();
        let t0 = Instant::now();
        assert!(!m.register_click(&[0, 0], t0));
        assert!(m.register_click(&[0, 0], t0 + Duration::from_millis(400)));
        // The completed double-click is consumed: a third click starts over.
        assert!(!m.register_click(&[0, 0], t0 + Duration::from_millis(450)));
        // Different row, or too slow, is not a double-click.
        assert!(!m.register_click(&[0], t0 + Duration::from_millis(500)));
        assert!(!m.register_click(&[0], t0 + Duration::from_millis(1100)));
    }
}
