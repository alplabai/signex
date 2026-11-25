//! The status footer (mockup row 195): the compile result (time and pages,
//! or the failure and the last good compile's time), the references and
//! the block counts, the save state, and the cursor's line and column.

use signex_i18n::{Localized, ids, t};
use signex_widgets::doc_tab_shell::Footer;

use super::Look;
use crate::app::notebook_tab::{Load, NotebookTab};

pub(super) fn view<'a>(look: Look<'a>, tab: &NotebookTab) -> Footer<'a> {
    if tab.load != Load::Open {
        return Footer::new(look.t(ids::NOTEBOOK_TAB_STATUS_NONE));
    }
    let status = t!(
        look.i18n,
        ids::NOTEBOOK_TAB_STATUS,
        "compile" => compile_text(look, tab).into_string(),
        "references" => references_text(look, tab).into_string(),
        "blocks" => blocks_text(look, tab).into_string(),
        "saved" => saved_text(look, tab).into_string()
    );
    let (line, column) = tab.text.as_ref().map_or((0, 0), |text| text.cursor());
    let cursor = t!(
        look.i18n,
        ids::NOTEBOOK_TAB_STATUS_CURSOR,
        "line" => tab.file_line(line),
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

fn references_text(look: Look<'_>, tab: &NotebookTab) -> Localized {
    let count = tab
        .compile
        .last
        .as_ref()
        .map_or(0, |last| last.status.references.len());
    t!(look.i18n, ids::NOTEBOOK_TAB_STATUS_REFERENCES, "count" => count)
}

fn blocks_text(look: Look<'_>, tab: &NotebookTab) -> Localized {
    let blocks = tab
        .compile
        .last
        .as_ref()
        .map(|last| last.status.blocks)
        .unwrap_or_default();
    t!(
        look.i18n,
        ids::NOTEBOOK_TAB_STATUS_BLOCKS,
        "calculations" => blocks.calculations,
        "measurements" => blocks.measurements,
        "issues" => blocks.issues
    )
}

/// Read-only, saving, failed, unsaved or saved.
pub(super) fn saved_text(look: Look<'_>, tab: &NotebookTab) -> Localized {
    if tab.is_read_only() {
        return look.t(ids::NOTEBOOK_TAB_STATUS_READ_ONLY);
    }
    if tab.saving {
        return look.t(ids::NOTEBOOK_TAB_STATUS_SAVING);
    }
    if let Some(why) = &tab.save_error {
        return t!(look.i18n, ids::NOTEBOOK_TAB_STATUS_SAVE_FAILED, "why" => why.as_str());
    }
    if tab.is_unsaved() {
        return look.t(ids::NOTEBOOK_TAB_STATUS_UNSAVED);
    }
    match &tab.saved_at {
        Some(time) => t!(look.i18n, ids::NOTEBOOK_TAB_STATUS_SAVED_AT, "time" => time.as_str()),
        None => look.t(ids::NOTEBOOK_TAB_STATUS_SAVED),
    }
}
