//! The Navigator's rows as a flat list for the view: one per visible tree
//! row, with its depth, its file name, its page number and how it is drawn.
//! With a filter, a row shows when its name or file name holds the text, or
//! when one of its descendants does; branches are then shown open.

use signex_i18n::I18n;
use signex_widgets::tree_view::{TreeKey, TreeNode};

use super::NavigatorModel;
use crate::app::sheet_hierarchy::SheetNodeKind;

/// One row of the Navigator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigatorRow {
    /// Where the row sits in the tree: what Select and Toggle resolve.
    pub key: TreeKey,
    pub path: Vec<usize>,
    pub depth: usize,
    /// The sheet name with its problem mark, in the panel's language.
    pub label: String,
    /// The file the sheet reference names.
    pub file: String,
    /// The page number of this placement.
    pub number: Option<usize>,
    /// Missing, unreadable, rejected, a cycle or not expanded.
    pub is_problem: bool,
    /// A page the project lists that no sheet places.
    pub is_loose: bool,
    /// The sheet of the focused document.
    pub is_active: bool,
    pub has_children: bool,
    pub is_expanded: bool,
}

impl NavigatorModel {
    /// The rows the panel draws in `i18n`'s language, narrowed by the filter.
    pub fn visible_rows(&self, i18n: &I18n) -> Vec<NavigatorRow> {
        let tree = self.shown_tree(i18n);
        let needle = self.filter.trim().to_lowercase();
        let mut rows = Vec::new();
        for (i, node) in tree.iter().enumerate() {
            self.push_rows(node, &mut vec![i], &mut Vec::new(), &needle, &mut rows);
        }
        rows
    }

    fn push_rows(
        &self,
        node: &TreeNode,
        path: &mut Vec<usize>,
        key: &mut TreeKey,
        needle: &str,
        out: &mut Vec<NavigatorRow>,
    ) {
        if !needle.is_empty() && !matches_below(node, needle) {
            return;
        }
        key.push(node.key.clone());
        let sheet = self.node_at(path);
        out.push(NavigatorRow {
            key: key.clone(),
            path: path.clone(),
            depth: path.len() - 1,
            label: node.label.clone(),
            file: node.badge.clone().unwrap_or_default(),
            number: self.number_at(path),
            is_problem: sheet.is_some_and(|s| s.kind != SheetNodeKind::Present),
            // The root is row 0; every other top-level row is a loose page.
            is_loose: path.len() == 1 && path[0] > 0,
            is_active: node.is_active,
            has_children: !node.children.is_empty(),
            is_expanded: node.expanded || !needle.is_empty(),
        });
        if node.expanded || !needle.is_empty() {
            for (i, child) in node.children.iter().enumerate() {
                path.push(i);
                self.push_rows(child, path, key, needle, out);
                path.pop();
            }
        }
        key.pop();
    }
}

