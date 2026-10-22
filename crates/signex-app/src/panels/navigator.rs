//! Navigator panel model — the project's sheet hierarchy as a [`TreeView`]
//! tree, plus the click state that tree needs.
//!
//! The hierarchy itself ([`SheetHierarchy`]) is built once from the assembled
//! project sheet set; this module only turns it into rows and remembers which
//! row is highlighted, so the panel keeps showing the whole project whichever
//! sheet is focused.
//!
//! Selection, the previous click and the collapsed branches are remembered by
//! *occurrence* (the chain of reference names down from the top-level row),
//! not by tree index, so a rebuild that inserts or removes a reference above
//! them does not move them onto a different sheet.
//!
//! [`TreeView`]: signex_widgets::tree_view::TreeView

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use signex_widgets::tree_view::{TreeIcon, TreeNode};

use crate::app::sheet_hierarchy::{SheetHierarchy, SheetNode, SheetNodeKind, sheet_path_key};

/// Two clicks on the same row within this window are a double-click — the same
/// 500 ms the Projects tree uses.
const DOUBLE_CLICK_WINDOW: Duration = Duration::from_millis(500);

/// A row identified by what it is rather than where it currently sits: one
/// `name|file|n` element per level, `n` counting earlier siblings with the
/// same name and file.
type OccurrenceKey = Vec<String>;

/// State behind the Navigator panel.
#[derive(Debug, Default)]
pub struct NavigatorModel {
    /// What the hierarchy was built from; unchanged means the cached
    /// `hierarchy` is still current and the disk is not read again.
    pub fingerprint: String,
    /// The occurrence tree; `None` when no project is in view.
    pub hierarchy: Option<SheetHierarchy>,
    /// Rows derived from `hierarchy`: the root, then any loose pages. Holds
    /// expand/collapse state.
    pub tree: Vec<TreeNode>,
    /// Highlighted row (single click).
    pub selected: Option<Vec<usize>>,
    /// Previous click, for double-click detection.
    pub last_click: Option<(Vec<usize>, Instant)>,
}

impl NavigatorModel {
    /// The hierarchy node a tree row path stands for.
    pub fn node_at(&self, tree_path: &[usize]) -> Option<&SheetNode> {
        self.hierarchy.as_ref()?.node_at(tree_path)
    }

    /// Swap in a freshly built hierarchy, carrying the selection, the pending
    /// double-click and the collapsed branches over by occurrence: whatever
    /// no longer exists is dropped, and nothing lands on a different sheet.
    pub fn replace_hierarchy(
        &mut self,
        hierarchy: SheetHierarchy,
        active: Option<&Path>,
        open: &[PathBuf],
    ) {
        let old = self.hierarchy.take();
        let remembered = |path: &[usize]| old.as_ref().and_then(|h| occurrence_key(h, path));
        let selected = self.selected.as_deref().and_then(remembered);
        let last_click = self
            .last_click
            .as_ref()
            .and_then(|(path, at)| remembered(path).map(|key| (key, *at)));
        let collapsed: Vec<OccurrenceKey> = collapsed_paths(&self.tree)
            .iter()
            .filter_map(|path| remembered(path))
            .collect();

        self.hierarchy = Some(hierarchy);
        let new = self.hierarchy.as_ref();
        let locate = |key: &OccurrenceKey| new.and_then(|h| resolve_occurrence(h, key));
        self.selected = selected.as_ref().and_then(locate);
        self.last_click = last_click.and_then(|(key, at)| locate(&key).map(|path| (path, at)));
        let collapsed_now: Vec<Vec<usize>> = collapsed.iter().filter_map(locate).collect();
        self.rebuild_with(active, open, &collapsed_now);
    }

    /// Re-derive the rows from the cached hierarchy, keeping the collapsed
    /// branches the user chose, and mark the active and open sheets.
    pub fn rebuild_tree(&mut self, active: Option<&Path>, open: &[PathBuf]) {
        let collapsed = collapsed_paths(&self.tree);
        self.rebuild_with(active, open, &collapsed);
    }

