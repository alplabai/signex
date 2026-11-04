//! Library edits that write through the mounted adapter, refused while a
//! library rename runs (#859).
//!
//! The adapter keeps pointing at the old `.snxlib` until the rename's
//! completion relocates it, so a write issued in between would recreate the
//! old directory or fail halfway. Only intents that write are refused; reads,
//! navigation, in-memory editing and completions of earlier async work still
//! go through (dropping a completion would strand its state).

use super::*;
use crate::app::path_ops::PathOpKind;
use crate::library::messages::DatasheetMsg;
use crate::library::stock_copy::StockCopyMsg;

impl Signex {
    /// Why `msg` cannot run now, or `None` when it can. The cause leads: the
    /// Messages panel clips a record near 160 characters.
    pub(super) fn library_write_refusal(&self, msg: &LibraryMessage) -> Option<String> {
        let ops = &self.document_state.pending_path_ops;
        if !ops.library_rename_running() || !writes_through_adapter(msg) {
            return None;
        }
        // A write that names its library is refused only for the library
        // being moved; one that names none cannot be told apart, so it waits.
        if let Some(library) = library_named_by(msg)
            && ops.blocking_write(library).is_none()
        {
            return None;
        }
        Some(
            "A library rename or delete is still running, so this library change was not applied. \
             Try again once it finishes."
                .to_string(),
        )
    }

    /// Why a symbol or footprint editor edit cannot apply now, or `None`.
    /// While a library delete runs, an edit to a primitive inside that
    /// library is refused rather than kept and reported afterwards (#1939):
    /// the editor stays as it was. A rename allows edits and rekeys the
    /// draft; saving stays refused during both. View-only
    /// events (cursor, pan, zoom, menus, selection) still go through.
    pub(super) fn primitive_edit_refusal(&self, msg: &LibraryMessage) -> Option<String> {
        let LibraryMessage::PrimitiveEditorEvent { path, msg } = msg else {
            return None;
        };
        let edits = match msg {
            PrimitiveEdit::Save => true,
            PrimitiveEdit::Footprint(m) if footprint_msg_is_completion(m) => false,
            PrimitiveEdit::Symbol(m) => !symbol_msg_is_view_only(m),
            PrimitiveEdit::Footprint(m) => !footprint_msg_is_view_only(m),
        };
        if !edits {
            return None;
        }
        self.editor_edit_refusal(path)
    }

    /// The one "may the editor at `path` be edited now" check, shared by the
    /// editor message stream, undo/redo and the Properties panel so a command
    /// behaves the same from every surface.
    pub(crate) fn editor_edit_refusal(&self, path: &std::path::Path) -> Option<String> {
        let library = self.deleting_library_name(path)?;
        Some(format!(
            "Library {library} is being deleted, so this edit was not applied. \
             Try again if the delete fails."
        ))
    }

    /// Display name of the library at `path` while a delete of it runs. A
    /// rename keeps edits: the dirty draft moves to the new path with the
    /// library. Only a delete discards the draft, so only it refuses.
    fn deleting_library_name(&self, path: &std::path::Path) -> Option<String> {
        let ops = &self.document_state.pending_path_ops;
        if !ops.library_rename_running()
            || !ops
                .blocking_write(path)
                .is_some_and(|c| c.kind == PathOpKind::Delete)
        {
            return None;
        }
        Some(
            self.library
                .write_target_library(path, self.identity_fold_for(path))
                .map(|lib| lib.display_name.clone())
                .unwrap_or_else(|| path.display().to_string()),
        )
    }

