//! The project's sheet hierarchy as a tree of *occurrences*, built from the
//! sheet map [`assemble_project_sheets`] already produced.
//!
//! This is a read-only view over that map — it adds no loading and no
//! connectivity logic. It mirrors the per-occurrence model the netlist
//! stitcher uses (`signex-net/src/project/mod.rs`): a file instantiated N
//! times is N nodes, never collapsed into one, because the electrical model
//! keeps the instances distinct.
//!
//! Every child reference goes through the one shared resolver,
//! [`resolve_child_reference`], so the Navigator names the same file the
//! netlist and the canvas do: a `sub/../x` reference is `x`, and a reference
//! that escapes its folder (#463) is a [`SheetNodeKind::Rejected`] row that
//! cannot be opened. Lookups and the ancestor set are keyed on the normalised
//! path, so a cycle spelled through `..` is still caught.
//!
//! Failure shapes are rendered rather than hidden:
//! - a referenced child absent from the map is a [`SheetNodeKind::Missing`]
//!   node, and one that exists but could not be read is
//!   [`SheetNodeKind::Unreadable`] (carrying the reason);
//! - a reference back to one of its own ancestors is a
//!   [`SheetNodeKind::Cycle`] node with no children;
//! - the node budget and the depth cap end in a [`SheetNodeKind::Truncated`]
//!   node, so neither a wide nor a deep project can freeze the panel.
//!
//! [`assemble_project_sheets`]: crate::app::project_sheets::assemble_project_sheets
//! [`resolve_child_reference`]: crate::app::project_sheets::resolve_child_reference

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use signex_types::schematic::SchematicSheet;

use crate::app::project_sheets::{lexically_normalize, resolve_child_reference};
use crate::app::state::scope::path_key;

/// Upper bound on nodes in one built tree. A shared sub-sheet instantiated
/// twice at every level of a deep hierarchy doubles per level; without a bound
/// that is a legitimate (acyclic) input that still freezes the panel.
const MAX_NODES: usize = 5_000;

/// Deepest nesting expanded. The tree is walked recursively (here, when
/// rendering rows, and when restoring collapse state), so depth — not just
/// node count — must be bounded to stay clear of a small thread stack.
const MAX_DEPTH: usize = 64;

/// What a node stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetNodeKind {
    /// The sheet is loaded; its children follow.
    Present,
    /// Referenced, but not in the assembled sheet map (absent).
    Missing,
    /// The file exists but could not be read or parsed; see `detail`.
    Unreadable,
    /// The reference escapes its folder or is empty/absolute (#463); it is
    /// refused exactly as the netlist and canvas refuse it, and never opened.
    Rejected,
    /// References one of its own ancestors; expansion stops here.
    Cycle,
    /// The [`MAX_NODES`] budget or the [`MAX_DEPTH`] cap ended expansion here.
    Truncated,
}

/// One occurrence of a sheet in the hierarchy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetNode {
    /// `ChildSheet.name` of the referencing symbol; the root has no
    /// referencing symbol, so it carries its file stem.
    pub name: String,
    /// The reference string as written (`ChildSheet.filename`).
    pub filename: String,
    /// Where the sheet lives (the normalised resolution of `filename`) — the
    /// key into the assembled sheet map. For a [`SheetNodeKind::Rejected`]
    /// node it is the unresolved join, shown only for the message.
    pub path: PathBuf,
    pub kind: SheetNodeKind,
    /// Why the node is not a plain sheet (the read error for `Unreadable`).
    pub detail: Option<String>,
    pub children: Vec<SheetNode>,
}

impl SheetNode {
    /// Whether this occurrence is the sheet stored at `path` (separator- and
    /// case-insensitive on Windows, like every other sheet-path comparison).
    pub fn is_at(&self, path: &Path) -> bool {
        path_key(&self.path) == path_key(path)
    }

    /// The node reached by following child indices from this one.
    pub fn descend(&self, indices: &[usize]) -> Option<&SheetNode> {
        indices
            .iter()
            .try_fold(self, |node, &i| node.children.get(i))
    }
}

/// The identity two sheet paths are compared by (separator- and case-folded
/// on Windows) — exposed so the panel keys its row markers the same way.
pub(crate) fn sheet_path_key(path: &Path) -> String {
    path_key(path)
}

