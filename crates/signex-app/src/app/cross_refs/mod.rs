//! Off-sheet port cross references (#1273): beside each Global label, the
//! page and zone of every other page its net continues on.
//!
//! Which labels are partners comes from the stitched project's net index
//! (`signex_net::NetIndex::port_cross_refs`), built with the index off the
//! UI thread; this module names each partner's page and frame zone, keyed by
//! the printed page (file and placement), and lays the marks out for the
//! canvas. The PDF export reads the same table (`menu::export`). Rules:
//! docs/PDF_EXPORT.md, "Cross references".

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use signex_net::{NetIndex, ProjectGraph, SheetKey, SheetNumbering};
use signex_output::template::{Frame, FramePrimitive, zone_at};
use signex_output::{CrossRefTarget, PortCrossRefs};
use signex_types::schematic::{Point, SchematicSheet};

use crate::app::Signex;

/// The cross references of every page of `project`, keyed by the file each
/// sheet was assembled from (`paths`) and numbered by `numbering`, the
/// project numbering the export prints. A label whose net stays on its page
/// is left out.
pub(crate) fn project_cross_refs(
    project: &ProjectGraph<'_>,
    paths: &HashMap<SheetKey, PathBuf>,
    index: &NetIndex,
    numbering: &SheetNumbering,
) -> PortCrossRefs {
    let mut out = PortCrossRefs::default();
    for port in index.port_cross_refs(numbering) {
        if port.partners.is_empty() {
            continue;
        }
        let Some(path) = paths.get(&port.sheet) else {
            continue;
        };
        let targets = port
            .partners
            .iter()
            .map(|partner| CrossRefTarget {
                page: partner.page,
                zone: project
                    .sheets
                    .get(&partner.sheet)
                    .and_then(|sheet| zone_of(sheet, partner.at)),
                x_mm: partner.at.x,
                y_mm: partner.at.y,
            })
            .collect();
        out.push(path, &port.occurrence, port.label, targets);
    }
    out
}

/// The frame zone `at` lies in on `sheet`, with the frame the canvas draws
/// for it; `None` when the sheet shows no zones.
pub(crate) fn zone_of(sheet: &SchematicSheet, at: Point) -> Option<String> {
    let (width, height) = sheet.paper_mm();
    let frame = Frame::default().with_sheet_settings(&sheet.frame);
    zone_at(width, height, &frame, at.x, at.y)
}

impl Signex {
    /// The cross-reference marks of the schematic at `path`, as frame text for
    /// the canvas, from the newest index its project has (an older one stays
    /// drawn until the rebuild an edit asked for lands). Empty for a sheet in
    /// no project or with no Global label that leaves its page.
    pub(crate) fn cross_ref_frame_text(&self, path: &Path) -> Vec<FramePrimitive> {
        let document_state = &self.document_state;
        let (Some(engine), Some(project)) = (
            document_state.engines.get(path),
            document_state.document_project(path),
        ) else {
            return Vec::new();
        };
        let Some(data) = document_state.net_nav.index.latest(project.id) else {
            return Vec::new();
        };
        let placement = document_state.tab_placements.get(path);
        let Some(page) = data.cross_refs.for_page(path, placement) else {
            return Vec::new();
        };
        signex_output::cross_ref::lower_cross_refs(engine.document(), page)
            .iter()
            .map(signex_output::cross_ref::CrossRefMark::frame_text)
            .collect()
    }
}

#[cfg(test)]
mod tests;
