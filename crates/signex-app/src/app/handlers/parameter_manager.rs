//! Parameter Manager: the message router, the sheets the table lists and
//! the table edits (cell writes, column add and remove, Revert changes).
//!
//! An edit of the active sheet goes through the mutation gateway; a cell of
//! another listed sheet goes through `execute_on_sheet`, one undo step on
//! that sheet. An edit that changes several sheets (a column, Revert
//! changes) is one undo group through `write_sheets_as_group`, so one
//! Ctrl+Z takes it back on every sheet. All of them settle through the one
//! edit tail, so dirty marks and repaints stay correct.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use iced::Task;
use iced::widget::operation::{Animation, scrollable::scroll_to};
use iced::widget::scrollable::AbsoluteOffset;
use signex_engine::Command;
use signex_i18n::{ids, t};
use signex_types::schematic::SchematicSheet;

use crate::app::contracts::{ParameterScope, ParameterTablePane};
use crate::app::sheet_write::{SheetWrite, SheetWriteRefusal};
use crate::app::state::{ModalId, ParameterCell, Signex};
use crate::app::{Message, ParameterManagerMsg, UndoGroupKind};

/// Widget id of the scrollable parameter cells.
pub(crate) const BODY_SCROLL_ID: &str = "parameter-manager-body";
/// Widget id of the header row right of Reference.
pub(crate) const HEADER_SCROLL_ID: &str = "parameter-manager-header";
/// Widget id of the Reference column.
pub(crate) const REFERENCE_SCROLL_ID: &str = "parameter-manager-references";

impl Signex {
    /// Parameter Manager dialog handler (namespaced family, ADR-0001 D3).
    pub(crate) fn dispatch_parameter_manager_message(
        &mut self,
        msg: ParameterManagerMsg,
    ) -> Task<Message> {
        match msg {
            ParameterManagerMsg::Open => {
                // Already open: keep the filter, the half-typed column name
                // and the session.
                if !self.ui_state.parameter_manager_open {
                    *self.ui_state.parameter_table = Default::default();
                }
                self.ui_state.parameter_manager_open = true;
                self.handle_detach_modal(ModalId::ParameterManager)
            }
            ParameterManagerMsg::Close => self.close_parameter_manager(),
            ParameterManagerMsg::Edit {
                sheet,
                symbol_uuid,
                key,
                value,
            } => self.handle_parameter_manager_edit(
                ParameterCell {
                    sheet: Some(sheet),
                    symbol: symbol_uuid,
                    key,
                },
                value,
            ),
            ParameterManagerMsg::CellSubmitted => {
                self.end_parameter_edit_runs();
                Task::none()
            }
            ParameterManagerMsg::ParameterRemoved { key } => self.handle_parameter_removed(&key),
            ParameterManagerMsg::NewKeyChanged(key) => {
                self.ui_state.parameter_table.new_key = key;
                Task::none()
            }
            ParameterManagerMsg::ColumnAdded => self.handle_parameter_column_added(),
            ParameterManagerMsg::ScopePicked(scope) => {
                self.ui_state.parameter_table.scope = scope;
                Task::none()
            }
            ParameterManagerMsg::FilterChanged(filter) => {
                self.ui_state.parameter_table.filter = filter;
                Task::none()
            }
            ParameterManagerMsg::FilterCleared => {
                self.ui_state.parameter_table.filter.clear();
                Task::none()
            }
            ParameterManagerMsg::SessionReverted => self.handle_parameter_session_reverted(),
            ParameterManagerMsg::ColumnHeaderHovered { key, is_hovered } => {
                let hovered = &mut self.ui_state.parameter_table.hovered_column;
                if is_hovered {
                    *hovered = Some(key);
                } else if hovered.as_deref() == Some(key.as_str()) {
                    *hovered = None;
                }
                Task::none()
            }
            ParameterManagerMsg::CellFocused {
                sheet,
                symbol_uuid,
                key,
                is_focused,
            } => {
                let cell = ParameterCell {
                    sheet: Some(sheet),
                    symbol: symbol_uuid,
                    key,
                };
                let editing = &mut self.ui_state.parameter_table.editing;
                if is_focused {
                    *editing = Some(cell);
                } else if editing.as_ref() == Some(&cell) {
                    *editing = None;
                }
                Task::none()
            }
            ParameterManagerMsg::TableScrolled { pane, x, y } => follow_table_scroll(pane, x, y),
            ParameterManagerMsg::Io(msg) => self.dispatch_parameter_io(msg),
        }
    }

