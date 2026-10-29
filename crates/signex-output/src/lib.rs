//! Output generation for Signex — PDF, BOM, netlist.
//!
//! See `docs/internal/docs/OUTPUT_PLAN.md` for the v0.8 design.
//!
//! v0.8.0 ships: PDF, netlist, print preview, sheet templates, text substitution.
//! v0.8.1 ships: BOM (CSV / HTML / XLSX).

use std::path::PathBuf;

use signex_types::schematic::SchematicSheet;
use thiserror::Error;

pub mod bom;
pub mod erc;
mod expression;
pub mod netlist;
pub mod pdf;
pub mod preview;
pub mod substitution;
pub mod svg;
pub mod template;

pub use bom::{
    BomColumn, BomError, BomExporter, BomFormat, BomGrouping, BomIssueSeverity, BomMetadata,
    BomOptions, BomOutput, BomRow, BomRule, BomRuleOptions, BomTable, BomValidationIssue,
    BomValidationReport, rollup,
};
pub use erc::{
    ErcReportError, ErcReportExporter, ErcReportFormat, ErcReportOptions, ErcReportOutput,
    ErcReportRow, ErcReportRuleSet, ErcReportSeverity, report as erc_report,
};
pub use netlist::{NetlistExporter, NetlistFormat, NetlistOptions, NetlistOutput};
pub use pdf::{
    ColourMode, MAX_PAGE_EDGE_MM, MAX_SCALE_PERCENT, MIN_CUSTOM_EDGE_MM, MIN_PRINTABLE_MM,
    MarginEdge, MarginUpdate, Margins, Orientation, PageRange, PageSetupError, PageSize,
    PageSizeKind, PdfExporter, PdfOptions, PdfOutput, PdfScale, PdfScaleMode, SchematicPalette,
    check_page_range, parse_custom_edge_mm, parse_mm, parse_percent,
};
pub use preview::{
    MAX_PREVIEW_EDGE_PX, PreviewOptions, PreviewPage, PreviewPageInfo, PreviewRasterizer,
    preview_raster_size,
};
pub use substitution::{SubstitutionContext, resolve};
pub use template::{Template, TemplateError, TemplateId, TitleBlockField};

/// The universal exporter trait — one impl per output format.
pub trait Exporter {
    type Options;
    type Output;
    type Error;

    fn export(
        &self,
        ctx: &ExportContext,
        opts: &Self::Options,
    ) -> Result<Self::Output, Self::Error>;
}

/// Everything an exporter needs to know about the project being exported.
///
/// The app layer builds this from `DocumentState` at export time — exporters
/// never touch the live application state directly.
#[derive(Debug, Clone)]
pub struct ExportContext {
    pub sheets: Vec<SheetSnapshot>,
    pub metadata: ProjectMetadata,
    /// The project's authoritative connectivity, derived once by the app via
    /// `signex_net::build_project_netlist` and handed to exporters that emit
    /// connectivity (the netlist). `None` when the caller didn't derive it
    /// (e.g. a PDF-only export). Exporters read this rather than re-deriving
    /// nets from `sheets` (ADR-0002 D7).
    pub netlist: Option<signex_types::net::Netlist>,
    /// Every sheet [`Self::netlist`] was derived from, in a stable order:
    /// the printed pages *plus* the hierarchical children reached through
    /// `child_sheets`, which are never printed pages. A terminal in the
    /// netlist can sit on any of these, so anything that joins netlist
    /// terminals back to symbols (the Protel component blocks) must walk this
    /// set, not [`Self::sheets`]. A sheet placed several times appears once.
    pub netlist_sheets: Vec<SchematicSheet>,
    /// Where each [`Self::netlist_sheets`] entry sits in the hierarchy: one
    /// entry per placement, so a sheet file placed twice is listed twice with
    /// distinct occurrence paths. Anything that joins netlist terminals back
    /// to symbols resolves each symbol's designator per placement, and the BOM
    /// lists a sheet's parts once per placement ([`Self::sheet_placements`]). A
    /// sheet with no placement is not part of the hierarchy. Empty means "not derived":
    /// each of `netlist_sheets` is one root page.
    pub netlist_placements: Vec<signex_types::net::SheetPlacement>,
    /// File of the sheet the user is looking at, for [`PageRange::Current`].
    /// Keyed by path rather than index so it survives the app filtering
    /// `sheets` down to the files ticked in the export dialog. `None`, or a
    /// path no longer in `sheets`, resolves to the first sheet. The path is
    /// compared exactly, so the caller supplies the spelling `sheets` uses
    /// (case folding is the project's rule, not this crate's), and a viewed
    /// sheet that is not a declared page must be put into `sheets` by the caller.
    pub current_sheet: Option<PathBuf>,
}

impl ExportContext {
    /// Zero-based index into [`Self::sheets`] that "Current page" prints.
    pub fn current_sheet_index(&self) -> usize {
        self.current_sheet
            .as_ref()
            .and_then(|path| self.sheets.iter().position(|s| &s.path == path))
            .unwrap_or(0)
    }

    /// Each [`Self::netlist_sheets`] entry once per placement, with the
    /// occurrence path it sits at. With no [`Self::netlist_placements`], every
    /// sheet counts once as a root page.
    pub fn sheet_placements(
        &self,
    ) -> Vec<(&SchematicSheet, signex_types::schematic::InstancePath)> {
        use signex_types::schematic::InstancePath;
        if self.netlist_placements.is_empty() {
            return self
                .netlist_sheets
                .iter()
                .map(|s| (s, InstancePath::root()))
                .collect();
        }
        self.netlist_placements
            .iter()
            .filter_map(|p| {
                Some((
                    self.netlist_sheets.get(p.sheet_index)?,
                    p.instance_path.clone(),
                ))
            })
            .collect()
    }
}

