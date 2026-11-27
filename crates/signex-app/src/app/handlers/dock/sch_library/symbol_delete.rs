//! SCH Library symbol Delete: the in-use warning and its closed-sheet scan
//! (#1845, #1903). Open sheets and mounted rows are read from memory; the
//! closed sheets of the open projects are read from disk by a `Task`.

use super::*;
use crate::app::CaseFold;
use crate::library::LibraryMessage;
use crate::library::closed_sheets::{
    ClosedSheetScan, ClosedSheetsReport, ClosedSheetsVerdict, ScanPlan, SymbolMatch,
    changed_since_read, scan_closed_sheets,
};
use crate::library::symbol_delete::{PendingSymbolDelete, SymbolUsage};
use crate::panels::SymbolDeleteWarning;

impl Signex {
    pub(super) fn sch_library_delete_symbol(&mut self, idx: usize) -> Task<Message> {
        let Some(editor) = self.active_symbol_editor_mut() else {
            crate::diagnostics::log_warning(
                "SCH Library: delete fired without an active Symbol editor",
            );
            return Task::none();
        };
        // A file that opened repaired takes no edit until it is saved (#3129).
        if editor.draft.is_read_only() {
            crate::library::editor::symbol::updates::refuse_read_only(editor);
            self.refresh_panel_ctx();
            return Task::none();
        }
        if editor.draft.file.symbols.len() <= 1 {
            crate::diagnostics::log_warning(format!(
                "{}: refused to delete the last symbol in the file",
                editor.path.display()
            ));
            return Task::none();
        }
        if idx >= editor.draft.file.symbols.len() {
            crate::diagnostics::log_warning(format!(
                "{}: symbol delete index {idx} is out of range ({} symbols)",
                editor.path.display(),
                editor.draft.file.symbols.len()
            ));
            return Task::none();
        }
        let (path, symbol) = (editor.path.clone(), editor.draft.file.symbols[idx].uuid);
        let history = editor.draft.history_generation;
        let name = editor.draft.file.symbols[idx].name.clone();
        let (mut usage, target) = self.symbol_usage_lines(&path, symbol);
        let plan = self.closed_scan_plan();
        let checking = plan.has_work();
        if usage.lines.is_empty() && !checking {
            crate::diagnostics::log_info(if self.document_state.projects.is_empty() {
                format!(
                    "{name} deleted. Not used by open sheets or mounted libraries; closed sheets were not checked (no project is open)."
                )
            } else {
                format!(
                    "{name} deleted. Not used by open sheets or mounted libraries; closed sheets of open projects checked, other projects not."
                )
            });
            self.delete_symbol_from_container(&path, idx, symbol);
            return Task::none();
        }
        let base_lines = usage.lines.len();
        if checking {
            usage
                .lines
                .push("Checking closed sheets of the open projects…".into());
        }
        self.library.symbol_delete_serial += 1;
        let serial = self.library.symbol_delete_serial;
        self.library.pending_symbol_delete = Some(PendingSymbolDelete {
            path,
            symbol,
            history,
            serial,
            rows: usage.rows,
            sheets: usage.sheets,
            closed: 0,
            base_lines,
            unchecked: usage.unchecked,
            warning: SymbolDeleteWarning {
                idx,
                name,
                only_unknown: usage.rows == 0 && usage.sheets == 0,
                checking,
                lines: usage.lines,
            },
        });
        self.refresh_panel_ctx();
        if !checking {
            return Task::none();
        }
        Self::spawn_closed_scan(serial, plan, target)
    }

