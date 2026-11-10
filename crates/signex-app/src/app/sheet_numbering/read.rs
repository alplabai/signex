//! The disk half of the dialog: the project's pages and their numbers, read
//! on a blocking thread. Nothing here runs inside `update`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use signex_types::schematic::{InstancePath, SchematicSheet};
use uuid::Uuid;

use crate::app::project_sheets::{self, ProjectSheetPlan};

/// Where a page's manual number is stored: a sheet symbol in its parent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowPlacement {
    /// The parent sheet's file.
    pub(crate) parent: PathBuf,
    /// The parent placement's path, the `path` of the number entry.
    pub(crate) parent_path: InstancePath,
    /// The sheet symbol's uuid.
    pub(crate) symbol: Uuid,
}

/// One page, as the project numbers it now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumberingRow {
    /// The sheet symbol's name, or the sheet itself for a top-level page.
    pub(crate) name: String,
    /// The sheet's file, relative to the project.
    pub(crate) sheet: String,
    /// How deep in the hierarchy the page sits (a top-level page: 0).
    pub(crate) depth: usize,
    /// The number printed today.
    pub(crate) number: usize,
    /// The page's position in hierarchy order, its automatic number.
    pub(crate) position: usize,
    /// The number comes from a manual entry.
    pub(crate) manual: bool,
    /// `None` for a top-level page, which has no symbol and keeps its position.
    pub(crate) placement: Option<RowPlacement>,
}

/// The project's pages, and the parent sheets that were not open, as read.
#[derive(Debug, Clone, Default)]
pub struct NumberingRead {
    pub(crate) rows: Vec<NumberingRow>,
    /// Parent sheets no engine held when the read started, for Apply to load.
    pub(crate) loaded: HashMap<PathBuf, SchematicSheet>,
    /// Sheets that exist but could not be parsed: `(path, why)`.
    pub(crate) unreadable: Vec<(PathBuf, String)>,
}

/// Read the project `plan` describes and number its pages. `open` names the
/// sheets an engine holds; their parents are not copied into `loaded`.
pub(crate) fn read_numbering(
    plan: &ProjectSheetPlan,
    project_dir: Option<&Path>,
    project_name: &str,
    open: &HashSet<PathBuf>,
) -> Result<NumberingRead, String> {
    let set = plan.assemble();
    let fold = set.fold;
    let root = plan.root();
    let base_dir = project_dir
        .map(Path::to_path_buf)
        .or_else(|| root.parent().map(PathBuf::from));
    let base = base_dir.as_deref();
    let graph = project_sheets::project_graph_with(&set.sheets, base, Some(root), fold);
    let root_key = project_sheets::sheet_key_with(root, base, fold);
    if !graph.sheets.contains_key(&root_key) {
        return Err(format!(
            "the root sheet {} could not be read",
            root.display()
        ));
    }
    let roots = project_sheets::project_roots(root_key, &set, &graph, base, fold);
    let project = signex_net::ProjectGraph {
        sheets: &graph.sheets,
        resolved: &graph.resolved,
        roots: &roots,
    };
    let (numbering, _) = signex_net::sheet_numbering(&project, project_name);
    let mut loaded = HashMap::new();
    let mut rows = Vec::with_capacity(numbering.count());
    for (index, page) in numbering.pages().iter().enumerate() {
        let placement = page.placed_by.as_ref().and_then(|by| {
            Some(RowPlacement {
                parent: graph.paths.get(&by.parent)?.clone(),
                parent_path: by.parent_path.clone(),
                symbol: by.symbol,
            })
        });
        if let Some(parent) = placement.as_ref().map(|p| &p.parent)
            && !open.contains(parent)
            && let Some(sheet) = set.sheets.get(parent)
        {
            loaded
                .entry(parent.clone())
                .or_insert_with(|| sheet.clone());
        }
        rows.push(NumberingRow {
            name: page
                .name_chain
                .last()
                .cloned()
                .unwrap_or_else(|| page.key.to_string()),
            sheet: page.key.to_string(),
            depth: page.instance_path.chain().len(),
            number: page.number,
            position: index + 1,
            manual: page.manual,
            placement,
        });
    }
    Ok(NumberingRead {
        rows,
        loaded,
        unreadable: set.unreadable,
    })
}
