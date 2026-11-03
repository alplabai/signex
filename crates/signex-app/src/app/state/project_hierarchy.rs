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

/// One project whose hierarchy is wanted, and everything its walk reads.
/// Equal jobs give equal answers, so an unchanged job is not walked again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchyJob {
    pub id: ProjectId,
    pub root: PathBuf,
    pub pages: Vec<PathBuf>,
    pub fold: CaseFold,
    /// The open sheets that can belong to this project (those under its
    /// folder) and the symbols they carry now, sorted by path.
    pub open_refs: Vec<(PathBuf, Vec<ChildRef>)>,
}

/// The projects to walk.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HierarchyRequest {
    pub jobs: Vec<HierarchyJob>,
}

/// A closed sheet the walk read that repeats object ids (#1830): what the
/// open tab would repair. `key` identifies the repairs, which are derived
/// from the content, so the same file is told to the user once.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepairedSheet {
    pub path: PathBuf,
    pub count: usize,
    pub key: u64,
}

/// Walk each requested project; also lists the repaired sheets it read.
/// Blocking: call it off the UI thread.
pub fn walk_hierarchies(
    request: &HierarchyRequest,
) -> (HashMap<ProjectId, SheetHierarchy>, Vec<RepairedSheet>) {
    let mut repaired: Vec<RepairedSheet> = Vec::new();
    let hierarchies = request
        .jobs
        .iter()
        .map(|job| {
            let open: OpenSheets = job
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
            let set = assemble_project_sheets_from(&open, &job.pages, &job.root, job.fold);
            repaired.extend(set.sheets.iter().filter_map(|(path, sheet)| {
                let repairs = &sheet.unknown.load_repairs;
                (!repairs.is_empty()).then(|| RepairedSheet {
                    path: path.clone(),
                    count: repairs.len(),
                    key: repairs_key(repairs),
                })
            }));
            let hierarchy = build_sheet_hierarchy(
                &set.sheets,
                &set.unreadable,
                &job.root,
                &set.pages_outside_the_hierarchy,
                job.fold,
            );
            (job.id, hierarchy)
        })
        .collect();
    repaired.sort_by(|a, b| a.path.cmp(&b.path));
    repaired.dedup_by(|a, b| a.path == b.path);
    (hierarchies, repaired)
}

/// A hash of what the repairs say, so a changed file is told again.
fn repairs_key(repairs: &[signex_types::format::SheetLoadRepair]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for repair in repairs {
        repair.describe().hash(&mut hasher);
    }
    hasher.finish()
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
