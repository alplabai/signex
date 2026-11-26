//! `export_schematic_pdf` (#2610, #2787): the schematic PDF from operands.
//! A CLI or an agent names the sheet (`document`) and the file (`path`); the
//! options start as the Export PDF dialog opens them, the operands set the
//! pages, the PDF layers and the variant, and the PDF is written by the
//! dialog's own export path (`handle_export_pdf_finished`). The answer lists
//! the sheet numbers printed, the variant and the layers written.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use iced::Task;
use serde_json::json;
use signex_output::{
    ExportContext, PageRange, PageSize, PdfLayer, PdfLayerVisibility, PdfOptions, SchematicPalette,
};
use signex_types::project::{BASE_VARIANT, is_base_variant};
use signex_types::schematic::{SheetOrientation, SheetSize};

use super::page_list::parse_page_list;
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::document::DocumentKey;
use crate::app::state::PdfExportRequest;
use crate::app::{Message, Signex};

type Ran = (CommandOutcome, Task<Message>);

fn refused(reason: impl Into<String>) -> Ran {
    (CommandOutcome::disabled(reason), Task::none())
}

fn text<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.as_str()),
        _ => None,
    }
}

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
    /// The options the Export PDF dialog opens with for `ctx`: the first
    /// sheet's own paper and the theme's colours, the rest the defaults.
    pub(super) fn opening_pdf_options(&self, ctx: &ExportContext) -> PdfOptions {
        let (sheet_size, sheet_orientation) = ctx
            .sheets
            .first()
            .map(|sheet| sheet.schematic.sheet_size())
            .unwrap_or((SheetSize::IsoA4, SheetOrientation::Landscape));
        let (page_size, orientation) = PageSize::from_sheet_size(&sheet_size, sheet_orientation);
        let palette = SchematicPalette::from(&self.canvas_colors_for(self.ui_state.theme_id));
        PdfOptions {
            page_size,
            orientation,
            palette,
            ..PdfOptions::default()
        }
    }

    /// The variant `variant` names for `source`: Base, or one its project
    /// defines; left out, the project's active variant, as the dialog opens.
    fn printed_variant(&self, source: &Path, variant: Option<&str>) -> Result<String, String> {
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

    /// The sheet `args` names in `document`, refused when it names another
    /// kind or a sheet that is not loaded.
    fn named_pdf_sheet(&self, args: &CommandArgs) -> Result<PathBuf, String> {
        let Some(OperandValue::Document(named)) = args.operand("document") else {
            return Err("needs the operand `document`".to_string());
        };
        let entry = self.resolve_document_target(named)?;
        match entry.key {
            Some(DocumentKey::Schematic(path)) => Ok(path),
            Some(_) => Err(format!(
                "{} is a {}, not a schematic sheet",
                entry.name,
                entry.kind.as_str()
            )),
            None => Err(format!(
                "{} is referenced by a project but not loaded; open it first",
                entry.name
            )),
        }
    }
}

/// `export_schematic_pdf document=<sheet> path=<file> [pages=..]
/// [layers=..] [variant=..]`: the PDF, written off the UI thread.
pub(crate) fn export_schematic_pdf(app: &mut Signex, args: &CommandArgs) -> Ran {
    let source = match app.named_pdf_sheet(args) {
        Ok(source) => source,
        Err(reason) => return refused(reason),
    };
    let Some(OperandValue::Path(path)) = args.operand("path") else {
        return refused("needs the operand `path`");
    };
    let path = PathBuf::from(path);
    let Some((ctx, issues)) = super::build_page_scope_for_source(&app.document_state, &source)
    else {
        return refused(format!("{} is not open", source.display()));
    };
    if issues.channel_naming_unreadable {
        return refused("the project's channel naming cannot be read, so nothing is printed");
    }
    if ctx.sheets.is_empty() {
        return refused("the document prints no sheet");
    }
    let mut options = app.opening_pdf_options(&ctx);
    match app.printed_variant(&source, text(args, "variant")) {
        Ok(variant) => options.variant = Some(variant),
        Err(reason) => return refused(reason),
    }
    if let Some(pages) = text(args, "pages") {
        match page_range_operand(pages) {
            Ok(range) => options.page_range = range,
            Err(reason) => return refused(reason),
        }
    }
    if let Some(layers) = text(args, "layers") {
        match layers_operand(layers) {
            Ok(visibility) => options.layers = Some(visibility),
            Err(reason) => return refused(reason),
        }
    }
    // The pages the preview of this range shows, and the file prints.
    let pages = match signex_output::printed_sheet_numbers(
        &options.page_range,
        &ctx.sheets,
        ctx.current_sheet_index(),
    ) {
        Ok(pages) => pages,
        Err(error) => return refused(error.to_string()),
    };
    let layers: Vec<&str> = options
        .written_layers()
        .into_iter()
        .map(PdfLayer::group_name)
        .collect();
    let answer = json!({
        "document": source.display().to_string(),
        "path": path.display().to_string(),
        "variant": options.variant,
        "pages": pages,
        "layers": layers,
    });
    let request = PdfExportRequest {
        descended_placement: app.document_state.tab_placements.get(&source).cloned(),
        files: ctx
            .sheets
            .iter()
            .map(|sheet| (sheet.path.clone(), sheet.placement.clone()))
            .collect(),
        source,
        options,
    };
    let task = app.handle_export_pdf_finished(Box::new(request), Ok(path));
    (CommandOutcome::Answered(answer), task)
}
