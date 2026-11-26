//! The files of an Export Job run and of `export_sheet_png` (#1299), from
//! the same scope as the PDF, BOM and netlist dialogs. The BOM and the
//! netlist are built in `update`, as their dialogs build them; the PDF and
//! the PNG pages are drawn off the UI thread by [`JobWork::render`].
//!
//! The pages print in the default print palette, not the window's theme,
//! so a job writes the same files on every machine (docs/OUTPUT_JOBS.md).

use std::path::Path;

use signex_output::{
    BomExporter, BomFormat, BomOptions, ExportContext, Exporter, NetlistExporter, NetlistOptions,
    PageSize, PdfExporter, PdfOptions, PreviewOptions, PreviewRasterizer, png_resolution_capped,
};
use signex_types::output_job::{OutputKind, PlannedOutput, png_page_file};
use signex_types::project::is_base_variant;

use super::ExportIssues;
use crate::app::Signex;

/// One output of a run, ready to be written.
pub(crate) enum JobWork {
    /// Bytes built in `update`: a BOM or a netlist.
    File {
        output: String,
        file: String,
        bytes: Vec<u8>,
    },
    /// The pages as one PDF, drawn off the UI thread.
    Pdf {
        output: String,
        file: String,
        ctx: Box<ExportContext>,
        options: Box<PdfOptions>,
    },
    /// The pages as PNG files, one a page, drawn off the UI thread.
    Png {
        output: String,
        file: String,
        ctx: Box<ExportContext>,
        options: Box<PreviewOptions>,
    },
}

/// One file of a run: the output it belongs to and its name in the folder.
pub(crate) struct JobFile {
    pub(crate) output: String,
    pub(crate) file: String,
    pub(crate) bytes: Vec<u8>,
}

/// The work of a run and what its scope warned about.
pub(crate) struct JobBuild {
    pub(crate) work: Vec<JobWork>,
    pub(crate) warnings: Vec<String>,
}

impl JobWork {
    /// The files of this output and its warnings. Blocking: run it off the
    /// UI thread. `Err` says why the output could not be built.
    pub(crate) fn render(self) -> Result<(Vec<JobFile>, Vec<String>), String> {
        match self {
            Self::File {
                output,
                file,
                bytes,
            } => Ok((
                vec![JobFile {
                    output,
                    file,
                    bytes,
                }],
                Vec::new(),
            )),
            Self::Pdf {
                output,
                file,
                ctx,
                options,
            } => {
                let pdf = PdfExporter
                    .export(&ctx, &options)
                    .map_err(|e| format!("{output}: PDF export failed: {e}"))?;
                let warnings = super::uncovered_chars_warning(&pdf.uncovered_chars)
                    .map(|warning| format!("{output}: {warning}"))
                    .into_iter()
                    .collect();
                let bytes = pdf.bytes;
                Ok((
                    vec![JobFile {
                        output,
                        file,
                        bytes,
                    }],
                    warnings,
                ))
            }
            Self::Png {
                output,
                file,
                ctx,
                options,
            } => render_png(output, &file, &ctx, &options),
        }
    }
}

fn render_png(
    output: String,
    file: &str,
    ctx: &ExportContext,
    options: &PreviewOptions,
) -> Result<(Vec<JobFile>, Vec<String>), String> {
    let expected = PreviewRasterizer.outline(ctx, options).len();
    if expected == 0 {
        return Err(format!("{output}: no page to draw"));
    }
    let pages = PreviewRasterizer.render_png(ctx, options);
    if pages.len() != expected {
        return Err(format!(
            "{output}: {} of {expected} pages could not be drawn",
            expected - pages.len()
        ));
    }
    let mut warnings = Vec::new();
    if png_resolution_capped(options) {
        warnings.push(format!(
            "{output}: at {} dpi the page is past {} pixels a side, so it was drawn at a lower resolution",
            options.dpi,
            signex_output::MAX_PREVIEW_EDGE_PX
        ));
    }
    // Named by print position, so two pages never share a file.
    let files = pages
        .into_iter()
        .enumerate()
        .map(|(i, page)| JobFile {
            output: output.clone(),
            file: png_page_file(file, i + 1),
            bytes: page.bytes,
        })
        .collect();
    Ok((files, warnings))
}

impl Signex {
    /// The work of `plan` for the project of the open sheet `source`, in
    /// `variant`. `Err` says why nothing can be written.
    pub(crate) fn output_job_work(
        &self,
        source: &Path,
        plan: &[PlannedOutput],
        variant: &str,
    ) -> Result<JobBuild, String> {
        let (ctx, issues) = super::build_export_scope_for_source(&self.document_state, source)
            .ok_or_else(|| format!("{} is not open", source.display()))?;
        if issues.channel_naming_unreadable {
            return Err(channel_naming_refusal());
        }
        super::log_stitch_issues(source, &ctx, &issues);
        let mut build = JobBuild {
            work: Vec::new(),
            warnings: issues.messages(),
        };
        for output in plan {
            let work = match output.kind {
                OutputKind::SchematicPdf => {
                    let (ctx, options) = self.job_pages(source, &ctx, variant)?;
                    JobWork::Pdf {
                        output: output.name.clone(),
                        file: output.file.clone(),
                        ctx: Box::new(ctx),
                        options: Box::new(options),
                    }
                }
                OutputKind::SchematicPng => {
                    let (ctx, pdf) = self.job_pages(source, &ctx, variant)?;
                    JobWork::Png {
                        output: output.name.clone(),
                        file: output.file.clone(),
                        ctx: Box::new(ctx),
                        options: Box::new(PreviewOptions {
                            pdf,
                            dpi: f64::from(output.dpi),
                        }),
                    }
                }
                OutputKind::Bom => {
                    self.job_bom(source, &ctx, variant, output, &mut build.warnings)?
                }
                OutputKind::Netlist => job_netlist(&ctx, &issues, output)?,
            };
            build.work.push(work);
        }
        Ok(build)
    }

