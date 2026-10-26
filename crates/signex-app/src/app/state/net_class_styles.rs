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

#[derive(Default)]
pub struct NetClassStyleCache {
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
        self.gate = Gate {
            sheets,
            projects: projects.iter().map(GateProject::of).collect(),
        };
        true
    }

    /// Bring the cache in line with `inputs`. Returns `true` when the merged
    /// map changed, so the caller can drop its drawn-content caches.
    pub fn refresh<'a>(&mut self, inputs: impl IntoIterator<Item = ProjectClassInput<'a>>) -> bool {
        let mut seen: Vec<ProjectId> = Vec::new();
        let mut dirty = false;
        for input in inputs {
            seen.push(input.project);
            let previous = self.projects.get(&input.project);
            if previous.is_some_and(|entry| {
                entry.revision == input.revision && entry.classes == input.classes
            }) {
                continue;
            }
            let styles = if input.classes.is_empty() {
                HashMap::new()
            } else {
                (input.netlist)()
                    .map(|netlist| signex_net::element_class_styles(&netlist, input.classes))
                    .unwrap_or_default()
            };
            dirty |= previous.map_or(!styles.is_empty(), |entry| entry.styles != styles);
            self.projects.insert(
                input.project,
                ProjectEntry {
                    revision: input.revision,
                    classes: input.classes.to_vec(),
                    styles,
                },
            );
        }
        let before = self.projects.len();
        self.projects.retain(|id, _| seen.contains(id));
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
