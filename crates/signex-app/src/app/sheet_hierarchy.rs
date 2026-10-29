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

use signex_types::schematic::{InstancePath, SchematicSheet};
use uuid::Uuid;

use crate::app::project_sheets::{lexically_normalize, resolve_child_reference};
pub(crate) use crate::app::state::scope::CaseFold;
use crate::app::state::scope::path_key_with;

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
    /// `ChildSheet.uuid` of the referencing symbol: the step this occurrence
    /// adds to an instance path. `None` for a page the walk starts at.
    pub placed_as: Option<Uuid>,
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
    /// Whether this occurrence is the sheet stored at `path`, compared under
    /// the project's `fold` like every other sheet-path comparison.
    pub fn is_at(&self, path: &Path, fold: CaseFold) -> bool {
        path_key_with(&self.path, fold) == path_key_with(path, fold)
    }

    /// The node reached by following child indices from this one.
    pub fn descend(&self, indices: &[usize]) -> Option<&SheetNode> {
        indices
            .iter()
            .try_fold(self, |node, &i| node.children.get(i))
    }
}

/// The identity two sheet paths are compared by under the project's `fold` —
/// exposed so the panel keys its row markers the same way.
pub(crate) fn sheet_path_key(path: &Path, fold: CaseFold) -> String {
    path_key_with(path, fold)
}

/// The whole Navigator tree: the root sheet's occurrences, followed by any
/// declared page the root does not reach (a flat project's other pages).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SheetHierarchy {
    /// The project's key rules the tree was built under; the panel keys its
    /// rows with the same ones.
    pub fold: CaseFold,
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

    /// The path of every node the walk placed, for the file-status checks
    /// that tell when a missing sheet appears or a present one goes (#1618).
    pub fn node_paths(&self) -> Vec<&Path> {
        fn collect<'a>(node: &'a SheetNode, out: &mut Vec<&'a Path>) {
            if node.kind != SheetNodeKind::Rejected {
                out.push(&node.path);
            }
            node.children.iter().for_each(|child| collect(child, out));
        }
        let mut out = Vec::new();
        self.top_level().for_each(|node| collect(node, &mut out));
        out
    }

    /// Whether `path` is a page the walk starts at (the root or a loose page)
    /// rather than a sheet some symbol places.
    pub fn starts_a_walk(&self, path: &Path) -> bool {
        self.top_level().any(|node| node.is_at(path, self.fold))
    }

    /// Every placement through which the walk reaches the sheet at `path`,
    /// as instance paths from the top-level page: the root chain for a page
    /// the walk starts at. Empty when the walk does not reach it.
    pub fn placements_of(&self, path: &Path) -> Vec<InstancePath> {
        fn walk(
            node: &SheetNode,
            chain: &mut Vec<Uuid>,
            path: &Path,
            fold: CaseFold,
            out: &mut Vec<InstancePath>,
        ) {
            chain.extend(node.placed_as);
            // A Cycle node points back at an ancestor and a Rejected one is
            // never opened: neither is another placement of the sheet.
            let a_placement = !matches!(node.kind, SheetNodeKind::Cycle | SheetNodeKind::Rejected);
            if a_placement && node.is_at(path, fold) {
                out.push(InstancePath::from_chain(chain.clone()));
            }
            for child in &node.children {
                walk(child, chain, path, fold, out);
            }
            if node.placed_as.is_some() {
                chain.pop();
            }
        }
        let mut out = Vec::new();
        for node in self.top_level() {
            walk(node, &mut Vec::new(), path, self.fold, &mut out);
        }
        out
    }

    /// Whether the walk saw the whole hierarchy: every node is a loaded
    /// sheet, none Missing, Unreadable, Rejected, a Cycle or Truncated. Only
    /// then is [`placements_of`](Self::placements_of) every placement there
    /// is (within this project).
    pub fn is_complete(&self) -> bool {
        fn complete(node: &SheetNode) -> bool {
            node.kind == SheetNodeKind::Present && node.children.iter().all(complete)
        }
        self.top_level().all(complete)
    }

    /// The designators a search reads at `path`: every placement the walk
    /// reaches the sheet through, and whether that list is certain to be
    /// complete (so a designator edit may write `reference` alone).
    pub fn designator_scope(&self, path: &Path) -> signex_engine::DesignatorScope {
        let placements = self.placements_of(path);
        let single = self.is_complete() && placements.len() <= 1;
        signex_engine::DesignatorScope { placements, single }
    }

    /// Every loaded sheet occurrence with the placement that reaches it,
    /// walk order: a child placed twice is two entries, a hierarchical child
    /// no page list names is one.
    pub fn occurrences(&self) -> Vec<(InstancePath, &SheetNode)> {
        fn walk<'a>(
            node: &'a SheetNode,
            chain: &mut Vec<Uuid>,
            out: &mut Vec<(InstancePath, &'a SheetNode)>,
        ) {
            chain.extend(node.placed_as);
            if node.kind == SheetNodeKind::Present {
                out.push((InstancePath::from_chain(chain.clone()), node));
            }
            for child in &node.children {
                walk(child, chain, out);
            }
            if node.placed_as.is_some() {
                chain.pop();
            }
        }
        let mut out = Vec::new();
        for node in self.top_level() {
            walk(node, &mut Vec::new(), &mut out);
        }
        out
    }

    /// The sheet file an occurrence path stands for. The root chain names the
    /// root sheet (loose pages share it and are not reachable this way).
    pub fn sheet_path_at(&self, occurrence: &InstancePath) -> Option<PathBuf> {
        self.occurrences()
            .into_iter()
            .find(|(chain, _)| chain == occurrence)
            .map(|(_, node)| node.path.clone())
    }

    /// The placement a tree-row path stands for: the sheet-symbol uuids
    /// walked from the top-level page down to that row. A top-level row is a
    /// page the walk starts at, so it is the root chain.
    pub fn placement_at(&self, tree_path: &[usize]) -> Option<InstancePath> {
        let (&first, rest) = tree_path.split_first()?;
        let mut node = self.top_level().nth(first)?;
        let mut chain = Vec::new();
        for &index in rest {
            node = node.children.get(index)?;
            chain.extend(node.placed_as);
        }
        Some(InstancePath::from_chain(chain))
    }
}

