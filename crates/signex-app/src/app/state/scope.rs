//! Which project the document on screen belongs to.
//!
//! `DocumentState.active_project` is a *sticky* workspace pointer: it keeps
//! naming the last-loaded project while the user focuses a tab that has no
//! project of its own (a loose schematic, a symbol / footprint editor). Every
//! subsystem that operates on "the sheet set the user is working in" — export,
//! ERC, annotate, hierarchical child-sheet resolution — must instead resolve
//! from the *active document*, or it silently pulls another project's sheets
//! into the run (#406).
//!
//! Membership is decided two ways, in order:
//!
//! 1. the path is in the project's persisted `sheets` list (`.snxprj`);
//! 2. the path is reachable from a listed sheet by following the loaded
//!    `child_sheets` graph. Descending into a hierarchical child opens it as a
//!    tab without adding it to `sheets`, so rule 1 alone reports every child
//!    sheet as loose.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::LoadedProject;
use crate::app::project_sheets::{normalize_clamped, ownership_reference};

/// How [`path_key_with`] treats case (and, with it, the separator) when it
/// builds a comparison key.
///
/// An injectable input rather than a `cfg!(windows)` read inside the key
/// function, so both policies run on every host — including the Linux-only
/// `cargo test` CI job. The call sites still default it from the host family
/// ([`CaseFold::host_default`]); see [`path_key`] for what that gets wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CaseFold {
    /// Case-insensitive volume: lowercase the path and normalize `\` to `/`.
    Fold,
    /// Case-sensitive volume: the path verbatim.
    Preserve,
}

impl CaseFold {
    /// The policy the running host family implies: [`CaseFold::Fold`] on
    /// Windows, [`CaseFold::Preserve`] elsewhere. Not a per-volume answer.
    pub(crate) const fn host_default() -> Self {
        if cfg!(windows) {
            Self::Fold
        } else {
            Self::Preserve
        }
    }
}

/// Comparison key for a filesystem path under the host-default [`CaseFold`].
///
/// The host family is only a proxy for the per-volume truth: it under-folds on
/// a case-insensitive macOS APFS volume and over-folds on a Windows directory
/// with NTFS per-directory case sensitivity (#543). Probing the volume is the
/// open follow-up; callers needing a specific policy use [`path_key_with`].
///
/// Windows filesystems are case-insensitive and accept either separator, but
/// Rust's `Path` comparison is neither — a `.snxprj` recording `Top.snxsch`
/// against a tab opened as `top.snxsch` (or a file dialog handing back a
/// differently-cased drive letter) would otherwise report the sheet as loose
/// and silently degrade a project export to a single page. Unix paths are
/// case-sensitive, so there the key is the path verbatim.
pub(crate) fn path_key(path: &Path) -> String {
    path_key_with(path, CaseFold::host_default())
}

/// [`path_key`] with an explicit case policy.
pub(crate) fn path_key_with(path: &Path, fold: CaseFold) -> String {
    let raw = path.to_string_lossy();
    match fold {
        CaseFold::Fold => raw.to_lowercase().replace('\\', "/"),
        CaseFold::Preserve => raw.into_owned(),
    }
}

/// Project whose persisted `sheets` list names `path`.
///
/// Stricter than `DocumentState::project_for_path`, which matches on the
/// parent directory alone: a schematic sitting inside a project's folder but
/// never added to it belongs to no project, and must not drag that project's
/// other sheets into an export.
fn project_listing_sheet<'a>(
    projects: &'a [LoadedProject],
    path: &Path,
    fold: CaseFold,
) -> Option<&'a LoadedProject> {
    let key = path_key_with(path, fold);
    projects.iter().find(|p| {
        let dir = p.dir();
        p.data
            .sheets
            .iter()
            .any(|s| path_key_with(&dir.join(&s.filename), fold) == key)
    })
}

/// `child path → parent path` over every loaded sheet, built from the parents'
/// own `child_sheets` references (each resolved against the parent's
/// directory, matching `project_sheets::project_graph`).
///
/// `loaded` is a `HashMap`, whose iteration order is per-instance random —
/// sorted by parent path first so that when two parents claim the same
/// resolved child key, the first-wins tie-break (lexicographically-smallest
/// parent path) is the same every run, matching the identical hazard fixed in
/// `project_sheets::project_graph`.
fn parent_of(loaded: &HashMap<PathBuf, Vec<String>>, fold: CaseFold) -> HashMap<String, PathBuf> {
    let mut entries: Vec<(&PathBuf, &Vec<String>)> = loaded.iter().collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));

    let mut parents: HashMap<String, PathBuf> = HashMap::new();
    for (parent_path, child_refs) in entries {
        let dir = parent_path.parent().unwrap_or_else(|| Path::new(""));
        for filename in child_refs {
            parents
                .entry(path_key_with(&dir.join(filename), fold))
                .or_insert_with(|| parent_path.clone());
        }
    }
    parents
}

