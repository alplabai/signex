//! `export_schematic_pdf` from its operands (#1298, #2609, #2610, #2787): a
//! CLI or an agent names the sheet (`document`) and the file (`path`) and
//! gets the PDF Export PDF writes for that sheet's pages, with the pages,
//! layers, variant, watermark and links the operands ask for (the page,
//! layer and variant rules are in `pdf_command`). The options start where
//! the dialog starts and the file goes through the dialog's completion path,
//! so both surfaces write the same PDF. Rules: docs/PDF_EXPORT.md.

use std::path::{Path, PathBuf};

use iced::Task;
use serde_json::{Value, json};
use signex_i18n::{Translatable, ids};
use signex_output::{ExportContext, PageSize, PdfLayer, PdfOptions, Watermark, WatermarkPages};
use signex_types::schematic::{SheetOrientation, SheetSize};

use super::pdf_command::{layers_operand, page_range_operand};
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::document_ids::DocumentKind;
use crate::app::state::{
    PageKey, PdfExportRequest, WatermarkDraft, WatermarkPreset, colour_hex, parse_watermark_angle,
    parse_watermark_opacity,
};
use crate::app::{Message, Signex};

type Ran = (CommandOutcome, Task<Message>);

fn refused(reason: Translatable) -> Ran {
    (CommandOutcome::Disabled { reason }, Task::none())
}

fn text_operand<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.as_str()),
        _ => None,
    }
}

fn flag_operand(args: &CommandArgs, name: &str) -> Option<bool> {
    match args.operand(name) {
        Some(OperandValue::Bool(value)) => Some(*value),
        _ => None,
    }
}

/// The operands that style a watermark; each needs `watermark` too.
const WATERMARK_STYLE: [&str; 5] = [
    "watermark_color",
    "watermark_opacity",
    "watermark_angle",
    "watermark_pages",
    "watermark_revision",
];

/// The watermark the operands ask for, built with the dialog's own rules
/// ([`WatermarkDraft`]); `None` without `watermark`. `revision` and `date`
/// fill the revision line when `watermark_revision` is on.
fn watermark_of(
    args: &CommandArgs,
    revision: String,
    date: String,
) -> Result<Option<Watermark>, Translatable> {
    let Some(text) = text_operand(args, "watermark") else {
        if let Some(name) = WATERMARK_STYLE.iter().find(|n| args.operand(n).is_some()) {
            return Err(
                Translatable::new(ids::PDF_PREVIEW_COMMAND_NEEDS_WATERMARK).arg("name", *name)
            );
        }
        return Ok(None);
    };
    let mut draft = WatermarkDraft {
        is_on: true,
        revision,
        date,
        ..WatermarkDraft::default()
    };
    match text.trim().to_ascii_lowercase().as_str() {
        "" => return Err(Translatable::new(ids::PDF_PREVIEW_COMMAND_WATERMARK_TEXT)),
        "draft" => draft.pick_preset(WatermarkPreset::Draft),
        "confidential" => draft.pick_preset(WatermarkPreset::Confidential),
        _ => draft.set_text(text.to_owned()),
    }
    if let Some(OperandValue::Color(r, g, b)) = args.operand("watermark_color") {
        draft.pick_colour([*r, *g, *b]);
    }
    if let Some(opacity) = text_operand(args, "watermark_opacity") {
        if parse_watermark_opacity(opacity).is_none() {
            return Err(
                Translatable::new(ids::PDF_PREVIEW_COMMAND_WATERMARK_OPACITY).arg("value", opacity),
            );
        }
        draft.set_opacity(opacity.to_owned());
    }
    if let Some(angle) = text_operand(args, "watermark_angle") {
        if parse_watermark_angle(angle).is_none() {
            return Err(
                Translatable::new(ids::PDF_PREVIEW_COMMAND_WATERMARK_ANGLE).arg("value", angle)
            );
        }
        draft.set_angle(angle.to_owned());
    }
    if let Some(pages) = text_operand(args, "watermark_pages") {
        draft.settings.pages = match pages.trim() {
            "all" => WatermarkPages::All,
            "first" => WatermarkPages::FirstOnly,
            other => {
                return Err(
                    Translatable::new(ids::PDF_PREVIEW_COMMAND_WATERMARK_PAGES).arg("value", other)
                );
            }
        };
    }
    draft.with_revision = flag_operand(args, "watermark_revision").unwrap_or(false);
    Ok(draft.output())
}