    /// `editor_edit_refusal` for a Component Preview event: its library is the
    /// preview's address. View-only events (tabs, selection, edit buffers,
    /// reload prompts) and answers to earlier async work are not refused.
    pub(super) fn preview_edit_refusal(
        &self,
        address: &EditorAddress,
        msg: &EditorMsg,
    ) -> Option<String> {
        if editor_msg_is_view_only(msg) {
            return None;
        }
        // The answer to a file dialog opened before the delete: name the
        // dropped pick instead of the generic line.
        if let EditorMsg::StepAttachResult(pick) = msg {
            let library = self.deleting_library_name(&address.library_path)?;
            let file = pick
                .as_ref()
                .map_or("The picked file", |(_, name)| name.as_str());
            return Some(format!(
                "{file} was not attached: library {library} is being deleted. \
                 Pick it again if the delete fails."
            ));
        }
        self.editor_edit_refusal(&address.library_path)
    }

    /// `editor_edit_refusal` for a mutation site outside the message stream:
    /// logs the refusal (once per run of identical lines) and says whether
    /// the edit may go ahead.
    pub(crate) fn editor_edit_allowed(&self, path: &std::path::Path) -> bool {
        match self.editor_edit_refusal(path) {
            Some(refusal) => {
                log_refusal_once(&refusal);
                false
            }
            None => true,
        }
    }

    /// Git or copy work running off the update loop that writes into the
    /// library at `root`: a queued library edit (#893), a datasheet PDF store,
    /// a "Commit now", or a stock copy (its target is not tracked, so any copy
    /// counts). Mounted roots are compared under `fold`. Moving or deleting the library under it
    /// would strand that work in a folder the app no longer mounts.
    pub(crate) fn library_background_write_running(
        &self,
        root: &std::path::Path,
        fold: crate::app::state::CaseFold,
    ) -> bool {
        let is_library = root
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("snxlib"));
        is_library
            && (any_root_under(
                self.library
                    .writes
                    .busy_roots()
                    .iter()
                    .map(std::path::PathBuf::as_path),
                root,
                fold,
            ) || self.document_state.document_saves.is_saving(root)
                || any_root_under(self.library.committing_roots(), root, fold)
                || !self.library.stock_copies_in_flight.is_empty())
    }
}

/// Symbol editor events that change no document state.
fn symbol_msg_is_view_only(msg: &SymbolEditorMsg) -> bool {
    use SymbolEditorMsg as S;
    matches!(
        msg,
        S::Pan { .. }
            | S::Zoom { .. }
            | S::Fit
            | S::ViewportMeasured { .. }
            | S::CursorAt { .. }
            | S::Select(_)
            | S::Deselect
            | S::ToggleActiveBarMenu(_)
            | S::CloseActiveBarMenu
            | S::ToggleSelectionFilter(_)
            | S::ShowContextMenu { .. }
            | S::CloseContextMenu
            | S::ContextMenuOpenSubmenu(_)
    )
}

/// Log `refusal` unless it is already the newest Messages line: a drag
/// refuses on every tick.
pub(crate) fn log_refusal_once(refusal: &str) {
    let repeated = crate::diagnostics::recent_entries()
        .last()
        .is_some_and(|e| e.message == refusal);
    if !repeated {
        crate::diagnostics::log_warning(refusal);
    }
}

/// Component Preview events that change neither the row copy nor the library:
/// navigation, edit buffers, reload prompts and answers to earlier async work.
/// Anything not listed counts as an edit, so a new variant is refused by default.
fn editor_msg_is_view_only(msg: &EditorMsg) -> bool {
    use EditorMsg as E;
    match msg {
        E::SelectTab(_)
        | E::OpenWhereUsedTab
        | E::CloseEditor
        | E::ReloadRequested
        | E::ReloadConfirmed
        | E::ReloadCancelled
        | E::SubmitForReview
        | E::SubmitForReviewNotesChanged(_)
        // Wired to nothing in the preview (it only logs), so it writes no row or file.
        | E::SubmitForReviewConfirm
        | E::SubmitForReviewCancel
        | E::SubmitForReviewResult(_)
        | E::PinMapOpenOverrideEdit(_)
        | E::PinMapOverrideBufChanged { .. }
        | E::PinMapCancelOverrideEdit
        | E::ParamSetNumberBuf { .. }
        | E::ParamSetMeasurementBuf { .. }
        | E::ParamNewNameChanged(_) => true,
        E::Datasheet(d) => matches!(
            d,
            DatasheetMsg::Open(_)
                | DatasheetMsg::UploadResult(_)
                | DatasheetMsg::AssetsStored { .. }
        ),
        E::SimBodyAction(action) => !action.is_edit(),
        E::Footprint(f) => footprint_msg_is_view_only(f),
        _ => false,
    }
}

