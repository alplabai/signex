//! Child-sheet references for ownership checks, resolved from the open
//! projects' own sheet lists rather than only from the loaded engines.
//!
//! A sibling's dirty child sheet whose parent tab was closed has no parent
//! engine to point at it; reading the unloaded parents from disk keeps that
//! child attributable to its project (#741).
//!
//! The disk walk blocks, so it never runs inside `update`: the caller takes a
//! [`SheetRefInputs`] snapshot, resolves it on a worker thread and gets the
//! [`SheetRefs`] back as a message.
//!
//! Closing project A splits A's dirty documents by one strict rule: `mine` is
//! what A's own ownership walk reaches (plus A's `.snxprj`) AND no other open
//! project's walk reaches; every other dirty document associated with A is
//! `kept`. If any open project has a sheet that cannot be read, ownership is
//! unknown and Discard All is off.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::scope::{CaseFold, KeyMap, ReachSets, path_key_with};
use super::{DocumentState, ProjectId, scope};
use crate::app::project_sheets::{normalize_clamped, ownership_reference};
use crate::library::state::EditorAddress;

/// How many file names a single Messages-panel or dialog line spells out
/// before it falls back to a count.
const MAX_NAMES_LISTED: usize = 5;

/// A sheet that exists but could not be read or parsed, so its children are
/// unknown.
#[derive(Debug, Clone)]
pub struct UnreadableSheet {
    pub(crate) path: PathBuf,
}

/// `a, b, c (+N more)`: at most [`MAX_NAMES_LISTED`] names, then a count, so a
/// line stays short however many files there are.
pub(crate) fn bounded_names(names: &[String]) -> String {
    let shown = names
        .iter()
        .take(MAX_NAMES_LISTED)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    match names.len().saturating_sub(MAX_NAMES_LISTED) {
        0 => shown,
        more => format!("{shown} (+{more} more)"),
    }
}

/// What the off-thread resolve learned: the saved files' child references,
/// the sheets that could not be read, and how paths map to file identity.
#[derive(Debug, Clone, Default)]
pub struct SheetRefs {
    /// Child references as saved on disk, per walked sheet (normalised path).
    /// An open engine's references are merged in at use time
    /// ([`DocumentState::merged_refs`]); they never replace these.
    pub(crate) disk: HashMap<PathBuf, Vec<String>>,
    pub(crate) unreadable: Vec<UnreadableSheet>,
    /// The open projects this was resolved for. A project opened afterwards
    /// has disk-only children nobody read, so a consumer re-resolves when
    /// [`DocumentState::refs_are_current`] says no.
    pub(crate) resolved_for: Vec<ProjectId>,
    /// Real (file identity) paths of everything the resolve touched.
    pub(crate) keys: KeyMap,
    /// Keys of the sheets the walk visited and read.
    pub(crate) walked: HashSet<String>,
}

impl SheetRefs {
    /// True when exactly the projects in `open` were open at resolve time.
    pub(crate) fn is_current_for(&self, open: &[ProjectId]) -> bool {
        let resolved: HashSet<ProjectId> = self.resolved_for.iter().copied().collect();
        let now: HashSet<ProjectId> = open.iter().copied().collect();
        resolved == now
    }

    /// Why Discard All is off, or `None` when every sheet could be read. A
    /// sheet that cannot be read hides its children, so which documents are
    /// the closing project's alone is unknown and nothing may be discarded.
    pub fn discard_blocked_reason(&self) -> Option<String> {
        if self.unreadable.is_empty() {
            return None;
        }
        let names: Vec<String> = self
            .unreadable
            .iter()
            .map(|u| file_label(&u.path))
            .collect();
        Some(format!(
            "Discard All is disabled: {} sheet(s) could not be read ({}), so ownership is unknown",
            names.len(),
            bounded_names(&names),
        ))
    }
}

/// Everything the disk walk needs, owned so it can move to a worker thread.
#[derive(Debug, Clone)]
pub(crate) struct SheetRefInputs {
    /// Child references of every loaded engine, followed IN ADDITION to the
    /// saved file's, so unsaved edits never hide a reference the file keeps.
    loaded: HashMap<PathBuf, Vec<String>>,
    /// Every sheet each open project lists.
    roots: Vec<PathBuf>,
    /// The projects the roots came from.
    projects: Vec<ProjectId>,
    /// Other paths ownership compares later (dirty documents, project files,
    /// library files), so they are keyed by file identity too.
    extra: Vec<PathBuf>,
}

