//! The resolved net-class style of every drawn wire and junction.
//!
//! The class table lives in the `.snxprj`; a net's members come from the
//! project's stitched netlist, so a net that reaches `VCC` only through a
//! sheet entry is styled on the child sheet exactly as on the parent. The
//! canvas needs only the join of the two, per element, and reads it from here
//! rather than from the project, so `view` never touches project data.
//! [`NetClassStyleCache::refresh`] rebuilds a project's entry only when one
//! of its open sheets changed (engine revision) or its class table changed;
//! it runs after an update, never inside `view`.

use std::collections::HashMap;

use signex_types::net::Netlist;
use signex_types::net_class::{NetClass, NetClassStyle};
use uuid::Uuid;

use super::{LoadedProject, ProjectId};

/// One project the cache should hold a resolution for.
pub struct ProjectClassInput<'a> {
    pub project: ProjectId,
    /// Changes whenever any open sheet of the project is edited.
    pub revision: u64,
    pub classes: &'a [NetClass],
    /// The project's stitched netlist, asked for only when the entry is
    /// stale. `None` means no connectivity could be derived.
    pub netlist: Box<dyn Fn() -> Option<Netlist> + 'a>,
}

struct ProjectEntry {
    revision: u64,
    classes: Vec<NetClass>,
    styles: HashMap<Uuid, NetClassStyle>,
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
    sheet_files: Vec<String>,
}

impl GateProject {
    fn of(project: &LoadedProject) -> Self {
        Self {
            id: project.id,
            classes: project.data.net_classes.clone(),
            sheet_files: project
                .data
                .sheets
                .iter()
                .map(|s| s.filename.clone())
                .collect(),
        }
    }

    fn matches(&self, project: &LoadedProject) -> bool {
        self.id == project.id
            && self.classes == project.data.net_classes
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
    pub revision: u64,
    pub classes: Vec<NetClass>,
    pub styles: HashMap<Uuid, NetClassStyle>,
}

/// What a background resolution hands back: the projects it recomputed and
/// the ids of every project that should stay cached, stamped with the gate
/// generation it was started for.
#[derive(Debug)]
pub struct ResolvedStyles {
    pub generation: u64,
    pub keep: Vec<ProjectId>,
    pub projects: Vec<ResolvedProject>,
}

#[derive(Default)]
pub struct NetClassStyleCache {
    generation: u64,
    gate: Gate,
    projects: HashMap<ProjectId, ProjectEntry>,
    merged: HashMap<Uuid, NetClassStyle>,
}

impl NetClassStyleCache {
    /// Element uuid to class style, over every cached project. Element uuids
    /// are unique across sheets, so one flat map serves every window.
    #[must_use]
    pub fn styles(&self) -> &HashMap<Uuid, NetClassStyle> {
        &self.merged
    }

    /// `true` when no element carries a class style, so a caller with no
    /// class table anywhere can skip gathering inputs.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.merged.is_empty()
    }

    /// `true` when the open sheets or the loaded projects differ from the
    /// last call: `sheets` is a stamp over every open sheet's revision, and
    /// the projects are compared by class table and sheet list. An update
    /// that touched neither (a pointer move) returns `false` without any
    /// per-sheet ownership work. Records what it saw, so call it once per
    /// update and refresh when it says so.
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
        self.begin_generation();
        self.gate = Gate {
            sheets,
            projects: projects.iter().map(GateProject::of).collect(),
        };
        true
    }

    /// Mark the next background resolution as the only one that counts:
    /// every result computed for an earlier gate opening is now stale.
    fn begin_generation(&mut self) {
        self.generation += 1;
    }

    /// Forget what the gate last saw, so the next [`Self::needs_refresh`]
    /// opens it.
    pub fn reopen_gate(&mut self) {
        self.gate = Gate::default();
    }

    /// The token a background resolution started now must carry back.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// `true` when the cache already holds this exact resolution for the
    /// project, so a worker need not recompute it.
    #[must_use]
    pub fn is_current(&self, project: ProjectId, revision: u64, classes: &[NetClass]) -> bool {
        self.projects
            .get(&project)
            .is_some_and(|entry| entry.revision == revision && entry.classes == classes)
    }

    /// Apply a background resolution computed for `resolved.generation`.
    /// Returns `true` when the merged map changed. A result older than the
    /// newest gate opening is dropped and the styles already drawn stay.
    pub fn apply(&mut self, resolved: &ResolvedStyles) -> bool {
        if resolved.generation != self.generation {
            return false;
        }
        self.store(&resolved.keep, &resolved.projects)
    }

    /// Bring the cache in line with `inputs`, resolving on this thread.
    /// Returns `true` when the merged map changed, so the caller can drop
    /// its drawn-content caches.
    pub fn refresh<'a>(&mut self, inputs: impl IntoIterator<Item = ProjectClassInput<'a>>) -> bool {
        let mut keep: Vec<ProjectId> = Vec::new();
        let mut resolved = Vec::new();
        for input in inputs {
            keep.push(input.project);
            if self.is_current(input.project, input.revision, input.classes) {
                continue;
            }
            let styles = if input.classes.is_empty() {
                HashMap::new()
            } else {
                (input.netlist)()
                    .map(|netlist| signex_net::element_class_styles(&netlist, input.classes))
                    .unwrap_or_default()
            };
            resolved.push(ResolvedProject {
                project: input.project,
                revision: input.revision,
                classes: input.classes.to_vec(),
                styles,
            });
        }
        self.store(&keep, &resolved)
    }

    fn store(&mut self, keep: &[ProjectId], resolved: &[ResolvedProject]) -> bool {
        let mut dirty = false;
        for entry in resolved {
            let previous = self.projects.get(&entry.project);
            dirty |= previous.map_or(!entry.styles.is_empty(), |old| old.styles != entry.styles);
            self.projects.insert(
                entry.project,
                ProjectEntry {
                    revision: entry.revision,
                    classes: entry.classes.clone(),
                    styles: entry.styles.clone(),
                },
            );
        }
        let before = self.projects.len();
        self.projects.retain(|id, _| keep.contains(id));
        if self.projects.len() != before {
            dirty = true;
        }
        if dirty {
            self.merged = self
                .projects
                .values()
                .flat_map(|entry| entry.styles.iter().map(|(id, style)| (*id, *style)))
                .collect();
        }
        dirty
    }
}

#[cfg(test)]
mod tests;
