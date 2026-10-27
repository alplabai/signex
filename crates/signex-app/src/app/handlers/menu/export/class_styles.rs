//! Net-class styles for a PDF export or its preview.
//!
//! Resolved from the export's own project netlist (`ExportContext::netlist`),
//! not from the canvas cache: the cache only knows the sheets open in a tab,
//! while the export also prints sheets read from disk. The class table is the
//! one of the project owning `source`, so two loaded projects never lend each
//! other classes.

use std::collections::HashMap;
use std::path::Path;

use signex_output::ExportContext;
use signex_types::net_class::NetClassStyle;
use uuid::Uuid;

use crate::app::state::DocumentState;

/// Element uuid to class style for every wire and junction of `ctx`'s netlist.
/// Empty when `source` belongs to no project, the project has no classes, or
/// no netlist was derived.
pub(super) fn export_class_styles(
    document_state: &DocumentState,
    source: &Path,
    ctx: &ExportContext,
) -> HashMap<Uuid, NetClassStyle> {
    let (Some(project), Some(netlist)) = (document_state.document_project(source), &ctx.netlist)
    else {
        return HashMap::new();
    };
    signex_net::element_class_styles(netlist, &project.data.net_classes)
}