/// The watermark in the answer: what prints, or `null`.
fn watermark_answer(watermark: Option<&Watermark>) -> Value {
    let Some(watermark) = watermark else {
        return Value::Null;
    };
    let pages = match watermark.pages {
        WatermarkPages::All => "all",
        WatermarkPages::FirstOnly => "first",
    };
    json!({
        "text": watermark.text.printed(),
        "color": colour_hex(watermark.colour_rgb),
        "opacity": watermark.opacity_percent,
        "angle": watermark.angle_deg,
        "pages": pages,
        "revision_line": watermark.revision_line,
    })
}

impl Signex {
    /// The open sheet `document` names. Refused with the reason when it names
    /// none, names another kind, or the sheet is not open.
    fn named_export_sheet(&self, args: &CommandArgs) -> Result<PathBuf, Translatable> {
        let Some(OperandValue::Document(named)) = args.operand("document") else {
            return Err(Translatable::new(ids::PDF_PREVIEW_COMMAND_NEEDS_DOCUMENT));
        };
        let entry = self.resolve_document_target(named).map_err(|reason| {
            Translatable::new(ids::PDF_PREVIEW_COMMAND_DOCUMENT).arg("reason", reason)
        })?;
        if entry.kind != DocumentKind::Schematic {
            return Err(Translatable::new(ids::PDF_PREVIEW_COMMAND_NOT_A_SHEET)
                .arg("name", entry.name.as_str())
                .arg("kind", entry.kind.as_str()));
        }
        let path = entry.path.ok_or_else(|| {
            Translatable::new(ids::PDF_PREVIEW_COMMAND_NO_FILE).arg("name", entry.name.as_str())
        })?;
        if !self.document_state.engines.contains_key(&path) {
            return Err(Translatable::new(ids::PDF_PREVIEW_COMMAND_NOT_OPEN)
                .arg("name", entry.name.as_str()));
        }
        Ok(path)
    }

    /// The options a PDF of `ctx`'s pages starts from, in the dialog and
    /// headless alike: the first sheet's paper, the theme's palette (so the
    /// PDF looks like the canvas) and the active variant of the project that
    /// owns `source`, always named, Base included (#2430).
    pub(crate) fn sheet_pdf_options(&self, ctx: &ExportContext, source: &Path) -> PdfOptions {
        let (sheet_size, sheet_orientation) = ctx
            .sheets
            .first()
            .map(|s| s.schematic.sheet_size())
            .unwrap_or((SheetSize::IsoA4, SheetOrientation::Landscape));
        let (page_size, orientation) = PageSize::from_sheet_size(&sheet_size, sheet_orientation);
        let palette =
            signex_output::SchematicPalette::from(&self.canvas_colors_for(self.ui_state.theme_id));
        let variant = self
            .document_state
            .document_project(source)
            .map_or(signex_types::project::BASE_VARIANT, |p| {
                p.data.active_variant_name()
            })
            .to_string();
        PdfOptions {
            page_size,
            orientation,
            palette,
            variant: Some(variant),
            // The user's "Cross out not-fitted parts" preference (#1294).
            draw_not_fitted: self.ui_state.cross_out_not_fitted,
            ..PdfOptions::default()
        }
    }
}

/// Sets the variant, the pages and the layers the operands name on `options`
/// and returns the sheet numbers the file prints (as the preview shows them),
/// or why an operand is refused.
fn set_page_operands(
    app: &Signex,
    args: &CommandArgs,
    source: &Path,
    ctx: &ExportContext,
    options: &mut PdfOptions,
) -> Result<Value, Translatable> {
    options.variant = Some(app.printed_variant(source, text_operand(args, "variant"))?);
    if let Some(pages) = text_operand(args, "pages") {
        options.page_range = page_range_operand(pages)?;
    }
    if let Some(layers) = text_operand(args, "layers") {
        options.layers = Some(layers_operand(layers)?);
    }
    let pages = signex_output::printed_sheet_numbers(
        &options.page_range,
        &ctx.sheets,
        ctx.current_sheet_index(),
    )
    .map_err(|error| {
        Translatable::new(ids::PDF_PREVIEW_COMMAND_PAGES_NOT_PRINTED)
            .arg("reason", error.to_string())
    })?;
    Ok(json!(pages))
}

