//! Shared assembly of the project's sheet view — [`assemble_project_sheets`]
//! answers "what sheets does this project consist of", and [`project_graph`]
//! re-keys that answer by resolved path the way
//! [`signex_net::build_project_netlist`] and ERC read it (ADR-0002 D8, #466).
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

/// The sheet-walk order every whole-project Annotate operation must agree
/// on: `project_set.sheets` in sorted-path order, with `active_path`
/// appended at the end if it has a path the set doesn't already cover (an
/// active document the assembler can't place, e.g. one that hasn't been
/// saved anywhere the project reaches).
///
/// One function for the action (`handle_annotate`) and the preview list it
/// must match. #406 made them agree on *which* sheets to cover; they still
/// disagreed on the *order* to walk them in, which is what decides which
/// designator number each `?` symbol gets — a preview showing `R1` on sheet
/// A while the action hands sheet A `R2` (#435).
pub(crate) fn ordered_project_sheet_paths(
    project_set: &ProjectSheetSet,
    active_path: Option<&Path>,
) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = project_set.sheets.keys().cloned().collect();
    paths.sort();
    if let Some(active) = active_path
        && !project_set.sheets.contains_key(active)
    {
        paths.push(active.to_path_buf());
    }
    paths
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

/// The result of [`project_graph`] re-keying [`ProjectSheetSet::sheets`]
/// (`PathBuf -> SchematicSheet`) into [`signex_net::ProjectGraph`]'s `sheets`
/// / `resolved` inputs — plus what went wrong while re-keying, and the
/// reverse map back to a path for navigation / `unreadable` messages.
pub(crate) struct AssembledGraph {
    /// Every sheet, keyed by its [`signex_net::SheetKey`] (root included).
    pub(crate) sheets: HashMap<signex_net::SheetKey, SchematicSheet>,
    /// Per-parent resolution: `resolved[parent_key][cs.filename]` is the
    /// child's key — the shape [`signex_net::ProjectGraph::resolved`] and
    /// [`signex_erc::run_with_project`] both take.
    pub(crate) resolved: HashMap<signex_net::SheetKey, HashMap<String, signex_net::SheetKey>>,
    /// Structural problems found while assembling the graph.
    pub(crate) issues: Vec<signex_net::StitchIssue>,
    /// The path each key came from — the reverse of `sheets`' keying, for
    /// navigation and for turning a key back into a path in a message.
    pub(crate) key_to_path: HashMap<signex_net::SheetKey, PathBuf>,
}

/// The [`signex_net::SheetKey`] for `path` — its path relative to `base`,
/// normalized the way [`path_key`] normalizes an absolute path (separators to
/// `/`, case-folded on a case-insensitive host), falling back to the bare
/// basename when `path` does not live under `base` (or there is no `base` —
/// a loose document with no project). Generalizes the old `root_reference_name`
/// to any sheet, not just the root, and folds case so `A.snxsch` and
/// `a.snxsch` compare equal — the correctness #466 needs to key a project by
/// resolved path rather than by the bare reference string.
pub(crate) fn sheet_key(path: &Path, base: Option<&Path>) -> signex_net::SheetKey {
    let relative = base
        .and_then(|dir| path.strip_prefix(dir).ok())
        .map(Path::to_path_buf)
        .or_else(|| path.file_name().map(PathBuf::from))
        .unwrap_or_else(|| path.to_path_buf());
    path_key(&relative)
}