    fn spawn_closed_scan(serial: u64, plan: ScanPlan, target: SymbolMatch) -> Task<Message> {
        let asked = plan.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || scan_closed_sheets(&plan, target))
                    .await
                    .unwrap_or_else(|e| {
                        crate::diagnostics::log_error(
                            "SCH Library: closed-sheet scan worker failed",
                            &anyhow::anyhow!("{e}"),
                        );
                        ClosedSheetScan::unfinished(&e.to_string())
                    })
            },
            move |scan| {
                Message::Library(LibraryMessage::SymbolDeleteClosedSheetsScanned(Box::new(
                    ClosedSheetsReport {
                        serial,
                        scan,
                        plan: asked,
                    },
                )))
            },
        )
    }

    /// The closed-sheet scan answered. A result for a warning that was
    /// replaced, answered or closed, or whose symbol file changed since, is
    /// dropped: it describes something else.
    pub(crate) fn handle_symbol_delete_closed_sheets_scanned(
        &mut self,
        report: ClosedSheetsReport,
    ) -> Task<Message> {
        let Some(pending) = self.library.pending_symbol_delete.as_ref() else {
            return Task::none();
        };
        if pending.serial != report.serial {
            return Task::none();
        }
        let (path, symbol) = (pending.path.clone(), pending.symbol);
        if !self.symbol_delete_is_current(&path) {
            return self.drop_stale_symbol_delete();
        }
        let mut scan = report.scan;
        self.note_unscanned(&mut scan, &report.plan);
        for line in &scan.unreadable {
            crate::diagnostics::log_warning(format!(
                "SCH Library delete: closed sheet not checked: {line}"
            ));
        }
        // Rows and open sheets may have changed while the disk was read.
        let (usage, _) = self.symbol_usage_lines(&path, symbol);
        let Some(pending) = self.library.pending_symbol_delete.as_mut() else {
            return Task::none();
        };
        pending.apply_closed_scan(usage, &scan);
        if pending.warning.lines.is_empty() && !scan.stamps.is_empty() {
            // Clean so far. A sheet can have been written after the worker
            // read it, so state the files again on a worker before deleting.
            pending.warning.checking = true;
            pending
                .warning
                .lines
                .push("Checking closed sheets of the open projects…".into());
            let (serial, stamps) = (pending.serial, scan.stamps);
            self.refresh_panel_ctx();
            return Task::perform(
                async move {
                    let changed = tokio::task::spawn_blocking(move || changed_since_read(&stamps))
                        .await
                        .unwrap_or_else(|e| {
                            vec![format!("closed sheets could not be re-checked ({e})")]
                        });
                    ClosedSheetsVerdict { serial, changed }
                },
                |verdict| {
                    Message::Library(LibraryMessage::SymbolDeleteClosedSheetsVerified(Box::new(
                        verdict,
                    )))
                },
            );
        }
        self.finish_closed_check()
    }

    /// The files a clean scan read were stated again.
    pub(crate) fn handle_symbol_delete_closed_sheets_verified(
        &mut self,
        verdict: ClosedSheetsVerdict,
    ) -> Task<Message> {
        let Some(pending) = self.library.pending_symbol_delete.as_ref() else {
            return Task::none();
        };
        if pending.serial != verdict.serial {
            return Task::none();
        }
        let (path, symbol) = (pending.path.clone(), pending.symbol);
        if !self.symbol_delete_is_current(&path) {
            return self.drop_stale_symbol_delete();
        }
        for line in &verdict.changed {
            crate::diagnostics::log_warning(format!("SCH Library delete: {line}"));
        }
        let scan = ClosedSheetScan {
            unreadable: verdict.changed,
            ..Default::default()
        };
        let (usage, _) = self.symbol_usage_lines(&path, symbol);
        if let Some(pending) = self.library.pending_symbol_delete.as_mut() {
            pending.apply_closed_scan(usage, &scan);
        }
        self.finish_closed_check()
    }

    /// The pending warning still describes the open container at `path`.
    fn symbol_delete_is_current(&self, path: &std::path::Path) -> bool {
        let pending = self.library.pending_symbol_delete.as_ref();
        let editor = self.document_state.symbol_editors.get(path);
        matches!((pending, editor), (Some(p), Some(e)) if p.is_current(e))
    }

    fn drop_stale_symbol_delete(&mut self) -> Task<Message> {
        if let Some(pending) = self.library.pending_symbol_delete.take() {
            crate::diagnostics::log_warning(format!(
                "{} changed since the warning; press Delete again.",
                pending.warning.name
            ));
        }
        self.refresh_panel_ctx();
        Task::none()
    }

    /// Run the Delete when the check found nothing and left nothing
    /// unchecked; otherwise leave the warning for the user.
    fn finish_closed_check(&mut self) -> Task<Message> {
        let Some(pending) = self.library.pending_symbol_delete.as_ref() else {
            return Task::none();
        };
        if pending.warning.lines.is_empty() {
            let pending = self.library.pending_symbol_delete.take().expect("pending");
            crate::diagnostics::log_info(format!(
                "{} deleted from {}. Not used by open sheets or mounted libraries; closed sheets of open projects checked, other projects not.",
                pending.warning.name,
                pending.path.file_name().map_or_else(
                    || pending.path.display().to_string(),
                    |n| n.to_string_lossy().into_owned()
                )
            ));
            self.delete_symbol_from_container(&pending.path, pending.warning.idx, pending.symbol);
            return Task::none();
        }
        self.refresh_panel_ctx();
        Task::none()
    }

    /// What changed while the scan ran, as unchecked entries: a sheet that was
    /// open and has been closed (its saved copy was never read), and any
    /// sheet or child reference the plan now has that the scan never reached.
    fn note_unscanned(&self, scan: &mut ClosedSheetScan, asked: &ScanPlan) {
        let name = |p: &std::path::Path| {
            p.file_name().map_or_else(
                || p.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            )
        };
        for sheet in &asked.open {
            if !self.document_state.engines.contains_key(sheet) {
                scan.unreadable
                    .push(format!("{} was closed while the check ran", name(sheet)));
            }
        }
        // A dead worker already marked everything unchecked; per-seed lines
        // would only mislabel the sheets as "added".
        if scan.unfinished {
            return;
        }
        let now = self.closed_scan_plan();
        for (path, fold) in &now.seeds {
            let key = crate::app::project_sheets::fold_key(path, *fold);
            // A page that was open when the scan began is reported as closed.
            let was_open = asked
                .open
                .iter()
                .any(|o| crate::app::project_sheets::fold_key(o, *fold) == key);
            if !was_open && !scan.visited.contains(&key) {
                scan.unreadable.push(format!(
                    "{} was added while the check ran and was not read",
                    name(path)
                ));
            }
        }
        for entry in &now.unresolved {
            if !asked.unresolved.contains(entry) {
                scan.unreadable.push(entry.clone());
            }
        }
    }

    /// The user answered "Delete anyway" to the in-use warning.
    pub(super) fn sch_library_confirm_delete_symbol(&mut self, idx: usize) -> bool {
        let Some(pending) = self.library.pending_symbol_delete.take() else {
            return true;
        };
        // The container changed under the warning: nothing is deleted.
        let unchanged = pending.warning.idx == idx
            && self
                .active_symbol_editor_mut()
                .is_some_and(|e| pending.is_current(e));
        if !unchanged {
            crate::diagnostics::log_warning(format!(
                "{} changed since the warning; press Delete again.",
                pending.warning.name
            ));
            self.refresh_panel_ctx();
            return true;
        }
        let name = &pending.warning.name;
        let unchecked = pending.unchecked;
        let skipped = if pending.warning.checking {
            " Closed sheets were not checked."
        } else {
            ""
        };
        let sheets = if pending.closed > 0 {
            format!(
                "{} open sheet(s) and {} closed sheet(s)",
                pending.sheets, pending.closed
            )
        } else {
            format!("{} open sheet(s)", pending.sheets)
        };
        crate::diagnostics::log_warning(if pending.warning.only_unknown && unchecked == 0 {
            format!("{name} deleted.{skipped}")
        } else if pending.warning.only_unknown {
            format!(
                "{name} deleted; {unchecked} table(s), table list(s) or sheet(s) could not be read, so whether anything used it is unknown.{skipped}"
            )
        } else if unchecked > 0 {
            format!(
                "{name} deleted: {} component row(s) and {sheets} now point at a missing library symbol, plus {unchecked} table(s), table list(s) or sheet(s) that could not be read.{skipped}",
                pending.rows
            )
        } else {
            format!(
                "{name} deleted: {} component row(s) and {sheets} now point at a missing library symbol.{skipped}",
                pending.rows
            )
        });
        self.delete_symbol_from_container(&pending.path, idx, pending.symbol)
    }

    pub(super) fn sch_library_cancel_delete_symbol(&mut self) -> bool {
        self.library.pending_symbol_delete = None;
        self.refresh_panel_ctx();
        true
    }

    /// Rows and open sheets that use `symbol` of the container at `path`.
    /// A container outside every mounted library cannot have its rows
    /// checked: that is reported as unknown usage.
    fn symbol_usage_lines(
        &self,
        path: &std::path::Path,
        symbol: uuid::Uuid,
    ) -> (SymbolUsage, SymbolMatch) {
        use crate::library::symbol_delete::{usage_lines, usage_outside_library};
        let sheets = self
            .document_state
            .engines
            .iter()
            .map(|(p, e)| (p.as_path(), e.document()));
        let fold = self.identity_fold_for(path);
        match self.library.containing_library_with(path, fold) {
            Some(lib) => {
                let reference = signex_library::PrimitiveRef::new(lib.library_id, symbol);
                let usage = usage_lines(&self.library.open_libraries, sheets, &reference);
                (usage, SymbolMatch::InLibrary(lib.library_id, symbol))
            }
            None => (
                usage_outside_library(
                    path,
                    sheets,
                    symbol,
                    self.library.libraries_share_folder_of(path, fold),
                ),
                SymbolMatch::AnyLibrary(symbol),
            ),
        }
    }

    /// Where the disk scan starts (#1903): every declared page and root of
    /// the open projects, plus the children of the open sheets, since a
    /// hierarchical child is reached only through a `child_sheets` reference.
    /// Open sheets are skipped; memory already answers for them.
    fn closed_scan_plan(&self) -> ScanPlan {
        use crate::app::project_sheets::{
            ChildReferenceRefusal, project_root_sheet_path, resolve_child_reference_quiet,
        };
        let engines = &self.document_state.engines;
        let mut seeds: Vec<(std::path::PathBuf, CaseFold)> = Vec::new();
        let mut unresolved = Vec::new();
        for project in &self.document_state.projects {
            let dir = project.dir();
            let fold = project.case_fold;
            seeds.extend(
                project
                    .data
                    .sheets
                    .iter()
                    .map(|s| (dir.join(&s.filename), fold)),
            );
            seeds.extend(project_root_sheet_path(project).map(|p| (p, fold)));
        }
        for (path, engine) in engines {
            let dir = path.parent().unwrap_or_else(|| std::path::Path::new(""));
            let fold = self
                .document_state
                .document_project(path)
                .map_or_else(CaseFold::host_default, |p| p.case_fold);
            for child in &engine.document().child_sheets {
                match resolve_child_reference_quiet(dir, &child.filename) {
                    Ok(resolved) => seeds.push((resolved, fold)),
                    Err(ChildReferenceRefusal::Empty) => {}
                    Err(ChildReferenceRefusal::Escapes { reference, .. }) => {
                        unresolved.push(format!(
                            "child sheet {reference} of an open sheet is outside its parent sheet's folder"
                        ))
                    }
                }
            }
        }
        seeds.retain(|(p, _)| !engines.contains_key(p));
        seeds.sort_by(|a, b| a.0.cmp(&b.0));
        seeds.dedup_by(|a, b| a.0 == b.0);
        ScanPlan {
            seeds,
            open: engines.keys().cloned().collect(),
            unresolved,
        }
    }

    /// Delete the symbol `symbol` at slot `idx` of the container `path`, and
    /// only that: the slot must still hold it. The container need not be the
    /// active tab (the scan answers later).
    fn delete_symbol_from_container(
        &mut self,
        path: &std::path::Path,
        idx: usize,
        symbol: uuid::Uuid,
    ) -> bool {
        let Some(editor) = self.document_state.symbol_editors.get_mut(path) else {
            return true;
        };
        if editor.draft.is_read_only()
            || editor.draft.file.symbols.len() <= 1
            || editor.draft.file.symbols.get(idx).map(|s| s.uuid) != Some(symbol)
        {
            return true;
        }
        // The delete is its own undo step (#760): snapshot the whole
        // container first so Undo restores the deleted symbol and leaves
        // the previous edit alone.
        let snapshot = editor.snapshot();
        editor.commit_history(snapshot);
        editor.draft.file.symbols.remove(idx);
        editor.draft.file.updated = chrono::Utc::now();
        // A sibling before the active one shifted it down a slot: follow
        // it. Deleting the active symbol lands on the one that took its
        // slot (or the last, when the tail went).
        let active_moved = idx <= editor.draft.active_idx;
        if idx < editor.draft.active_idx {
            editor.draft.active_idx -= 1;
        } else if editor.draft.active_idx >= editor.draft.file.symbols.len() {
            editor.draft.active_idx = editor.draft.file.symbols.len() - 1;
        }
        if active_moved {
            editor.reset_view_for_symbol_switch();
        }
        self.document_state.mark_document_dirty(path);
        self.refresh_panel_ctx();
        true
    }
}

