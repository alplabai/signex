//! Shared assembly of the project's sheet view — [`assemble_project_sheets`]
//! answers "what sheets does this project consist of", and
//! [`project_children_map`] keys that answer the way
//! [`signex_net::build_project_netlist`] and ERC read it (ADR-0002 D8).
//!
//! There is exactly one assembler on purpose. Five operations ask this
//! question — the export scope, the cached canvas/ERC netlist, the ERC run,
//! annotate and the duplicate-designator reset — and each used to build its own
//! input set from slightly different rules. They disagreed: on the same state
//! at the same instant, one reported a `MissingChild` for a sheet another had
//! already stitched in (#406).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use signex_types::schematic::SchematicSheet;

use crate::app::state::DocumentState;
use crate::app::state::scope::path_key;

/// The sheets a project consists of, plus what could not be made sense of
/// while assembling them.
pub(crate) struct ProjectSheetSet {
    /// `absolute path → sheet`: the declared page set *plus* everything
    /// reachable from those pages and from the root down the `child_sheets`
    /// graph. Live engine snapshots for open tabs, disk parses for the rest.
    pub(crate) sheets: HashMap<PathBuf, SchematicSheet>,
    /// Declared pages that are **not** reachable from the root down
    /// `child_sheets`, and which the stitcher therefore never visits — a flat
    /// project's second page, a subtree whose parent reference was deleted.
    /// Sorted, so a caller's message is stable.
    pub(crate) pages_outside_the_hierarchy: Vec<PathBuf>,
    /// Sheets that EXIST but could not be turned into a `SchematicSheet`:
    /// `(path, why)`. Distinguished from an absent file so the user is not
    /// sent hunting for a corrupt sheet sitting right where it should be.
    pub(crate) unreadable: Vec<(PathBuf, String)>,
    /// The sheet this set was rooted at — what the netlist stitcher starts
    /// from. Carried here so no caller re-derives it and drifts.
    pub(crate) root: Option<PathBuf>,
}

impl ProjectSheetSet {
    /// No project in view (no active document).
    pub(crate) fn empty() -> Self {
        Self {
            sheets: HashMap::new(),
            pages_outside_the_hierarchy: Vec::new(),
            unreadable: Vec::new(),
            root: None,
        }
    }
}

/// The declared page paths of the project owning the active document, plus
/// [`assemble_project_sheets`] over them — rooted at that project's root sheet,
/// or at the active document itself when it belongs to no project.
///
/// The entry point every whole-project operation calls: the export scope, the
/// cached canvas/ERC netlist, the ERC run, annotate and the duplicate-designator
/// reset. Deriving the page list and the root here, once, is the point — each
/// of those used to do it slightly differently and they disagreed (#406).
pub(crate) fn assemble_active_project_sheets(
    document_state: &DocumentState,
) -> (Vec<PathBuf>, ProjectSheetSet) {
    let pages: Vec<PathBuf> = document_state
        .active_document_project()
        .map(|p| {
            let dir = p.dir();
            p.data
                .sheets
                .iter()
                .map(|s| dir.join(&s.filename))
                .collect()
        })
        .unwrap_or_default();
    let Some(active_path) = document_state.active_path.clone() else {
        return (pages, ProjectSheetSet::empty());
    };
    let root = document_state
        .active_document_project()
        .and_then(project_root_sheet_path)
        .unwrap_or(active_path);
    let set = assemble_project_sheets(document_state, &pages, &root);
    (pages, set)
}

/// Absolute path of a project's root schematic — its declared
/// `schematic_root`, falling back to the first entry in the sheet list.
pub(crate) fn project_root_sheet_path(
    project: &crate::app::state::LoadedProject,
) -> Option<PathBuf> {
    let filename = project
        .data
        .schematic_root
        .clone()
        .or_else(|| project.data.sheets.first().map(|s| s.filename.clone()))?;
    Some(project.dir().join(filename))
}

