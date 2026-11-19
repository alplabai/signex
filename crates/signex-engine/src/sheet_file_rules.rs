//! Whether a sheet file may be placed as a sheet symbol on a sheet (#1276,
//! #2199): the one rule set behind Create Sheet Symbol From Sheet in the GUI
//! and `create_sheet_symbol_from_sheet` / `place_sheet_symbol` from operands.
//!
//! The caller resolves paths and folds them to keys (the engine holds no
//! directory) and hands in what it read: the sheets around the edit as a
//! [`SheetGraph`], the sheets above the edited one and whether the file leaves
//! the project folder through a link. The rules here decide, in order: the
//! project folder, the sheet itself, a sheet above it, a loop at any depth, a
//! sheet of the file's subtree that cannot be read, and a file placed once
//! already. Rules: `docs/CLI.md`, "Placing parts and hierarchy".

use std::collections::{HashMap, HashSet};
use std::fmt;

use signex_types::schematic::{LabelType, SchematicSheet};

/// One sheet of a [`SheetGraph`], by its folded key.
#[derive(Debug, Clone, PartialEq)]
pub enum SheetGraphNode {
    /// The sheet was read (or is open).
    Read {
        /// The file name shown in a refusal.
        name: String,
        /// The keys of the files its sheet symbols place.
        children: Vec<String>,
        /// It carries a name other sheets see: a global label or a power
        /// port naming a net.
        reaching_names: bool,
        /// It is a sheet of the edited sheet's project.
        in_project: bool,
    },
    /// The sheet could not be read.
    Unreadable {
        /// The file name shown in a refusal.
        name: String,
        /// Why it could not be read.
        why: String,
    },
}

/// The sheets around an edit, by folded key.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SheetGraph {
    nodes: HashMap<String, SheetGraphNode>,
}

impl SheetGraph {
    /// Add or replace the sheet `key`; a later insert of a key wins.
    pub fn insert(&mut self, key: String, node: SheetGraphNode) {
        self.nodes.insert(key, node);
    }

    /// The sheet `key`, when known.
    pub fn get(&self, key: &str) -> Option<&SheetGraphNode> {
        self.nodes.get(key)
    }
}

/// A global label or a power port naming a net: a name other sheets see.
pub fn carries_reaching_name(sheet: &SchematicSheet) -> bool {
    sheet
        .labels
        .iter()
        .any(|l| matches!(l.label_type, LabelType::Global | LabelType::Power))
        || sheet
            .symbols
            .iter()
            .any(|s| s.is_power && !s.value.is_empty())
}

/// The walk down a file's child references.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SubtreeFacts {
    /// A sheet of the subtree places the edited sheet: a loop.
    pub reaches: bool,
    /// `<file> is unreadable (<why>)` for each sheet of it that cannot be read.
    pub unreadable: Vec<String>,
    /// A sheet of it carries a name that can reach other sheets.
    pub names: bool,
}

/// Walk down from `file` over `graph`; `edited` is the key of the sheet the
/// edit acts on. Sheets the graph does not know end the walk there.
pub fn subtree_facts(graph: &SheetGraph, file: &str, edited: &str) -> SubtreeFacts {
    let mut facts = SubtreeFacts::default();
    let mut seen = HashSet::new();
    let mut queue = vec![file.to_string()];
    while let Some(key) = queue.pop() {
        if !seen.insert(key.clone()) {
            continue;
        }
        match graph.get(&key) {
            Some(SheetGraphNode::Unreadable { name, why }) => {
                facts.unreadable.push(format!("{name} is unreadable ({why})"));
            }
            Some(SheetGraphNode::Read {
                children,
                reaching_names,
                ..
            }) => {
                facts.names |= *reaching_names;
                for child in children {
                    facts.reaches |= child == edited;
                    queue.push(child.clone());
                }
            }
            None => {}
        }
    }
    facts
}