    /// The sheets the table lists, the active one first: with Active sheet
    /// only it; with All sheets also every other open sheet of its project
    /// (for a sheet outside any project, every other open sheet outside
    /// one), by path. Empty without an active schematic.
    pub(crate) fn parameter_sheets(&self) -> Vec<PathBuf> {
        let docs = &self.document_state;
        let Some(active) = docs.active_path.clone() else {
            return Vec::new();
        };
        if !docs.engines.contains_key(&active) {
            return Vec::new();
        }
        if self.ui_state.parameter_table.scope == ParameterScope::ActiveSheet {
            return vec![active];
        }
        let project_of = |path: &Path| docs.document_project(path).map(|p| p.id);
        let owner = project_of(&active);
        let mut others: Vec<PathBuf> = docs
            .engines
            .keys()
            .filter(|path| **path != active && project_of(path) == owner)
            .cloned()
            .collect();
        others.sort();
        std::iter::once(active).chain(others).collect()
    }

    /// [`Self::parameter_sheets`] with each sheet's live document.
    pub(crate) fn parameter_documents(&self) -> Vec<(&Path, &SchematicSheet)> {
        self.parameter_sheets()
            .iter()
            .filter_map(|path| {
                let (path, engine) = self.document_state.engines.get_key_value(path)?;
                Some((path.as_path(), engine.document()))
            })
            .collect()
    }

    /// Write one parameter value now. An empty value empties the parameter,
    /// it does not delete it. Keystrokes in one cell are one undo step on
    /// the cell's sheet.
    pub(crate) fn handle_parameter_manager_edit(
        &mut self,
        cell: ParameterCell,
        value: String,
    ) -> Task<Message> {
        let Some(sheet) = cell.sheet.clone() else {
            return Task::none();
        };
        // A run belongs to one document: moving to another sheet ends the
        // previous sheet's, so coming back never folds into an old step.
        if self.ui_state.parameter_table.run_path.as_ref() != Some(&sheet) {
            self.end_parameter_edit_runs();
            self.ui_state.parameter_table.run_path = Some(sheet.clone());
        }
        // The value before the session's first edit of this cell, for the
        // changed mark and Revert changes. A symbol the sheet lacks is not
        // a cell; the edit below changes nothing either.
        let before = self.document_state.engines.get(&sheet).and_then(|engine| {
            engine
                .document()
                .symbols
                .iter()
                .find(|s| s.uuid == cell.symbol)
                .map(|s| s.fields.get(&cell.key).cloned())
        });
        let command = Command::SetSymbolField {
            symbol_id: cell.symbol,
            key: cell.key.clone(),
            value,
        };
        if let Some(before) = before {
            self.ui_state.parameter_table.session.record(cell, before);
        }
        if self.document_state.active_path.as_deref() == Some(sheet.as_path()) {
            // Same reasoning as `handle_move_selection_apply` (#585): the
            // gateway clears the content cache, syncs the canvas and
            // refreshes the panel context; the overlay cache and the
            // selection info are not touched.
            self.apply_engine_command(command, false, false);
        } else if self.refuse_if_read_only_at(&sheet).is_none()
            && let Err(reason) = self.execute_on_sheet(&sheet, command)
        {
            crate::diagnostics::log_warning(format!("Parameter Manager: {reason}"));
        }
        Task::none()
    }

    /// Close every engine's cell-typing run, so the next edit of the same
    /// cell is a new undo step. Called on Enter, on Close, and before an
    /// app-level history step (net colour, net-class pick) is stamped with
    /// the engine's undo position: a run that kept amending the entry at
    /// that position would order the engine edit after the step recorded
    /// later.
    pub(crate) fn end_parameter_edit_runs(&mut self) {
        for engine in self.document_state.engines.values_mut() {
            engine.end_field_edit_run();
        }
    }

