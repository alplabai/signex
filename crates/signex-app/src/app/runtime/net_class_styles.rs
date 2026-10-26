//! Keeps the canvas's resolved net-class styles current.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use crate::app::state::ProjectId;
use crate::app::state::net_class_styles::ProjectClassInput;
use crate::app::state::scope;

use super::Signex;

impl Signex {
    /// Re-resolve the net-class style of every project with open sheets
    /// after an update.
    ///
    /// A cheap stamp over the open sheets' revisions and the projects' class
    /// tables gates the work, so an update that edited neither (a pointer
    /// move) does nothing. When it did, each project's class table is joined
    /// against the project's stitched netlist, rebuilt only for a project
    /// whose sheets or class table changed. When anything drawn changed, the
    /// schematic canvases drop their content caches so the next frame
    /// repaints with the new style. A session with no classes anywhere
    /// returns before gathering any input.
    pub(crate) fn refresh_net_class_styles(&mut self) {
        let document = &self.document_state;
        let any_classes = document
            .projects
            .iter()
            .any(|project| !project.data.net_classes.is_empty());
        if !any_classes && self.ui_state.net_class_styles.is_empty() {
            return;
        }
        let stamp = document.engines.iter().fold(0u64, |acc, (path, engine)| {
            let mut hasher = DefaultHasher::new();
            (path, engine.revision()).hash(&mut hasher);
            acc.wrapping_add(hasher.finish())
        });
        if !self
            .ui_state
            .net_class_styles
            .needs_refresh(stamp, &document.projects)
        {
            return;
        }

        // Per project: the stamp over its open sheets, and the first open
        // sheet by path, which names the project when its netlist is built.
        let refs = document.child_sheet_refs();
        let mut open: std::collections::HashMap<ProjectId, (u64, PathBuf)> = Default::default();
        for (path, engine) in &document.engines {
            let Some(project) = scope::project_owning_sheet(&document.projects, &refs, path) else {
                continue;
            };
            let entry = open.entry(project.id).or_insert_with(|| (0, path.clone()));
            entry.0 = entry.0.wrapping_add(engine.revision());
            if *path < entry.1 {
                entry.1 = path.clone();
            }
        }
        let inputs = document.projects.iter().filter_map(|project| {
            let (revision, source) = open.get(&project.id)?;
            Some(ProjectClassInput {
                project: project.id,
                revision: *revision,
                classes: project.data.net_classes.as_slice(),
                netlist: Box::new(move || {
                    crate::app::stitched_netlist::stitched_project_netlist(document, source)
                }),
            })
        });
        if self.ui_state.net_class_styles.refresh(inputs) {
            for canvas in self.interaction_state.all_schematic_canvases_mut() {
                canvas.clear_content_cache();
            }
        }
    }
}
