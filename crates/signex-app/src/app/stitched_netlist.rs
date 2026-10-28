//! The stitched project netlist of the project that owns a document.
//!
//! Same sheet set, graph and roots the export derives its netlist from
//! (`menu::export::build_scope`): open tabs supply their live content and
//! every other sheet the hierarchy or the project list reaches is read from
//! disk. A consumer that must agree with the exported connectivity (the
//! net-class styling of the canvas) calls this instead of resolving a net on
//! one sheet alone.

use std::path::{Path, PathBuf};

use signex_types::net::Netlist;

use super::project_sheets::{OpenSheets, ProjectSheetSet, stitch_issue_message};
use super::state::scope::CaseFold;

/// A stitched netlist and what went wrong while assembling it.
pub(crate) struct Stitched {
    /// `None` when the root sheet never assembled, so there is no
    /// connectivity to answer with.
    pub(crate) netlist: Option<Netlist>,
    /// One line per unreadable sheet and stitch issue, as the Messages panel
    /// shows them for the project netlist the canvas and ERC read.
    pub(crate) warnings: Vec<String>,
}

/// The project netlist rooted at `root`, over a snapshot of the open sheets,
/// so it can run on a blocking thread (it must not run inside `update`): only
/// the sheets absent from `open` are read from disk.
pub(crate) fn stitched_project_netlist_from(
    open: &OpenSheets,
    pages: &[PathBuf],
    root: &Path,
    fold: CaseFold,
    project_dir: Option<PathBuf>,
) -> Stitched {
    let set = super::project_sheets::assemble_project_sheets_from(open, pages, root, fold);
    netlist_of_set(set, project_dir)
}

/// The Messages line for a sheet that exists but would not read or parse. The
/// panel keeps only the first ~157 bytes and a parser error can be long, so
/// the file name comes right after a short reason and the full path last.
fn unreadable_message(path: &Path, why: &str) -> String {
    format!(
        "Netlist: {} {why}; net-class styles missing: '{}'",
        path.file_name().unwrap_or_default().to_string_lossy(),
        path.display()
    )
}

fn netlist_of_set(set: ProjectSheetSet, project_dir: Option<PathBuf>) -> Stitched {
    let mut warnings: Vec<String> = set
        .unreadable
        .iter()
        .map(|(path, why)| unreadable_message(path, why))
        .collect();
    let Some(root_path) = set.root.clone() else {
        return Stitched {
            netlist: None,
            warnings,
        };
    };
    let base_dir: Option<PathBuf> = project_dir.or_else(|| root_path.parent().map(PathBuf::from));
    let fold = set.fold;
    let graph = super::project_sheets::project_graph_with(
        &set.sheets,
        base_dir.as_deref(),
        Some(&root_path),
        fold,
    );
    let root_key = super::project_sheets::sheet_key_with(&root_path, base_dir.as_deref(), fold);
    if !graph.sheets.contains_key(&root_key) {
        warnings.extend(graph.issues.iter().map(stitch_issue_message));
        warnings.push(format!(
            "Netlist: {} is not part of the assembled project; net-class styles not \
             refreshed: '{}'",
            root_path.file_name().unwrap_or_default().to_string_lossy(),
            root_path.display()
        ));
        return Stitched {
            netlist: None,
            warnings,
        };
    }
    let roots =
        super::project_sheets::project_roots(root_key, &set, &graph, base_dir.as_deref(), fold);
    let project = signex_net::ProjectGraph {
        sheets: &graph.sheets,
        resolved: &graph.resolved,
        roots: &roots,
    };
    let result = signex_net::build_project_netlist(&project);
    warnings.extend(graph.issues.iter().map(stitch_issue_message));
    warnings.extend(result.issues.iter().map(stitch_issue_message));
    Stitched {
        netlist: Some(result.netlist),
        warnings,
    }
}

/// Test seam: stitch the project of the active document the way the
/// net-class job plans it (declared root, open tabs live, no extra pages).
#[cfg(test)]
pub(crate) fn stitched_for_active_project(document: &super::state::DocumentState) -> Stitched {
    let root = super::project_sheets::resolve_netlist_root(document).expect("an active document");
    let project = document.active_document_project();
    let fold = super::state::project_case::fold_of(project);
    let project_dir = project.map(|p| p.dir().to_path_buf());
    stitched_project_netlist_from(
        &super::project_sheets::open_sheets_snapshot(document),
        &[],
        &root,
        fold,
        project_dir,
    )
}

#[cfg(test)]
mod tests;