/// What a sheet-symbol file is checked against.
#[derive(Debug, Clone, Copy)]
pub struct SheetFilePlacement<'a> {
    /// The key of the sheet the symbol lands on.
    pub edited: &'a str,
    /// The key of the file the symbol places.
    pub file: &'a str,
    /// The file leaves the project folder through a link.
    pub escapes_project: bool,
    /// The keys of the sheets above the edited one, however far up.
    pub above: &'a HashSet<String>,
    /// The sheets around the edit, the file's subtree among them.
    pub graph: &'a SheetGraph,
    /// The keys of the project's pages; `None` for a loose sheet, which has
    /// no placed-once rule.
    pub project_pages: Option<&'a HashSet<String>>,
    /// Place the file even when its subtree cannot be read or it is placed
    /// already (`allow_merge=true`).
    pub allow_merge: bool,
}

/// Why a sheet-symbol file is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SheetFileRefusal {
    /// It leaves the project folder through a link.
    EscapesProject,
    /// It is the edited sheet.
    IsEditedSheet,
    /// It is a sheet above the edited one.
    IsAncestor,
    /// A sheet of its subtree places the edited sheet.
    PlacesEditedSheet,
    /// Sheets of its subtree cannot be read (`<file> is unreadable (<why>)`).
    Unreadable(Vec<String>),
    /// It is a page of the project.
    ProjectPage,
    /// A sheet symbol on the sheet named here places it already.
    PlacedOn(String),
}

impl fmt::Display for SheetFileRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const AGAIN: &str =
            "placing it again changes its nets; pass allow_merge=true to place it anyway";
        match self {
            Self::EscapesProject => f.write_str(
                "`file`: the sheet must be inside the project folder (a linked folder leaves it)",
            ),
            Self::IsEditedSheet => {
                f.write_str("`file` is this sheet: a sheet cannot contain itself")
            }
            Self::IsAncestor => {
                f.write_str("`file` is an ancestor of this sheet: a sheet cannot contain itself")
            }
            Self::PlacesEditedSheet => {
                f.write_str("`file` places this sheet below it: a sheet cannot contain itself")
            }
            Self::Unreadable(why) => write!(
                f,
                "`file`: {}; its names and the sheets it places are unknown; pass allow_merge=true to place it anyway",
                why.join("; ")
            ),
            Self::ProjectPage => write!(f, "`file` is a page of the project; {AGAIN}"),
            Self::PlacedOn(sheet) => write!(f, "`file` is already placed on {sheet}; {AGAIN}"),
        }
    }
}

/// Whether the file of `placement` may be placed: the subtree walk when it
/// may, else why not.
pub fn check_sheet_file(placement: &SheetFilePlacement<'_>) -> Result<SubtreeFacts, SheetFileRefusal> {
    let SheetFilePlacement {
        edited,
        file,
        graph,
        allow_merge,
        ..
    } = *placement;
    if placement.escapes_project {
        return Err(SheetFileRefusal::EscapesProject);
    }
    if file == edited {
        return Err(SheetFileRefusal::IsEditedSheet);
    }
    if placement.above.contains(file) {
        return Err(SheetFileRefusal::IsAncestor);
    }
    let facts = subtree_facts(graph, file, edited);
    if facts.reaches {
        return Err(SheetFileRefusal::PlacesEditedSheet);
    }
    if allow_merge {
        return Ok(facts);
    }
    if !facts.unreadable.is_empty() {
        return Err(SheetFileRefusal::Unreadable(facts.unreadable));
    }
    if let Some(pages) = placement.project_pages {
        refuse_placed_file(graph, edited, file, pages)?;
    }
    Ok(facts)
}

/// A `file` that is a page of the project, or that a sheet symbol on another
/// sheet of it places already: placing it here too changes its nets (its
/// local names become this placement's). The first such sheet by name is
/// named, so the refusal does not depend on map order.
fn refuse_placed_file(
    graph: &SheetGraph,
    edited: &str,
    file: &str,
    pages: &HashSet<String>,
) -> Result<(), SheetFileRefusal> {
    if pages.contains(file) {
        return Err(SheetFileRefusal::ProjectPage);
    }
    let placed_on = graph
        .nodes
        .iter()
        .filter(|(key, _)| key.as_str() != edited)
        .filter_map(|(_, node)| match node {
            SheetGraphNode::Read {
                name,
                children,
                in_project: true,
                ..
            } if children.iter().any(|child| child == file) => Some(name.clone()),
            _ => None,
        })
        .min();
    match placed_on {
        Some(name) => Err(SheetFileRefusal::PlacedOn(name)),
        None => Ok(()),
    }
}
