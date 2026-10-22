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

/// True when `path` belongs to an open project other than `projects[closing]`:
/// another project's own `.snxprj`, a sheet another project lists or reaches
/// through its child-sheet graph, or anything under a project nested inside
/// the closing one's directory. A directory-ancestor test alone cannot tell
/// projects in the same or nested directories apart.
///
/// A sheet the closing project owns is never claimed by directory nesting,
/// but a sheet listed by several projects counts as claimed by the others so
/// closing one never discards a document another still has open.
pub(crate) fn claimed_by_other_project(
    projects: &[LoadedProject],
    loaded: &HashMap<PathBuf, Vec<String>>,
    closing: usize,
    path: &Path,
    fold: CaseFold,
) -> bool {
    let Some(mine) = projects.get(closing) else {
        return false;
    };
    let key = path_key_with(path, fold);
    let lists = |p: &LoadedProject| {
        let dir = p.dir();
        p.data
            .sheets
            .iter()
            .any(|s| path_key_with(&dir.join(&s.filename), fold) == key)
    };
    let other_lists = projects
        .iter()
        .enumerate()
        .any(|(i, p)| i != closing && lists(p));
    let owner = project_owning_sheet_with(projects, loaded, path, fold);
    let owned_by_closing = lists(mine) || owner.is_some_and(|o| o.id == mine.id);
    let owned_by_other = other_lists || owner.is_some_and(|o| o.id != mine.id);
    if owned_by_other {
        return true;
    }
    projects.iter().enumerate().any(|(i, other)| {
        if i == closing {
            return false;
        }
        let other_dir = other.dir();
        path_key_with(&other.path, fold) == key
            || (!owned_by_closing
                && path_key_with(other_dir, fold) != path_key_with(mine.dir(), fold)
                && path_is_under(other_dir, mine.dir(), fold)
                && path_is_under(path, other_dir, fold))
    })
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

    fn loaded(entries: &[(&str, &[&str])]) -> HashMap<PathBuf, Vec<String>> {
        entries
            .iter()
            .map(|(p, kids)| {
                (
                    PathBuf::from(p),
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
        p.path = PathBuf::from(prj);
        p
    }

    fn claimed(
        projects: &[LoadedProject],
        open: &HashMap<PathBuf, Vec<String>>,
        path: &str,
        fold: CaseFold,
    ) -> bool {
        claimed_by_other_project(projects, open, 0, &PathBuf::from(path), fold)
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
    fn a_nested_project_does_not_override_the_closing_projects_own_listing() {
        let projects = vec![
            project_at(1, "/w/A.snxprj", &["sub/x.snxsch"]),
            project_at(2, "/w/sub/B.snxprj", &["b.snxsch"]),
        ];
        let open = HashMap::new();
        assert!(!claimed(
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
