//! Off-sheet port cross references (#1273): beside each Global label, the
//! page and zone of every other page its net continues on, `[2/B3, 4/A1]`.
//!
//! The app derives which labels are partners from the stitched project
//! (`signex_net::NetIndex::port_cross_refs`) and names each partner's page
//! and zone; this module lays the text out next to the label's flag. The
//! canvas, the print preview and the PDF draw the same marks, and the PDF
//! makes each entry a link to its page. Rules: docs/PDF_EXPORT.md,
//! "Cross references".

use std::path::{Path, PathBuf};

use signex_types::schematic::{InstancePath, Label, LabelType, SCHEMATIC_TEXT_MM, SchematicSheet};
use uuid::Uuid;

use crate::template::{FontStyle, FramePrimitive, FrameTextAnchor};

/// The face the marks are drawn in: the frame's regular face.
const MARK_FAMILY: &str = "Roboto";
/// Cap height of the marks against their label's.
const MARK_SCALE: f64 = 0.8;
/// Gap between the flag's far end and the mark, in mark ems.
const MARK_GAP_EM: f64 = 0.5;

/// One page a label's net continues on.
#[derive(Debug, Clone, PartialEq)]
pub struct CrossRefTarget {
    /// The page number printed for it.
    pub page: usize,
    /// The frame zone of the partner on its page; `None` when that page
    /// draws no zones.
    pub zone: Option<String>,
    /// Where the partner sits on its sheet, in mm.
    pub x_mm: f64,
    pub y_mm: f64,
}

impl CrossRefTarget {
    /// The entry as printed: `2/B3`, or `2` without a zone.
    pub fn entry(&self) -> String {
        match &self.zone {
            Some(zone) => format!("{}/{zone}", self.page),
            None => self.page.to_string(),
        }
    }
}

/// The cross references of one printed page: a sheet file in one placement.
#[derive(Debug, Clone, PartialEq)]
pub struct PageCrossRefs {
    pub path: PathBuf,
    pub placement: InstancePath,
    /// Each Global label with its targets, in the order they were added.
    pub labels: Vec<(Uuid, Vec<CrossRefTarget>)>,
}

/// Every page's cross references.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PortCrossRefs {
    pages: Vec<PageCrossRefs>,
}

impl PortCrossRefs {
    /// Add `label`'s targets to the page `path` in `placement`.
    pub fn push(
        &mut self,
        path: &Path,
        placement: &InstancePath,
        label: Uuid,
        targets: Vec<CrossRefTarget>,
    ) {
        let at = self
            .pages
            .iter()
            .position(|p| p.path == path && &p.placement == placement);
        let page = match at {
            Some(at) => &mut self.pages[at],
            None => {
                self.pages.push(PageCrossRefs {
                    path: path.to_path_buf(),
                    placement: placement.clone(),
                    labels: Vec::new(),
                });
                let last = self.pages.len() - 1;
                &mut self.pages[last]
            }
        };
        page.labels.push((label, targets));
    }

    pub fn pages(&self) -> &[PageCrossRefs] {
        &self.pages
    }

    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }

    /// The page `path` in `placement`; the first placement of `path` when
    /// `placement` is `None` or not one of its pages.
    pub fn for_page(
        &self,
        path: &Path,
        placement: Option<&InstancePath>,
    ) -> Option<&PageCrossRefs> {
        let mut of_path = self.pages.iter().filter(|p| p.path == path);
        match placement {
            Some(placement) => self
                .pages
                .iter()
                .find(|p| p.path == path && &p.placement == placement)
                .or_else(|| of_path.next()),
            None => of_path.next(),
        }
    }
}