/// Footprint editor answers to earlier async work. Dropping one strands the
/// editor's slot for that work in flight. The symbol editor has none.
fn footprint_msg_is_completion(msg: &FootprintEditorMsg) -> bool {
    matches!(
        msg,
        FootprintEditorMsg::SketchSolved { .. } | FootprintEditorMsg::RedundancyChecked { .. }
    )
}

/// Footprint editor events that change no document state.
fn footprint_msg_is_view_only(msg: &FootprintEditorMsg) -> bool {
    use FootprintEditorMsg as F;
    matches!(
        msg,
        F::CursorAt { .. }
            | F::ViewScaleChanged { .. }
            | F::FitConsumed { .. }
            | F::SelectPad(_)
            | F::SelectPads(_)
            | F::SelectSilkF(_)
            | F::ToggleLayer(_)
            | F::ToggleSelectionFilter(_)
            | F::ToggleActiveBarMenu(_)
            | F::CloseActiveBarMenu
            | F::ShowContextMenu { .. }
            | F::CloseContextMenu
            | F::ContextMenuOpenSubmenu(_)
            | F::ActiveBarSelectAll
            | F::ActiveBarClearSelection
    )
}

/// One of `roots` is `path` or lies under it, compared under `fold` so a case
/// variant of the mounted root still matches.
fn any_root_under<'a>(
    mut roots: impl Iterator<Item = &'a std::path::Path>,
    path: &std::path::Path,
    fold: crate::app::state::CaseFold,
) -> bool {
    roots.any(|root| crate::app::state::scope::path_is_under(root, path, fold))
}

/// Why a library rename or delete waits for `library_background_write_running`.
/// The copy's target is not tracked, so the text names no library.
pub(crate) const LIBRARY_BACKGROUND_WRITE_REFUSAL: &str =
    "A library edit, commit or copy is still running; try again once it finishes.";

/// The library a message addresses, when it carries its path.
fn library_named_by(msg: &LibraryMessage) -> Option<&std::path::Path> {
    use LibraryMessage as L;
    match msg {
        L::BrowserConfirmAddTable { library_path }
        | L::BrowserDeleteTable { library_path, .. }
        | L::BrowserConfirmRenameTable { library_path }
        | L::BrowserAddComponent { library_path, .. }
        | L::BrowserDeleteRowConfirm { library_path, .. }
        | L::BrowserCellCommit { library_path, .. }
        | L::BrowserEdit { library_path, .. }
        | L::BrowserCommitPending { library_path }
        | L::ComponentsPanelPromoteToGlobal(library_path) => Some(library_path),
        // A copy into a library, or Copy Again of a copy inside one. A new
        // library's chosen path may still gain its extension, so it names
        // none and waits for any library rename.
        L::StockCopy(StockCopyMsg::TargetPicked(path) | StockCopyMsg::CopyAgainRequested(path)) => {
            Some(path)
        }
        _ => None,
    }
}