impl SheetSnapshot {
    /// This page as the copy for `placement`: every symbol's `reference`
    /// becomes the designator it resolves to there
    /// (`Symbol::designator_in`), so every renderer prints the same designators
    /// the netlist names without knowing about placements. `label` (the
    /// sheet-symbol name chain) is appended to the page title so copies of one
    /// file are told apart. The snapshot is a render copy; no document is
    /// touched.
    pub fn for_placement(
        mut self,
        placement: signex_types::schematic::InstancePath,
        label: &str,
    ) -> Self {
        for sym in &mut self.schematic.symbols {
            let resolved = sym.designator_in(&placement).to_string();
            sym.reference = resolved;
        }
        if !label.is_empty() {
            self.sheet_name = format!("{} ({label})", self.sheet_name);
        }
        self.placement = placement;
        self
    }
}

#[derive(Debug, Clone)]
pub struct SheetSnapshot {
    pub path: PathBuf,
    pub schematic: SchematicSheet,
    pub sheet_name: String,
    pub sheet_number: usize,
    pub sheet_count: usize,
    /// The sheet placement this page is a copy of; the root chain for a page
    /// printed as itself. Pin→net names are read at this occurrence, and
    /// [`Self::for_placement`] has already resolved the symbols' designators
    /// for it.
    pub placement: signex_types::schematic::InstancePath,
}

/// Title-block / project-file metadata used to resolve `${TITLE}`, `${REV}`,
/// etc. and to stamp the exported artifact.
///
/// `title`..`comments` and the title-block entries of `custom_fields` are the
/// values for one sheet: the project's parameters with that sheet's own
/// `title_block` laid over them ([`Self::for_sheet`]). `project_parameters`
/// keeps the project-level values alone, so another sheet can start from them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectMetadata {
    pub title: String,
    pub revision: String,
    pub date: String,
    pub company: String,
    pub comments: [String; 4],
    pub custom_fields: std::collections::BTreeMap<String, String>,
    /// The project's parameters (`.snxprj` `parameters`), before any sheet
    /// override. `None` for metadata built by hand, whose own values then
    /// stand in for the project's.
    pub project_parameters: Option<std::collections::BTreeMap<String, String>>,
}

/// Entries of `custom_fields` that describe the export, not a title-block
/// value; a sheet's metadata keeps them from the metadata it derives from.
const EXPORT_FIELDS: [&str; 2] = ["active_variant", "project_name"];

impl ProjectMetadata {
    /// Metadata from title-block `values`: the keys `title`, `rev`, `date`,
    /// `company` and `comment1`-`comment4` fill their slots; every other key
    /// (`document_number`, `drawn_by`, user parameters, ...) is a custom field.
    pub fn from_values(values: &std::collections::BTreeMap<String, String>) -> Self {
        let mut out = Self::default();
        for (key, value) in values {
            let slot = match key.as_str() {
                "title" => &mut out.title,
                "rev" => &mut out.revision,
                "date" => &mut out.date,
                "company" => &mut out.company,
                "comment1" => &mut out.comments[0],
                "comment2" => &mut out.comments[1],
                "comment3" => &mut out.comments[2],
                "comment4" => &mut out.comments[3],
                _ => {
                    out.custom_fields.insert(key.clone(), value.clone());
                    continue;
                }
            };
            *slot = value.clone();
        }
        out
    }

    /// The title-block values this metadata holds, keyed as a sheet's
    /// `title_block` (the inverse of [`Self::from_values`]).
    fn values(&self) -> std::collections::BTreeMap<String, String> {
        let mut out: std::collections::BTreeMap<String, String> = self
            .custom_fields
            .iter()
            .filter(|(k, _)| !EXPORT_FIELDS.contains(&k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (key, value) in [
            ("title", &self.title),
            ("rev", &self.revision),
            ("date", &self.date),
            ("company", &self.company),
        ] {
            out.insert(key.to_string(), value.clone());
        }
        for (n, value) in self.comments.iter().enumerate() {
            out.insert(format!("comment{}", n + 1), value.clone());
        }
        out
    }

    /// This metadata for one sheet: the project parameters, each overridden by
    /// the sheet's `title_block` value when that is not empty (an empty value
    /// falls back to the project's).
    pub fn for_sheet(&self, sheet_title_block: &std::collections::HashMap<String, String>) -> Self {
        let mut values = self
            .project_parameters
            .clone()
            .unwrap_or_else(|| self.values());
        for (key, value) in sheet_title_block {
            if !value.is_empty() {
                values.insert(key.clone(), value.clone());
            }
        }
        let mut out = Self::from_values(&values);
        for key in EXPORT_FIELDS {
            if let Some(value) = self.custom_fields.get(key) {
                out.custom_fields.insert(key.to_string(), value.clone());
            }
        }
        out.project_parameters = self.project_parameters.clone();
        out
    }
}

#[derive(Debug, Error)]
pub enum ExportError {
    #[error("bom: {0}")]
    Bom(#[from] bom::BomError),

    #[error("pdf: {0}")]
    Pdf(#[from] pdf::PdfError),

    #[error("netlist: {0}")]
    Netlist(#[from] netlist::NetlistError),

    #[error("erc report: {0}")]
    Erc(#[from] erc::ErcReportError),

    #[error("template: {0}")]
    Template(#[from] template::TemplateError),
}