    /// The cells of the listed sheets edited since the dialog opened whose
    /// value now differs from before, each with that earlier value. It
    /// reads the documents, so Undo and Redo move the count too.
    pub(crate) fn parameter_session_changes(&self) -> Vec<(ParameterCell, Option<String>)> {
        let session = &self.ui_state.parameter_table.session;
        let mut changes = Vec::new();
        for (path, doc) in self.parameter_documents() {
            let changed = session.cells_on(Some(path)).filter(|(cell, before)| {
                doc.symbols
                    .iter()
                    .find(|s| s.uuid == cell.symbol)
                    .is_some_and(|s| s.fields.get(&cell.key) != before.as_ref())
            });
            changes.extend(changed.map(|(cell, before)| (cell.clone(), before.clone())));
        }
        changes
    }

    /// Put every changed cell of the session back, as one undo step across
    /// the sheets it touches, so one Undo brings the edits back.
    pub(crate) fn handle_parameter_session_reverted(&mut self) -> Task<Message> {
        let mut by_sheet: BTreeMap<PathBuf, Vec<Command>> = BTreeMap::new();
        for (cell, before) in self.parameter_session_changes() {
            let Some(sheet) = cell.sheet else {
                continue;
            };
            let command = match before {
                Some(value) => Command::SetSymbolField {
                    symbol_id: cell.symbol,
                    key: cell.key,
                    value,
                },
                None => Command::RemoveSymbolField {
                    symbol_id: cell.symbol,
                    key: cell.key,
                },
            };
            by_sheet.entry(sheet).or_default().push(command);
        }
        // An open typing run would fold the revert into the last cell's step.
        self.end_parameter_edit_runs();
        self.apply_parameter_commands(by_sheet);
        Task::none()
    }

    /// Delete parameter `key` from every listed non-power symbol that
    /// carries it (the table hides power symbols), as one undo step.
    pub(crate) fn handle_parameter_removed(&mut self, key: &str) -> Task<Message> {
        let by_sheet = self.parameter_commands_by_sheet(|doc| {
            (doc.symbols.iter())
                .filter(|s| !s.is_power && s.fields.contains_key(key))
                .map(|s| Command::RemoveSymbolField {
                    symbol_id: s.uuid,
                    key: key.to_string(),
                })
                .collect()
        });
        self.apply_parameter_commands(by_sheet);
        if self.ui_state.parameter_table.hovered_column.as_deref() == Some(key) {
            self.ui_state.parameter_table.hovered_column = None;
        }
        Task::none()
    }

    /// Add the typed name as a parameter on every listed non-power symbol,
    /// empty, as one undo step. A blank or reserved name adds nothing; a
    /// symbol that already carries the name keeps its value.
    pub(crate) fn handle_parameter_column_added(&mut self) -> Task<Message> {
        let key = self.ui_state.parameter_table.new_key.trim().to_string();
        if key.is_empty() || signex_engine::is_reserved_symbol_key(&key) {
            return Task::none();
        }
        let by_sheet = self.parameter_commands_by_sheet(|doc| {
            (doc.symbols.iter())
                .filter(|s| !s.is_power && !s.fields.contains_key(&key))
                .map(|s| Command::SetSymbolField {
                    symbol_id: s.uuid,
                    key: key.clone(),
                    value: String::new(),
                })
                .collect()
        });
        if self.apply_parameter_commands(by_sheet) {
            self.ui_state.parameter_table.new_key.clear();
        }
        Task::none()
    }

    /// `commands` of each listed sheet's live document, by sheet.
    fn parameter_commands_by_sheet(
        &self,
        commands: impl Fn(&SchematicSheet) -> Vec<Command>,
    ) -> BTreeMap<PathBuf, Vec<Command>> {
        self.parameter_documents()
            .into_iter()
            .map(|(path, doc)| (path.to_path_buf(), commands(doc)))
            .collect()
    }