/// Re-key `sheets` (the app's `path → SchematicSheet` set) into the
/// [`signex_net::ProjectGraph`] shape — every sheet under its own
/// [`signex_net::SheetKey`], plus a per-parent `ChildSheet.filename ->
/// SheetKey` resolution map — so a cross-directory same-filename child
/// stitches from its own file instead of colliding with another parent's
/// (#466).
///
/// `base_dir` is the fixed base every key is made relative to: the project
/// directory when the sheet set belongs to one, else the root sheet's own
/// directory (a loose document still keys its children relative to itself).
///
/// Two distinct loaded paths can still collapse onto the same `SheetKey` — a
/// case-fold collision on a case-insensitive host (`A.snxsch` vs
/// `a.snxsch`). Sorted-path first-wins keeps that deterministic; the loser is
/// reported as [`signex_net::StitchIssue::AmbiguousChildFilename`] and its
/// subtree is absent from `sheets` without a trace otherwise.
pub(crate) fn project_graph(
    sheets: &HashMap<PathBuf, SchematicSheet>,
    base_dir: Option<&Path>,
) -> AssembledGraph {
    let mut keyed_sheets: HashMap<signex_net::SheetKey, SchematicSheet> = HashMap::new();
    let mut key_to_path: HashMap<signex_net::SheetKey, PathBuf> = HashMap::new();
    let mut issues: Vec<signex_net::StitchIssue> = Vec::new();

    let mut paths: Vec<&PathBuf> = sheets.keys().collect();
    paths.sort();
    for path in paths {
        let key = sheet_key(path, base_dir);
        if let Some(existing) = key_to_path.get(&key) {
            issues.push(signex_net::StitchIssue::AmbiguousChildFilename {
                filename: key.clone(),
                path_a: existing.display().to_string(),
                path_b: path.display().to_string(),
            });
            continue;
        }
        key_to_path.insert(key.clone(), path.clone());
        keyed_sheets.insert(key, sheets[path].clone());
    }

    // Built from `key_to_path` (the post-collision winners), not the raw
    // `sheets` input — a case-fold-collision loser must not clobber the
    // winner's resolution submap by sorting later here than it did above.
    let mut resolved: HashMap<signex_net::SheetKey, HashMap<String, signex_net::SheetKey>> =
        HashMap::new();
    let mut winners: Vec<(&signex_net::SheetKey, &PathBuf)> = key_to_path.iter().collect();
    winners.sort_by(|a, b| a.1.cmp(b.1));
    for (parent_key, parent_path) in winners {
        let sheet = &keyed_sheets[parent_key];
        let dir = parent_path.parent().unwrap_or_else(|| Path::new(""));
        let mut submap: HashMap<String, signex_net::SheetKey> = HashMap::new();
        for cs in &sheet.child_sheets {
            let Some(child_path) = resolve_child_reference(dir, &cs.filename) else {
                continue;
            };
            let child_key = sheet_key(&child_path, base_dir);
            if keyed_sheets.contains_key(&child_key) {
                submap.insert(cs.filename.clone(), child_key);
            }
        }
        resolved.insert(parent_key.clone(), submap);
    }

    AssembledGraph {
        sheets: keyed_sheets,
        resolved,
        issues,
        key_to_path,
    }
}

