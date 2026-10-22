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

/// Comparison keys of every sheet `project` reaches: the sheets it lists plus
/// every child reachable through `loaded`'s references, on normalized paths.
///
/// A sheet shared by two projects is in BOTH sets — unlike
/// [`project_owning_sheet_with`], which keeps one parent per child and so
/// names a single owner. Ownership questions that must not miss a sibling
/// ask this instead.
///
/// Every reference is followed, absolute and `../` alike, through the one
/// ownership rule ([`ownership_reference`]); containment applies only to
/// opening a file in the editor. Claiming too much is the safe direction,
/// so this never refuses an escape. `..` is clamped at the filesystem root and
/// each key is visited once, so a loop or a `/../..` chain terminates.
fn reachable_keys(
    project: &LoadedProject,
    loaded: &HashMap<PathBuf, Vec<String>>,
    fold: CaseFold,
) -> HashSet<String> {
    let by_key: HashMap<String, &Vec<String>> = loaded
        .iter()
        .map(|(path, refs)| (path_key_with(path, fold), refs))
        .collect();
    let dir = project.dir();
    let mut pending: Vec<PathBuf> = project
        .data
        .sheets
        .iter()
        .map(|s| normalize_clamped(&dir.join(&s.filename)))
        .collect();
    let mut reached: HashSet<String> = HashSet::new();
    while let Some(path) = pending.pop() {
        let key = path_key_with(&path, fold);
        if !reached.insert(key.clone()) {
            continue;
        }
        let Some(children) = by_key.get(&key) else {
            continue;
        };
        let parent_dir = path.parent().unwrap_or_else(|| Path::new(""));
        pending.extend(
            children
                .iter()
                .filter_map(|c| ownership_reference(parent_dir, c)),
        );
    }
    reached
}

/// True when `projects[closing]` owns `path`: it lists the sheet or reaches it
/// through the child-sheet graph, wherever the file sits on disk.
pub(crate) fn owned_by_project(
    projects: &[LoadedProject],
    loaded: &HashMap<PathBuf, Vec<String>>,
    closing: usize,
    path: &Path,
    fold: CaseFold,
) -> bool {
    projects
        .get(closing)
        .is_some_and(|mine| reachable_keys(mine, loaded, fold).contains(&path_key_with(path, fold)))
}

/// The `.snxlib` FILE paths `project` mounts, as comparison keys. Only the
/// library path itself counts, never its folder: a `.snxprj` is file content,
/// so an entry of `.` or `..` would otherwise "own" a whole directory.
/// An entry that resolves to, or contains, any open project's directory is
/// ignored.
fn library_file_keys(
    project: &LoadedProject,
    projects: &[LoadedProject],
    fold: CaseFold,
) -> HashSet<String> {
    project
        .data
        .libraries
        .iter()
        .map(|l| normalize_clamped(&project.dir().join(&l.path)))
        .filter(|lib| !projects.iter().any(|p| path_is_under(p.dir(), lib, fold)))
        .map(|lib| path_key_with(&lib, fold))
        .collect()
}

/// Claim precedence. A claim by another project (its walk reaches the file,
/// or the file is its `.snxprj` or sits under a project nested in the closing
/// one's directory) ALWAYS wins; the closing project's own proof only lifts a
/// file out of the fail-closed claim of an unreadable sibling sheet.
fn is_claimed(
    reached_by_other: bool,
    in_other_project: bool,
    under_unreadable_sibling: bool,
    proven_by_closing: bool,
) -> bool {
    reached_by_other || in_other_project || (under_unreadable_sibling && !proven_by_closing)
}