    /// The pages of the open sheet `source`'s scope (its project, or the open
    /// loose sheets) as PNG files named from `file` at `dpi`, in the
    /// project's active variant, which comes back with the work.
    pub(crate) fn sheet_png_work(
        &self,
        source: &Path,
        file: &str,
        dpi: u32,
    ) -> Result<(JobWork, String), String> {
        let (ctx, issues) = super::build_page_scope_for_source(&self.document_state, source)
            .ok_or_else(|| format!("{} is not open", source.display()))?;
        if issues.channel_naming_unreadable {
            return Err(channel_naming_refusal());
        }
        super::log_stitch_issues(source, &ctx, &issues);
        let variant = self
            .document_state
            .document_project(source)
            .map_or(signex_types::project::BASE_VARIANT, |project| {
                project.data.active_variant_name()
            })
            .to_string();
        let (ctx, pdf) = self.job_pages(source, &ctx, &variant)?;
        let work = JobWork::Png {
            output: "PNG".to_string(),
            file: file.to_string(),
            ctx: Box::new(ctx),
            options: Box::new(PreviewOptions {
                pdf,
                dpi: f64::from(dpi),
            }),
        };
        Ok((work, variant))
    }

    /// The scope and print options of every page in `variant`: the first
    /// page's paper, the default print palette, every page.
    fn job_pages(
        &self,
        source: &Path,
        ctx: &ExportContext,
        variant: &str,
    ) -> Result<(ExportContext, PdfOptions), String> {
        let Some(first) = ctx.sheets.first() else {
            return Err("the project has no page to print".to_string());
        };
        let (sheet_size, sheet_orientation) = first.schematic.sheet_size();
        let (page_size, orientation) = PageSize::from_sheet_size(&sheet_size, sheet_orientation);
        let mut options = PdfOptions {
            page_size,
            orientation,
            ..PdfOptions::default()
        };
        options.net_class_styles =
            super::class_styles::export_class_styles(&self.document_state, source, ctx);
        options.schematic_defaults = self.document_state.schematic_defaults_for(source);
        options.variant = Some(variant.to_string());
        let mut ctx = ctx.clone();
        super::apply_printed_variant(&mut ctx.metadata, &mut options.variant);
        Ok((ctx, options))
    }

    /// The BOM of `ctx` in `variant`, as the BOM dialog writes it; its file's
    /// extension picks CSV, HTML or XLSX.
    fn job_bom(
        &self,
        source: &Path,
        ctx: &ExportContext,
        variant: &str,
        output: &PlannedOutput,
        warnings: &mut Vec<String>,
    ) -> Result<JobWork, String> {
        let mut ctx = ctx.clone();
        crate::library::drift::apply_library_names(&mut ctx, &self.library.drift.names);
        if ctx.netlist_sheets.is_empty() {
            warnings.push(format!(
                "{}: the project hierarchy could not be derived, so parts on child sheets are not listed",
                output.name
            ));
        }
        let options = BomOptions {
            active_variant: (!is_base_variant(variant)).then(|| variant.to_string()),
            format: BomFormat::from_output_path(Path::new(&output.file)),
            price_book: crate::library::bom_prices::price_book(&self.library),
            board_left_out: self.board_left_out_of_bom(source),
            ..BomOptions::default()
        };
        let bom = BomExporter
            .export(&ctx, &options)
            .map_err(|e| format!("{}: BOM export failed: {e}", output.name))?;
        let report = &bom.validation_report;
        if !report.issues.is_empty() {
            warnings.push(format!(
                "{}: the BOM checks found {} error(s) and {} warning(s)",
                output.name,
                report.error_count(),
                report.warning_count()
            ));
        }
        Ok(JobWork::File {
            output: output.name.clone(),
            file: output.file.clone(),
            bytes: bom.bytes,
        })
    }
}

/// The `.NET` netlist of `ctx`. A netlist is read by a board tool, so a
/// run refuses one with a hole in it, as the dialog does by default.
fn job_netlist(
    ctx: &ExportContext,
    issues: &ExportIssues,
    output: &PlannedOutput,
) -> Result<JobWork, String> {
    if let Some(refusal) = issues.shared_reference_refusal(ctx) {
        return Err(format!("{}: {refusal}", output.name));
    }
    if issues.netlist_is_incomplete() {
        return Err(format!(
            "{}: the project netlist is incomplete, and a run never writes a partial netlist: {}",
            output.name,
            issues.messages().join("; ")
        ));
    }
    let netlist = NetlistExporter
        .export(ctx, &NetlistOptions::default())
        .map_err(|e| format!("{}: netlist export failed: {e}", output.name))?;
    Ok(JobWork::File {
        output: output.name.clone(),
        file: output.file.clone(),
        bytes: netlist.bytes,
    })
}

fn channel_naming_refusal() -> String {
    "the project's channel naming settings cannot be read, so its channel names would be wrong"
        .to_string()
}