/// Read one sheet: the live engine snapshot when the file is open as a tab (so
/// unsaved edits are in the answer), a disk parse otherwise.
///
/// `Ok(None)` means genuinely absent; `Err` means present but unusable.
fn load_sheet(
    document_state: &DocumentState,
    path: &Path,
) -> Result<Option<SchematicSheet>, String> {
    if let Some(engine) = document_state.engines.get(path) {
        return Ok(Some(engine.document().clone()));
    }
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("could not be read: {e}")),
    };
    match signex_types::format::SnxSchematic::parse(&text) {
        Ok(parsed) => Ok(Some(parsed.sheet)),
        Err(e) => Err(format!("could not be parsed: {e}")),
    }
}

/// Every sheet this project consists of: the declared `pages` **union**
/// everything reachable from `root_path` and from those pages down the
/// `child_sheets` graph.
///
/// Neither half alone is the project. `pages` (a project's `sheets` list) are
/// what it *prints*: a hierarchical child is reached by a `child_sheets`
/// reference and is never added to that list — descending into one opens a tab
/// without registering it — so the pages alone drop whole subtrees and then
/// report a `MissingChild` for a sheet sitting in memory or right next to its
/// parent on disk. Reachability alone is no better: nothing requires a listed
/// page to be referenced as a child by anything (`project_navigation/add.rs`
/// appends unconditionally), so a flat project loses every page but the root.
///
/// Only paths reached from this project are loaded — an open tab belonging to
/// some *other* project never rides along. Cycle-safe on the visited set.
pub(crate) fn assemble_project_sheets(
    document_state: &DocumentState,
    pages: &[PathBuf],
    root_path: &Path,
) -> ProjectSheetSet {
    let mut set = ProjectSheetSet {
        sheets: HashMap::new(),
        pages_outside_the_hierarchy: Vec::new(),
        unreadable: Vec::new(),
        root: Some(root_path.to_path_buf()),
    };
    let mut visited: HashSet<String> = HashSet::new();

    // Walk the root's subtree first and remember what it covered: that — not
    // the assembled set — is what the netlist stitcher will actually traverse,
    // so it is the only honest answer to "is this page in the netlist?".
    let reachable = walk(
        document_state,
        &[root_path.to_path_buf()],
        &mut visited,
        &mut set,
    );
    // Then the declared pages and their own subtrees, so ERC and the children
    // map see the whole project even where the hierarchy does not connect it.
    walk(document_state, pages, &mut visited, &mut set);

    set.pages_outside_the_hierarchy = pages
        .iter()
        .filter(|p| !reachable.contains(&path_key(p)))
        .cloned()
        .collect();
    set.pages_outside_the_hierarchy.sort();
    set
}

/// Breadth-first load of `seeds` and their `child_sheets` descendants into
/// `set`. Returns the `path_key`s actually loaded by this walk.
fn walk(
    document_state: &DocumentState,
    seeds: &[PathBuf],
    visited: &mut HashSet<String>,
    set: &mut ProjectSheetSet,
) -> HashSet<String> {
    let mut loaded: HashSet<String> = HashSet::new();
    let mut queue: Vec<PathBuf> = seeds.to_vec();
    while let Some(path) = queue.pop() {
        let key = path_key(&path);
        if !visited.insert(key.clone()) {
            // Already handled by this walk or an earlier one. Still counts as
            // covered *by this walk* if it is in the set.
            if set.sheets.contains_key(&path) {
                loaded.insert(key);
            }
            continue;
        }
        let sheet = match load_sheet(document_state, &path) {
            Ok(Some(sheet)) => sheet,
            // Absent: the stitcher reports this as the MissingChild it is.
            Ok(None) => continue,
            Err(why) => {
                set.unreadable.push((path.clone(), why));
                continue;
            }
        };
        let dir = path.parent().unwrap_or_else(|| Path::new("")).to_path_buf();
        for child in &sheet.child_sheets {
            queue.push(dir.join(&child.filename));
        }
        loaded.insert(key);
        set.sheets.insert(path, sheet);
    }
    loaded
}

