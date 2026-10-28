//! The resolved net-class style of every drawn wire and junction.
//!
//! The class table lives in the `.snxprj`; a net's members come from the
//! project's stitched netlist, so a net that reaches `VCC` only through a
//! sheet entry is styled on the child sheet exactly as on the parent. The
//! canvas needs only the join of the two, per element, and reads it from here
//! rather than from the project, so `view` never touches project data.
//!
//! Resolution is per project and keyed by a [`ProjectKey`]: everything the
//! answer depends on. A project is re-resolved only when its key changed, and
//! an answer is applied only if its key is still the one wanted for that
//! project, so a late answer for a project that has not changed since is kept
//! while one for a project that has moved on is dropped. Work for one project
//! never invalidates another's.

use std::collections::HashMap;
use std::path::PathBuf;

use signex_types::net_class::{NetClass, NetClassStyle};
use uuid::Uuid;

use super::scope::CaseFold;
use super::{LoadedProject, ProjectId};

/// Everything one project's resolution depends on. Two equal keys resolve to
/// the same styles.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectKey {
    /// Stamp over the project's open sheets (which sheets, which engine, which
    /// connectivity revision). Moves on an edit that can change a net, not on
    /// one that cannot (moving a text note).
    pub connectivity: u64,
    /// The project's declared pages: the stitcher reads them from disk.
    pub pages: Vec<PathBuf>,
    /// The sheet the netlist is rooted at.
    pub root: PathBuf,
    /// The project's key rules: they decide whether two spellings of a child
    /// sheet are one file, so a probe that moves them changes the stitch.
    pub fold: CaseFold,
    pub classes: Vec<NetClass>,
}

struct ProjectEntry {
    key: ProjectKey,
    styles: HashMap<Uuid, NetClassStyle>,
    nets: HashMap<Uuid, String>,
    warnings: Vec<String>,
}

/// What [`NetClassStyleCache::needs_refresh`] saw last time.
#[derive(Default)]
struct Gate {
    sheets: u64,
    projects: Vec<GateProject>,
}

struct GateProject {
    id: ProjectId,
    classes: Vec<NetClass>,
    root: Option<String>,
    sheet_files: Vec<String>,
    case_fold: CaseFold,
}

impl GateProject {
    fn of(project: &LoadedProject) -> Self {
        Self {
            id: project.id,
            classes: project.data.net_classes.clone(),
            root: project.data.schematic_root.clone(),
            sheet_files: project
                .data
                .sheets
                .iter()
                .map(|s| s.filename.clone())
                .collect(),
            case_fold: project.case_fold,
        }
    }

    fn matches(&self, project: &LoadedProject) -> bool {
        self.id == project.id
            && self.classes == project.data.net_classes
            && self.root == project.data.schematic_root
            && self.case_fold == project.case_fold
            && self
                .sheet_files
                .iter()
                .eq(project.data.sheets.iter().map(|s| &s.filename))
    }
}

/// One project's resolution, computed off the UI thread.
#[derive(Debug)]
pub struct ResolvedProject {
    pub project: ProjectId,
    pub key: ProjectKey,
    pub styles: HashMap<Uuid, NetClassStyle>,
    /// Net name of every wire and junction, for the label picker. Empty for a
    /// project with no classes.
    pub nets: HashMap<Uuid, String>,
    /// Sheets that would not parse and stitch issues, for the Messages panel.
    pub warnings: Vec<String>,
}

/// What a background resolution hands back: the projects it recomputed.
#[derive(Debug)]
pub struct ResolvedStyles {
    pub projects: Vec<ResolvedProject>,
}

/// A background resolution that never produced an answer (its task panicked),
/// and the projects it was asked about, so only their requests are forgotten.
#[derive(Debug)]
pub struct ResolveFailed {
    pub projects: Vec<ProjectId>,
    pub reason: String,
}

/// What applying a resolution changed, so the caller knows what to redraw.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Applied {
    /// The merged style map changed: the schematic canvases must repaint.
    pub styles_changed: bool,
    /// A project's element-to-net names changed: the picker must be rebuilt.
    pub nets_changed: bool,
    /// Warnings that are new since the project's previous resolution.
    pub warnings: Vec<String>,
}

#[derive(Default)]
pub struct NetClassStyleCache {
    gate: Gate,
    /// The key each project was last asked to resolve for: the answer in
    /// flight, or the one already stored.
    wanted: HashMap<ProjectId, ProjectKey>,
    /// The key whose resolution failed for each project. It is not asked for
    /// again until the project's key moves, so a task that fails every time
    /// reports once instead of on every click.
    failed: HashMap<ProjectId, ProjectKey>,
    projects: HashMap<ProjectId, ProjectEntry>,
    merged: HashMap<Uuid, NetClassStyle>,
    /// Counts rebuilds of `merged`, so a consumer that caches something drawn
    /// from the map can tell it changed without comparing it entry by entry.
    revision: u64,
}

impl NetClassStyleCache {
    /// Element uuid to class style, over every cached project. Element uuids
    /// are unique across sheets, so one flat map serves every window.
    #[must_use]
    pub fn styles(&self) -> &HashMap<Uuid, NetClassStyle> {
        &self.merged
    }