/// What the builder looks sheets up in, keyed on the normalised path.
struct Lookup<'a> {
    fold: CaseFold,
    sheets: HashMap<String, &'a SchematicSheet>,
    unreadable: HashMap<String, &'a str>,
}

fn normalised_key(path: &Path, fold: CaseFold) -> String {
    path_key_with(&lexically_normalize(path), fold)
}

impl<'a> Lookup<'a> {
    fn new(
        sheets: &'a HashMap<PathBuf, SchematicSheet>,
        unreadable: &'a [(PathBuf, String)],
        fold: CaseFold,
    ) -> Self {
        let mut by_key: HashMap<String, &SchematicSheet> = HashMap::new();
        // Entries already stored under their canonical spelling win over an
        // alias of the same file, so the choice does not depend on map order.
        for (path, sheet) in sheets {
            if lexically_normalize(path) == *path {
                by_key.insert(normalised_key(path, fold), sheet);
            }
        }
        for (path, sheet) in sheets {
            by_key.entry(normalised_key(path, fold)).or_insert(sheet);
        }
        let unreadable = unreadable
            .iter()
            .map(|(path, why)| (normalised_key(path, fold), why.as_str()))
            .collect();
        Self {
            fold,
            sheets: by_key,
            unreadable,
        }
    }
}