/// True when `path` belongs to an open project other than `projects[closing]`:
/// another project's own `.snxprj`, a sheet ANY other project lists or reaches
/// through its child-sheet graph, or anything under a project nested inside
/// the closing one's directory. A directory-ancestor test alone cannot tell
/// projects in the same or nested directories apart.
///
/// `unreadable` are sheets that exist but could not be read or parsed, so
/// their children are unknown. Fail closed: when another project reaches such
/// a sheet, everything under its directory counts as claimed unless the
/// closing project proves the file is its own: its reachable set, its
/// `.snxprj` or one of its `.snxlib` files. Where a file sits, which tab shows
/// it, or a library folder is NOT proof (see [`is_claimed`] for precedence).
pub(crate) fn claimed_by_other_project(
    projects: &[LoadedProject],
    loaded: &HashMap<PathBuf, Vec<String>>,
    unreadable: &[PathBuf],
    closing: usize,
    path: &Path,
    fold: CaseFold,
) -> bool {
    let Some(mine) = projects.get(closing) else {
        return false;
    };
    let key = path_key_with(path, fold);
    let reach: Vec<HashSet<String>> = projects
        .iter()
        .map(|p| reachable_keys(p, loaded, fold))
        .collect();
    let others = || {
        reach
            .iter()
            .enumerate()
            .filter(move |(i, _)| *i != closing)
            .map(|(_, r)| r)
    };
    let reached_by_other = others().any(|r| r.contains(&key));
    let in_other_project = projects.iter().enumerate().any(|(i, other)| {
        if i == closing {
            return false;
        }
        let other_dir = other.dir();
        path_key_with(&other.path, fold) == key
            || (path_key_with(other_dir, fold) != path_key_with(mine.dir(), fold)
                && path_is_under(other_dir, mine.dir(), fold)
                && path_is_under(path, other_dir, fold))
    });
    let under_unreadable_sibling = unreadable.iter().any(|sheet| {
        others().any(|r| r.contains(&path_key_with(sheet, fold)))
            && sheet
                .parent()
                .is_some_and(|dir| path_is_under(path, dir, fold))
    });
    let proven_by_closing = reach[closing].contains(&key)
        || path_key_with(&mine.path, fold) == key
        || library_file_keys(mine, projects, fold).contains(&key);
    is_claimed(
        reached_by_other,
        in_other_project,
        under_unreadable_sibling,
        proven_by_closing,
    )
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

    fn claimed(
        projects: &[LoadedProject],
        open: &HashMap<PathBuf, Vec<String>>,
        path: &str,
        fold: CaseFold,
    ) -> bool {
        claimed_by_other_project(projects, open, &[], 0, &PathBuf::from(native(path)), fold)
    }

    #[test]
    fn child_sheet_of_a_same_directory_project_is_claimed() {
        let projects = vec![
            project_at(1, "/w/A.snxprj", &["a.snxsch"]),
            project_at(2, "/w/B.snxprj", &["b.snxsch"]),
        ];
        let open = loaded(&[("/w/b.snxsch", &["b_child.snxsch"])]);
        assert!(claimed(
            &projects,
            &open,
            "/w/b_child.snxsch",
            CaseFold::Preserve
        ));
        assert!(!claimed(
            &projects,
            &open,
            "/w/a.snxsch",
            CaseFold::Preserve
        ));
    }

    #[test]
    fn a_child_shared_by_two_projects_outside_both_directories_is_claimed() {
        // #741 review: `parent_of` keeps only the first parent, so a child
        // referenced by A's and B's roots used to resolve to A alone and be
        // offered for A's Discard All although B's link reaches it too.
        let projects = vec![
            project_at(1, "/w/A/A.snxprj", &["a.snxsch"]),
            project_at(2, "/w/B/B.snxprj", &["b.snxsch"]),
        ];
        let shared = native("/w/shared/s.snxsch");
        let open = loaded(&[
            ("/w/A/a.snxsch", &[shared.as_str()]),
            ("/w/B/b.snxsch", &[shared.as_str()]),
        ]);
        assert!(claimed(
            &projects,
            &open,
            "/w/shared/s.snxsch",
            CaseFold::Preserve
        ));
    }

    #[test]
    fn a_dotdot_cycle_in_the_references_terminates() {
        let projects = vec![project_at(1, "/w/A/A.snxprj", &["a.snxsch"])];
        let open = loaded(&[("/w/A/a.snxsch", &["../A/a.snxsch"])]);
        assert!(owned_by_project(
            &projects,
            &open,
            0,
            &PathBuf::from(native("/w/A/a.snxsch")),
            CaseFold::Preserve
        ));
    }

    #[test]
    fn everything_under_an_unreadable_sibling_sheet_is_claimed_but_not_the_closing_projects_own() {
        let projects = vec![
            project_at(1, "/w/A.snxprj", &["a.snxsch"]),
            project_at(2, "/w/B.snxprj", &["b.snxsch"]),
        ];
        let unreadable = [PathBuf::from(native("/w/b.snxsch"))];
        let open = HashMap::new();
        let is_claimed = |path: &str| {
            claimed_by_other_project(
                &projects,
                &open,
                &unreadable,
                0,
                &PathBuf::from(native(path)),
                CaseFold::Preserve,
            )
        };
        assert!(is_claimed("/w/unknown_child.snxsch"));
        assert!(!is_claimed("/w/a.snxsch"), "A's own sheet stays A's");
        assert!(!is_claimed("/w/A.snxprj"), "A's own .snxprj stays A's");
    }

    #[test]
    fn a_child_reached_through_an_escaping_reference_is_claimed() {
        // #741 review: B's sheet reaches `../A/s.snxsch`, which references
        // `g.snxsch`; closing A must not treat g as A's alone.
        let projects = vec![
            project_at(1, "/w/A/A.snxprj", &["a.snxsch"]),
            project_at(2, "/w/B/B.snxprj", &["b.snxsch"]),
        ];
        let open = loaded(&[
            ("/w/B/b.snxsch", &["../A/s.snxsch"]),
            ("/w/A/s.snxsch", &["g.snxsch"]),
        ]);
        assert!(claimed(
            &projects,
            &open,
            "/w/A/g.snxsch",
            CaseFold::Preserve
        ));
    }

    #[test]
    fn a_child_reached_through_an_absolute_reference_is_claimed() {
        let projects = vec![
            project_at(1, "/w/A/A.snxprj", &["a.snxsch"]),
            project_at(2, "/w/B/B.snxprj", &["b.snxsch"]),
        ];
        let s = native("/w/A/s.snxsch");
        let open = loaded(&[
            ("/w/B/b.snxsch", &[s.as_str()]),
            ("/w/A/s.snxsch", &["g.snxsch"]),
        ]);
        assert!(claimed(
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
        assert!(claimed(&projects, &open, "/w/x.snxsch", CaseFold::Preserve));
        assert!(claimed(
            &projects,
            &open,
            "/w/B/b.snxsch",
            CaseFold::Preserve
        ));
    }

    #[test]
    fn an_unreadable_sheet_above_the_closing_project_keeps_only_proven_files_of_the_closing_one() {
        // B lists `../b.snxsch`, unreadable, in /w -- above A's /w/A. Only
        // what is provably A's (reach, `.snxprj`, own `.snxlib` file) is exempt; a file
        // merely sitting in A's folder may be the unreadable sheet's child.
        let mut a = project_at(1, "/w/A/A.snxprj", &["a.snxsch"]);
        a.data.libraries.push(signex_types::project::LibraryEntry {
            path: PathBuf::from("lib.snxlib"),
            kind: signex_types::project::LibraryEntryKind::ProjectLocal,
            library_id: None,
        });
        let projects = vec![a, project_at(2, "/w/B/B.snxprj", &["../b.snxsch"])];
        let unreadable = [PathBuf::from(native("/w/b.snxsch"))];
        let open = HashMap::new();
        let is_claimed = |path: &str| {
            claimed_by_other_project(
                &projects,
                &open,
                &unreadable,
                0,
                &PathBuf::from(native(path)),
                CaseFold::Preserve,
            )
        };
        assert!(!is_claimed("/w/A/a.snxsch"), "A's reachable sheet");
        assert!(!is_claimed("/w/A/A.snxprj"), "A's project file");
        assert!(is_claimed("/w/A/stray.snxsch"), "a tab is not proof");
        assert!(!is_claimed("/w/A/lib.snxlib"), "A's .snxlib file");
        assert!(
            is_claimed("/w/A/lib.snxlib/symbols/d.snxsym"),
            "a library folder is not proof"
        );
        assert!(
            is_claimed("/w/A/g.snxsch"),
            "no evidence it is A's: fail closed"
        );
        assert!(is_claimed("/w/elsewhere.snxsch"), "outside A stays claimed");
    }

    fn with_library(mut project: LoadedProject, path: &str) -> LoadedProject {
        project
            .data
            .libraries
            .push(signex_types::project::LibraryEntry {
                path: PathBuf::from(path),
                kind: signex_types::project::LibraryEntryKind::ProjectLocal,
                library_id: None,
            });
        project
    }

    /// Is `path` claimed when A (index 0) closes, with B's `../b.snxsch`
    /// unreadable?
    fn claimed_under_corrupt_b(a: LoadedProject, path: &str) -> bool {
        let projects = vec![a, project_at(2, "/w/B/B.snxprj", &["../b.snxsch"])];
        let unreadable = [PathBuf::from(native("/w/b.snxsch"))];
        claimed_by_other_project(
            &projects,
            &HashMap::new(),
            &unreadable,
            0,
            &PathBuf::from(native(path)),
            CaseFold::Preserve,
        )
    }

    #[test]
    fn a_library_at_dot_does_not_make_the_whole_directory_the_closing_projects() {
        let a = with_library(project_at(1, "/w/A/A.snxprj", &["a.snxsch"]), ".");
        assert!(claimed_under_corrupt_b(a, "/w/A/g.snxsch"));
    }

    #[test]
    fn a_library_at_dotdot_does_not_make_the_parent_directory_the_closing_projects() {
        let a = with_library(project_at(1, "/w/A/A.snxprj", &["a.snxsch"]), "..");
        assert!(claimed_under_corrupt_b(a, "/w/g.snxsch"));
    }

    #[test]
    fn a_library_file_of_the_closing_project_is_proof_but_its_contents_are_not() {
        let a = with_library(project_at(1, "/w/A/A.snxprj", &["a.snxsch"]), "lib.snxlib");
        assert!(!claimed_under_corrupt_b(a.clone(), "/w/A/lib.snxlib"));
        assert!(claimed_under_corrupt_b(a, "/w/A/lib.snxlib/g.snxsch"));
    }

    #[test]
    fn a_library_at_dot_does_not_hide_a_nested_projects_unreached_draft() {
        // N lives inside A's directory; `draft.snxsch` is under N's folder
        // and reached by nobody. A's library at `.` must not lift it.
        let a = with_library(project_at(1, "/w/A/A.snxprj", &["a.snxsch"]), ".");
        let projects = vec![a, project_at(2, "/w/A/N/N.snxprj", &["n.snxsch"])];
        assert!(claimed_by_other_project(
            &projects,
            &HashMap::new(),
            &[],
            0,
            &PathBuf::from(native("/w/A/N/draft.snxsch")),
            CaseFold::Preserve,
        ));
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
    fn other_projects_sheet_is_claimed_across_case_on_a_folding_host() {
        let projects = vec![
            project_at(1, "/w/A.snxprj", &["a.snxsch"]),
            project_at(2, "/w/B.snxprj", &["B.snxsch"]),
        ];
        let open = HashMap::new();
        assert!(claimed(&projects, &open, "/w/b.snxsch", CaseFold::Fold));
        assert!(!claimed(
            &projects,
            &open,
            "/w/b.snxsch",
            CaseFold::Preserve
        ));
    }

    #[test]
    fn a_nested_project_claim_beats_the_closing_projects_own_listing() {
        // Precedence: another project's claim beats A's proof. Keeping A's
        // own listed sheet open is the safe side; discarding it is not.
        let listed = native("sub/x.snxsch");
        let projects = vec![
            project_at(1, "/w/A.snxprj", &[listed.as_str()]),
            project_at(2, "/w/sub/B.snxprj", &["b.snxsch"]),
        ];
        let open = HashMap::new();
        assert!(claimed(
            &projects,
            &open,
            "/w/sub/x.snxsch",
            CaseFold::Preserve
        ));
        assert!(claimed(
            &projects,
            &open,
            "/w/sub/other.snxsch",
            CaseFold::Preserve
        ));
    }

    #[test]
    fn a_sheet_listed_by_two_projects_is_claimed_by_the_other() {
        let projects = vec![
            project_at(1, "/w/A.snxprj", &["shared.snxsch"]),
            project_at(2, "/w/B.snxprj", &["shared.snxsch"]),
        ];
        let open = HashMap::new();
        assert!(claimed(
            &projects,
            &open,
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
