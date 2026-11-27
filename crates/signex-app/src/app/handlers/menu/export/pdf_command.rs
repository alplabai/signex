//! The page, layer and variant operands of `export_schematic_pdf` (#2610,
//! #2787), read with the Export PDF dialog's own rules: `pages` as the Pages
//! field takes it, `layers` as the PDF Layers list, `variant` as the variant
//! picker. The command itself is `pdf_operands::export_schematic_pdf`.

use std::collections::BTreeSet;
use std::path::Path;

use signex_output::{PageRange, PdfLayer, PdfLayerVisibility};
use signex_types::project::{BASE_VARIANT, is_base_variant};

use super::page_list::parse_page_list;
use crate::app::Signex;

/// The page range `pages` names: `all`, `current`, or sheet numbers and
/// ranges as the Pages box takes them (`1-2, 4`, mockup row 111).
pub(crate) fn page_range_operand(pages: &str) -> Result<PageRange, String> {
    match pages.trim() {
        "all" => Ok(PageRange::All),
        "current" => Ok(PageRange::Current),
        list => parse_page_list(list).map(PageRange::Specific).ok_or_else(|| {
            format!(
                "`pages` is all, current or sheet numbers and ranges such as `1-2, 4`, not `{list}`"
            )
        }),
    }
}

/// The PDF layers `layers` names: `all`, or the ids (`PdfLayer::id`) of the
/// groups visible when the PDF opens; every other group opens hidden.
pub(crate) fn layers_operand(layers: &str) -> Result<PdfLayerVisibility, String> {
    if layers.trim() == "all" {
        return Ok(PdfLayerVisibility::default());
    }
    let mut shown = BTreeSet::new();
    for id in layers.split(',').filter(|id| !id.trim().is_empty()) {
        let layer = PdfLayer::from_id(id).ok_or_else(|| {
            let ids: Vec<&str> = PdfLayer::ALL.iter().map(|layer| layer.id()).collect();
            format!(
                "`{}` is not a PDF layer (all, or one of {})",
                id.trim(),
                ids.join(", ")
            )
        })?;
        shown.insert(layer);
    }
    if shown.is_empty() {
        return Err("`layers` names no PDF layer".to_string());
    }
    Ok(PdfLayerVisibility {
        hidden_on_open: PdfLayer::ALL
            .into_iter()
            .filter(|layer| !shown.contains(layer))
            .collect(),
    })
}

impl Signex {
    /// The variant `variant` names for `source`: Base, or one its project
    /// defines; left out, the project's active variant, as the dialog opens.
    pub(super) fn printed_variant(
        &self,
        source: &Path,
        variant: Option<&str>,
    ) -> Result<String, String> {
        let project = self.document_state.document_project(source);
        let Some(name) = variant.map(str::trim) else {
            return Ok(project
                .map_or(BASE_VARIANT, |project| project.data.active_variant_name())
                .to_string());
        };
        if is_base_variant(name) {
            return Ok(BASE_VARIANT.to_string());
        }
        let defined = project.map_or(&[][..], |project| &project.data.variant_definitions[..]);
        if defined.iter().any(|known| known == name) {
            return Ok(name.to_string());
        }
        let mut names = vec![BASE_VARIANT.to_string()];
        names.extend(defined.iter().cloned());
        Err(format!(
            "`variant` is one of {}, not `{name}`",
            names.join(", ")
        ))
    }
}