/// The file each printed page prints, in print order, and the table rows the
/// PDF carries as links to a page (#2663, docs/PDF_EXPORT.md, "Table of
/// contents links"), pages and rows counted from 1. The pages were checked
/// already, so an error here leaves both lists empty.
fn table_link_answer(ctx: &ExportContext, options: &PdfOptions) -> (Vec<Value>, Vec<Value>) {
    let current = ctx.current_sheet_index();
    let page_files = signex_output::printed_sheet_paths(&options.page_range, &ctx.sheets, current)
        .unwrap_or_default()
        .iter()
        .map(|file| json!(file.display().to_string()))
        .collect();
    let table_links = signex_output::pdf_table_links(ctx, options)
        .unwrap_or_default()
        .iter()
        .map(|link| {
            json!({
                "page": link.page + 1,
                "row": link.row + 1,
                "link": link.link,
                "target_page": link.target_page + 1,
            })
        })
        .collect();
    (page_files, table_links)
}

/// `export_schematic_pdf document=<sheet> path=<file> [options]`: the pages
/// `pages` names (every page of the sheet's project when left out), as Export
/// PDF opens on it, built and written off the UI thread; the answer says what
/// is written where.
pub(crate) fn export_schematic_pdf(app: &mut Signex, args: &CommandArgs) -> Ran {
    let source = match app.named_export_sheet(args) {
        Ok(source) => source,
        Err(reason) => return refused(reason),
    };
    let Some(OperandValue::Path(path)) = args.operand("path") else {
        return refused(Translatable::new(ids::PDF_PREVIEW_COMMAND_NEEDS_PATH));
    };
    let path = PathBuf::from(path);
    // A Properties field still being typed on a sheet of the scope prints
    // as typed, as Export PDF in the dialog commits it (#2663).
    app.commit_drafts_in_scope_of(Some(&source));
    let Some((ctx, issues)) = super::build_export_scope_for_source(&app.document_state, &source)
    else {
        return refused(Translatable::new(ids::PDF_PREVIEW_COMMAND_SHEET_CLOSED));
    };
    if issues.channel_naming_unreadable {
        return refused(Translatable::new(ids::PDF_PREVIEW_COMMAND_CHANNEL_NAMING));
    }
    if ctx.sheets.is_empty() {
        return refused(Translatable::new(ids::PDF_PREVIEW_COMMAND_NO_SHEET));
    }
    let revision = ctx.metadata.revision.clone();
    let watermark = match watermark_of(args, revision, super::print_preview::export_date()) {
        Ok(watermark) => watermark,
        Err(reason) => return refused(reason),
    };
    let mut options = app.sheet_pdf_options(&ctx, &source);
    let pages = match set_page_operands(app, args, &source, &ctx, &mut options) {
        Ok(pages) => pages,
        Err(reason) => return refused(reason),
    };
    options.watermark = watermark;
    options.links = flag_operand(args, "links").unwrap_or(true);
    let files: std::collections::HashSet<PageKey> = ctx
        .sheets
        .iter()
        .map(|s| (s.path.clone(), s.placement.clone()))
        .collect();
    let layers: Vec<&str> = options
        .written_layers()
        .into_iter()
        .map(PdfLayer::group_name)
        .collect();
    let (page_files, table_links) = table_link_answer(&ctx, &options);
    let answer = json!({
        "document": source.display().to_string(),
        "path": path.display().to_string(),
        "sheets": files.len(),
        "variant": options.variant,
        "pages": pages,
        "page_files": page_files,
        "layers": layers,
        "watermark": watermark_answer(options.watermark.as_ref()),
        "links": options.links,
        "table_links": table_links,
    });
    // The tab's descended placement picks the page `pages=current` prints.
    let request = Box::new(PdfExportRequest {
        descended_placement: app.document_view.tab_placements.get(&source).cloned(),
        source,
        options,
        files,
    });
    let task = app.handle_export_pdf_finished(request, Ok(path));
    (CommandOutcome::Answered(answer), task)
}

#[cfg(test)]
mod tests;