/// The real spelling of `path`, remembered in `keys`. Disk IO.
///
/// An existing path is canonicalised, so a case variant on a case-insensitive
/// volume and a path through a symlinked directory collapse onto the one real
/// file. A path that does not exist gets its nearest existing ancestor
/// canonicalised and the rest appended; if even that fails the normalised
/// path is kept.
fn real_path(path: &Path) -> PathBuf {
    if let Ok(real) = std::fs::canonicalize(path) {
        return real;
    }
    match (path.parent(), path.file_name()) {
        (Some(parent), Some(name)) if !parent.as_os_str().is_empty() => {
            real_path(parent).join(name)
        }
        _ => path.to_path_buf(),
    }
}

/// Record and return the ownership key of `path`.
fn identity_key(keys: &mut KeyMap, path: &Path) -> String {
    let lexical = keys.lexical_key(path);
    let real = keys
        .real
        .entry(lexical)
        .or_insert_with(|| real_path(&normalize_clamped(path)));
    path_key_with(real, keys.fold)
}

fn push_unique(into: &mut Vec<String>, from: impl IntoIterator<Item = String>) {
    for item in from {
        if !into.contains(&item) {
            into.push(item);
        }
    }
}

impl SheetRefInputs {
    /// Walk the open projects' sheets, reading every one from disk.
    ///
    /// A sheet an engine holds follows the UNION of the engine's references
    /// and the saved file's. Every reference is followed (see
    /// [`ownership_reference`]): ownership never refuses an escape, only
    /// opening a file does. Each sheet is visited once per file identity and
    /// `..` is clamped at the filesystem root, so a `../` loop or chain
    /// terminates.
    ///
    /// Blocking file IO: call from a worker thread.
    pub(crate) fn resolve(self, fold: CaseFold) -> SheetRefs {
        let Self {
            loaded,
            roots,
            projects,
            extra,
        } = self;
        let mut keys = KeyMap::lexical(fold);
        let mut loaded_by_key: HashMap<String, Vec<String>> = HashMap::new();
        for (path, refs) in &loaded {
            let key = identity_key(&mut keys, path);
            push_unique(loaded_by_key.entry(key).or_default(), refs.iter().cloned());
        }
        for path in &extra {
            identity_key(&mut keys, path);
        }
        let mut disk: HashMap<PathBuf, Vec<String>> = HashMap::new();
        let mut unreadable = Vec::new();
        let mut walked: HashSet<String> = HashSet::new();
        let mut pending: Vec<PathBuf> = roots.iter().map(|r| normalize_clamped(r)).collect();
        while let Some(path) = pending.pop() {
            let key = identity_key(&mut keys, &path);
            if !walked.insert(key.clone()) {
                continue;
            }
            let saved = read_child_refs(&path).unwrap_or_else(|_| {
                unreadable.push(UnreadableSheet { path: path.clone() });
                Vec::new()
            });
            let mut children = saved.clone();
            if let Some(engine) = loaded_by_key.get(&key) {
                push_unique(&mut children, engine.iter().cloned());
            }
            disk.insert(path.clone(), saved);
            let dir = keys.child_dir(&path);
            pending.extend(children.iter().filter_map(|c| ownership_reference(&dir, c)));
        }
        SheetRefs {
            disk,
            unreadable,
            resolved_for: projects,
            keys,
            walked,
        }
    }
}

/// A closing project's dirty documents, split so none falls through: each
/// one is either in `mine` (offered to Save All / Discard All) or in `kept`
/// (never discarded: closing leaves it open and says so). Component Preview
/// rows follow the same split in `mine_rows` / `kept_rows`.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct CloseScan {
    pub(crate) mine: Vec<PathBuf>,
    pub(crate) kept: Vec<PathBuf>,
    pub(crate) mine_rows: Vec<EditorAddress>,
    pub(crate) kept_rows: Vec<EditorAddress>,
}