/// Mockup row 137: a Delete of a symbol that may still be in use asks in
/// the "Delete Symbol?" modal; its buttons and Esc answer it.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Message, Signex};
    use crate::dock::DockMessage;
    use crate::layout_probe::{lay_out_texts, messages_on_click};
    use crate::panels::PanelMsg;
    use signex_library::{Symbol, SymbolFile};
    use std::path::PathBuf;

    const WINDOW: iced::Size = iced::Size::new(900.0, 700.0);

    fn panel(app: &mut Signex, msg: PanelMsg) {
        let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
    }

    /// A container outside every mounted library: its rows cannot be
    /// checked, so its Delete always asks.
    fn app_asking() -> (Signex, PathBuf) {
        let (mut app, _task) = Signex::new();
        let path = PathBuf::from("loose.snxsym");
        let mut file = SymbolFile::from_symbol(Symbol::empty("R"));
        file.symbols.push(Symbol::empty("C"));
        app.open_symbol_editor_tab(path.clone(), file);
        panel(&mut app, PanelMsg::SchLibraryDeleteSymbol(0));
        (app, path)
    }

    fn warning(app: &Signex) -> Option<&SymbolDeleteWarning> {
        app.document_view
            .panel_ctx
            .symbol_editor
            .as_ref()?
            .delete_warning
            .as_ref()
    }

    fn count(app: &Signex, path: &std::path::Path) -> usize {
        app.document_state.symbol_editors[path]
            .draft
            .file
            .symbols
            .len()
    }

    fn shown(texts: &[crate::layout_probe::LaidOutText]) -> Vec<String> {
        texts
            .iter()
            .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
            .collect()
    }

    #[test]
    fn delete_anyway_in_the_modal_deletes_the_symbol() {
        let (mut app, path) = app_asking();
        assert_eq!(count(&app, &path), 2, "nothing deleted before the answer");
        let tokens = app.document_view.panel_ctx.tokens;
        let i18n = signex_i18n::I18n::en_us();
        let warning = warning(&app).expect("the modal is up").clone();

        let texts = lay_out_texts(
            crate::library::symbol_delete::view(&warning, &tokens, &i18n),
            WINDOW,
        );
        let shown = shown(&texts);
        assert!(shown.iter().any(|t| t == "Delete Symbol?"), "{shown:?}");
        assert!(
            shown
                .iter()
                .any(|t| t == "Symbol \"R\" may still be in use"),
            "{shown:?}"
        );
        assert!(
            shown
                .iter()
                .any(|t| t.contains("not inside a mounted library")),
            "{shown:?}"
        );

        let clicked = messages_on_click(
            crate::library::symbol_delete::view(&warning, &tokens, &i18n),
            WINDOW,
            "Delete anyway",
        );
        assert!(
            matches!(clicked.as_slice(), [PanelMsg::SchLibraryDeleteConfirmed(0)]),
            "{clicked:?}"
        );
        for msg in clicked {
            panel(&mut app, msg);
        }
        assert_eq!(count(&app, &path), 1);
        assert!(warning_gone(&app));
    }

    #[test]
    fn cancel_in_the_modal_keeps_the_symbol() {
        let (mut app, path) = app_asking();
        let tokens = app.document_view.panel_ctx.tokens;
        let i18n = signex_i18n::I18n::en_us();
        let warning = warning(&app).expect("the modal is up").clone();

        let clicked = messages_on_click(
            crate::library::symbol_delete::view(&warning, &tokens, &i18n),
            WINDOW,
            "Cancel",
        );
        assert!(
            matches!(clicked.as_slice(), [PanelMsg::SchLibraryDeleteCancelled]),
            "{clicked:?}"
        );
        for msg in clicked {
            panel(&mut app, msg);
        }
        assert_eq!(count(&app, &path), 2);
        assert!(warning_gone(&app));
    }

    #[test]
    fn esc_cancels_the_modal_and_keeps_the_symbol() {
        let (mut app, path) = app_asking();
        let esc = app.escape_overlay_message().expect("the modal claims Esc");
        assert!(
            matches!(
                esc,
                Message::Dock(DockMessage::Panel(PanelMsg::SchLibraryDeleteCancelled))
            ),
            "{esc:?}"
        );
        assert!(app.has_locking_confirm_modal(), "a confirm locks the menus");

        let _ = app.update(esc);
        assert_eq!(count(&app, &path), 2);
        assert!(warning_gone(&app));
        assert!(
            !matches!(
                app.escape_overlay_message(),
                Some(Message::Dock(DockMessage::Panel(
                    PanelMsg::SchLibraryDeleteCancelled
                )))
            ),
            "the answered modal no longer claims Esc"
        );
    }

    fn warning_gone(app: &Signex) -> bool {
        app.library.pending_symbol_delete.is_none() && warning(app).is_none()
    }
}