/// Whether `node` or a row below it holds `needle` in its name or file name.
fn matches_below(node: &TreeNode, needle: &str) -> bool {
    let holds = |text: &str| text.to_lowercase().contains(needle);
    holds(&node.label)
        || node.badge.as_deref().is_some_and(holds)
        || node.children.iter().any(|c| matches_below(c, needle))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use signex_types::schematic::InstancePath;
    use uuid::Uuid;

    use super::*;
    use crate::app::sheet_hierarchy::{SheetHierarchy, SheetNode, normalised_key};

    fn node(name: &str, file: &str, placed: Option<u128>, children: Vec<SheetNode>) -> SheetNode {
        SheetNode {
            name: name.to_string(),
            filename: file.to_string(),
            placed_as: placed.map(Uuid::from_u128),
            path: PathBuf::from("/proj").join(file),
            kind: SheetNodeKind::Present,
            detail: None,
            children,
        }
    }

    /// `dcdc` places `Power`, which places `reg.snxsch` twice (3V3, 1V8);
    /// `Notes` is a page no sheet places. Numbered 1-4 in walk order, the
    /// loose page 5.
    fn model() -> NavigatorModel {
        let hierarchy = SheetHierarchy {
            fold: Default::default(),
            root: Some(node(
                "dcdc_board",
                "dcdc.snxsch",
                None,
                vec![node(
                    "Power",
                    "power.snxsch",
                    Some(1),
                    vec![
                        node("Regulator 3V3", "reg.snxsch", Some(2), vec![]),
                        node("Regulator 1V8", "reg.snxsch", Some(3), vec![]),
                    ],
                )],
            )),
            loose: vec![node("Notes", "notes.snxsch", None, vec![])],
        };
        let fold = hierarchy.fold;
        let key = |file: &str| normalised_key(&PathBuf::from("/proj").join(file), fold);
        let at = |chain: &[u128]| {
            InstancePath::from_chain(chain.iter().map(|n| Uuid::from_u128(*n)).collect())
        };
        let mut m = NavigatorModel::default();
        m.replace_hierarchy(hierarchy, None, &[]);
        m.numbers = [
            ((key("dcdc.snxsch"), at(&[])), 1),
            ((key("power.snxsch"), at(&[1])), 2),
            ((key("reg.snxsch"), at(&[1, 2])), 3),
            ((key("reg.snxsch"), at(&[1, 3])), 4),
            ((key("notes.snxsch"), at(&[])), 5),
        ]
        .into_iter()
        .collect();
        m
    }

    fn listed(m: &NavigatorModel) -> Vec<(String, Option<usize>)> {
        m.visible_rows(&I18n::en_us())
            .into_iter()
            .map(|r| (r.label, r.number))
            .collect()
    }

    /// A reused child sheet is one row per placement, each with its own
    /// number from the project's sheet numbering.
    #[test]
    fn a_reused_child_sheet_has_a_row_and_a_number_per_placement() {
        let m = model();
        let rows = listed(&m);
        assert_eq!(
            rows,
            [
                ("dcdc_board".to_string(), Some(1)),
                ("Power".to_string(), Some(2)),
                ("Regulator 3V3".to_string(), Some(3)),
                ("Regulator 1V8".to_string(), Some(4)),
                ("Notes".to_string(), Some(5)),
            ]
        );
        let rows = m.visible_rows(&I18n::en_us());
        assert_eq!(rows[2].file, "reg.snxsch");
        assert_eq!(rows[3].file, "reg.snxsch");
        assert_eq!((rows[2].depth, rows[0].depth), (2, 0));
    }

    /// Loose pages are extra top-level rows, drawn as loose; the root is not.
    #[test]
    fn loose_pages_are_listed_after_the_root_and_marked() {
        let rows = model().visible_rows(&I18n::en_us());
        let loose: Vec<&str> = rows
            .iter()
            .filter(|r| r.is_loose)
            .map(|r| r.label.as_str())
            .collect();
        assert_eq!(loose, ["Notes"]);
        assert_eq!(rows.last().map(|r| r.path.clone()), Some(vec![1]));
    }

    /// The filter keeps matching rows and the rows above them, open, and
    /// matches file names too; a missing sheet has no number.
    #[test]
    fn the_filter_narrows_the_rows_and_keeps_their_parents() {
        let mut m = model();
        m.filter = "1v8".into();
        let labels: Vec<String> = listed(&m).into_iter().map(|(l, _)| l).collect();
        assert_eq!(labels, ["dcdc_board", "Power", "Regulator 1V8"]);

        m.filter = "NOTES.snx".into();
        let labels: Vec<String> = listed(&m).into_iter().map(|(l, _)| l).collect();
        assert_eq!(labels, ["Notes"]);

        m.filter = "vref".into();
        assert!(m.visible_rows(&I18n::en_us()).is_empty());

        m.filter.clear();
        m.tree[0].children[0].expanded = false;
        assert_eq!(listed(&m).len(), 3, "a folded branch hides its rows");
    }

    #[test]
    fn a_problem_row_is_marked_and_has_no_number() {
        let mut m = model();
        if let Some(root) = m.hierarchy.as_mut().and_then(|h| h.root.as_mut()) {
            root.children[0].children[1].kind = SheetNodeKind::Missing;
        }
        m.rebuild_tree(None, &[]);
        let rows = m.visible_rows(&I18n::en_us());
        assert!(rows[3].is_problem);
        assert_eq!(rows[3].label, "Regulator 1V8 (missing)");
        assert_eq!(rows[3].number, None);
        assert!(!rows[2].is_problem);
    }
}
