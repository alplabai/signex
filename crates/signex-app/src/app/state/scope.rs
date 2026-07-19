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

/// Comparison key for a filesystem path.
///
/// Windows filesystems are case-insensitive and accept either separator, but
/// Rust's `Path` comparison is neither — a `.snxprj` recording `Top.snxsch`
/// against a tab opened as `top.snxsch` (or a file dialog handing back a
/// differently-cased drive letter) would otherwise report the sheet as loose
/// and silently degrade a project export to a single page. Unix paths are
/// case-sensitive, so there the key is the path verbatim.
pub(crate) fn path_key(path: &Path) -> String {
    let raw = path.to_string_lossy();
    if cfg!(windows) {
        raw.to_lowercase().replace('\\', "/")
    } else {
        raw.into_owned()
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
) -> Option<&'a LoadedProject> {
    let key = path_key(path);
    projects.iter().find(|p| {
        let dir = p.dir();
        p.data
            .sheets
            .iter()
            .any(|s| path_key(&dir.join(&s.filename)) == key)
    })
}

/// `child path → parent path` over every loaded sheet, built from the parents'
/// own `child_sheets` references (each resolved against the parent's
/// directory, matching `project_sheets::project_children_map`).
fn parent_of(loaded: &HashMap<PathBuf, Vec<String>>) -> HashMap<String, PathBuf> {
    let mut parents: HashMap<String, PathBuf> = HashMap::new();
    for (parent_path, child_refs) in loaded {
        let dir = parent_path.parent().unwrap_or_else(|| Path::new(""));
        for filename in child_refs {
            parents
                .entry(path_key(&dir.join(filename)))
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
    if let Some(project) = project_listing_sheet(projects, path) {
        return Some(project);
    }
    let parents = parent_of(loaded);
    let mut seen: HashSet<String> = HashSet::new();
    let mut current = path.to_path_buf();
    while seen.insert(path_key(&current)) {
        let Some(parent) = parents.get(&path_key(&current)) else {
            return None;
        };
        if let Some(project) = project_listing_sheet(projects, parent) {
            return Some(project);
        }
        current = parent.clone();
    }
    None
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
    fn path_case_and_separator_do_not_change_ownership_on_windows() {
        // A `.snxprj` records `Top.snxsch`; the tab was opened through a file
        // dialog that handed back `top.snxsch`. Same file on Windows.
        let projects = vec![project(1, "/w/a", &["Top.snxsch"])];
        let found = project_owning_sheet(
            &projects,
            &HashMap::new(),
            &PathBuf::from("/w/a/top.snxsch"),
        );
        if cfg!(windows) {
            assert_eq!(found.map(|p| p.id), Some(ProjectId(1)));
        } else {
            assert!(found.is_none(), "unix paths are case-sensitive");
        }
    }
}