    /// Run each sheet's commands as one undo step: through the gateway when
    /// only the active sheet has any, else as one undo group across the
    /// sheets, all or nothing. `true` when a sheet changed.
    fn apply_parameter_commands(&mut self, mut by_sheet: BTreeMap<PathBuf, Vec<Command>>) -> bool {
        by_sheet.retain(|_, commands| !commands.is_empty());
        let active = self.document_state.active_path.clone();
        let has_active = active
            .as_ref()
            .is_some_and(|path| by_sheet.contains_key(path));
        if by_sheet.is_empty() || (has_active && by_sheet.len() == 1) {
            let commands = by_sheet.into_values().next().unwrap_or_default();
            return self.apply_engine_commands(commands, false, false);
        }
        // A channel tab of a repeated sheet is a compiled view (#1279).
        if has_active && self.refuse_if_channel_view().is_some() {
            return false;
        }
        // An open typing run would fold this step into the last cell's.
        self.end_parameter_edit_runs();
        let loaded = HashMap::new();
        let write = SheetWrite {
            label: "Parameter Manager",
            outcome: "edited",
            kind: UndoGroupKind::Parameters,
            loaded: &loaded,
        };
        match self.write_sheets_as_group(&write, by_sheet, |_, commands| commands.clone()) {
            Ok(sheets) => sheets > 0,
            Err(refusal) => {
                self.log_parameter_refusal(&refusal);
                false
            }
        }
    }

    /// Say in Messages why an edit across sheets changed nothing.
    fn log_parameter_refusal(&self, refusal: &SheetWriteRefusal) {
        let (id, path) = match refusal {
            SheetWriteRefusal::Busy(path) => (ids::DIALOG_PARAMETER_MANAGER_REFUSED_BUSY, path),
            SheetWriteRefusal::ReadOnly(path) => {
                (ids::DIALOG_PARAMETER_MANAGER_REFUSED_READ_ONLY, path)
            }
            SheetWriteRefusal::NotLoaded(path) | SheetWriteRefusal::NotChanged(path) => {
                (ids::DIALOG_PARAMETER_MANAGER_REFUSED_FAILED, path)
            }
        };
        let sheet = path.file_name().map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        let line = t!(self.ui_state.i18n, id, "sheet" => sheet).into_string();
        crate::diagnostics::log_warning(line);
    }

    /// Close the manager's window and mark it closed; the session ends.
    pub(crate) fn close_parameter_manager(&mut self) -> Task<Message> {
        self.end_parameter_edit_runs();
        self.ui_state.parameter_manager_open = false;
        *self.ui_state.parameter_table = Default::default();
        self.close_detached_modal(ModalId::ParameterManager)
    }
}

/// Scroll the other panes to where `pane` went: the header row follows the
/// body sideways, the Reference column follows it up and down, and either
/// of them moves the body.
fn follow_table_scroll(pane: ParameterTablePane, x: f32, y: f32) -> Task<Message> {
    let to = |id: &'static str, x: Option<f32>, y: Option<f32>| -> Task<Message> {
        scroll_to(id, AbsoluteOffset { x, y }, Animation::Instant)
    };
    match pane {
        ParameterTablePane::Body => Task::batch([
            to(HEADER_SCROLL_ID, Some(x), None),
            to(REFERENCE_SCROLL_ID, None, Some(y)),
        ]),
        ParameterTablePane::Header => to(BODY_SCROLL_ID, Some(x), None),
        ParameterTablePane::References => to(BODY_SCROLL_ID, None, Some(y)),
    }
}