/// Project that owns `path`, directly or as a hierarchical descendant of one
/// of its sheets. `loaded` maps each open sheet's path to the `filename`
/// strings it references as children.
///
/// Cycles in the hierarchy (a child re-referencing an ancestor) terminate on
/// the visited set rather than looping — ERC reports those separately.
pub(crate) fn project_owning_sheet<'a>(
    projects: &'a [LoadedProject],
    loaded: &HashMap<PathBuf, Vec<String>>,
    path: &Path,
) -> Option<&'a LoadedProject> {
    project_owning_sheet_with(projects, loaded, path, CaseFold::host_default())
}

/// [`project_owning_sheet`] with an explicit case policy.
pub(crate) fn project_owning_sheet_with<'a>(
    projects: &'a [LoadedProject],
    loaded: &HashMap<PathBuf, Vec<String>>,
    path: &Path,
    fold: CaseFold,
) -> Option<&'a LoadedProject> {
    if let Some(project) = project_listing_sheet(projects, path, fold) {
        return Some(project);
    }
    let parents = parent_of(loaded, fold);
    let mut seen: HashSet<String> = HashSet::new();
    let mut current = path.to_path_buf();
    while seen.insert(path_key_with(&current, fold)) {
        let parent = parents.get(&path_key_with(&current, fold))?;
        if let Some(project) = project_listing_sheet(projects, parent, fold) {
            return Some(project);
        }
        current = parent.clone();
    }
    None
}

/// True when `path` is `root` or lies under it, compared component-wise on
/// [`path_key_with`] keys so a differently-cased or `\\`-separated spelling of
/// the same file still matches.
pub(crate) fn path_is_under(path: &Path, root: &Path, fold: CaseFold) -> bool {
    let path_key = path_key_with(path, fold);
    let root_key = path_key_with(root, fold);
    Path::new(&path_key).starts_with(Path::new(&root_key))
}

/// Comparison keys for ownership: the lexical [`path_key_with`] key of the
/// NORMALISED path, replaced by the key of its real (canonical) spelling when
/// the off-thread resolve could find one.
///
/// Two spellings of one file (a case variant on a case-insensitive volume, a
/// path through a symlinked directory) have different lexical keys but the
/// same real path, so ownership compares file identity, not path text. `real`
/// is filled by the resolve Task (it is disk IO); a path it never saw falls
/// back to the lexical key.
///
/// A sheet's children are resolved against its REAL directory
/// ([`KeyMap::child_dir`]), so every spelling of a sheet yields the same
/// child paths and the same keys below it.
#[derive(Debug, Clone)]
pub(crate) struct KeyMap {
    pub(crate) fold: CaseFold,
    /// Lexical key of a normalised path -> its real path.
    pub(crate) real: HashMap<String, PathBuf>,
}

impl Default for KeyMap {
    fn default() -> Self {
        Self::lexical(CaseFold::host_default())
    }
}

impl KeyMap {
    /// A map with no real paths: every key is lexical.
    pub(crate) fn lexical(fold: CaseFold) -> Self {
        Self {
            fold,
            real: HashMap::new(),
        }
    }

    /// The lexical key of `path`, normalised first.
    pub(crate) fn lexical_key(&self, path: &Path) -> String {
        path_key_with(&normalize_clamped(path), self.fold)
    }

    /// The ownership key of `path`.
    pub(crate) fn key(&self, path: &Path) -> String {
        match self.real.get(&self.lexical_key(path)) {
            Some(real) => path_key_with(real, self.fold),
            None => self.lexical_key(path),
        }
    }

    /// The directory `path`'s child references resolve against: the real
    /// directory when the resolve found one, else the lexical parent.
    pub(crate) fn child_dir(&self, path: &Path) -> PathBuf {
        let real = self.real.get(&self.lexical_key(path));
        let dir = real
            .map(PathBuf::as_path)
            .unwrap_or(path)
            .parent()
            .unwrap_or_else(|| Path::new(""));
        dir.to_path_buf()
    }