    /// Moves at least whenever [`Self::styles`] changes (it may also move on a
    /// rebuild that left the map as it was), so the scene cache compares one
    /// number per frame instead of the whole map.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// `true` when no element carries a class style, so a caller with no
    /// class table anywhere can skip gathering inputs.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.merged.is_empty()
    }

    /// The name the project netlist gives the net of `element` (a wire or
    /// junction) in `project`, or `None` while that project has not been
    /// resolved or has no classes.
    #[must_use]
    pub fn net_of(&self, project: ProjectId, element: Uuid) -> Option<&str> {
        self.projects
            .get(&project)?
            .nets
            .get(&element)
            .map(String::as_str)
    }

    /// `true` when the open sheets or the loaded projects differ from the
    /// last call: `sheets` is a stamp over every open sheet's connectivity
    /// revision, and the projects are compared by class table, root, sheet
    /// list and case fold. An update that touched neither (a pointer move, a text-note edit)
    /// returns `false` without any per-project work. Records what it saw, so
    /// call it once per update and plan when it says so.
    pub fn needs_refresh(&mut self, sheets: u64, projects: &[LoadedProject]) -> bool {
        let unchanged = self.gate.sheets == sheets
            && self.gate.projects.len() == projects.len()
            && self
                .gate
                .projects
                .iter()
                .zip(projects)
                .all(|(seen, project)| seen.matches(project));
        if unchanged {
            return false;
        }
        self.gate = Gate {
            sheets,
            projects: projects.iter().map(GateProject::of).collect(),
        };
        true
    }

    /// Forget what the gate last saw, so the next [`Self::needs_refresh`]
    /// opens it.
    pub fn reopen_gate(&mut self) {
        self.gate = Gate::default();
    }

    /// Record that `project` should hold the resolution for `key`. Returns
    /// `true` when a worker has to compute it: the cache neither holds it nor
    /// has it in flight. A project moving back to a key the cache already
    /// holds cancels the answer in flight for the key it left.
    pub fn want(&mut self, project: ProjectId, key: &ProjectKey) -> bool {
        if self.failed.get(&project) == Some(key) {
            return false;
        }
        self.failed.remove(&project);
        let held = self
            .projects
            .get(&project)
            .is_some_and(|entry| entry.key == *key);
        let in_flight = self.wanted.get(&project) == Some(key);
        if !in_flight {
            self.wanted.insert(project, key.clone());
        }
        !held && !in_flight
    }

    /// Drop every project not in `keep`. Returns `true` when that changed the
    /// merged style map.
    pub fn retain_projects(&mut self, keep: &[ProjectId]) -> bool {
        self.wanted.retain(|id, _| keep.contains(id));
        let before = self.projects.len();
        self.projects.retain(|id, _| keep.contains(id));
        let dropped = self.projects.len() != before;
        if dropped {
            self.rebuild_merged();
        }
        dropped
    }

    /// A background resolution failed, so no answer is coming for `failed`'s
    /// projects: forget their requests still in flight and remember the key
    /// that failed, so it is not retried (and not reported again) until the
    /// project's key moves. Other projects' requests are left alone.
    pub fn give_up(&mut self, failed: &[ProjectId]) {
        for id in failed {
            let Some(key) = self.wanted.get(id) else {
                continue;
            };
            if self.projects.get(id).is_none_or(|entry| entry.key != *key) {
                self.failed.insert(*id, key.clone());
            }
        }
        self.forget_pending(|id| failed.contains(id));
    }

    /// Forget every request in flight and every recorded failure, and reopen
    /// the gate, for a caller that lost its tasks.
    pub fn give_up_pending(&mut self) {
        self.forget_pending(|_| true);
        self.failed.clear();
        self.reopen_gate();
    }

    fn forget_pending(&mut self, forget: impl Fn(&ProjectId) -> bool) {
        let projects = &self.projects;
        self.wanted.retain(|id, key| {
            !forget(id) || projects.get(id).is_some_and(|entry| entry.key == *key)
        });
    }

    /// Apply a background resolution. A project whose answer is not the one
    /// now wanted for it is skipped; the styles already drawn for it stay.
    pub fn apply(&mut self, resolved: &ResolvedStyles) -> Applied {
        let mut applied = Applied::default();
        for entry in &resolved.projects {
            if self.wanted.get(&entry.project) != Some(&entry.key) {
                continue;
            }
            let previous = self.projects.get(&entry.project);
            applied.styles_changed |=
                previous.map_or(!entry.styles.is_empty(), |old| old.styles != entry.styles);
            applied.nets_changed |=
                previous.map_or(!entry.nets.is_empty(), |old| old.nets != entry.nets);
            let old_warnings = previous.map(|old| old.warnings.as_slice()).unwrap_or(&[]);
            applied.warnings.extend(
                entry
                    .warnings
                    .iter()
                    .filter(|w| !old_warnings.contains(w))
                    .cloned(),
            );
            self.projects.insert(
                entry.project,
                ProjectEntry {
                    key: entry.key.clone(),
                    styles: entry.styles.clone(),
                    nets: entry.nets.clone(),
                    warnings: entry.warnings.clone(),
                },
            );
        }
        if applied.styles_changed {
            self.rebuild_merged();
        }
        applied
    }

    fn rebuild_merged(&mut self) {
        self.revision += 1;
        self.merged = self
            .projects
            .values()
            .flat_map(|entry| entry.styles.iter().map(|(id, style)| (*id, *style)))
            .collect();
    }
}

#[cfg(test)]
mod tests;