/// Build the occurrence tree rooted at `root` over `sheets`, plus a subtree
/// for every page in `loose_pages`, all keyed under the project's `fold`.
pub(crate) fn build_sheet_hierarchy(
    sheets: &HashMap<PathBuf, SchematicSheet>,
    unreadable: &[(PathBuf, String)],
    root: &Path,
    loose_pages: &[PathBuf],
    fold: CaseFold,
) -> SheetHierarchy {
    let lookup = Lookup::new(sheets, unreadable, fold);
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
            Reference {
                name,
                filename,
                placed_as: None,
            },
            path.to_path_buf(),
            0,
            &mut Vec::new(),
            &mut budget,
        )
    };
    SheetHierarchy {
        fold,
        root: Some(build(root)),
        loose: loose_pages.iter().map(|page| build(page)).collect(),
    }
}

/// How a sheet is referenced: the symbol's name and file text, and the symbol
/// itself (`None` for a page the walk starts at).
struct Reference {
    name: String,
    filename: String,
    placed_as: Option<Uuid>,
}

fn expand(
    lookup: &Lookup<'_>,
    reference: Reference,
    path: PathBuf,
    depth: usize,
    ancestors: &mut Vec<String>,
    budget: &mut usize,
) -> SheetNode {
    let Reference {
        name,
        filename,
        placed_as,
    } = reference;
    let key = normalised_key(&path, lookup.fold);
    let leaf = |kind, detail: Option<String>| SheetNode {
        name: name.clone(),
        filename: filename.clone(),
        placed_as,
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
                Reference {
                    name: cs.name.clone(),
                    filename: cs.filename.clone(),
                    placed_as: Some(cs.uuid),
                },
                resolved,
                depth + 1,
                ancestors,
                budget,
            ),
            None => SheetNode {
                name: cs.name.clone(),
                filename: cs.filename.clone(),
                placed_as: Some(cs.uuid),
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
        placed_as,
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
    fold: CaseFold,
) -> Option<PathBuf> {
    let child_key = normalised_key(child, fold);
    let mut candidates: Vec<(&PathBuf, &SchematicSheet)> = sheets.iter().collect();
    candidates.sort_by_key(|(path, _)| path_key_with(path, fold));
    candidates
        .into_iter()
        .filter(|(path, _)| normalised_key(path, fold) != child_key)
        .find(|(path, sheet)| {
            let dir = path.parent().unwrap_or_else(|| Path::new(""));
            sheet.child_sheets.iter().any(|cs| {
                resolve_child_reference(dir, &cs.filename)
                    .is_some_and(|resolved| normalised_key(&resolved, fold) == child_key)
            })
        })
        .map(|(path, _)| path.clone())
}

/// The sheet whose symbol `placed_as` is, when exactly one holds it. A copied
/// parent file carries the same symbol uuids as the original, so a uuid held by
/// several sheets names no particular parent: `None`, and the caller falls back
/// to [`parent_sheet`] without claiming a placement.
pub(crate) fn parent_sheet_placing(
    sheets: &HashMap<PathBuf, SchematicSheet>,
    placed_as: Uuid,
) -> Option<PathBuf> {
    let mut holders = sheets
        .iter()
        .filter(|(_, sheet)| sheet.child_sheets.iter().any(|cs| cs.uuid == placed_as))
        .map(|(path, _)| path);
    let only = holders.next()?;
    holders.next().is_none().then(|| only.clone())
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
            root_sheet_page: String::new(),
            child_sheets: children.iter().map(|(n, f)| child(n, f)).collect(),
            ..Default::default()
        }
    }

    fn build(sheets: &HashMap<PathBuf, SchematicSheet>, root: &Path) -> SheetNode {
        build_sheet_hierarchy(sheets, &[], root, &[], CaseFold::host_default())
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
    fn a_symbol_uuid_held_by_two_sheets_names_no_parent() {
        let mut sheets = HashMap::new();
        sheets.insert(p("a"), sheet(&[("X", "x.snxsch")]));
        let held = sheets[&p("a")].child_sheets[0].uuid;
        assert_eq!(parent_sheet_placing(&sheets, held), Some(p("a")));
        sheets.insert(p("copy"), sheets[&p("a")].clone());
        assert_eq!(parent_sheet_placing(&sheets, held), None);
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
            parent_sheet(&sheets, &p("adc.snxsch"), CaseFold::host_default()),
            Some(p("mcu.snxsch"))
        );
        assert_eq!(
            parent_sheet(&sheets, &p("mcu.snxsch"), CaseFold::host_default()),
            Some(p("root.snxsch"))
        );
        assert_eq!(
            parent_sheet(&sheets, &p("root.snxsch"), CaseFold::host_default()),
            None
        );
    }

    #[test]
    fn multiple_parents_pick_the_first_by_path_every_time() {
        let mut sheets = HashMap::new();
        sheets.insert(p("b.snxsch"), sheet(&[("S", "shared.snxsch")]));
        sheets.insert(p("a.snxsch"), sheet(&[("S", "shared.snxsch")]));
        sheets.insert(p("shared.snxsch"), sheet(&[]));

        for _ in 0..8 {
            assert_eq!(
                parent_sheet(&sheets, &p("shared.snxsch"), CaseFold::host_default()),
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

        let tree = build_sheet_hierarchy(
            &sheets,
            &unreadable,
            &p("root.snxsch"),
            &[],
            CaseFold::host_default(),
        )
        .root
        .expect("root");

        assert_eq!(tree.children[0].kind, SheetNodeKind::Unreadable);
        assert_eq!(tree.children[0].detail.as_deref(), Some("expected a sheet"));
    }

    #[test]
    fn pages_outside_the_hierarchy_become_extra_top_level_rows() {
        let mut sheets = three_level();
        sheets.insert(p("extra.snxsch"), sheet(&[]));

        let h = build_sheet_hierarchy(
            &sheets,
            &[],
            &p("root.snxsch"),
            &[p("extra.snxsch")],
            CaseFold::host_default(),
        );

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
            parent_sheet(&sheets, &p("x.snxsch"), CaseFold::host_default()),
            Some(p("root.snxsch"))
        );
    }

    #[test]
    fn an_apfs_style_fold_finds_a_child_referenced_in_another_case() {
        // One file, loaded as `child.snxsch`, referenced as `Child.snxsch`.
        let mut sheets = HashMap::new();
        sheets.insert(p("root.snxsch"), sheet(&[("C", "Child.snxsch")]));
        sheets.insert(p("child.snxsch"), sheet(&[]));
        let kind_under = |fold| {
            build_sheet_hierarchy(&sheets, &[], &p("root.snxsch"), &[], fold)
                .root
                .expect("root")
                .children[0]
                .kind
        };

        assert_eq!(
            kind_under(CaseFold::from_probe(true)),
            SheetNodeKind::Present,
            "the netlist stitches this sheet, so the Navigator must too"
        );
        assert_eq!(
            kind_under(CaseFold::from_probe(false)),
            SheetNodeKind::Missing,
            "a case-sensitive volume has no such file"
        );
    }

    #[test]
    fn a_case_sensitive_fold_keeps_two_case_variant_files_apart() {
        let mut sheets = HashMap::new();
        sheets.insert(
            p("root.snxsch"),
            sheet(&[("U", "A.snxsch"), ("L", "a.snxsch")]),
        );
        sheets.insert(p("A.snxsch"), sheet(&[("X", "x.snxsch")]));
        sheets.insert(p("a.snxsch"), sheet(&[]));
        sheets.insert(p("x.snxsch"), sheet(&[]));

        let root = build_sheet_hierarchy(
            &sheets,
            &[],
            &p("root.snxsch"),
            &[],
            CaseFold::from_probe(false),
        )
        .root
        .expect("root");

        assert_eq!(root.children[0].children.len(), 1, "A.snxsch has its child");
        assert!(
            root.children[1].children.is_empty(),
            "a.snxsch is its own file, not A.snxsch's twin"
        );
    }
}