    /// True when the resolve Task has already keyed `path`.
    pub(crate) fn knows(&self, path: &Path) -> bool {
        self.real.contains_key(&self.lexical_key(path))
    }
}

/// Comparison keys of every sheet `project` reaches: the sheets it lists plus
/// every child reachable through `by_key`'s references, on normalized paths.
///
/// A sheet shared by two projects is in BOTH sets — unlike
/// [`project_owning_sheet_with`], which keeps one parent per child and so
/// names a single owner.
///
/// Every reference is followed, absolute and `../` alike, through the one
/// ownership rule ([`ownership_reference`]); containment applies only to
/// opening a file in the editor. `..` is clamped at the filesystem root and
/// each key is visited once, so a loop or a `/../..` chain terminates.
fn reachable_keys(
    project: &LoadedProject,
    by_key: &HashMap<String, &Vec<String>>,
    keys: &KeyMap,
) -> HashSet<String> {
    let dir = project.dir();
    let mut pending: Vec<PathBuf> = project
        .data
        .sheets
        .iter()
        .map(|s| normalize_clamped(&dir.join(&s.filename)))
        .collect();
    let mut reached: HashSet<String> = HashSet::new();
    while let Some(path) = pending.pop() {
        let key = keys.key(&path);
        if !reached.insert(key.clone()) {
            continue;
        }
        let Some(children) = by_key.get(&key) else {
            continue;
        };
        let parent_dir = keys.child_dir(&path);
        pending.extend(
            children
                .iter()
                .filter_map(|c| ownership_reference(&parent_dir, c)),
        );
    }
    reached
}

/// Loaded child references keyed on the NORMALISED path, so a sheet opened
/// through a raw `../b.snxsch` spelling is found by the walk's lookups.
pub(crate) fn refs_by_key<'a>(
    loaded: &'a HashMap<PathBuf, Vec<String>>,
    keys: &KeyMap,
) -> HashMap<String, &'a Vec<String>> {
    loaded
        .iter()
        .map(|(path, refs)| (keys.key(path), refs))
        .collect()
}

/// What every open project's ownership walk reaches, computed once per close.
///
/// A file is reached by a project when the project lists it or follows a
/// reference to it. That is the only evidence of ownership the close flow
/// accepts: where a file sits, which tab shows it or a library mount proves
/// nothing.
pub(crate) struct ReachSets {
    per_project: Vec<HashSet<String>>,
    keys: KeyMap,
}

impl ReachSets {
    /// Lexical keys under `fold`; no disk was consulted.
    #[cfg(test)]
    pub(crate) fn new(
        projects: &[LoadedProject],
        loaded: &HashMap<PathBuf, Vec<String>>,
        fold: CaseFold,
    ) -> Self {
        Self::with_keys(projects, loaded, &KeyMap::lexical(fold))
    }

    /// Keys through `keys`, so file identity (not path text) decides.
    pub(crate) fn with_keys(
        projects: &[LoadedProject],
        loaded: &HashMap<PathBuf, Vec<String>>,
        keys: &KeyMap,
    ) -> Self {
        let by_key = refs_by_key(loaded, keys);
        Self {
            per_project: projects
                .iter()
                .map(|p| reachable_keys(p, &by_key, keys))
                .collect(),
            keys: keys.clone(),
        }
    }

    /// Every key some open project reaches.
    pub(crate) fn all_keys(&self) -> impl Iterator<Item = &String> {
        self.per_project.iter().flatten()
    }

    /// True when `projects[project]` reaches `path`.
    pub(crate) fn reached_by(&self, project: usize, path: &Path) -> bool {
        let key = self.keys.key(path);
        self.per_project
            .get(project)
            .is_some_and(|set| set.contains(&key))
    }