/// One entry of a mark that the PDF links: the box it covers on its sheet
/// (mm, top-left origin, Y down) and the page and point it leads to.
#[derive(Debug, Clone, PartialEq)]
pub struct CrossRefLink {
    pub page: usize,
    pub target_mm: (f64, f64),
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

/// The cross-reference text of one label, laid out on its sheet.
#[derive(Debug, Clone, PartialEq)]
pub struct CrossRefMark {
    pub label: Uuid,
    /// Top-left corner of the text line, sheet mm.
    pub x: f64,
    pub y: f64,
    /// Em size of the text, in mm, as frame text is sized.
    pub size_mm: f64,
    pub text: String,
    pub links: Vec<CrossRefLink>,
}

impl CrossRefMark {
    /// The mark as a frame text primitive, so every frame consumer draws it.
    pub fn frame_text(&self) -> FramePrimitive {
        FramePrimitive::Text {
            x: self.x,
            y: self.y,
            size_mm: self.size_mm,
            style: FontStyle::Normal,
            anchor: FrameTextAnchor::TopLeft,
            text: self.text.clone(),
        }
    }
}

/// The marks of `page` on `sheet`: one per Global label of the sheet that has
/// a target. A label no longer on the sheet is skipped.
pub fn lower_cross_refs(sheet: &SchematicSheet, page: &PageCrossRefs) -> Vec<CrossRefMark> {
    page.labels
        .iter()
        .filter_map(|(uuid, targets)| {
            let label = sheet
                .labels
                .iter()
                .find(|l| l.uuid == *uuid && l.label_type == LabelType::Global)?;
            mark_for(label, sheet.font_family.as_deref(), targets)
        })
        .collect()
}

/// `targets` once per printed entry (page and zone), in order.
fn unique_entries(targets: &[CrossRefTarget]) -> Vec<(String, &CrossRefTarget)> {
    let mut entries: Vec<(String, &CrossRefTarget)> = Vec::new();
    for target in targets {
        let entry = target.entry();
        if !entries.iter().any(|(seen, _)| *seen == entry) {
            entries.push((entry, target));
        }
    }
    entries
}

fn joined(entries: &[(String, &CrossRefTarget)]) -> String {
    let parts: Vec<&str> = entries.iter().map(|(e, _)| e.as_str()).collect();
    format!("[{}]", parts.join(", "))
}

/// The text printed beside a label with `targets`, `[2/B3, 4/A1]`, one entry
/// per page and zone; `None` with no target.
pub fn cross_ref_text(targets: &[CrossRefTarget]) -> Option<String> {
    let entries = unique_entries(targets);
    (!entries.is_empty()).then(|| joined(&entries))
}

/// The mark beside `label`'s flag listing `targets`; `None` with no target.
fn mark_for(
    label: &Label,
    document_family: Option<&str>,
    targets: &[CrossRefTarget],
) -> Option<CrossRefMark> {
    let entries = unique_entries(targets);
    if entries.is_empty() {
        return None;
    }
    let cap_mm = label.font_size.max(SCHEMATIC_TEXT_MM) * MARK_SCALE;
    let size_mm = cap_mm / f64::from(signex_types::font::mm_per_em(Some(MARK_FAMILY)));
    let width = |text: &str| signex_types::font::family_width_mm(text, Some(MARK_FAMILY), cap_mm);
    let text = joined(&entries);
    let line_h = size_mm * f64::from(signex_types::font::TEXT_LINE_HEIGHT_EM);
    let (x, y) = mark_origin(label, document_family, width(&text), line_h, size_mm);

    let mut prefix = String::from("[");
    let mut links = Vec::with_capacity(entries.len());
    for (i, (entry, target)) in entries.iter().enumerate() {
        if i > 0 {
            prefix.push_str(", ");
        }
        let x0 = x + width(&prefix);
        prefix.push_str(entry);
        links.push(CrossRefLink {
            page: target.page,
            target_mm: (target.x_mm, target.y_mm),
            x0,
            y0: y,
            x1: x + width(&prefix),
            y1: y + line_h,
        });
    }
    Some(CrossRefMark {
        label: label.uuid,
        x,
        y,
        size_mm,
        text,
        links,
    })
}

/// Top-left corner of a `text_w` × `line_h` mark past the far end of
/// `label`'s flag, on the side the flag points to. The flag runs from the
/// anchor along the label's rotation, as wide as its text plus padding (the
/// canvas flag geometry).
fn mark_origin(
    label: &Label,
    document_family: Option<&str>,
    text_w: f64,
    line_h: f64,
    size_mm: f64,
) -> (f64, f64) {
    let cap_mm = label.font_size.max(SCHEMATIC_TEXT_MM);
    let family = signex_types::font::resolve_family(label.font_family.as_deref(), document_family);
    let em_mm = cap_mm / f64::from(signex_types::font::mm_per_em(family.as_deref()));
    let body = signex_types::font::family_width_mm(&label.text, family.as_deref(), cap_mm)
        + 2.0 * 0.325 * em_mm;
    let (sin, cos) = label.rotation.to_radians().sin_cos();
    let far = (label.position.x + body * cos, label.position.y + body * sin);
    let gap = MARK_GAP_EM * size_mm;
    if cos > 0.5 {
        (far.0 + gap, far.1 - line_h / 2.0)
    } else if cos < -0.5 {
        (far.0 - gap - text_w, far.1 - line_h / 2.0)
    } else if sin > 0.0 {
        (far.0 - text_w / 2.0, far.1 + gap)
    } else {
        (far.0 - text_w / 2.0, far.1 - gap - line_h)
    }
}

#[cfg(test)]
#[path = "cross_ref_tests.rs"]
mod tests;