impl CloseScan {
    /// Messages-panel line naming the closing project's kept documents,
    /// bounded.
    pub(crate) fn warning(&self, project_name: &str) -> Option<String> {
        let mut names: Vec<String> = self.kept.iter().map(|p| file_label(p)).collect();
        names.extend(self.kept_rows.iter().map(|r| file_label(&r.library_path)));
        if names.is_empty() {
            return None;
        }
        Some(format!(
            "Close Project '{project_name}': kept {} unsaved document(s) open, not provably this project's alone: {}",
            names.len(),
            bounded_names(&names),
        ))
    }
}

/// File name of `path` for display, the whole path when it has none.
pub(crate) fn file_label(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

impl DocumentState {
    /// Snapshot what [`SheetRefInputs::resolve`] needs; cheap, no IO.
    pub(crate) fn sheet_ref_inputs(&self) -> SheetRefInputs {
        let mut extra: Vec<PathBuf> = self.dirty_paths.iter().cloned().collect();
        for project in &self.projects {
            extra.push(project.path.clone());
            extra.extend(
                project
                    .data
                    .libraries
                    .iter()
                    .map(|entry| project.data.resolve_library_path(entry)),
            );
        }
        SheetRefInputs {
            loaded: self.child_sheet_refs(),
            roots: self
                .projects
                .iter()
                .flat_map(|p| {
                    let dir = p.dir();
                    p.data.sheets.iter().map(move |s| dir.join(&s.filename))
                })
                .collect(),
            projects: self.projects.iter().map(|p| p.id).collect(),
            extra,
        }
    }

    /// Every sheet's child references right now: the saved files' (from the
    /// resolve) UNIONED with the open engines' current ones. Engines are
    /// re-read here, so an edit made after the resolve started is seen. No IO.
    pub(crate) fn merged_refs(&self, sheet_refs: &SheetRefs) -> HashMap<PathBuf, Vec<String>> {
        let keys = &sheet_refs.keys;
        let engines = self.child_sheet_refs();
        let mut by_key: HashMap<String, (PathBuf, Vec<String>)> = HashMap::new();
        for (path, refs) in sheet_refs.disk.iter().chain(engines.iter()) {
            let entry = by_key
                .entry(keys.key(path))
                .or_insert_with(|| (path.clone(), Vec::new()));
            push_unique(&mut entry.1, refs.iter().cloned());
        }
        by_key.into_values().collect()
    }

    /// What every open project's ownership walk reaches, for `sheet_refs`.
    pub(crate) fn reach_sets(&self, sheet_refs: &SheetRefs) -> ReachSets {
        ReachSets::with_keys(
            &self.projects,
            &self.merged_refs(sheet_refs),
            &sheet_refs.keys,
        )
    }

    /// True when `sheet_refs` still describes the workspace: the same projects
    /// are open, every sheet the walk reaches now (engine edits included) was
    /// read from disk, and every dirty document was keyed by file identity.
    /// When false the caller resolves again rather than act on stale evidence.
    pub(crate) fn refs_are_current(&self, sheet_refs: &SheetRefs) -> bool {
        let open: Vec<ProjectId> = self.projects.iter().map(|p| p.id).collect();
        sheet_refs.is_current_for(&open)
            && self
                .dirty_paths
                .iter()
                .all(|path| sheet_refs.keys.knows(path))
            && self
                .reach_sets(sheet_refs)
                .all_keys()
                .all(|key| sheet_refs.walked.contains(key))
    }

    /// Split the dirty documents of `projects[closing]` into [`CloseScan`]'s
    /// lists. `rows` are the dirty Component Preview editors to place.
    ///
    /// A dirty path is associated with the project when it sits in the
    /// project directory, backs one of its tabs, is reached by its walk, or is
    /// its `.snxprj`. It is `mine` only when the project's own walk reaches it
    /// (or it is the project's `.snxprj`) AND no other open project's walk
    /// does; everything else associated is `kept`. Location, tabs and library
    /// mounts are never evidence of ownership.
    ///
    /// A row is `mine` only when its library file is one of the project's own
    /// library entries, no other open project lists the same library, and it
    /// does not sit inside another open project's directory.
    pub(crate) fn close_scan(
        &self,
        sheet_refs: &SheetRefs,
        closing: usize,
        rows: &[EditorAddress],
    ) -> CloseScan {
        let Some(project) = self.projects.get(closing) else {
            return CloseScan::default();
        };
        let keys = &sheet_refs.keys;
        let reach = self.reach_sets(sheet_refs);
        let project_file = keys.key(&project.path);
        let mut scan = CloseScan::default();
        for path in &self.dirty_paths {
            let reached = reach.reached_by(closing, path);
            let is_project_file = keys.key(path) == project_file;
            let backs_tab = self
                .tabs
                .iter()
                .any(|t| t.path == *path && t.project_id == Some(project.id));
            let associated =
                path.starts_with(project.dir()) || backs_tab || reached || is_project_file;
            if !associated {
                continue;
            }
            if (reached || is_project_file) && !reach.reached_by_other(closing, path) {
                scan.mine.push(path.clone());
            } else {
                scan.kept.push(path.clone());
            }
        }
        for row in rows {
            let library = keys.key(&row.library_path);
            let own = self.library_keys(closing, keys).contains(&library);
            let listed_elsewhere = (0..self.projects.len())
                .filter(|i| *i != closing)
                .any(|i| self.library_keys(i, keys).contains(&library));
            let inside_other = self
                .projects
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != closing)
                .any(|(_, p)| scope::path_is_under(&row.library_path, p.dir(), keys.fold));
            if own && !listed_elsewhere && !inside_other {
                scan.mine_rows.push(row.clone());
            } else if own || row.library_path.starts_with(project.dir()) {
                scan.kept_rows.push(row.clone());
            }
        }
        scan.mine.sort();
        scan.kept.sort();
        scan
    }

    /// File-identity keys of the library files `projects[idx]` mounts.
    fn library_keys(&self, idx: usize, keys: &KeyMap) -> HashSet<String> {
        self.projects.get(idx).map_or_else(HashSet::new, |p| {
            p.data
                .libraries
                .iter()
                .map(|entry| keys.key(&p.data.resolve_library_path(entry)))
                .collect()
        })
    }
}