/// Build the project's child sheet-map keyed by the exact `ChildSheet.filename`
/// string each parent uses to reference a child — the key both
/// [`signex_net::build_project_netlist`] and ERC's `BadHierSheetPin` look
/// children up by.
///
/// Each parent's `ChildSheet.filename` is resolved against that parent's own
/// directory to locate the child in `sheets` (the app's already-loaded
/// `path → sheet` set: live engine snapshots for open tabs, disk parses for the
/// rest). Keying on the reference string — not the bare basename the app used to
/// strip to — is what stops two sheets that share a basename in different
/// directories from silently overwriting each other.
///
/// Parents are visited in path order. Two loaded parents in *different*
/// directories can still reference the same filename string, and the map has
/// one slot for it; iterating a `HashMap` there made the winner — i.e. the
/// connectivity written into a machine-consumed `.net` — depend on hash order.
/// First-wins by sorted path is arbitrary but at least the same every run.
pub(crate) fn project_children_map(
    sheets: &HashMap<PathBuf, SchematicSheet>,
) -> HashMap<String, SchematicSheet> {
    let mut children: HashMap<String, SchematicSheet> = HashMap::new();
    let mut parents: Vec<(&PathBuf, &SchematicSheet)> = sheets.iter().collect();
    parents.sort_by(|a, b| a.0.cmp(b.0));
    for (parent_path, sheet) in parents {
        let dir = parent_path.parent().unwrap_or_else(|| Path::new(""));
        for cs in &sheet.child_sheets {
            if children.contains_key(&cs.filename) {
                continue;
            }
            let Some(child_path) = resolve_child_reference(dir, &cs.filename) else {
                continue;
            };
            if let Some(child) = sheets.get(&child_path) {
                children.insert(cs.filename.clone(), child.clone());
            }
        }
    }
    children
}

/// Resolve a `ChildSheet.filename` reference against the directory of the sheet
/// that carries it — the single definition of child-reference resolution shared
/// by the project children-map assembly ([`project_children_map`]) and in-app
/// Open Child Sheet navigation (`resolve_child_sheet_path`). Sharing one
/// definition is what keeps navigation landing on the same file the netlist
/// stitched for a given reference, instead of the two agreeing only by
/// coincidence.
///
/// - An empty reference (after trimming) yields `None`.
/// - Both an absolute reference and a relative one are joined onto
///   `parent_dir` (`Path::join` already replaces `parent_dir` outright when the
///   reference is absolute, so one join covers both shapes) and the result is
///   lexically normalized — `.`/`..` resolved without touching the
///   filesystem, since the child file may not exist yet and
///   `std::fs::canonicalize` both requires existence and adds a `\\?\` prefix
///   on Windows.
/// - A reference whose normalized path escapes `parent_dir` — a `..`
///   traversal, or an absolute path elsewhere entirely — is rejected: logged
///   through the app's normal error-surfacing path and returned as `None`,
///   the same as a reference that does not resolve to a loaded sheet.
pub(crate) fn resolve_child_reference(parent_dir: &Path, child_filename: &str) -> Option<PathBuf> {
    let trimmed = child_filename.trim();
    if trimmed.is_empty() {
        return None;
    }
    let raw = PathBuf::from(trimmed);
    let resolved = lexically_normalize(&parent_dir.join(&raw));
    let root = lexically_normalize(parent_dir);

    if resolved.starts_with(&root) {
        Some(resolved)
    } else {
        crate::diagnostics::log_error(
            "Rejected child-sheet reference outside its resolution root",
            &anyhow::anyhow!(
                "'{trimmed}' resolves to {} which escapes {}",
                resolved.display(),
                root.display()
            ),
        );
        None
    }
}

/// Lexically normalize `path` — resolve `.` and `..` components without
/// touching the filesystem (unlike `std::fs::canonicalize`, which needs the
/// path to exist). A `..` with nothing left to pop is kept as a literal `..`
/// component, so a reference that escapes its root still fails the caller's
/// `starts_with(root)` check instead of silently collapsing onto the root.
fn lexically_normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => match out.components().next_back() {
                Some(std::path::Component::Normal(_)) => {
                    out.pop();
                }
                _ => out.push(".."),
            },
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// The filename string the root sheet is referenced by — its path relative to
/// `project_dir`, falling back to the bare basename. Passed to
/// [`signex_net::build_project_netlist`] as `root_filename` so a child that
/// re-references the root is reported as a cycle rather than recursed into.
pub(crate) fn root_reference_name(root_path: &Path, project_dir: Option<&Path>) -> Option<String> {
    project_dir
        .and_then(|dir| root_path.strip_prefix(dir).ok())
        .map(Path::to_path_buf)
        .or_else(|| root_path.file_name().map(PathBuf::from))
        .and_then(|p| p.to_str().map(str::to_string))
}

