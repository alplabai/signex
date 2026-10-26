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

use super::state::DocumentState;

/// The project netlist for the project owning `source` (or `source` alone when
/// it is loose). `None` when the root sheet never assembled, so there is no
/// connectivity to answer with.
pub(crate) fn stitched_project_netlist(
    document_state: &DocumentState,
    source: &Path,
) -> Option<Netlist> {
    let owning_project = document_state.document_project(source);
    let (_pages, set) = super::project_sheets::assemble_project_sheets_for(document_state, source);
    let root_path = set.root.clone()?;
    let base_dir: Option<PathBuf> = owning_project
        .map(|p| p.dir().to_path_buf())
        .or_else(|| root_path.parent().map(PathBuf::from));
    let fold = set.fold;
    let graph = super::project_sheets::project_graph_with(
        &set.sheets,
        base_dir.as_deref(),
        Some(&root_path),
        fold,
    );
    let root_key = super::project_sheets::sheet_key_with(&root_path, base_dir.as_deref(), fold);
    if !graph.sheets.contains_key(&root_key) {
        return None;
    }
    let roots =
        super::project_sheets::project_roots(root_key, &set, &graph, base_dir.as_deref(), fold);
    let project = signex_net::ProjectGraph {
        sheets: &graph.sheets,
        resolved: &graph.resolved,
        roots: &roots,
    };
    Some(signex_net::build_project_netlist(&project).netlist)
}
