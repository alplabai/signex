//! The project's sheet hierarchy as a tree of *occurrences*, built from the
//! sheet map [`assemble_project_sheets`] already produced.
//!
//! This is a read-only view over that map — it adds no loading and no
//! connectivity logic. It mirrors the per-occurrence model the netlist
//! stitcher uses (`signex-net/src/project/mod.rs`): a file instantiated N
//! times is N nodes, never collapsed into one, because the electrical model
//! keeps the instances distinct.
//!
//! Two failure shapes are rendered rather than hidden:
//! - a referenced child absent from the map is a [`SheetNodeKind::Missing`]
//!   node, so a broken reference does not make a sheet vanish;
//! - a reference back to one of its own ancestors is a
//!   [`SheetNodeKind::Cycle`] node with no children, so a cyclic project can
//!   neither hang the UI nor recurse forever.
//!
//! [`assemble_project_sheets`]: crate::app::project_sheets::assemble_project_sheets

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use signex_types::schematic::SchematicSheet;

use crate::app::state::scope::path_key;

/// Upper bound on nodes in one built tree. A shared sub-sheet instantiated
/// twice at every level of a deep hierarchy doubles per level; without a bound
/// that is a legitimate (acyclic) input that still freezes the panel.
const MAX_NODES: usize = 5_000;

/// What a node stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetNodeKind {
    /// The sheet is loaded; its children follow.
    Present,
    /// Referenced, but not in the assembled sheet map (absent or unreadable).
    Missing,
    /// References one of its own ancestors; expansion stops here.
    Cycle,
    /// The [`MAX_NODES`] budget ran out before this occurrence was expanded.
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
    /// Where the sheet lives (parent directory joined with `filename`) — the
    /// key into the assembled sheet map.
    pub path: PathBuf,
    pub kind: SheetNodeKind,
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

/// Build the occurrence tree rooted at `root` over `sheets`.
pub(crate) fn build_sheet_hierarchy(
    sheets: &HashMap<PathBuf, SchematicSheet>,
    root: &Path,
) -> SheetNode {
    let filename = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let name = root
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| filename.clone());
    let mut budget = MAX_NODES;
    let mut ancestors = Vec::new();
    expand(
        sheets,
        name,
        filename,
        root.to_path_buf(),
        &mut ancestors,
        &mut budget,
    )
}

fn expand(
    sheets: &HashMap<PathBuf, SchematicSheet>,
    name: String,
    filename: String,
    path: PathBuf,
    ancestors: &mut Vec<String>,
    budget: &mut usize,
) -> SheetNode {
    let key = path_key(&path);
    let leaf = |kind| SheetNode {
        name: name.clone(),
        filename: filename.clone(),
        path: path.clone(),
        kind,
        children: Vec::new(),
    };
    if ancestors.contains(&key) {
        return leaf(SheetNodeKind::Cycle);
    }
    let Some(sheet) = sheets.get(&path) else {
        return leaf(SheetNodeKind::Missing);
    };
    if *budget == 0 {
        return leaf(SheetNodeKind::Truncated);
    }
    *budget -= 1;

    let dir = path.parent().unwrap_or_else(|| Path::new("")).to_path_buf();
    ancestors.push(key);
    let children = sheet
        .child_sheets
        .iter()
        .map(|cs| {
            expand(
                sheets,
                cs.name.clone(),
                cs.filename.clone(),
                dir.join(&cs.filename),
                ancestors,
                budget,
            )
        })
        .collect();
    ancestors.pop();

    SheetNode {
        name,
        filename,
        path,
        kind: SheetNodeKind::Present,
        children,
    }
}

/// The sheet that references `child`, when it has one.
///
/// A file instantiated from several parents has several; the answer is the
/// one whose path sorts first, so "ascend" is repeatable instead of depending
/// on hash-map iteration order. `None` for the root (nothing references it)
/// and for a sheet no loaded sheet references.
pub(crate) fn parent_sheet(
    sheets: &HashMap<PathBuf, SchematicSheet>,
    child: &Path,
) -> Option<PathBuf> {
    let child_key = path_key(child);
    let mut candidates: Vec<(&PathBuf, &SchematicSheet)> = sheets.iter().collect();
    candidates.sort_by_key(|(path, _)| path_key(path));
    candidates
        .into_iter()
        .filter(|(path, _)| path_key(path) != child_key)
        .find(|(path, sheet)| {
            let dir = path.parent().unwrap_or_else(|| Path::new(""));
            sheet
                .child_sheets
                .iter()
                .any(|cs| path_key(&dir.join(&cs.filename)) == child_key)
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
        }
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
        let tree = build_sheet_hierarchy(&three_level(), &p("root.snxsch"));

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

        let tree = build_sheet_hierarchy(&sheets, &p("root.snxsch"));

        let back = tree.descend(&[1, 0, 0]).expect("the back-reference node");
        assert_eq!(back.kind, SheetNodeKind::Cycle);
        assert!(back.children.is_empty(), "a cycle node is not expanded");
    }

    #[test]
    fn self_reference_is_a_cycle_not_a_hang() {
        let mut sheets = HashMap::new();
        sheets.insert(p("root.snxsch"), sheet(&[("Me", "root.snxsch")]));

        let tree = build_sheet_hierarchy(&sheets, &p("root.snxsch"));

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

        let tree = build_sheet_hierarchy(&sheets, &p("root.snxsch"));

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

        let tree = build_sheet_hierarchy(&sheets, &p("root.snxsch"));

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
        let tree = build_sheet_hierarchy(&sheets, &p("l0.snxsch"));

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
}