/// A one-line, user-facing message for a cross-sheet stitch issue (ADR-0002 D7,
/// part 3) — shown in the Messages panel alongside other diagnostics.
///
/// Lives here rather than in one consumer because *every* caller of
/// [`signex_net::build_project_netlist`] must surface its issues: the netlist
/// is always produced, so a dropped `MissingChild` means an exported netlist
/// that is quietly missing a whole subtree.
pub(crate) fn stitch_issue_message(issue: &signex_net::StitchIssue) -> String {
    use signex_net::StitchIssue as I;
    match issue {
        I::MissingChild {
            parent_path,
            sheet_name,
            filename,
        } => format!(
            "Netlist: sheet '{sheet_name}' on '{parent_path}' references a child '{filename}' that could not be found"
        ),
        I::SheetCycle {
            parent_path,
            filename,
        } => format!("Netlist: sheet cycle — '{parent_path}' re-enters '{filename}'"),
        I::DuplicateSheetUuid {
            filename_a,
            filename_b,
        } => format!(
            "Netlist: sheets '{filename_a}' and '{filename_b}' share a UUID (copied as a template?)"
        ),
        I::SharedReferenceAcrossInstances {
            filename,
            reference,
        } => format!(
            "Netlist: reference '{reference}' in '{filename}' is shared across sheet instances"
        ),
        I::NameCollision { name } => {
            format!(
                "Netlist: two distinct nets are both named '{name}'; the later one was suffixed"
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::{ChildSheet, FillType, Point};
    use uuid::Uuid;

    fn child_ref(filename: &str) -> ChildSheet {
        ChildSheet {
            uuid: Uuid::nil(),
            name: String::new(),
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

    fn sheet(uuid: u128, children: &[&str]) -> SchematicSheet {
        SchematicSheet {
            uuid: Uuid::from_u128(uuid),
            version: 0,
            generator: String::new(),
            generator_version: String::new(),
            paper_size: String::new(),
            root_sheet_page: String::new(),
            symbols: Vec::new(),
            wires: Vec::new(),
            junctions: Vec::new(),
            labels: Vec::new(),
            child_sheets: children.iter().map(|f| child_ref(f)).collect(),
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

    #[test]
    fn same_basename_children_in_different_dirs_do_not_collide() {
        // A root references two children that share a basename but live in
        // different directories, via distinct relative filenames. The old
        // basename key ("power.snxsch" for both) overwrote one; keying by the
        // reference string keeps both distinct.
        let mut sheets = HashMap::new();
        sheets.insert(
            PathBuf::from("/proj/root.snxsch"),
            sheet(1, &["a/power.snxsch", "b/power.snxsch"]),
        );
        sheets.insert(PathBuf::from("/proj/a/power.snxsch"), sheet(0xA, &[]));
        sheets.insert(PathBuf::from("/proj/b/power.snxsch"), sheet(0xB, &[]));

        let children = project_children_map(&sheets);

        assert_eq!(children.len(), 2, "both same-basename children present");
        assert_eq!(children["a/power.snxsch"].uuid, Uuid::from_u128(0xA));
        assert_eq!(children["b/power.snxsch"].uuid, Uuid::from_u128(0xB));
    }

    #[test]
    fn resolves_a_bare_child_reference_against_the_parent_dir() {
        // The common flat case: a root in /proj references "child.snxsch",
        // which lives beside it. Keyed by the bare reference string.
        let mut sheets = HashMap::new();
        sheets.insert(
            PathBuf::from("/proj/root.snxsch"),
            sheet(1, &["child.snxsch"]),
        );
        sheets.insert(PathBuf::from("/proj/child.snxsch"), sheet(2, &[]));

        let children = project_children_map(&sheets);

        assert_eq!(children.len(), 1);
        assert_eq!(children["child.snxsch"].uuid, Uuid::from_u128(2));
    }

    #[test]
    fn unreferenced_and_unloadable_sheets_are_absent() {
        // A reference whose file isn't in the loaded set is skipped (no panic,
        // no phantom entry); a sheet nobody references is not added either.
        let mut sheets = HashMap::new();
        sheets.insert(
            PathBuf::from("/proj/root.snxsch"),
            sheet(1, &["missing.snxsch"]),
        );
        sheets.insert(PathBuf::from("/proj/orphan.snxsch"), sheet(3, &[]));

        let children = project_children_map(&sheets);
        assert!(children.is_empty());
    }

    #[test]
    fn root_reference_name_is_relative_to_project_dir() {
        assert_eq!(
            root_reference_name(Path::new("/proj/sub/root.snxsch"), Some(Path::new("/proj"))),
            Some("sub/root.snxsch".to_string())
        );
        assert_eq!(
            root_reference_name(
                Path::new("/elsewhere/root.snxsch"),
                Some(Path::new("/proj"))
            ),
            Some("root.snxsch".to_string())
        );
    }

    // #339 — child-reference resolution shared by the children map and in-app
    // navigation. The bug: navigation used to resolve against the project dir,
    // so a grandchild referenced from a subdirectory sheet resolved beside the
    // `.snxprj` (wrong / missing file) while the netlist resolved it correctly
    // parent-relative. These pin the single shared helper both now use.

    #[test]
    fn grandchild_resolves_beside_its_parent_not_the_project_root() {
        // /proj/sub/mid.snxsch references "leaf.snxsch"; it must resolve to
        // /proj/sub/leaf.snxsch (beside mid), NOT /proj/leaf.snxsch.
        assert_eq!(
            resolve_child_reference(Path::new("/proj/sub"), "leaf.snxsch"),
            Some(PathBuf::from("/proj/sub/leaf.snxsch"))
        );
    }

    #[test]
    fn absolute_child_reference_outside_the_root_is_rejected() {
        // #463 — an absolute reference used to win outright, letting a
        // malicious `.snxsch`/`.snxprj` point a child sheet anywhere on disk.
        assert_eq!(
            resolve_child_reference(Path::new("/proj/sub"), "/elsewhere/x.snxsch"),
            None
        );
    }

    #[test]
    fn traversal_child_reference_outside_the_root_is_rejected() {
        // #463 — a relative `..` reference used to be joined onto `parent_dir`
        // verbatim with no containment check, walking out of the project.
        assert_eq!(
            resolve_child_reference(Path::new("/proj/sub"), "../../../etc/passwd"),
            None
        );
    }

    #[test]
    fn sibling_child_reference_still_resolves() {
        assert_eq!(
            resolve_child_reference(Path::new("/proj/sub"), "sibling.snxsch"),
            Some(PathBuf::from("/proj/sub/sibling.snxsch"))
        );
    }

    #[test]
    fn empty_child_reference_resolves_to_none() {
        assert_eq!(resolve_child_reference(Path::new("/proj"), "   "), None);
    }

    #[test]
    fn navigation_and_children_map_agree_on_the_same_reference() {
        // The children map stitches "leaf.snxsch" from a parent in /proj/sub to
        // the loaded /proj/sub/leaf.snxsch; the shared helper (what navigation
        // now uses) resolves the same reference against the same parent dir to
        // that exact path, so navigation opens the file the netlist stitched.
        let mut sheets = HashMap::new();
        sheets.insert(
            PathBuf::from("/proj/sub/mid.snxsch"),
            sheet(0x111, &["leaf.snxsch"]),
        );
        sheets.insert(PathBuf::from("/proj/sub/leaf.snxsch"), sheet(0x222, &[]));

        let children = project_children_map(&sheets);
        assert_eq!(children["leaf.snxsch"].uuid, Uuid::from_u128(0x222));

        let nav = resolve_child_reference(Path::new("/proj/sub"), "leaf.snxsch")
            .expect("relative reference resolves");
        assert_eq!(nav, PathBuf::from("/proj/sub/leaf.snxsch"));
        assert!(
            sheets.contains_key(&nav),
            "navigation lands on the loaded child, not a phantom sibling"
        );
    }
}
