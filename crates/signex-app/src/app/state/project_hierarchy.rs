//! The sheet hierarchy of every open project, for the Projects tree (#1618).
//!
//! Reading closed sheets blocks, so the walk runs inside the project-tree
//! probe ([`super::project_fs`]) rather than in `update`. It is the same
//! assembly and builder the Navigator uses — [`assemble_project_sheets_from`]
//! and [`build_sheet_hierarchy`] — over a snapshot of what the open sheets
//! reference, so an unsaved edit is the truth and the disk only answers for
//! sheets nobody has open.

use std::collections::HashMap;
use std::path::PathBuf;

use signex_types::schematic::{ChildSheet, FillType, Point, SchematicSheet};
use uuid::Uuid;

use super::ProjectId;
use super::scope::CaseFold;
use crate::app::project_sheets::{OpenSheets, assemble_project_sheets_from};
use crate::app::sheet_hierarchy::{SheetHierarchy, build_sheet_hierarchy};

/// One sheet symbol of an open sheet: `(name, file, uuid)`.
pub type ChildRef = (String, String, Uuid);

/// One project whose hierarchy is wanted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchyJob {
    pub id: ProjectId,
    pub root: PathBuf,
    pub pages: Vec<PathBuf>,
    pub fold: CaseFold,
}

/// What the walk is built from. Equal requests give equal answers, so an
/// unchanged request is not walked again.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HierarchyRequest {
    pub jobs: Vec<HierarchyJob>,
    /// Every open sheet and the symbols it carries now, sorted by path.
    pub open_refs: Vec<(PathBuf, Vec<ChildRef>)>,
}

/// Walk each requested project. Blocking: call it off the UI thread.
pub fn walk_hierarchies(request: &HierarchyRequest) -> HashMap<ProjectId, SheetHierarchy> {
    let open: OpenSheets = request
        .open_refs
        .iter()
        .map(|(path, refs)| {
            let sheet = SchematicSheet {
                child_sheets: refs.iter().map(child_sheet).collect(),
                ..SchematicSheet::default()
            };
            (path.clone(), sheet)
        })
        .collect();
    request
        .jobs
        .iter()
        .map(|job| {
            let set = assemble_project_sheets_from(&open, &job.pages, &job.root, job.fold);
            let hierarchy = build_sheet_hierarchy(
                &set.sheets,
                &set.unreadable,
                &job.root,
                &set.pages_outside_the_hierarchy,
                job.fold,
            );
            (job.id, hierarchy)
        })
        .collect()
}

/// A sheet symbol with only the fields the hierarchy reads.
fn child_sheet((name, filename, uuid): &ChildRef) -> ChildSheet {
    ChildSheet {
        uuid: *uuid,
        name: name.clone(),
        filename: filename.clone(),
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