/// Resolve a `ChildSheet.filename` reference against the directory of the sheet
/// that carries it — the single definition of child-reference resolution shared
/// by the project graph assembly ([`project_graph`]) and in-app
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
/// - An empty `parent_dir` (an unsaved tab with no project loaded, or a bare
///   filename tab — `resolve_child_sheet_path`'s `unwrap_or_default()`) has
///   no real directory to contain anything against, so it is rejected
///   outright: `Path::starts_with` treats an empty path as a prefix of *any*
///   path (zero components to match), so checking containment against it is
///   vacuously true and would let an absolute or `..`-escaping reference
///   through unchecked (#463, re-opened at this exact boundary).
/// - Lexical normalization resolves `.`/`..` components only; it does not
///   touch the filesystem, so a symlink planted inside `parent_dir` that
///   points back out is not detected here. That residual is accepted: it
///   requires an attacker who can already write inside the project
///   directory.
pub(crate) fn resolve_child_reference(parent_dir: &Path, child_filename: &str) -> Option<PathBuf> {
    let trimmed = child_filename.trim();
    if trimmed.is_empty() {
        return None;
    }
    let raw = PathBuf::from(trimmed);
    let resolved = lexically_normalize(&parent_dir.join(&raw));
    let root = lexically_normalize(parent_dir);

    // A resolved path that leads with a literal ".." (nothing left to pop,
    // see `lexically_normalize`'s doc) or whose absoluteness disagrees with
    // root's has escaped upward past whatever root it started from — kept as
    // an explicit, independent check rather than trusting `starts_with`
    // alone a second time.
    let escapes_upward = matches!(
        resolved.components().next(),
        Some(std::path::Component::ParentDir)
    );
    let contained = !root.as_os_str().is_empty()
        && resolved.is_absolute() == root.is_absolute()
        && !escapes_upward
        && resolved.starts_with(&root);

    if contained {
        Some(resolved)
    } else {
        crate::diagnostics::log_error(
            "Rejected child-sheet reference outside its resolution root",
            &anyhow::anyhow!(
                "'{trimmed}' resolves to {} which escapes {}",
                resolved.display(),
                if root.as_os_str().is_empty() {
                    "an empty resolution root".to_string()
                } else {
                    root.display().to_string()
                }
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
        I::AmbiguousChildFilename {
            filename,
            path_a,
            path_b,
        } => format!(
            "Netlist: child reference '{filename}' resolves to two different files ('{path_a}' \
             and '{path_b}'); the netlist used '{path_a}'"
        ),
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
        // different directories, via distinct relative filenames. Keyed by
        // resolved path, both are distinct entries.
        let mut sheets = HashMap::new();
        sheets.insert(
            PathBuf::from("/proj/root.snxsch"),
            sheet(1, &["a/power.snxsch", "b/power.snxsch"]),
        );
        sheets.insert(PathBuf::from("/proj/a/power.snxsch"), sheet(0xA, &[]));
        sheets.insert(PathBuf::from("/proj/b/power.snxsch"), sheet(0xB, &[]));

        let graph = project_graph(&sheets, Some(Path::new("/proj")));

        assert_eq!(graph.sheets.len(), 3, "root + both same-basename children");
        assert_eq!(graph.sheets["a/power.snxsch"].uuid, Uuid::from_u128(0xA));
        assert_eq!(graph.sheets["b/power.snxsch"].uuid, Uuid::from_u128(0xB));
        assert!(
            graph.issues.is_empty(),
            "distinct resolved paths, no collision"
        );
        let root_key = sheet_key(Path::new("/proj/root.snxsch"), Some(Path::new("/proj")));
        assert_eq!(
            graph.resolved[&root_key]["a/power.snxsch"],
            "a/power.snxsch"
        );
        assert_eq!(
            graph.resolved[&root_key]["b/power.snxsch"],
            "b/power.snxsch"
        );
    }

    #[test]
    fn same_reference_string_across_different_parent_dirs_stitches_each_from_its_own_file() {
        // #466 — the bug this replaces: two parents in *different*
        // directories both reference a child by the SAME relative filename
        // string ("power.snxsch"). The old bare-filename map had one slot
        // for that string, so one parent's subtree was silently stitched
        // from the wrong file. Keyed by resolved path, each parent's OWN
        // submap resolves the string against its OWN directory, landing on
        // its OWN file — no ambiguity at all.
        let mut sheets = HashMap::new();
        sheets.insert(
            PathBuf::from("/proj/a/root.snxsch"),
            sheet(1, &["power.snxsch"]),
        );
        sheets.insert(
            PathBuf::from("/proj/b/root.snxsch"),
            sheet(2, &["power.snxsch"]),
        );
        sheets.insert(PathBuf::from("/proj/a/power.snxsch"), sheet(0xA, &[]));
        sheets.insert(PathBuf::from("/proj/b/power.snxsch"), sheet(0xB, &[]));

        let graph = project_graph(&sheets, Some(Path::new("/proj")));

        assert!(
            graph.issues.is_empty(),
            "each parent resolves its own file, no ambiguity: {:?}",
            graph.issues
        );
        let a_root_key = sheet_key(Path::new("/proj/a/root.snxsch"), Some(Path::new("/proj")));
        let b_root_key = sheet_key(Path::new("/proj/b/root.snxsch"), Some(Path::new("/proj")));
        let a_power_key = graph.resolved[&a_root_key]["power.snxsch"].clone();
        let b_power_key = graph.resolved[&b_root_key]["power.snxsch"].clone();
        assert_ne!(
            a_power_key, b_power_key,
            "the two parents resolve \"power.snxsch\" to two different children"
        );
        assert_eq!(graph.sheets[&a_power_key].uuid, Uuid::from_u128(0xA));
        assert_eq!(graph.sheets[&b_power_key].uuid, Uuid::from_u128(0xB));
    }

    #[test]
    fn a_diamond_sharing_one_child_file_is_not_a_collision() {
        // The safety-critical "do not cry wolf" case: two different parents in
        // the same directory both pull in ONE shared sub-sheet by the same
        // reference string, resolving to the SAME file. A legitimate diamond
        // (a shared power / decoupling sub-sheet) — the same file reached two
        // ways is NOT an ambiguity and must emit no issue.
        let mut sheets = HashMap::new();
        sheets.insert(
            PathBuf::from("/proj/a.snxsch"),
            sheet(1, &["shared.snxsch"]),
        );
        sheets.insert(
            PathBuf::from("/proj/b.snxsch"),
            sheet(2, &["shared.snxsch"]),
        );
        sheets.insert(PathBuf::from("/proj/shared.snxsch"), sheet(0xC, &[]));

        let graph = project_graph(&sheets, Some(Path::new("/proj")));

        let a_key = sheet_key(Path::new("/proj/a.snxsch"), Some(Path::new("/proj")));
        let b_key = sheet_key(Path::new("/proj/b.snxsch"), Some(Path::new("/proj")));
        let shared_key = sheet_key(Path::new("/proj/shared.snxsch"), Some(Path::new("/proj")));
        assert_eq!(graph.resolved[&a_key]["shared.snxsch"], shared_key);
        assert_eq!(graph.resolved[&b_key]["shared.snxsch"], shared_key);
        assert_eq!(graph.sheets[&shared_key].uuid, Uuid::from_u128(0xC));
        assert!(
            graph.issues.is_empty(),
            "the same file reached from two parents is not a collision: {:?}",
            graph.issues
        );
    }

    #[test]
    fn case_fold_collision_between_two_loaded_paths_is_the_remaining_ambiguity() {
        // The genuine ambiguity left after keying by resolved path: two
        // DISTINCT loaded paths that differ only in case collapse onto one
        // `SheetKey` on a case-insensitive host (Windows) — one sheet's
        // content silently wins the slot, so the caller must be told. Unix
        // paths are case-sensitive: no collision there.
        let mut sheets = HashMap::new();
        sheets.insert(PathBuf::from("/proj/A.snxsch"), sheet(0xA, &[]));
        sheets.insert(PathBuf::from("/proj/a.snxsch"), sheet(0xB, &[]));

        let graph = project_graph(&sheets, Some(Path::new("/proj")));

        if cfg!(windows) {
            assert_eq!(graph.sheets.len(), 1, "one key wins the collision");
            assert_eq!(graph.issues.len(), 1, "the collision must not be silent");
            assert!(matches!(
                &graph.issues[0],
                signex_net::StitchIssue::AmbiguousChildFilename { .. }
            ));
            // Sorted-path first-wins: "/proj/A.snxsch" < "/proj/a.snxsch".
            assert_eq!(
                graph.sheets.values().next().unwrap().uuid,
                Uuid::from_u128(0xA)
            );
        } else {
            assert_eq!(graph.sheets.len(), 2, "unix paths are case-sensitive");
            assert!(graph.issues.is_empty());
        }
    }

    #[test]
    fn resolves_a_bare_child_reference_against_the_parent_dir() {
        // The common flat case: a root in /proj references "child.snxsch",
        // which lives beside it.
        let mut sheets = HashMap::new();
        sheets.insert(
            PathBuf::from("/proj/root.snxsch"),
            sheet(1, &["child.snxsch"]),
        );
        sheets.insert(PathBuf::from("/proj/child.snxsch"), sheet(2, &[]));

        let graph = project_graph(&sheets, Some(Path::new("/proj")));

        let root_key = sheet_key(Path::new("/proj/root.snxsch"), Some(Path::new("/proj")));
        let child_key = graph.resolved[&root_key]["child.snxsch"].clone();
        assert_eq!(graph.sheets[&child_key].uuid, Uuid::from_u128(2));
        assert!(graph.issues.is_empty());
    }

    #[test]
    fn unreferenced_and_unloadable_sheets_are_absent_from_resolution() {
        // A reference whose file isn't in the loaded set resolves to nothing
        // (no panic, no phantom entry); a sheet nobody references is still
        // loaded into `sheets` — only its resolution is empty.
        let mut sheets = HashMap::new();
        sheets.insert(
            PathBuf::from("/proj/root.snxsch"),
            sheet(1, &["missing.snxsch"]),
        );
        sheets.insert(PathBuf::from("/proj/orphan.snxsch"), sheet(3, &[]));

        let graph = project_graph(&sheets, Some(Path::new("/proj")));
        let root_key = sheet_key(Path::new("/proj/root.snxsch"), Some(Path::new("/proj")));
        assert!(graph.resolved[&root_key].is_empty());
        assert!(graph.issues.is_empty());
        let orphan_key = sheet_key(Path::new("/proj/orphan.snxsch"), Some(Path::new("/proj")));
        assert!(graph.sheets.contains_key(&orphan_key));
    }

    #[test]
    fn sheet_key_is_relative_to_the_base_dir() {
        assert_eq!(
            sheet_key(Path::new("/proj/sub/root.snxsch"), Some(Path::new("/proj"))),
            "sub/root.snxsch"
        );
        assert_eq!(
            sheet_key(
                Path::new("/elsewhere/root.snxsch"),
                Some(Path::new("/proj"))
            ),
            "root.snxsch"
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
    fn empty_parent_dir_rejects_an_escaping_reference() {
        // The #463 escape re-opened at the fix's own boundary: an empty
        // `parent_dir` (unsaved File->New sheet, or a bare-filename tab with
        // no project loaded) normalizes to an empty root, which used to make
        // `resolved.starts_with(&root)` vacuously true for ANY reference —
        // absolute or `..`-traversal alike.
        assert_eq!(resolve_child_reference(Path::new(""), "/etc/passwd"), None);
        assert_eq!(
            resolve_child_reference(Path::new(""), "../etc/passwd"),
            None
        );
    }

    #[test]
    fn interior_dotdot_that_stays_inside_the_root_still_resolves() {
        // Pins the `lexically_normalize` pop-branch against over-rejection:
        // an interior ".." that never escapes `parent_dir` is not a
        // traversal, just a slightly indirect way of writing a sibling path.
        assert_eq!(
            resolve_child_reference(Path::new("/proj/sub"), "a/../leaf.snxsch"),
            Some(PathBuf::from("/proj/sub/leaf.snxsch"))
        );
    }

    #[test]
    fn navigation_and_project_graph_agree_on_the_same_reference() {
        // The graph stitches "leaf.snxsch" from a parent in /proj/sub to the
        // loaded /proj/sub/leaf.snxsch; the shared helper (what navigation
        // now uses) resolves the same reference against the same parent dir
        // to that exact path, and it keys to the same `SheetKey` the graph
        // resolved — so navigation opens the file the netlist stitched.
        let mut sheets = HashMap::new();
        sheets.insert(
            PathBuf::from("/proj/sub/mid.snxsch"),
            sheet(0x111, &["leaf.snxsch"]),
        );
        sheets.insert(PathBuf::from("/proj/sub/leaf.snxsch"), sheet(0x222, &[]));

        let graph = project_graph(&sheets, Some(Path::new("/proj")));
        let mid_key = sheet_key(Path::new("/proj/sub/mid.snxsch"), Some(Path::new("/proj")));
        let leaf_key = graph.resolved[&mid_key]["leaf.snxsch"].clone();
        assert_eq!(graph.sheets[&leaf_key].uuid, Uuid::from_u128(0x222));

        let nav = resolve_child_reference(Path::new("/proj/sub"), "leaf.snxsch")
            .expect("relative reference resolves");
        assert_eq!(nav, PathBuf::from("/proj/sub/leaf.snxsch"));
        assert_eq!(
            sheet_key(&nav, Some(Path::new("/proj"))),
            leaf_key,
            "navigation lands on the key the graph resolved"
        );
    }
}