    fn rebuild_with(&mut self, active: Option<&Path>, open: &[PathBuf], collapsed: &[Vec<usize>]) {
        // Path keys are computed once per call, not once per node and tab.
        let active_key = active.map(sheet_path_key);
        let open_keys: HashSet<String> = open.iter().map(|p| sheet_path_key(p)).collect();
        self.tree = self
            .hierarchy
            .as_ref()
            .map(|h| {
                h.top_level()
                    .map(|node| tree_node(node, active_key.as_deref(), &open_keys))
                    .collect()
            })
            .unwrap_or_default();
        for path in collapsed {
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

fn sibling_key(siblings: &[&SheetNode], index: usize) -> String {
    let node = siblings[index];
    let earlier = siblings[..index]
        .iter()
        .filter(|s| s.name == node.name && s.filename == node.filename)
        .count();
    format!("{}|{}|{earlier}", node.name, node.filename)
}

/// The occurrence a tree row path stands for, in `hierarchy`.
fn occurrence_key(hierarchy: &SheetHierarchy, path: &[usize]) -> Option<OccurrenceKey> {
    let mut siblings: Vec<&SheetNode> = hierarchy.top_level().collect();
    let mut key = Vec::with_capacity(path.len());
    for &i in path {
        let node = *siblings.get(i)?;
        key.push(sibling_key(&siblings, i));
        siblings = node.children.iter().collect();
    }
    Some(key)
}

/// The tree row path of `key` in `hierarchy`, when that occurrence still exists.
fn resolve_occurrence(hierarchy: &SheetHierarchy, key: &OccurrenceKey) -> Option<Vec<usize>> {
    let mut siblings: Vec<&SheetNode> = hierarchy.top_level().collect();
    let mut path = Vec::with_capacity(key.len());
    for wanted in key {
        let i = (0..siblings.len()).find(|&i| sibling_key(&siblings, i) == *wanted)?;
        path.push(i);
        siblings = siblings[i].children.iter().collect();
    }
    Some(path)
}

fn tree_node(node: &SheetNode, active: Option<&str>, open: &HashSet<String>) -> TreeNode {
    let label = match node.kind {
        SheetNodeKind::Present => node.name.clone(),
        SheetNodeKind::Missing => format!("{} (missing)", node.name),
        SheetNodeKind::Unreadable => format!("{} (unreadable)", node.name),
        SheetNodeKind::Rejected => format!("{} (rejected)", node.name),
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
    let key = sheet_path_key(&node.path);
    // The file name rides along as the secondary text, the same pair
    // (name, file) the Properties panel shows for a selected sheet.
    row.with_badge(node.filename.clone())
        .with_active(active == Some(key.as_str()))
        .with_open(open.contains(&key))
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
            detail: None,
            children,
        }
    }

    fn model() -> NavigatorModel {
        let mut m = NavigatorModel {
            hierarchy: Some(SheetHierarchy {
                root: Some(node(
                    "root",
                    "root.snxsch",
                    vec![node(
                        "MCU",
                        "mcu.snxsch",
                        vec![node("ADC", "adc.snxsch", vec![])],
                    )],
                )),
                loose: Vec::new(),
            }),
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
        let h = m
            .hierarchy
            .as_mut()
            .and_then(|h| h.root.as_mut())
            .expect("hierarchy");
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

    fn two_children(first: &str, second: &str) -> SheetHierarchy {
        SheetHierarchy {
            root: Some(node(
                "root",
                "root.snxsch",
                vec![
                    node(
                        first,
                        &format!("{first}.snxsch"),
                        vec![node("Deep", "deep.snxsch", vec![])],
                    ),
                    node(second, &format!("{second}.snxsch"), vec![]),
                ],
            )),
            loose: Vec::new(),
        }
    }

    #[test]
    fn selection_and_collapse_follow_the_sheet_across_a_rebuild() {
        let mut m = NavigatorModel::default();
        m.replace_hierarchy(two_children("A", "B"), None, &[]);
        m.selected = Some(vec![0, 1]);
        m.tree[0].children[0].expanded = false;

        // A new reference is inserted above both: A and B shift down one row.
        let mut grown = two_children("A", "B");
        let root = grown.root.as_mut().expect("root");
        root.children.insert(0, node("New", "new.snxsch", vec![]));
        m.replace_hierarchy(grown, None, &[]);

        assert_eq!(m.selected, Some(vec![0, 2]), "still B");
        assert!(!m.tree[0].children[1].expanded, "A stays collapsed");
    }

    #[test]
    fn a_removed_sheet_drops_its_selection_and_pending_click() {
        let mut m = NavigatorModel::default();
        m.replace_hierarchy(two_children("A", "B"), None, &[]);
        m.selected = Some(vec![0, 1]);
        let t0 = Instant::now();
        assert!(!m.register_click(&[0, 1], t0));

        m.replace_hierarchy(two_children("A", "C"), None, &[]);

        assert_eq!(m.selected, None);
        assert!(m.last_click.is_none());
    }

    #[test]
    fn loose_pages_are_extra_top_level_rows() {
        let mut m = model();
        let loose = node("Extra", "extra.snxsch", vec![]);
        if let Some(h) = m.hierarchy.as_mut() {
            h.loose.push(loose);
        }
        m.rebuild_tree(None, &[]);
        assert_eq!(m.tree.len(), 2);
        assert_eq!(m.tree[1].label, "Extra");
        assert_eq!(m.node_at(&[1]).map(|n| n.name.as_str()), Some("Extra"));
    }
}