#[cfg(test)]
#[path = "parameter_manager_scope_tests.rs"]
mod scope_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{EditMsg, ParameterManagerMsg as Pm};
    use crate::app::{TabInfo, TabKind};
    use signex_types::schematic::{SchematicSheet, Symbol};
    use uuid::Uuid;

    const SHEET: &str = "/w/parameters.snxsch";

    fn part(n: u128, reference: &str, tolerance: &str) -> Symbol {
        let mut sym = Symbol {
            uuid: Uuid::from_u128(n),
            lib_id: "Device:R".to_string(),
            reference: reference.to_string(),
            value: "10k".to_string(),
            footprint: "R_0603".to_string(),
            ..Default::default()
        };
        sym.fields
            .insert("Tolerance".to_string(), tolerance.to_string());
        sym
    }

    /// An app with one sheet of three resistors, each with a Tolerance.
    fn app() -> Signex {
        let (mut app, _task) = Signex::new();
        app.ui_state.first_run_tour_open = false;
        let sheet = SchematicSheet {
            uuid: Uuid::now_v7(),
            paper_size: "A4".to_string(),
            symbols: vec![part(1, "R1", "1%"), part(2, "R2", "5%"), part(3, "R3", "")],
            ..Default::default()
        };
        let path = std::path::PathBuf::from(SHEET);
        app.document_state.engines.insert(
            path.clone(),
            signex_engine::Engine::new(sheet).expect("engine"),
        );
        app.document_state.tabs.push(TabInfo {
            title: "parameters".to_string(),
            path: path.clone(),
            cached_document: None,
            project_id: None,
            kind: TabKind::Schematic,
        });
        app.document_state.active_tab = 0;
        app.document_state.active_path = Some(path);
        let _ = app.update(Message::ParameterManager(Pm::Open));
        app
    }

    fn tolerance(app: &Signex, n: u128) -> Option<String> {
        app.document_state
            .active_engine()
            .expect("engine")
            .document()
            .symbols
            .iter()
            .find(|s| s.uuid == Uuid::from_u128(n))
            .and_then(|s| s.fields.get("Tolerance").cloned())
    }

    fn edit(app: &mut Signex, n: u128, value: &str) {
        let _ = app.update(Message::ParameterManager(Pm::Edit {
            sheet: SHEET.into(),
            symbol_uuid: Uuid::from_u128(n),
            key: "Tolerance".to_string(),
            value: value.to_string(),
        }));
        let _ = app.update(Message::ParameterManager(Pm::CellSubmitted));
    }

    fn edit_three(app: &mut Signex) {
        edit(app, 1, "2%");
        edit(app, 2, "10%");
        edit(app, 3, "0.1%");
    }

    #[test]
    fn revert_restores_three_cells_and_one_undo_takes_the_revert_back() {
        let mut app = app();
        edit_three(&mut app);
        assert_eq!(app.parameter_session_changes().len(), 3);

        let _ = app.update(Message::ParameterManager(Pm::SessionReverted));
        assert_eq!(tolerance(&app, 1).as_deref(), Some("1%"));
        assert_eq!(tolerance(&app, 2).as_deref(), Some("5%"));
        assert_eq!(tolerance(&app, 3).as_deref(), Some(""));
        assert!(app.parameter_session_changes().is_empty());

        let _ = app.update(Message::Edit(EditMsg::Undo));
        assert_eq!(tolerance(&app, 1).as_deref(), Some("2%"));
        assert_eq!(tolerance(&app, 2).as_deref(), Some("10%"));
        assert_eq!(
            tolerance(&app, 3).as_deref(),
            Some("0.1%"),
            "one Undo must take the whole revert back"
        );
        assert_eq!(app.parameter_session_changes().len(), 3);
    }

    #[test]
    fn revert_removes_a_parameter_the_session_created() {
        let mut app = app();
        let _ = app.update(Message::ParameterManager(Pm::Edit {
            sheet: SHEET.into(),
            symbol_uuid: Uuid::from_u128(1),
            key: "MPN".to_string(),
            value: "GRM31CR71E106KA12".to_string(),
        }));
        let _ = app.update(Message::ParameterManager(Pm::SessionReverted));
        let doc = app.document_state.active_engine().unwrap().document();
        assert!(!doc.symbols[0].fields.contains_key("MPN"));
    }

    #[test]
    fn a_cell_typed_back_to_its_old_value_is_not_counted() {
        let mut app = app();
        edit(&mut app, 1, "2%");
        edit(&mut app, 1, "1%");
        assert!(app.parameter_session_changes().is_empty());
    }

    #[test]
    fn the_changed_count_is_cleared_on_reopen() {
        let mut app = app();
        edit_three(&mut app);
        let _ = app.update(Message::ParameterManager(Pm::Close));
        assert!(app.parameter_session_changes().is_empty());

        let _ = app.update(Message::ParameterManager(Pm::Open));
        assert!(
            app.parameter_session_changes().is_empty(),
            "a new opening starts a new session"
        );
        assert_eq!(
            tolerance(&app, 1).as_deref(),
            Some("2%"),
            "closing keeps the edits"
        );
    }

    #[test]
    fn opening_again_while_open_keeps_the_session() {
        let mut app = app();
        edit(&mut app, 1, "2%");
        let _ = app.update(Message::ParameterManager(Pm::Open));
        assert_eq!(app.parameter_session_changes().len(), 1);
    }

    #[test]
    fn a_filter_with_no_match_shows_the_card_and_clear_filter_brings_the_rows_back() {
        use crate::app::view::parameter_manager::{TableBody, table_body};
        let mut app = app();
        let _ = app.update(Message::ParameterManager(Pm::FilterChanged(
            "QFN-32".to_string(),
        )));
        let filter = &app.ui_state.parameter_table.filter;
        assert!(matches!(
            table_body(&app.ui_state.i18n, filter, &app.parameter_documents()),
            TableBody::NoMatch
        ));

        let _ = app.update(Message::ParameterManager(Pm::FilterCleared));
        assert!(app.ui_state.parameter_table.filter.is_empty());
        let sheets = app.parameter_documents();
        let TableBody::Rows(rows) = table_body(&app.ui_state.i18n, "", &sheets) else {
            panic!("every row must show again");
        };
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn removing_a_hovered_column_forgets_the_hover() {
        let mut app = app();
        let _ = app.update(Message::ParameterManager(Pm::ColumnHeaderHovered {
            key: "Tolerance".to_string(),
            is_hovered: true,
        }));
        let _ = app.update(Message::ParameterManager(Pm::ParameterRemoved {
            key: "Tolerance".to_string(),
        }));
        assert_eq!(app.ui_state.parameter_table.hovered_column, None);
    }

    #[test]
    fn leaving_a_header_keeps_the_hover_of_the_header_entered_first() {
        let mut app = app();
        let hover = |key: &str, is_hovered: bool| {
            Message::ParameterManager(Pm::ColumnHeaderHovered {
                key: key.to_string(),
                is_hovered,
            })
        };
        let _ = app.update(hover("MPN", true));
        let _ = app.update(hover("Tolerance", true));
        let _ = app.update(hover("MPN", false));
        assert_eq!(
            app.ui_state.parameter_table.hovered_column.as_deref(),
            Some("Tolerance")
        );
        let _ = app.update(hover("Tolerance", false));
        assert_eq!(app.ui_state.parameter_table.hovered_column, None);
    }

    /// Mockup row 70: the row of the focused cell is the row being edited.
    /// Moving to R3 and then the blur of R1 keeps R3; R3's own blur ends it,
    /// and closing the dialog forgets it.
    #[test]
    fn the_focused_cell_marks_its_row_as_edited_until_it_blurs() {
        let mut app = app();
        let focus = |n: u128, is_focused: bool| {
            Message::ParameterManager(Pm::CellFocused {
                sheet: SHEET.into(),
                symbol_uuid: Uuid::from_u128(n),
                key: "Tolerance".to_string(),
                is_focused,
            })
        };
        let editing = |app: &Signex| {
            let cell = app.ui_state.parameter_table.editing.as_ref();
            cell.map(|cell| cell.symbol)
        };
        let _ = app.update(focus(1, true));
        assert_eq!(editing(&app), Some(Uuid::from_u128(1)));
        let _ = app.update(focus(3, true));
        let _ = app.update(focus(1, false));
        assert_eq!(editing(&app), Some(Uuid::from_u128(3)));
        let _ = app.update(focus(3, false));
        assert_eq!(editing(&app), None);
        let _ = app.update(focus(2, true));
        let _ = app.update(Message::ParameterManager(Pm::Close));
        assert_eq!(editing(&app), None);
    }
}