/// `child_sheets` filenames of the sheet at `path`. A missing file has none;
/// an unreadable or unparsable one returns why.
fn read_child_refs(path: &Path) -> Result<Vec<String>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("could not be read: {e}")),
    };
    signex_types::format::SnxSchematic::parse(&text)
        .map(|parsed| {
            parsed
                .sheet
                .child_sheets
                .into_iter()
                .map(|c| c.filename)
                .collect()
        })
        .map_err(|e| format!("could not be parsed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank_sheet() -> signex_types::schematic::SchematicSheet {
        signex_types::schematic::SchematicSheet {
            uuid: uuid::Uuid::nil(),
            version: 0,
            generator: String::new(),
            generator_version: String::new(),
            paper_size: "A4".to_string(),
            root_sheet_page: "1".to_string(),
            symbols: Vec::new(),
            wires: Vec::new(),
            junctions: Vec::new(),
            labels: Vec::new(),
            child_sheets: Vec::new(),
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

    fn write_sheet(path: &Path, children: &[&str]) {
        let mut sheet = blank_sheet();
        sheet.child_sheets = children
            .iter()
            .map(|f| signex_types::schematic::ChildSheet {
                uuid: uuid::Uuid::nil(),
                name: String::new(),
                filename: (*f).to_string(),
                position: signex_types::schematic::Point::new(0.0, 0.0),
                size: (0.0, 0.0),
                stroke_width: 0.0,
                fill: signex_types::schematic::FillType::None,
                stroke_color: None,
                fill_color: None,
                fields_autoplaced: false,
                pins: Vec::new(),
                instances: Vec::new(),
            })
            .collect();
        let text = signex_types::format::SnxSchematic::new(sheet)
            .write_string()
            .expect("serialise sheet");
        std::fs::write(path, text).expect("write sheet");
    }

    #[test]
    fn name_lists_are_bounded_to_a_count_plus_the_first_few() {
        let names: Vec<String> = (0..200).map(|n| format!("s{n}.snxsch")).collect();
        let line = bounded_names(&names);
        assert!(line.starts_with("s0.snxsch, s1.snxsch"), "{line}");
        assert!(line.ends_with("(+195 more)"), "{line}");
        assert!(line.len() < 100, "{line}");
        assert_eq!(bounded_names(&names[..2]), "s0.snxsch, s1.snxsch");

        let scan = CloseScan {
            mine: Vec::new(),
            kept: names.iter().map(PathBuf::from).collect(),
            ..CloseScan::default()
        };
        let warning = scan.warning("P").expect("kept files warn");
        assert!(warning.contains("kept 200"), "{warning}");
        assert!(warning.len() < 250, "{warning}");

        let refs = SheetRefs {
            unreadable: names
                .iter()
                .map(|n| UnreadableSheet {
                    path: PathBuf::from(n),
                })
                .collect(),
            ..SheetRefs::default()
        };
        let reason = refs.discard_blocked_reason().expect("blocked");
        assert!(reason.len() < 250, "{reason}");
        assert!(SheetRefs::default().discard_blocked_reason().is_none());
    }

    #[test]
    fn a_dotdot_cycle_reads_each_sheet_once_and_reports_nothing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("d");
        std::fs::create_dir_all(&dir).expect("mkdir");
        let root = dir.join("x.snxsch");
        write_sheet(&root, &["../d/x.snxsch"]);
        let inputs = SheetRefInputs {
            loaded: HashMap::new(),
            roots: vec![root.clone()],
            projects: Vec::new(),
            extra: Vec::new(),
        };

        let resolved = inputs.resolve(CaseFold::Preserve);

        assert!(resolved.unreadable.is_empty(), "{:?}", resolved.unreadable);
        assert_eq!(resolved.disk.len(), 1, "{:?}", resolved.disk.keys());
        assert!(resolved.disk.contains_key(&root));
    }

    #[test]
    fn a_case_and_separator_variant_reference_resolves_to_the_loaded_engine() {
        let tmp = tempfile::tempdir().expect("tempdir");
        // Children resolve against the REAL directory, so expect real paths.
        let base = tmp.path().canonicalize().expect("canonical tmp");
        let root = base.join("root.snxsch");
        write_sheet(&root, &["CHILD.snxsch"]);
        // Loaded under the lowercase spelling, with a grandchild the disk
        // never saw (no such file exists).
        let loaded_child = base.join("child.snxsch");
        let inputs = SheetRefInputs {
            loaded: HashMap::from([(loaded_child.clone(), vec!["grand.snxsch".to_string()])]),
            roots: vec![root],
            projects: Vec::new(),
            extra: Vec::new(),
        };

        let resolved = inputs.resolve(CaseFold::Fold);

        // The engine's grandchild is followed through the variant spelling.
        assert!(
            resolved.disk.contains_key(&base.join("grand.snxsch")),
            "{:?}",
            resolved.disk.keys(),
        );
        assert!(resolved.unreadable.is_empty());
    }

    #[test]
    fn a_reference_that_escapes_its_parent_directory_is_followed_for_ownership() {
        let tmp = tempfile::tempdir().expect("tempdir");
        // Children resolve against the REAL directory, so expect real paths.
        let base = tmp.path().canonicalize().expect("canonical tmp");
        let dir = base.join("d");
        std::fs::create_dir_all(&dir).expect("mkdir");
        let outside = base.join("outside.snxsch");
        let grandchild = base.join("grand.snxsch");
        write_sheet(&outside, &["grand.snxsch"]);
        write_sheet(&grandchild, &[]);
        let root = dir.join("x.snxsch");
        write_sheet(&root, &["../outside.snxsch"]);
        let inputs = SheetRefInputs {
            loaded: HashMap::new(),
            roots: vec![root],
            projects: Vec::new(),
            extra: Vec::new(),
        };

        let resolved = inputs.resolve(CaseFold::Preserve);

        assert!(
            resolved.disk.contains_key(&outside),
            "{:?}",
            resolved.disk.keys()
        );
        assert!(
            resolved.disk.contains_key(&grandchild),
            "{:?}",
            resolved.disk.keys()
        );
    }

    #[test]
    fn an_absolute_reference_is_followed_for_ownership() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("d");
        std::fs::create_dir_all(&dir).expect("mkdir");
        let outside = tmp.path().join("outside.snxsch");
        write_sheet(&outside, &[]);
        let root = dir.join("x.snxsch");
        write_sheet(&root, &[outside.to_str().expect("utf8 path")]);
        let inputs = SheetRefInputs {
            loaded: HashMap::new(),
            roots: vec![root],
            projects: Vec::new(),
            extra: Vec::new(),
        };

        let resolved = inputs.resolve(CaseFold::Preserve);

        assert!(
            resolved.disk.contains_key(&outside),
            "{:?}",
            resolved.disk.keys()
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_chain_of_parent_references_past_the_root_terminates() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("d");
        std::fs::create_dir_all(&dir).expect("mkdir");
        let root = dir.join("x.snxsch");
        // `/../../..` clamps to `/`, so this names the sheet itself and the
        // walk folds onto the key it already saw instead of chasing `..`.
        let chained = format!("/../../../..{}", root.display());
        write_sheet(&root, &[chained.as_str(), "../../../../../../../x.snxsch"]);
        let inputs = SheetRefInputs {
            loaded: HashMap::new(),
            roots: vec![root.clone()],
            projects: Vec::new(),
            extra: Vec::new(),
        };

        let resolved = inputs.resolve(CaseFold::Preserve);

        assert!(resolved.unreadable.is_empty(), "{:?}", resolved.unreadable);
        assert!(resolved.disk.contains_key(&root));
        assert!(resolved.disk.len() <= 3, "{:?}", resolved.disk.keys());
    }

    fn loaded_project(id: u32, dir: &Path, sheet: &str) -> crate::app::state::LoadedProject {
        crate::app::state::LoadedProject {
            id: ProjectId(id),
            path: dir.join("p.snxprj"),
            data: signex_types::project::ProjectData {
                name: format!("p{id}"),
                dir: dir.to_string_lossy().into_owned(),
                schematic_root: Some(sheet.to_string()),
                pcb_file: None,
                sheets: vec![signex_types::project::SheetEntry {
                    name: sheet.to_string(),
                    filename: sheet.to_string(),
                    symbols_count: 0,
                    wires_count: 0,
                    labels_count: 0,
                    unknown: Default::default(),
                }],
                variant_definitions: Vec::new(),
                active_variant: None,
                libraries: Vec::new(),
                enable_git: false,
                unknown: Default::default(),
            },
            pending_libraries: HashMap::new(),
        }
    }

    #[test]
    fn a_case_variant_reference_is_the_same_file_under_the_fold_policy() {
        // Deterministic on every host: the fold policy is explicit, and the
        // variant file never exists on a case-sensitive volume.
        let tmp = tempfile::tempdir().expect("tempdir");
        let (a_dir, b_dir) = (tmp.path().join("A"), tmp.path().join("B"));
        std::fs::create_dir_all(&a_dir).expect("mkdir A");
        std::fs::create_dir_all(&b_dir).expect("mkdir B");
        write_sheet(&b_dir.join("b.snxsch"), &["../A/s.snxsch"]);
        let (mut app, _task) = crate::app::Signex::new();
        let docs = &mut app.document_state;
        docs.projects.push(loaded_project(1, &a_dir, "S.snxsch"));
        docs.projects.push(loaded_project(2, &b_dir, "b.snxsch"));
        let shared = a_dir.join("S.snxsch");
        docs.dirty_paths.insert(shared.clone());

        let refs = docs.sheet_ref_inputs().resolve(CaseFold::Fold);
        let scan = docs.close_scan(&refs, 0, &[]);

        assert!(scan.mine.is_empty(), "{:?}", scan.mine);
        assert_eq!(scan.kept, vec![shared]);
    }

    #[test]
    fn an_open_sheets_saved_references_are_followed_as_well_as_its_own() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let base = tmp.path().canonicalize().expect("canonical tmp");
        let root = base.join("root.snxsch");
        write_sheet(&root, &["saved.snxsch"]);
        write_sheet(&base.join("saved.snxsch"), &[]);
        write_sheet(&base.join("edited.snxsch"), &[]);
        // The open copy dropped `saved` and added `edited`.
        let inputs = SheetRefInputs {
            loaded: HashMap::from([(root.clone(), vec!["edited.snxsch".to_string()])]),
            roots: vec![root.clone()],
            projects: Vec::new(),
            extra: Vec::new(),
        };

        let resolved = inputs.resolve(CaseFold::Preserve);

        assert!(resolved.disk.contains_key(&base.join("saved.snxsch")));
        assert!(resolved.disk.contains_key(&base.join("edited.snxsch")));
        assert_eq!(
            resolved.disk.get(&root),
            Some(&vec!["saved.snxsch".to_string()]),
            "`disk` holds the saved file's references; the engine's merge at use time",
        );
    }
}