    /// True when any open project other than `projects[closing]` reaches
    /// `path`.
    pub(crate) fn reached_by_other(&self, closing: usize, path: &Path) -> bool {
        let key = self.keys.key(path);
        self.per_project
            .iter()
            .enumerate()
            .any(|(i, set)| i != closing && set.contains(&key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::state::ProjectId;
    use signex_types::project::{ProjectData, SheetEntry};

    fn project(id: u32, dir: &str, filenames: &[&str]) -> LoadedProject {
        LoadedProject {
            id: ProjectId(id),
            path: PathBuf::from(dir).join("proj.snxprj"),
            data: ProjectData {
                name: "proj".to_string(),
                dir: dir.to_string(),
                schematic_root: filenames.first().map(|f| (*f).to_string()),
                pcb_file: None,
                sheets: filenames
                    .iter()
                    .map(|f| SheetEntry {
                        name: (*f).to_string(),
                        filename: (*f).to_string(),
                        symbols_count: 0,
                        wires_count: 0,
                        labels_count: 0,
                    })
                    .collect(),
                variant_definitions: Vec::new(),
                active_variant: None,
                libraries: Vec::new(),
                enable_git: false,
            },
            pending_libraries: std::collections::HashMap::new(),
        }
    }

    /// Spell a `/`-written fixture path with the host's separator.
    ///
    /// `Path::join` inserts the host separator, so a `/`-spelled directory
    /// joined to a filename is `/w\b.snxsch` on Windows. Under
    /// [`CaseFold::Preserve`] (verbatim key, no separator folding) that never
    /// equals the `/`-spelled lookup path; spelling every fixture path
    /// natively keeps both sides identical on any host.
    fn native(path: &str) -> String {
        path.replace('/', std::path::MAIN_SEPARATOR_STR)
    }

    fn loaded(entries: &[(&str, &[&str])]) -> HashMap<PathBuf, Vec<String>> {
        entries
            .iter()
            .map(|(p, kids)| {
                (
                    PathBuf::from(native(p)),
                    kids.iter().map(|k| (*k).to_string()).collect(),
                )
            })
            .collect()
    }

    #[test]
    fn owning_project_is_the_one_listing_the_sheet() {
        let projects = vec![project(1, "/w/a", &["top.snxsch", "power.snxsch"])];
        let found = project_owning_sheet(
            &projects,
            &HashMap::new(),
            &PathBuf::from("/w/a").join("power.snxsch"),
        );
        assert_eq!(found.map(|p| p.id), Some(ProjectId(1)));
    }

    #[test]
    fn loose_sheet_in_another_directory_belongs_to_no_project() {
        let projects = vec![project(1, "/w/a", &["top.snxsch"])];
        assert!(
            project_owning_sheet(
                &projects,
                &HashMap::new(),
                &PathBuf::from("/w/loose").join("scratch.snxsch")
            )
            .is_none()
        );
    }

    #[test]
    fn sheet_inside_the_project_directory_but_unlisted_belongs_to_no_project() {
        // The case parent-directory matching (`project_for_path`) gets wrong —
        // it would claim this file for the project and export the project's
        // whole sheet list instead of this one document. Nothing references it
        // as a child either, so the hierarchy walk finds no parent.
        let projects = vec![project(1, "/w/a", &["top.snxsch"])];
        let open = loaded(&[("/w/a/top.snxsch", &[])]);
        assert!(
            project_owning_sheet(
                &projects,
                &open,
                &PathBuf::from("/w/a").join("stray.snxsch")
            )
            .is_none()
        );
    }

    #[test]
    fn hierarchical_child_resolves_to_the_project_of_its_root() {
        // #406 critical: descending into a child sheet opens a tab that is
        // never added to `data.sheets`. Rule 1 alone calls it loose, and the
        // export ships one page instead of the project.
        let projects = vec![project(1, "/w/a", &["top.snxsch"])];
        let open = loaded(&[
            ("/w/a/top.snxsch", &["child.snxsch"]),
            ("/w/a/child.snxsch", &[]),
        ]);
        let found = project_owning_sheet(&projects, &open, &PathBuf::from("/w/a/child.snxsch"));
        assert_eq!(found.map(|p| p.id), Some(ProjectId(1)));
    }

    #[test]
    fn grandchild_resolves_through_two_hops() {
        let projects = vec![project(1, "/w/a", &["top.snxsch"])];
        let open = loaded(&[
            ("/w/a/top.snxsch", &["sub/mid.snxsch"]),
            ("/w/a/sub/mid.snxsch", &["leaf.snxsch"]),
            ("/w/a/sub/leaf.snxsch", &[]),
        ]);
        let found = project_owning_sheet(&projects, &open, &PathBuf::from("/w/a/sub/leaf.snxsch"));
        assert_eq!(found.map(|p| p.id), Some(ProjectId(1)));
    }

    #[test]
    fn a_hierarchy_cycle_terminates_instead_of_looping() {
        let projects = vec![project(1, "/w/a", &["top.snxsch"])];
        // Two loose sheets referencing each other, neither reachable from the
        // project root. Without the visited set this walks forever.
        let open = loaded(&[
            ("/w/loose/x.snxsch", &["y.snxsch"]),
            ("/w/loose/y.snxsch", &["x.snxsch"]),
        ]);
        assert!(
            project_owning_sheet(&projects, &open, &PathBuf::from("/w/loose/x.snxsch")).is_none()
        );
    }

    #[test]
    fn shared_child_ownership_is_deterministic_regardless_of_map_order() {
        // Two loaded sheets in different directories both reference a child at
        // the same resolved path — a real project would never intend this, but
        // `parent_of` must pick the SAME owner every run rather than whichever
        // the HashMap iterates first.
        //
        // The nondeterminism only surfaces ACROSS separately-built maps
        // (production rebuilds `child_sheet_refs()` on every call), so reusing
        // one fixed instance proves nothing — a fixed map's iteration order
        // never changes between reads. Rebuild a FRESH map, in BOTH insertion
        // orders, on every iteration: sorted by parent path
        // "/w/a/top.snxsch" < "/w/b/top.snxsch", so project 1 must win every
        // time. Under the pre-fix unsorted first-wins the winner tracked the
        // fresh map's arbitrary iteration order, so across this many fresh
        // alternating-order builds a revert of the sort flips at least one.
        let projects = vec![
            project(1, "/w/a", &["top.snxsch"]),
            project(2, "/w/b", &["top.snxsch"]),
        ];
        // Both "top.snxsch" sheets reference the same child by an absolute
        // path, so they resolve to the exact same key regardless of the
        // parent's own directory (/w/a vs /w/b).
        let child = PathBuf::from("/w/shared/child.snxsch");
        let child_ref = child.to_str().expect("utf8 test path");
        for i in 0..64 {
            let open = if i % 2 == 0 {
                loaded(&[
                    ("/w/a/top.snxsch", &[child_ref]),
                    ("/w/b/top.snxsch", &[child_ref]),
                ])
            } else {
                loaded(&[
                    ("/w/b/top.snxsch", &[child_ref]),
                    ("/w/a/top.snxsch", &[child_ref]),
                ])
            };
            let found = project_owning_sheet(&projects, &open, &child).map(|p| p.id);
            assert_eq!(
                found,
                Some(ProjectId(1)),
                "owner must be the lexicographically-smallest parent path (iter {i})",
            );
        }
    }

    #[test]
    fn fold_policy_ignores_case_and_separator_on_any_host() {
        let a = Path::new("/w/a/Top.snxsch");
        let b = Path::new("/w/a/top.snxsch");
        assert_eq!(
            path_key_with(a, CaseFold::Fold),
            path_key_with(b, CaseFold::Fold)
        );
        assert_eq!(
            path_key_with(Path::new("W\\Sub\\Top.snxsch"), CaseFold::Fold),
            "w/sub/top.snxsch"
        );
    }

    #[test]
    fn preserve_policy_keeps_case_and_separator_on_any_host() {
        let a = Path::new("/w/a/Top.snxsch");
        let b = Path::new("/w/a/top.snxsch");
        assert_ne!(
            path_key_with(a, CaseFold::Preserve),
            path_key_with(b, CaseFold::Preserve)
        );
        assert_eq!(
            path_key_with(Path::new("W\\Top.snxsch"), CaseFold::Preserve),
            "W\\Top.snxsch"
        );
    }

    #[test]
    fn path_key_follows_the_host_default_policy() {
        let p = Path::new("/w/A/Top.snxsch");
        assert_eq!(path_key(p), path_key_with(p, CaseFold::host_default()));
        assert_eq!(CaseFold::host_default() == CaseFold::Fold, cfg!(windows));
    }

    #[test]
    fn fold_policy_matches_sheet_ownership_across_case() {
        // A `.snxprj` records `Top.snxsch`; the tab was opened as `top.snxsch`.
        let projects = vec![project(1, "/w/a", &["Top.snxsch"])];
        let found = project_owning_sheet_with(
            &projects,
            &HashMap::new(),
            &PathBuf::from("/w/a/top.snxsch"),
            CaseFold::Fold,
        );
        assert_eq!(found.map(|p| p.id), Some(ProjectId(1)));
    }

    #[test]
    fn preserve_policy_keeps_sheet_ownership_case_sensitive() {
        let projects = vec![project(1, "/w/a", &["Top.snxsch"])];
        let found = project_owning_sheet_with(
            &projects,
            &HashMap::new(),
            &PathBuf::from("/w/a/top.snxsch"),
            CaseFold::Preserve,
        );
        assert!(found.is_none(), "preserve policy is case-sensitive");
    }

    #[test]
    fn fold_policy_resolves_hierarchical_child_across_case() {
        let projects = vec![project(1, "/w/a", &["Top.snxsch"])];
        let open = loaded(&[("/w/a/Top.snxsch", &["Child.snxsch"])]);
        let child = PathBuf::from("/w/a/child.snxsch");
        let fold = project_owning_sheet_with(&projects, &open, &child, CaseFold::Fold);
        let keep = project_owning_sheet_with(&projects, &open, &child, CaseFold::Preserve);
        assert_eq!(fold.map(|p| p.id), Some(ProjectId(1)));
        assert!(keep.is_none());
    }

    fn project_at(id: u32, prj: &str, filenames: &[&str]) -> LoadedProject {
        let mut p = project(id, "", filenames);
        p.path = PathBuf::from(native(prj));
        p
    }

    /// Is `path` reached by a project other than A (index 0)?
    fn other_reaches(
        projects: &[LoadedProject],
        open: &HashMap<PathBuf, Vec<String>>,
        path: &str,
        fold: CaseFold,
    ) -> bool {
        ReachSets::new(projects, open, fold).reached_by_other(0, &PathBuf::from(native(path)))
    }

    #[test]
    fn child_sheet_of_a_same_directory_project_is_reached_by_the_other() {
        let projects = vec![
            project_at(1, "/w/A.snxprj", &["a.snxsch"]),
            project_at(2, "/w/B.snxprj", &["b.snxsch"]),
        ];
        let open = loaded(&[("/w/b.snxsch", &["b_child.snxsch"])]);
        assert!(other_reaches(
            &projects,
            &open,
            "/w/b_child.snxsch",
            CaseFold::Preserve
        ));
        assert!(!other_reaches(
            &projects,
            &open,
            "/w/a.snxsch",
            CaseFold::Preserve
        ));
    }

    #[test]
    fn a_child_shared_by_two_projects_outside_both_directories_is_reached_by_both() {
        let projects = vec![
            project_at(1, "/w/A/A.snxprj", &["a.snxsch"]),
            project_at(2, "/w/B/B.snxprj", &["b.snxsch"]),
        ];
        let shared = native("/w/shared/s.snxsch");
        let open = loaded(&[
            ("/w/A/a.snxsch", &[shared.as_str()]),
            ("/w/B/b.snxsch", &[shared.as_str()]),
        ]);
        let reach = ReachSets::new(&projects, &open, CaseFold::Preserve);
        let s = PathBuf::from(&shared);
        assert!(reach.reached_by(0, &s));
        assert!(reach.reached_by_other(0, &s));
    }

    #[test]
    fn a_dotdot_cycle_in_the_references_terminates() {
        let projects = vec![project_at(1, "/w/A/A.snxprj", &["a.snxsch"])];
        let open = loaded(&[("/w/A/a.snxsch", &["../A/a.snxsch"])]);
        let reach = ReachSets::new(&projects, &open, CaseFold::Preserve);
        assert!(reach.reached_by(0, &PathBuf::from(native("/w/A/a.snxsch"))));
    }

    #[test]
    fn a_child_reached_through_an_escaping_reference_is_reached_by_the_other() {
        // B's sheet reaches `../A/s.snxsch`, which references `g.snxsch`.
        let projects = vec![
            project_at(1, "/w/A/A.snxprj", &["a.snxsch"]),
            project_at(2, "/w/B/B.snxprj", &["b.snxsch"]),
        ];
        let open = loaded(&[
            ("/w/B/b.snxsch", &["../A/s.snxsch"]),
            ("/w/A/s.snxsch", &["g.snxsch"]),
        ]);
        assert!(other_reaches(
            &projects,
            &open,
            "/w/A/g.snxsch",
            CaseFold::Preserve
        ));
    }

    #[test]
    fn a_child_reached_through_an_absolute_reference_is_reached_by_the_other() {
        let projects = vec![
            project_at(1, "/w/A/A.snxprj", &["a.snxsch"]),
            project_at(2, "/w/B/B.snxprj", &["b.snxsch"]),
        ];
        let s = native("/w/A/s.snxsch");
        let open = loaded(&[
            ("/w/B/b.snxsch", &[s.as_str()]),
            ("/w/A/s.snxsch", &["g.snxsch"]),
        ]);
        assert!(other_reaches(
            &projects,
            &open,
            "/w/A/g.snxsch",
            CaseFold::Preserve
        ));
    }

    #[test]
    fn a_chain_of_parent_references_past_the_root_terminates_and_is_followed() {
        let projects = vec![
            project_at(1, "/w/A/A.snxprj", &["a.snxsch"]),
            project_at(2, "/w/B/B.snxprj", &["b.snxsch"]),
        ];
        // `/../..` clamps to `/`: the first reference folds onto b itself, the
        // second lands on `/w/x.snxsch`.
        let open = loaded(&[(
            "/w/B/b.snxsch",
            &["/../../w/B/b.snxsch", "../../../../../w/x.snxsch"],
        )]);
        assert!(other_reaches(
            &projects,
            &open,
            "/w/x.snxsch",
            CaseFold::Preserve
        ));
    }

    #[test]
    fn an_engine_loaded_through_a_raw_dotdot_path_is_found_by_the_walk() {
        // B lists `../b.snxsch`; its engine was opened under the RAW spelling
        // `/w/B/../b.snxsch` and holds a child `c`. The walk normalises its
        // lookups, so the loaded map must be keyed the same way.
        let projects = vec![
            project_at(1, "/w/A/A.snxprj", &["a.snxsch"]),
            project_at(2, "/w/B/B.snxprj", &["../b.snxsch"]),
        ];
        let open = loaded(&[("/w/B/../b.snxsch", &["c.snxsch"])]);
        assert!(other_reaches(
            &projects,
            &open,
            "/w/c.snxsch",
            CaseFold::Preserve
        ));
    }

    #[test]
    fn a_nested_projects_unreached_file_is_reached_by_nobody() {
        // Where a file sits proves nothing: a draft under a nested project's
        // folder is reached by no project.
        let projects = vec![
            project_at(1, "/w/A/A.snxprj", &["a.snxsch"]),
            project_at(2, "/w/A/N/N.snxprj", &["n.snxsch"]),
        ];
        let reach = ReachSets::new(&projects, &HashMap::new(), CaseFold::Preserve);
        let draft = PathBuf::from(native("/w/A/N/draft.snxsch"));
        assert!(!reach.reached_by(0, &draft));
        assert!(!reach.reached_by_other(0, &draft));
    }

    #[test]
    fn normalize_clamped_stops_at_the_root_but_keeps_a_relative_head() {
        assert_eq!(
            normalize_clamped(Path::new(&native("/../../x"))),
            PathBuf::from(native("/x"))
        );
        assert_eq!(normalize_clamped(Path::new("../x")), PathBuf::from("../x"));
    }

    #[test]
    fn other_projects_sheet_is_reached_across_case_on_a_folding_host() {
        let projects = vec![
            project_at(1, "/w/A.snxprj", &["a.snxsch"]),
            project_at(2, "/w/B.snxprj", &["B.snxsch"]),
        ];
        let open = HashMap::new();
        assert!(other_reaches(
            &projects,
            &open,
            "/w/b.snxsch",
            CaseFold::Fold
        ));
        assert!(!other_reaches(
            &projects,
            &open,
            "/w/b.snxsch",
            CaseFold::Preserve
        ));
    }

    #[test]
    fn a_sheet_listed_by_two_projects_is_reached_by_the_other() {
        let projects = vec![
            project_at(1, "/w/A.snxprj", &["shared.snxsch"]),
            project_at(2, "/w/B.snxprj", &["shared.snxsch"]),
        ];
        assert!(other_reaches(
            &projects,
            &HashMap::new(),
            "/w/shared.snxsch",
            CaseFold::Preserve
        ));
    }

    #[test]
    fn path_is_under_is_component_wise_and_case_policy_aware() {
        let root = PathBuf::from("/w/Lib.snxlib");
        let inner = PathBuf::from("/w/lib.snxlib/footprints/g.snxfpt");
        assert!(path_is_under(&inner, &root, CaseFold::Fold));
        assert!(!path_is_under(&inner, &root, CaseFold::Preserve));
        assert!(!path_is_under(
            &PathBuf::from("/w/Lib.snxlib2/x"),
            &root,
            CaseFold::Fold
        ));
    }
}