/// The whole Navigator tree: the root sheet's occurrences, followed by any
/// declared page the root does not reach (a flat project's other pages).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SheetHierarchy {
    pub root: Option<SheetNode>,
    /// Pages the project lists that no sheet reference reaches, as extra
    /// top-level rows after the root.
    pub loose: Vec<SheetNode>,
}

impl SheetHierarchy {
    /// The top-level nodes in row order: the root, then the loose pages.
    pub fn top_level(&self) -> impl Iterator<Item = &SheetNode> {
        self.root.iter().chain(self.loose.iter())
    }

    /// The node a tree-row path (first index = top-level row) stands for.
    pub fn node_at(&self, tree_path: &[usize]) -> Option<&SheetNode> {
        let (&first, rest) = tree_path.split_first()?;
        self.top_level().nth(first)?.descend(rest)
    }
}

/// What the builder looks sheets up in, keyed on the normalised path.
struct Lookup<'a> {
    sheets: HashMap<String, &'a SchematicSheet>,
    unreadable: HashMap<String, &'a str>,
}

fn normalised_key(path: &Path) -> String {
    path_key(&lexically_normalize(path))
}

impl<'a> Lookup<'a> {
    fn new(
        sheets: &'a HashMap<PathBuf, SchematicSheet>,
        unreadable: &'a [(PathBuf, String)],
    ) -> Self {
        let mut by_key: HashMap<String, &SchematicSheet> = HashMap::new();
        // Entries already stored under their canonical spelling win over an
        // alias of the same file, so the choice does not depend on map order.
        for (path, sheet) in sheets {
            if lexically_normalize(path) == *path {
                by_key.insert(normalised_key(path), sheet);
            }
        }
        for (path, sheet) in sheets {
            by_key.entry(normalised_key(path)).or_insert(sheet);
        }
        let unreadable = unreadable
            .iter()
            .map(|(path, why)| (normalised_key(path), why.as_str()))
            .collect();
        Self {
            sheets: by_key,
            unreadable,
        }
    }
}

/// Build the occurrence tree rooted at `root` over `sheets`, plus a subtree
/// for every page in `loose_pages`.
pub(crate) fn build_sheet_hierarchy(
    sheets: &HashMap<PathBuf, SchematicSheet>,
    unreadable: &[(PathBuf, String)],
    root: &Path,
    loose_pages: &[PathBuf],
) -> SheetHierarchy {
    let lookup = Lookup::new(sheets, unreadable);
    let build = |path: &Path| {
        let filename = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let name = path
            .file_stem()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| filename.clone());
        let mut budget = MAX_NODES;
        expand(
            &lookup,
            name,
            filename,
            path.to_path_buf(),
            0,
            &mut Vec::new(),
            &mut budget,
        )
    };
    SheetHierarchy {
        root: Some(build(root)),
        loose: loose_pages.iter().map(|page| build(page)).collect(),
    }
}

fn expand(
    lookup: &Lookup<'_>,
    name: String,
    filename: String,
    path: PathBuf,
    depth: usize,
    ancestors: &mut Vec<String>,
    budget: &mut usize,
) -> SheetNode {
    let key = normalised_key(&path);
    let leaf = |kind, detail: Option<String>| SheetNode {
        name: name.clone(),
        filename: filename.clone(),
        path: path.clone(),
        kind,
        detail,
        children: Vec::new(),
    };
    if ancestors.contains(&key) {
        return leaf(SheetNodeKind::Cycle, None);
    }
    let Some(sheet) = lookup.sheets.get(&key) else {
        return match lookup.unreadable.get(&key) {
            Some(why) => leaf(SheetNodeKind::Unreadable, Some((*why).to_string())),
            None => leaf(SheetNodeKind::Missing, None),
        };
    };
    if *budget == 0 || depth >= MAX_DEPTH {
        return leaf(SheetNodeKind::Truncated, None);
    }
    *budget -= 1;

    let dir = path.parent().unwrap_or_else(|| Path::new("")).to_path_buf();
    ancestors.push(key);
    let children = sheet
        .child_sheets
        .iter()
        .map(|cs| match resolve_child_reference(&dir, &cs.filename) {
            Some(resolved) => expand(
                lookup,
                cs.name.clone(),
                cs.filename.clone(),
                resolved,
                depth + 1,
                ancestors,
                budget,
            ),
            None => SheetNode {
                name: cs.name.clone(),
                filename: cs.filename.clone(),
                path: dir.join(&cs.filename),
                kind: SheetNodeKind::Rejected,
                detail: None,
                children: Vec::new(),
            },
        })
        .collect();
    ancestors.pop();

    SheetNode {
        name,
        filename,
        path,
        kind: SheetNodeKind::Present,
        detail: None,
        children,
    }
}

