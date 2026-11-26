//! `export_schematic_pdf document=<sheet> path=<file>` (#2663): every page of
//! the sheet's project as one PDF, built and written off the UI thread by the
//! Print Preview's own export path. The answer lists the pages in print order
//! and the table rows the PDF carries as links to a page (the contents
//! table's rows, docs/PDF_EXPORT.md, "Table of contents links").

use std::path::PathBuf;

use iced::Task;
use serde_json::{Value, json};
use signex_i18n::{Translatable, ids};

use super::document_operands::named;
use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::document::DocumentKey;
use crate::app::{Message, Signex};

type Ran = (CommandOutcome, Task<Message>);

fn disabled(reason: Translatable) -> Ran {
    (CommandOutcome::Disabled { reason }, Task::none())
}

/// The open sheet `document` names; any other answer is the refusal.
fn named_open_sheet(app: &Signex, args: &CommandArgs) -> Result<PathBuf, Translatable> {
    let shown = match args.operand("document") {
        Some(OperandValue::Document(target)) => target.to_string(),
        _ => String::new(),
    };
    let not_a_sheet =
        || Translatable::new(ids::PDF_EXPORT_NOT_A_SHEET).arg("document", shown.clone());
    match named(app, args).map(|entry| entry.key) {
        Ok(Some(DocumentKey::Schematic(path)))
            if app.document_state.engines.contains_key(&path) =>
        {
            Ok(path)
        }
        _ => Err(not_a_sheet()),
    }
}

pub(super) fn export_schematic_pdf(app: &mut Signex, args: &CommandArgs) -> Ran {
    let source = match named_open_sheet(app, args) {
        Ok(source) => source,
        Err(reason) => return disabled(reason),
    };
    let Some(OperandValue::Path(path)) = args.operand("path") else {
        return disabled(Translatable::new(ids::PDF_EXPORT_PATH_NEEDED));
    };
    let path = PathBuf::from(path);
    let (ctx, options) = match app.schematic_pdf_job(&source) {
        Ok(job) => job,
        Err(reason) => return disabled(reason),
    };
    // Every page prints, so the links cannot miss a page; an error here is
    // a project with no page, which the job refused already.
    let links = signex_output::pdf_table_links(&ctx, &options).unwrap_or_default();
    let pages: Vec<Value> = ctx
        .sheets
        .iter()
        .map(|s| json!(s.path.display().to_string()))
        .collect();
    let table_links: Vec<Value> = links
        .iter()
        .map(|l| {
            json!({
                "page": l.page + 1,
                "row": l.row + 1,
                "link": l.link,
                "target_page": l.target_page + 1,
            })
        })
        .collect();
    let answer = json!({
        "document": source.display().to_string(),
        "path": path.display().to_string(),
        "pages": pages,
        "table_links": table_links,
    });
    let task = app.export_pdf(ctx, options, path, source);
    (CommandOutcome::Answered(answer), task)
}
