//! The status footer (mockup rows 195, 196 and 199): the compile result
//! (time and pages, or the failure and the last good compile's time), the
//! references and how many are stale, the block counts with the
//! measurement results and stale attachments, the save state, and the
//! cursor's line and column.

use signex_i18n::{Localized, ids, t};
use signex_widgets::doc_tab_shell::Footer;

use super::Look;
use crate::app::notebook_measurements::views::strip_counts;
use crate::app::notebook_references::views::summary_line;
use crate::app::notebook_tab::{Load, NotebookTab};

pub(super) fn view<'a>(look: Look<'a>, tab: &NotebookTab) -> Footer<'a> {
    if look.doc.load != Load::Open {
        return Footer::new(look.t(ids::NOTEBOOK_TAB_STATUS_NONE));
    }
    let status = t!(
        look.i18n,
        ids::NOTEBOOK_TAB_STATUS,
        "compile" => compile_text(look, tab).into_string(),
        "references" => references_text(look, tab).into_string(),
        "blocks" => blocks_text(look, tab),
        "saved" => saved_text(look).into_string()
    );
    let (line, column) = tab.text.as_ref().map_or((0, 0), |text| text.cursor());
    let cursor = t!(
        look.i18n,
        ids::NOTEBOOK_TAB_STATUS_CURSOR,
        "line" => look.doc.file_line(line),
        "column" => column + 1
    );
    Footer::new(status).note(cursor)
}

/// "Compiled in 41 ms · 12 pages", "Compile failed · 1 error · preview
/// from 14:02:18", or "Compiling…" before the first compile.
pub(super) fn compile_text(look: Look<'_>, tab: &NotebookTab) -> Localized {
    let Some(last) = &tab.compile.last else {
        return look.t(ids::NOTEBOOK_TAB_COMPILING);
    };
    if last.status.compiled {
        return t!(
            look.i18n,
            ids::NOTEBOOK_TAB_STATUS_COMPILED,
            "ms" => last.elapsed_ms,
            "pages" => last.status.pages.len()
        );
    }
    let time = tab
        .compile
        .last_good
        .as_ref()
        .map(|good| good.finished_at.clone())
        .unwrap_or_default();
    t!(
        look.i18n,
        ids::NOTEBOOK_TAB_STATUS_FAILED,
        "errors" => last.status.errors,
        "time" => time.as_str()
    )
}

/// "31 references · 3 stale" once the references are resolved against the
/// design (mockup row 196), else the count the compile found.
fn references_text(look: Look<'_>, tab: &NotebookTab) -> Localized {
    if let Some(report) = look.references {
        return summary_line(look.i18n, report);
    }
    let count = tab
        .compile
        .last
        .as_ref()
        .map_or(0, |last| last.status.references.len());
    t!(look.i18n, ids::NOTEBOOK_TAB_STATUS_REFERENCES, "count" => count)
}

/// The block counts, then the measurement results and the stale
/// attachments when the notebook has readings (mockup row 199).
fn blocks_text(look: Look<'_>, tab: &NotebookTab) -> String {
    let blocks = tab
        .compile
        .last
        .as_ref()
        .map(|last| last.status.blocks)
        .unwrap_or_default();
    let mut parts = vec![t!(
        look.i18n,
        ids::NOTEBOOK_TAB_STATUS_BLOCKS,
        "calculations" => blocks.calculations,
        "measurements" => blocks.measurements,
        "issues" => blocks.issues
    )];
    if let Some(lane) = tab.compile.lane()
        && (lane.summary.readings > 0 || lane.summary.stale_attachments > 0)
    {
        parts.extend(strip_counts(look.i18n, &lane.summary));
    }
    parts
        .into_iter()
        .map(Localized::into_string)
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Read-only, saving, failed, unsaved or saved.
pub(super) fn saved_text(look: Look<'_>) -> Localized {
    let doc = look.doc;
    if doc.is_read_only() {
        return look.t(ids::NOTEBOOK_TAB_STATUS_READ_ONLY);
    }
    if doc.saving {
        return look.t(ids::NOTEBOOK_TAB_STATUS_SAVING);
    }
    if let Some(why) = &doc.save_error {
        return t!(look.i18n, ids::NOTEBOOK_TAB_STATUS_SAVE_FAILED, "why" => why.as_str());
    }
    if doc.is_unsaved() {
        return look.t(ids::NOTEBOOK_TAB_STATUS_UNSAVED);
    }
    match &doc.saved_at {
        Some(time) => t!(look.i18n, ids::NOTEBOOK_TAB_STATUS_SAVED_AT, "time" => time.as_str()),
        None => look.t(ids::NOTEBOOK_TAB_STATUS_SAVED),
    }
}
