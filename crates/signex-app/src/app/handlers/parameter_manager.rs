//! Parameter Manager: the message router and the table edits (cell writes,
//! column add and remove, Revert changes).
//!
//! Every write goes through the mutation gateway, so each committed edit is
//! exactly one engine history entry and dirty-flagging stays correct.

use iced::Task;
use iced::widget::operation::{Animation, scrollable::scroll_to};
use iced::widget::scrollable::AbsoluteOffset;
use signex_engine::Command;
use uuid::Uuid;

use crate::app::contracts::ParameterTablePane;
use crate::app::state::{ModalId, ParameterCell, Signex};
use crate::app::{Message, ParameterManagerMsg};

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
                    self.ui_state.parameter_table = Default::default();
                }
                self.ui_state.parameter_manager_open = true;
                self.handle_detach_modal(ModalId::ParameterManager)
            }
            ParameterManagerMsg::Close => self.close_parameter_manager(),
            ParameterManagerMsg::Edit {
                symbol_uuid,
                key,
                value,
            } => self.handle_parameter_manager_edit(symbol_uuid, key, value),
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
                symbol_uuid,
                is_focused,
            } => {
                let editing = &mut self.ui_state.parameter_table.editing_row;
                if is_focused {
                    *editing = Some(symbol_uuid);
                } else if *editing == Some(symbol_uuid) {
                    *editing = None;
                }
                Task::none()
            }
            ParameterManagerMsg::TableScrolled { pane, x, y } => follow_table_scroll(pane, x, y),
        }
    }

    /// Write one parameter value now. An empty value empties the parameter,
    /// it does not delete it. Keystrokes in one cell are one undo step.
    pub(crate) fn handle_parameter_manager_edit(
        &mut self,
        symbol_uuid: Uuid,
        key: String,
        value: String,
    ) -> Task<Message> {
        // A run belongs to one document: moving to another sheet ends the
        // previous sheet's, so coming back never folds into an old step.
        let path = self.document_state.active_path.clone();
        if self.ui_state.parameter_table.run_path != path {
            self.end_parameter_edit_runs();
            self.ui_state.parameter_table.run_path = path.clone();
        }
        // The value before the session's first edit of this cell, for the
        // changed mark and Revert changes. A symbol the sheet lacks is not
        // a cell; the edit below changes nothing either.
        let before = self.document_state.active_engine().and_then(|engine| {
            engine
                .document()
                .symbols
                .iter()
                .find(|s| s.uuid == symbol_uuid)
                .map(|s| s.fields.get(&key).cloned())
        });
        if let Some(before) = before {
            self.ui_state.parameter_table.session.record(
                ParameterCell {
                    sheet: path,
                    symbol: symbol_uuid,
                    key: key.clone(),
                },
                before,
            );
        }
        // Same reasoning as `handle_move_selection_apply` (#585). The
        // gateway clears the content cache, syncs the canvas and refreshes
        // the panel context, so the hand-rolled versions of those are gone;
        // the overlay cache and the selection info were not touched before
        // and still are not.
        self.apply_engine_command(
            Command::SetSymbolField {
                symbol_id: symbol_uuid,
                key,
                value,
            },
            false,
            false,
        );
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

    /// The cells of the active sheet edited since the dialog opened whose
    /// value now differs from before, each with that earlier value. It
    /// reads the document, so Undo and Redo move the count too.
    pub(crate) fn parameter_session_changes(&self) -> Vec<(ParameterCell, Option<String>)> {
        let Some(engine) = self.document_state.active_engine() else {
            return Vec::new();
        };
        let symbols = &engine.document().symbols;
        let sheet = self.document_state.active_path.as_deref();
        self.ui_state
            .parameter_table
            .session
            .cells_on(sheet)
            .filter(|(cell, before)| {
                symbols
                    .iter()
                    .find(|s| s.uuid == cell.symbol)
                    .is_some_and(|s| s.fields.get(&cell.key) != before.as_ref())
            })
            .map(|(cell, before)| (cell.clone(), before.clone()))
            .collect()
    }

    /// Put every changed cell of the session back, as one undo step: one
    /// engine batch through the gateway, so one Undo brings the edits back.
    pub(crate) fn handle_parameter_session_reverted(&mut self) -> Task<Message> {
        let commands: Vec<Command> = self
            .parameter_session_changes()
            .into_iter()
            .map(|(cell, before)| match before {
                Some(value) => Command::SetSymbolField {
                    symbol_id: cell.symbol,
                    key: cell.key,
                    value,
                },
                None => Command::RemoveSymbolField {
                    symbol_id: cell.symbol,
                    key: cell.key,
                },
            })
            .collect();
        // An open typing run would fold the revert into the last cell's step.
        self.end_parameter_edit_runs();
        self.apply_engine_commands(commands, false, false);
        Task::none()
    }

    /// Delete parameter `key` from every non-power symbol that carries it
    /// (the table hides power symbols), as one undo step.
    pub(crate) fn handle_parameter_removed(&mut self, key: &str) -> Task<Message> {
        let Some(engine) = self.document_state.active_engine() else {
            return Task::none();
        };
        let commands: Vec<Command> = engine
            .document()
            .symbols
            .iter()
            .filter(|s| !s.is_power && s.fields.contains_key(key))
            .map(|s| Command::RemoveSymbolField {
                symbol_id: s.uuid,
                key: key.to_string(),
            })
            .collect();
        self.apply_engine_commands(commands, false, false);
        if self.ui_state.parameter_table.hovered_column.as_deref() == Some(key) {
            self.ui_state.parameter_table.hovered_column = None;
        }
        Task::none()
    }

    /// Add the typed name as a parameter on every non-power symbol, empty,
    /// as one undo step. A blank or reserved name adds nothing; a symbol
    /// that already carries the name keeps its value.
    pub(crate) fn handle_parameter_column_added(&mut self) -> Task<Message> {
        let key = self.ui_state.parameter_table.new_key.trim().to_string();
        if key.is_empty() || signex_engine::is_reserved_symbol_key(&key) {
            return Task::none();
        }
        let Some(engine) = self.document_state.active_engine() else {
            return Task::none();
        };
        let commands: Vec<Command> = engine
            .document()
            .symbols
            .iter()
            .filter(|s| !s.is_power && !s.fields.contains_key(&key))
            .map(|s| Command::SetSymbolField {
                symbol_id: s.uuid,
                key: key.clone(),
                value: String::new(),
            })
            .collect();
        if self.apply_engine_commands(commands, false, false) {
            self.ui_state.parameter_table.new_key.clear();
        }
        Task::none()
    }

    /// Close the manager's window and mark it closed; the session ends.
    pub(crate) fn close_parameter_manager(&mut self) -> Task<Message> {
        self.end_parameter_edit_runs();
        self.ui_state.parameter_manager_open = false;
        self.ui_state.parameter_table = Default::default();
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
mod tests {
    use super::*;
    use crate::app::{EditMsg, ParameterManagerMsg as Pm};
    use crate::app::{TabInfo, TabKind};
    use signex_types::schematic::{SchematicSheet, Symbol};

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
        let doc = app.document_state.active_engine().unwrap().document();
        let filter = &app.ui_state.parameter_table.filter;
        assert!(matches!(
            table_body(&app.ui_state.i18n, filter, doc),
            TableBody::NoMatch
        ));

        let _ = app.update(Message::ParameterManager(Pm::FilterCleared));
        assert!(app.ui_state.parameter_table.filter.is_empty());
        let doc = app.document_state.active_engine().unwrap().document();
        let TableBody::Rows(rows) = table_body(&app.ui_state.i18n, "", doc) else {
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
                symbol_uuid: Uuid::from_u128(n),
                is_focused,
            })
        };
        let editing = |app: &Signex| app.ui_state.parameter_table.editing_row;
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