/// The sheet that references `child`, when it has one.
///
/// A file instantiated from several parents has several; the answer is the
/// one whose path sorts first, so "ascend" is repeatable instead of depending
/// on hash-map iteration order. `None` for the root (nothing references it)
/// and for a sheet no loaded sheet references. References are resolved with
/// the shared [`resolve_child_reference`], so a refused reference never makes
/// a sheet anyone's parent.
pub(crate) fn parent_sheet(
    sheets: &HashMap<PathBuf, SchematicSheet>,
    child: &Path,
) -> Option<PathBuf> {
    let child_key = normalised_key(child);
    let mut candidates: Vec<(&PathBuf, &SchematicSheet)> = sheets.iter().collect();
    candidates.sort_by_key(|(path, _)| path_key(path));
    candidates
        .into_iter()
        .filter(|(path, _)| normalised_key(path) != child_key)
        .find(|(path, sheet)| {
            let dir = path.parent().unwrap_or_else(|| Path::new(""));
            sheet.child_sheets.iter().any(|cs| {
                resolve_child_reference(dir, &cs.filename)
                    .is_some_and(|resolved| normalised_key(&resolved) == child_key)
            })
        })
        .map(|(path, _)| path.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::{ChildSheet, FillType, Point};
    use uuid::Uuid;

    fn child(name: &str, filename: &str) -> ChildSheet {
        ChildSheet {
            uuid: Uuid::nil(),
            name: name.to_string(),
            filename: filename.to_string(),
            position: Point::ZERO,
            size: (0.0, 0.0),
            stroke_width: 0.0,
            fill: FillType::None,
            stroke_color: None,
            fill_color: None,
            fields_autoplaced: false,
            pins: Vec::new(),
            instances: Vec::new(),
        }
    }

    fn sheet(children: &[(&str, &str)]) -> SchematicSheet {
        SchematicSheet {
            uuid: Uuid::nil(),
            version: 0,
            generator: String::new(),
            generator_version: String::new(),
            paper_size: String::new(),
            root_sheet_page: String::new(),
            symbols: Vec::new(),
            wires: Vec::new(),
            junctions: Vec::new(),
            labels: Vec::new(),
            child_sheets: children.iter().map(|(n, f)| child(n, f)).collect(),
            no_connects: Vec::new(),
            text_notes: Vec::new(),
            buses: Vec::new(),
            bus_entries: Vec::new(),
            drawings: Vec::new(),
            no_erc_directives: Vec::new(),
            title_block: HashMap::new(),
            lib_symbols: HashMap::new(),
            unknown: Default::default(),
        }
    }

    fn build(sheets: &HashMap<PathBuf, SchematicSheet>, root: &Path) -> SheetNode {
        build_sheet_hierarchy(sheets, &[], root, &[])
            .root
            .expect("root")
    }

    fn p(name: &str) -> PathBuf {
        PathBuf::from("/proj").join(name)
    }

    /// root -> {power, mcu}; mcu -> {adc}. Three levels.
    fn three_level() -> HashMap<PathBuf, SchematicSheet> {
        let mut sheets = HashMap::new();
        sheets.insert(
            p("root.snxsch"),
            sheet(&[("Power", "power.snxsch"), ("MCU", "mcu.snxsch")]),
        );
        sheets.insert(p("power.snxsch"), sheet(&[]));
        sheets.insert(p("mcu.snxsch"), sheet(&[("ADC", "adc.snxsch")]));
        sheets.insert(p("adc.snxsch"), sheet(&[]));
        sheets
    }

    #[test]
    fn three_level_project_nests_root_child_grandchild() {
        let tree = build(&three_level(), &p("root.snxsch"));

        assert_eq!(tree.name, "root");
        assert_eq!(tree.kind, SheetNodeKind::Present);
        let names: Vec<_> = tree.children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Power", "MCU"]);
        let adc = tree.descend(&[1, 0]).expect("grandchild");
        assert_eq!(adc.name, "ADC");
        assert_eq!(adc.filename, "adc.snxsch");
        assert_eq!(adc.path, p("adc.snxsch"));
        assert_eq!(adc.kind, SheetNodeKind::Present);
    }

    #[test]
    fn reference_cycle_terminates_and_is_marked() {
        let mut sheets = three_level();
        // adc points back at the root: root -> mcu -> adc -> root.
        sheets.insert(p("adc.snxsch"), sheet(&[("Top", "root.snxsch")]));

        let tree = build(&sheets, &p("root.snxsch"));

        let back = tree.descend(&[1, 0, 0]).expect("the back-reference node");
        assert_eq!(back.kind, SheetNodeKind::Cycle);
        assert!(back.children.is_empty(), "a cycle node is not expanded");
    }

    #[test]
    fn self_reference_is_a_cycle_not_a_hang() {
        let mut sheets = HashMap::new();
        sheets.insert(p("root.snxsch"), sheet(&[("Me", "root.snxsch")]));

        let tree = build(&sheets, &p("root.snxsch"));

        assert_eq!(tree.children.len(), 1);
        assert_eq!(tree.children[0].kind, SheetNodeKind::Cycle);
    }

    #[test]
    fn a_file_instantiated_twice_appears_twice() {
        let mut sheets = HashMap::new();
        sheets.insert(
            p("root.snxsch"),
            sheet(&[("Ch1", "chan.snxsch"), ("Ch2", "chan.snxsch")]),
        );
        sheets.insert(p("chan.snxsch"), sheet(&[]));

        let tree = build(&sheets, &p("root.snxsch"));

        let names: Vec<_> = tree.children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Ch1", "Ch2"]);
        assert!(
            tree.children
                .iter()
                .all(|c| c.kind == SheetNodeKind::Present)
        );
    }

    #[test]
    fn a_missing_child_is_kept_with_a_marker() {
        let mut sheets = HashMap::new();
        sheets.insert(p("root.snxsch"), sheet(&[("Gone", "gone.snxsch")]));

        let tree = build(&sheets, &p("root.snxsch"));

        assert_eq!(tree.children.len(), 1);
        assert_eq!(tree.children[0].kind, SheetNodeKind::Missing);
        assert_eq!(tree.children[0].name, "Gone");
    }

    #[test]
    fn exponential_fan_out_is_bounded() {
        // Each level instantiates the next one twice: 2^30 occurrences.
        let mut sheets = HashMap::new();
        for level in 0..30 {
            let next = format!("l{}.snxsch", level + 1);
            sheets.insert(
                p(&format!("l{level}.snxsch")),
                sheet(&[("A", &next), ("B", &next)]),
            );
        }
        sheets.insert(p("l30.snxsch"), sheet(&[]));

        fn count(n: &SheetNode) -> usize {
            1 + n.children.iter().map(count).sum::<usize>()
        }
        let tree = build(&sheets, &p("l0.snxsch"));

        assert!(count(&tree) <= 2 * MAX_NODES + 2);
    }

    #[test]
    fn parent_is_the_referencing_sheet() {
        let sheets = three_level();
        assert_eq!(
            parent_sheet(&sheets, &p("adc.snxsch")),
            Some(p("mcu.snxsch"))
        );
        assert_eq!(
            parent_sheet(&sheets, &p("mcu.snxsch")),
            Some(p("root.snxsch"))
        );
        assert_eq!(parent_sheet(&sheets, &p("root.snxsch")), None);
    }

    #[test]
    fn multiple_parents_pick_the_first_by_path_every_time() {
        let mut sheets = HashMap::new();
        sheets.insert(p("b.snxsch"), sheet(&[("S", "shared.snxsch")]));
        sheets.insert(p("a.snxsch"), sheet(&[("S", "shared.snxsch")]));
        sheets.insert(p("shared.snxsch"), sheet(&[]));

        for _ in 0..8 {
            assert_eq!(
                parent_sheet(&sheets, &p("shared.snxsch")),
                Some(p("a.snxsch"))
            );
        }
    }

    #[test]
    fn a_dot_dot_reference_names_the_file_it_really_names() {
        let mut sheets = HashMap::new();
        sheets.insert(p("root.snxsch"), sheet(&[("X", "sub/../x.snxsch")]));
        sheets.insert(p("x.snxsch"), sheet(&[]));

        let tree = build(&sheets, &p("root.snxsch"));

        assert_eq!(tree.children[0].kind, SheetNodeKind::Present);
        assert_eq!(tree.children[0].path, p("x.snxsch"));
    }

    #[test]
    fn escaping_and_absolute_references_are_rejected_not_opened() {
        let mut sheets = HashMap::new();
        sheets.insert(
            p("root.snxsch"),
            sheet(&[("Up", "../x.snxsch"), ("Abs", "/elsewhere/x.snxsch")]),
        );

        let tree = build(&sheets, &p("root.snxsch"));

        assert_eq!(tree.children[0].kind, SheetNodeKind::Rejected);
        assert_eq!(tree.children[1].kind, SheetNodeKind::Rejected);
    }

    #[test]
    fn a_cycle_spelled_through_dot_dot_across_directories_is_marked() {
        // root -> sub/mid -> deep/../mid (= sub/mid again).
        let mut sheets = HashMap::new();
        sheets.insert(p("root.snxsch"), sheet(&[("Mid", "sub/mid.snxsch")]));
        sheets.insert(
            p("sub/mid.snxsch"),
            sheet(&[("Again", "deep/../mid.snxsch")]),
        );

        let tree = build(&sheets, &p("root.snxsch"));

        let again = tree.descend(&[0, 0]).expect("depth 2");
        assert_eq!(again.kind, SheetNodeKind::Cycle);
        assert!(again.children.is_empty());
    }

    #[test]
    fn a_child_pointing_back_out_of_its_folder_is_rejected_not_looped() {
        let mut sheets = HashMap::new();
        sheets.insert(p("root.snxsch"), sheet(&[("C", "sub/child.snxsch")]));
        sheets.insert(p("sub/child.snxsch"), sheet(&[("Top", "../root.snxsch")]));

        let tree = build(&sheets, &p("root.snxsch"));

        assert_eq!(
            tree.descend(&[0, 0]).map(|n| n.kind),
            Some(SheetNodeKind::Rejected)
        );
    }

    #[test]
    fn nesting_stops_at_the_depth_cap_with_a_truncated_row() {
        let mut sheets = HashMap::new();
        for level in 0..=MAX_DEPTH + 10 {
            let next = format!("l{}.snxsch", level + 1);
            sheets.insert(p(&format!("l{level}.snxsch")), sheet(&[("N", &next)]));
        }

        let tree = build(&sheets, &p("l0.snxsch"));

        let mut node = &tree;
        let mut depth = 0;
        while let Some(child) = node.children.first() {
            node = child;
            depth += 1;
        }
        assert_eq!(node.kind, SheetNodeKind::Truncated);
        assert_eq!(depth, MAX_DEPTH);
    }

    #[test]
    fn an_unreadable_child_keeps_its_reason() {
        let mut sheets = HashMap::new();
        sheets.insert(p("root.snxsch"), sheet(&[("Bad", "bad.snxsch")]));
        let unreadable = vec![(p("bad.snxsch"), "expected a sheet".to_string())];

        let tree = build_sheet_hierarchy(&sheets, &unreadable, &p("root.snxsch"), &[])
            .root
            .expect("root");

        assert_eq!(tree.children[0].kind, SheetNodeKind::Unreadable);
        assert_eq!(tree.children[0].detail.as_deref(), Some("expected a sheet"));
    }

    #[test]
    fn pages_outside_the_hierarchy_become_extra_top_level_rows() {
        let mut sheets = three_level();
        sheets.insert(p("extra.snxsch"), sheet(&[]));

        let h = build_sheet_hierarchy(&sheets, &[], &p("root.snxsch"), &[p("extra.snxsch")]);

        let names: Vec<_> = h.top_level().map(|n| n.name.as_str()).collect();
        assert_eq!(names, ["root", "extra"]);
        assert_eq!(
            h.node_at(&[1]).map(|n| n.kind),
            Some(SheetNodeKind::Present)
        );
        assert_eq!(h.node_at(&[0, 1, 0]).map(|n| n.name.as_str()), Some("ADC"));
    }

    #[test]
    fn parent_lookup_resolves_dot_dot_references() {
        let mut sheets = HashMap::new();
        sheets.insert(p("root.snxsch"), sheet(&[("X", "sub/../x.snxsch")]));
        sheets.insert(p("x.snxsch"), sheet(&[]));

        assert_eq!(
            parent_sheet(&sheets, &p("x.snxsch")),
            Some(p("root.snxsch"))
        );
    }
}