/// The messages whose handlers call a mutating `LibraryAdapter` method.
fn writes_through_adapter(msg: &LibraryMessage) -> bool {
    use LibraryMessage as L;
    match msg {
        L::NewComponent
        | L::BrowserConfirmAddTable { .. }
        | L::BrowserDeleteTable { .. }
        | L::BrowserConfirmRenameTable { .. }
        | L::BrowserAddComponent { .. }
        | L::BrowserDeleteRowConfirm { .. }
        | L::BrowserCellCommit { .. }
        | L::LibraryUpdatesApply
        | L::DocumentOptionsApply
        | L::PrimitivePicker(PrimitivePickerMsg::Pick(_) | PrimitivePickerMsg::CopyConfirmed)
        | L::CloseLibraryConfirm(_)
        | L::ComponentsPanelPromoteToGlobal(_)
        | L::BrowserCommitPending { .. }
        | L::StockCopy(
            StockCopyMsg::TargetPicked(_)
            | StockCopyMsg::NewLibraryPathChosen(Some(_))
            | StockCopyMsg::CopyAgainRequested(_),
        ) => true,
        L::BrowserEdit { msg, .. } => matches!(msg, BrowserEditMsg::Save),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::state::scope::CaseFold;

    fn app_with_library_rename() -> Signex {
        let (mut app, _task) = Signex::new();
        app.document_state.pending_path_ops.begin(
            PathOpKind::RenameLibrary,
            vec!["/w/a.snxlib".into(), "/w/b.snxlib".into()],
            CaseFold::PRESERVE,
        );
        app
    }

    #[test]
    fn writes_are_refused_while_a_library_rename_runs_and_reads_are_not() {
        let app = app_with_library_rename();
        let write = LibraryMessage::BrowserCellCommit {
            library_path: "/w/a.snxlib".into(),
            table: "t".into(),
            row_id: signex_library::RowId::default(),
            column: String::new(),
        };
        let refusal = app.library_write_refusal(&write).expect("refused");
        assert!(refusal.starts_with("A library rename or delete is still running"));
        assert!(
            app.library_write_refusal(&LibraryMessage::BrowserSearchChanged {
                library_path: "/w/a.snxlib".into(),
                value: "x".into(),
            })
            .is_none()
        );
    }

    /// The refusal is a card as well as a Messages line, like every other
    /// refusal while a path operation runs.
    #[test]
    fn a_refused_library_write_shows_a_card_and_reaches_messages() {
        let _ = crate::diagnostics::init_logging();
        let mut app = app_with_library_rename();
        let since = crate::diagnostics::newest_entry_id();
        let _task = app.update(crate::app::Message::Library(LibraryMessage::NewComponent));
        let notice = app.document_state.error_notice.clone().expect("a card");
        assert_eq!(notice.title, "Library Change Not Applied");
        assert!(
            notice
                .detail
                .starts_with("A library rename or delete is still running")
        );
        assert!(
            crate::diagnostics::recent_entries()
                .iter()
                .any(|e| Some(e.id) > since && e.message.contains("library change was not applied"))
        );
    }

    fn symbol_edit(path: &str, msg: SymbolEditorMsg) -> crate::app::Message {
        crate::app::Message::Library(LibraryMessage::PrimitiveEditorEvent {
            path: path.into(),
            msg: PrimitiveEdit::Symbol(msg),
        })
    }

    fn app_with_library_delete() -> Signex {
        let (mut app, _task) = Signex::new();
        app.document_state.pending_path_ops.begin(
            PathOpKind::Delete,
            vec!["/w/a.snxlib".into()],
            CaseFold::PRESERVE,
        );
        app
    }

    fn app_with_open_symbol(app: &mut Signex) -> std::path::PathBuf {
        let path = std::path::PathBuf::from("/w/a.snxlib/symbols/r.snxsym");
        let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("S"));
        app.document_state.symbol_editors.insert(
            path.clone(),
            crate::app::SymbolDocument::new(path.clone(), file),
        );
        path
    }

    /// #1939: an editor edit during a library delete is refused, not kept and
    /// reported afterwards. The editor stays as it was and Messages names the
    /// library; a view-only event still goes through.
    #[test]
    fn a_symbol_edit_during_a_library_delete_is_refused_and_logged() {
        let _ = crate::diagnostics::init_logging();
        let mut app = app_with_library_delete();
        let path = app_with_open_symbol(&mut app);
        let pins =
            |app: &Signex| format!("{:?}", app.document_state.symbol_editors[&path].snapshot());
        let before = pins(&app);
        let since = crate::diagnostics::newest_entry_id();

        let p = path.to_string_lossy().into_owned();
        let _task = app.update(symbol_edit(&p, SymbolEditorMsg::AddPin { x: 1.0, y: 2.0 }));

        assert_eq!(pins(&app), before, "the editor is unchanged");
        assert!(!app.document_state.symbol_editors[&path].is_dirty());
        assert!(
            crate::diagnostics::recent_entries().iter().any(|e| {
                Some(e.id) > since
                    && e.message.contains("a.snxlib")
                    && e.message.contains("is being deleted")
                    && e.message.contains("edit was not applied")
            }),
            "Messages names the library"
        );

        // A pan is view-only and is not refused.
        assert!(
            app.primitive_edit_refusal(&match symbol_edit(
                &p,
                SymbolEditorMsg::Pan { dx: 1.0, dy: 1.0 },
            ) {
                crate::app::Message::Library(m) => m,
                _ => unreachable!(),
            })
            .is_none()
        );
    }

    /// A rename keeps trunk's behaviour: the edit lands, and the draft is
    /// rekeyed with the library when the rename completes.
    #[test]
    fn a_symbol_edit_during_a_library_rename_is_not_refused() {
        let mut app = app_with_library_rename();
        let path = app_with_open_symbol(&mut app);
        let p = path.to_string_lossy().into_owned();

        let _task = app.update(symbol_edit(&p, SymbolEditorMsg::AddPin { x: 1.0, y: 2.0 }));

        assert!(app.document_state.symbol_editors[&path].is_dirty());
        assert!(app.editor_edit_refusal(&path).is_none());
    }

    #[test]
    fn a_sketch_solve_answer_during_a_library_rename_is_not_refused() {
        let app = app_with_library_rename();
        let msg = LibraryMessage::PrimitiveEditorEvent {
            path: "/w/a.snxlib/footprints/f.snxfpt".into(),
            msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchSolved {
                instance: uuid::Uuid::nil(),
                generation: 1,
                footprint_idx: 0,
                outcome: std::sync::Arc::new(
                    crate::library::editor::footprint::sketch_solve_task::SolveOutcome::SolveFailed(
                        signex_sketch::SolveError::DidNotConverge {
                            iters: 0,
                            final_residual_norm: 0.0,
                        },
                    ),
                ),
            }),
        };
        assert!(
            app.primitive_edit_refusal(&msg).is_none(),
            "a completion must land, or the solve slot stays in flight"
        );
    }

    /// An app with a symbol editor open as the active tab and one undoable
    /// edit made, then a library delete started.
    fn app_with_symbol_edited_then_delete() -> (Signex, std::path::PathBuf) {
        let (mut app, _task) = Signex::new();
        let path = std::path::PathBuf::from("/w/a.snxlib/symbols/r.snxsym");
        let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("S"));
        app.document_state.symbol_editors.insert(
            path.clone(),
            crate::app::SymbolDocument::new(path.clone(), file),
        );
        app.document_state.tabs.push(crate::app::TabInfo {
            title: "r".into(),
            path: path.clone(),
            cached_document: None,
            project_id: None,
            kind: crate::app::TabKind::SymbolEditor(path.clone()),
        });
        app.document_state.active_tab = app.document_state.tabs.len() - 1;
        let p = path.to_string_lossy().into_owned();
        let _ = app.update(symbol_edit(&p, SymbolEditorMsg::AddPin { x: 1.0, y: 2.0 }));
        assert!(app.document_state.symbol_editors[&path].is_dirty());
        app.document_state.pending_path_ops.begin(
            PathOpKind::Delete,
            vec!["/w/a.snxlib".into()],
            CaseFold::PRESERVE,
        );
        (app, path)
    }

    fn symbol_state(app: &Signex, path: &std::path::Path) -> String {
        format!("{:?}", app.document_state.symbol_editors[path].snapshot())
    }

    /// Ctrl+Z reaches the editor outside the message stream, so it carries
    /// its own refusal: same command, same answer from every surface.
    #[test]
    fn ctrl_z_on_an_editor_during_a_library_delete_is_refused() {
        let (mut app, path) = app_with_symbol_edited_then_delete();
        let before = symbol_state(&app, &path);

        let _ = app.update(crate::app::Message::Edit(crate::app::EditMsg::Undo));

        assert_eq!(symbol_state(&app, &path), before, "the editor is unchanged");
        // Once the delete ends, the same Ctrl+Z undoes the edit.
        app.document_state.pending_path_ops = Default::default();
        let _ = app.update(crate::app::Message::Edit(crate::app::EditMsg::Undo));
        assert_ne!(symbol_state(&app, &path), before);
    }

    #[test]
    fn a_properties_panel_edit_during_a_library_delete_is_refused() {
        let (mut app, path) = app_with_symbol_edited_then_delete();
        let before = symbol_state(&app, &path);

        let _ = app.update(crate::app::Message::Dock(crate::dock::DockMessage::Panel(
            crate::panels::PanelMsg::SymEditorSetSymbolName("Renamed".into()),
        )));

        assert_eq!(symbol_state(&app, &path), before, "the editor is unchanged");
        app.document_state.pending_path_ops = Default::default();
        let _ = app.update(crate::app::Message::Dock(crate::dock::DockMessage::Panel(
            crate::panels::PanelMsg::SymEditorSetSymbolName("Renamed".into()),
        )));
        assert_ne!(
            symbol_state(&app, &path),
            before,
            "the edit works unguarded"
        );
    }

    #[test]
    fn a_redundancy_answer_during_a_library_rename_is_not_refused() {
        let app = app_with_library_rename();
        let msg = LibraryMessage::PrimitiveEditorEvent {
            path: "/w/a.snxlib/footprints/f.snxfpt".into(),
            msg: PrimitiveEdit::Footprint(FootprintEditorMsg::RedundancyChecked {
                instance: uuid::Uuid::nil(),
                generation: 1,
                footprint_idx: 0,
                report: Err("task panicked".to_string()),
            }),
        };
        assert!(
            app.primitive_edit_refusal(&msg).is_none(),
            "a completion must land, or the redundancy slot stays in flight"
        );
    }

    #[test]
    fn a_write_to_an_unrelated_library_is_not_refused() {
        let app = app_with_library_rename();
        let commit = |library: &str| LibraryMessage::BrowserCellCommit {
            library_path: library.into(),
            table: "t".into(),
            row_id: signex_library::RowId::default(),
            column: String::new(),
        };
        assert!(
            app.library_write_refusal(&commit("/w/other.snxlib"))
                .is_none()
        );
        assert!(app.library_write_refusal(&commit("/w/a.snxlib")).is_some());
        assert!(app.library_write_refusal(&commit("/w/b.snxlib")).is_some());
    }

    #[test]
    fn a_library_delete_blocks_adapter_writes_and_so_does_a_picker_pick() {
        let (mut app, _task) = Signex::new();
        app.document_state.pending_path_ops.begin(
            PathOpKind::Delete,
            vec!["/w/a.snxlib".into()],
            CaseFold::PRESERVE,
        );
        let pick = LibraryMessage::PrimitivePicker(PrimitivePickerMsg::Pick(
            signex_library::PrimitiveRef::new(uuid::Uuid::nil(), uuid::Uuid::nil()),
        ));
        assert!(app.library_write_refusal(&pick).is_some());
    }

    /// "Commit now" and Copy to Library write into the library on a worker
    /// through its old path; their completions still go through.
    #[test]
    fn commit_now_and_stock_copies_are_refused_while_their_library_moves() {
        let app = app_with_library_rename();
        let refused = |msg: LibraryMessage| app.library_write_refusal(&msg).is_some();
        assert!(refused(LibraryMessage::BrowserCommitPending {
            library_path: "/w/a.snxlib".into(),
        }));
        assert!(refused(LibraryMessage::StockCopy(
            StockCopyMsg::TargetPicked("/w/a.snxlib".into())
        )));
        assert!(refused(LibraryMessage::StockCopy(
            StockCopyMsg::CopyAgainRequested("/w/a.snxlib/symbols/r.snxsym".into())
        )));
        assert!(refused(LibraryMessage::StockCopy(
            StockCopyMsg::NewLibraryPathChosen(Some("/w/new".into()))
        )));
        assert!(!refused(LibraryMessage::StockCopy(
            StockCopyMsg::TargetPicked("/w/other.snxlib".into())
        )));
        assert!(!refused(LibraryMessage::BrowserCommitPendingDone {
            library_path: "/w/a.snxlib".into(),
            result: Err("x".into()),
        }));
    }

    #[test]
    fn a_stock_copy_in_flight_holds_any_library_but_not_a_plain_file() {
        let (mut app, _task) = Signex::new();
        let running = |app: &Signex, path: &str| {
            app.library_background_write_running(std::path::Path::new(path), CaseFold::PRESERVE)
        };
        assert!(!running(&app, "/w/a.snxlib"));
        app.library
            .stock_copies_in_flight
            .insert("/stock/r.snxsym".into());
        assert!(running(&app, "/w/a.snxlib"));
        assert!(!running(&app, "/w/a.snxsch"));
    }

    /// A library edit queued on its worker (#893) holds a rename or delete
    /// of that library, and only that one.
    #[test]
    fn a_queued_library_edit_holds_its_library() {
        let (mut app, _task) = Signex::new();
        let library = std::path::Path::new("/w/a.snxlib");
        let in_flight = app.library.writes.turn(library);
        assert!(app.library_background_write_running(library, CaseFold::PRESERVE));
        let variant = std::path::Path::new("/w/A.snxlib");
        assert!(app.library_background_write_running(variant, CaseFold::FOLD));
        assert!(!app.library_background_write_running(variant, CaseFold::PRESERVE));
        let other = std::path::Path::new("/w/b.snxlib");
        assert!(!app.library_background_write_running(other, CaseFold::PRESERVE));
        drop(in_flight);
        assert!(!app.library_background_write_running(library, CaseFold::PRESERVE));
    }

    /// A datasheet PDF store writes beside the library on a worker, so it
    /// holds a rename or delete of that library too.
    #[test]
    fn a_datasheet_store_in_flight_holds_its_library() {
        let (mut app, _task) = Signex::new();
        let library = std::path::Path::new("/w/a.snxlib");
        let store = app
            .document_state
            .document_saves
            .begin_work(library.to_path_buf());
        assert!(app.library_background_write_running(library, CaseFold::PRESERVE));
        drop(store);
        app.document_state.document_saves.finish(library);
        assert!(!app.library_background_write_running(library, CaseFold::PRESERVE));
    }

    #[test]
    fn a_commit_at_a_case_variant_of_the_root_counts_only_when_folding() {
        let roots = [std::path::Path::new("/w/alpha.snxlib")];
        let variant = std::path::Path::new("/w/ALPHA.snxlib");
        assert!(any_root_under(roots.into_iter(), variant, CaseFold::FOLD));
        assert!(!any_root_under(
            roots.into_iter(),
            variant,
            CaseFold::PRESERVE
        ));
        let other = std::path::Path::new("/w/beta.snxlib");
        assert!(!any_root_under(roots.into_iter(), other, CaseFold::FOLD));
    }

    #[test]
    fn deleting_a_plain_file_does_not_block_library_edits() {
        let (mut app, _task) = Signex::new();
        app.document_state.pending_path_ops.begin(
            PathOpKind::Delete,
            vec!["/w/a.snxsch".into()],
            CaseFold::PRESERVE,
        );
        assert!(
            app.library_write_refusal(&LibraryMessage::NewComponent)
                .is_none()
        );
    }

    #[test]
    fn nothing_is_refused_without_a_library_rename() {
        let (app, _task) = Signex::new();
        assert!(
            app.library_write_refusal(&LibraryMessage::NewComponent)
                .is_none()
        );
    }
}
