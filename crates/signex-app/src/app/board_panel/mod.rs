//! Panelisation (#2410): Tools ▸ Panelize Board and Remove Panel, the
//! Properties panel's Panel section, and the `panelize_board`,
//! `remove_board_panel`, `set_board_panel` and `board_panel` commands. The
//! panel is board data (`BoardOptions::panel`) changed only through
//! `BoardCommand::SetBoardPanel`, one undo step per edit; the geometry is
//! `pcb_geom::panel`. A panel that does not lay out on the board's outline
//! is refused before the command runs. Rules: `docs/PANELISATION.md`.

mod commands;

#[cfg(test)]
mod tests;

use std::path::Path;

use iced::Task;
use signex_engine::pcb::BoardCommand;
use signex_i18n::{ids, t};
use signex_types::measure::parse_length_field_mm;
use signex_types::pcb_panel::{BoardPanel, PanelError, PanelField, PanelFieldKind};

pub(crate) use commands::{panelize_board, remove_board_panel, set_board_panel};

use crate::app::state::pcb_edit::CheckedStep;
use crate::app::{Message, Signex};
use crate::panels::pcb_board_properties;

impl From<BoardPanelMsg> for Message {
    fn from(msg: BoardPanelMsg) -> Self {
        Self::Pcb(crate::app::PcbMsg::Panel(msg))
    }
}

/// What a person asked of the board in view's panel, namespaced under
/// `PcbMsg::Panel` and routed to `dispatch_board_panel_message`. The
/// Properties panel's settings arrive as `PcbBoardMsg` and meet these in
/// [`Signex::edit_active_board_panel`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardPanelMsg {
    /// `panelize_board` without operands: the default panel on the board
    /// in view.
    PanelizeRequested,
    /// `remove_board_panel` without operands.
    RemoveRequested,
}

/// What a panel edit did, or why it did nothing.
pub(crate) type PanelEdit = Result<bool, String>;

impl Signex {
    pub(crate) fn dispatch_board_panel_message(&mut self, msg: BoardPanelMsg) -> Task<Message> {
        let Some(path) = self.active_tab_path().filter(|_| self.has_active_pcb()) else {
            let line = t!(self.ui_state.i18n, ids::BOARD_PANEL_NO_PCB);
            crate::diagnostics::log_info(line.into_string());
            return Task::none();
        };
        let result = match msg {
            BoardPanelMsg::PanelizeRequested => self.panelize(&path),
            BoardPanelMsg::RemoveRequested => self.remove_panel(&path),
        };
        if let Err(why) = result {
            self.say_panel_refused(why);
        }
        Task::none()
    }

    /// The default panel on the board at `path`. A board that already has
    /// one keeps it, said in Messages.
    pub(crate) fn panelize(&mut self, path: &Path) -> PanelEdit {
        if self.board_panel_of(path)?.is_some() {
            let line = t!(self.ui_state.i18n, ids::BOARD_PANEL_ALREADY);
            crate::diagnostics::log_info(line.into_string());
            return Ok(false);
        }
        self.apply_board_panel(path, Some(BoardPanel::default()))
    }

    /// The board at `path` on its own again.
    pub(crate) fn remove_panel(&mut self, path: &Path) -> PanelEdit {
        if self.board_panel_of(path)?.is_none() {
            return Err(t!(self.ui_state.i18n, ids::BOARD_PANEL_NOTHING_TO_REMOVE).into_string());
        }
        self.apply_board_panel(path, None)
    }

    /// The panel of the open board at `path`.
    fn board_panel_of(&self, path: &Path) -> Result<Option<BoardPanel>, String> {
        self.document_state
            .pcb_board(path)
            .map(|board| board.options.panel)
            .ok_or_else(|| format!("no PCB is open at {}", path.display()))
    }

    /// Set `panel` on the board at `path` as one undo step; whether it
    /// changed. A panel that does not lay out on the board's outline is
    /// refused with the reason, and so is a refused command.
    pub(crate) fn apply_board_panel(
        &mut self,
        path: &Path,
        panel: Option<BoardPanel>,
    ) -> PanelEdit {
        let Some(engine) = self.document_state.board_engine(path) else {
            return Err(format!("no PCB is open at {}", path.display()));
        };
        let board = engine.board();
        if board.options.panel == panel {
            return Ok(false);
        }
        if let Some(panel) = &panel {
            pcb_geom::panel::layout(&board.outline, panel).map_err(|e| e.to_string())?;
        }
        let label = t!(self.ui_state.i18n, ids::BOARD_PANEL_UNDO).into_string();
        let step = CheckedStep {
            label: &label,
            expected_revision: engine.revision(),
        };
        let command = BoardCommand::SetBoardPanel {
            panel: panel.map(Box::new),
        };
        self.execute_board_commands(path, vec![command], Some(step))
            .map(|_| true)
            .map_err(|e| e.to_string())
    }

    /// Run `edit` on a copy of the board's panel and set it when it says it
    /// changed. Refused for a board without a panel.
    pub(crate) fn edit_board_panel(
        &mut self,
        path: &Path,
        edit: impl FnOnce(&mut BoardPanel) -> Result<bool, PanelError>,
    ) -> PanelEdit {
        let Some(mut panel) = self.board_panel_of(path)? else {
            return Err("the board has no panel: run panelize_board first".to_string());
        };
        if !edit(&mut panel).map_err(|e| e.to_string())? {
            return Ok(false);
        }
        panel.check().map_err(|e| e.to_string())?;
        self.apply_board_panel(path, Some(panel))
    }

    /// [`Self::edit_board_panel`] on the board in view; a refusal is said
    /// in Messages.
    pub(crate) fn edit_active_board_panel(
        &mut self,
        edit: impl FnOnce(&mut BoardPanel) -> Result<bool, PanelError>,
    ) {
        let Some(path) = self.active_tab_path().filter(|_| self.has_active_pcb()) else {
            return;
        };
        if let Err(why) = self.edit_board_panel(&path, edit) {
            self.say_panel_refused(why);
        }
    }

    /// Enter in a Panel setting of the Properties panel: a whole number,
    /// or a length in the panel's unit.
    pub(crate) fn submit_board_panel_setting(&mut self, field: PanelField, text: &str) {
        match field.kind() {
            PanelFieldKind::Length => {
                let unit = pcb_board_properties::shown_unit(self.ui_state.unit);
                let Some(mm) = parse_length_field_mm(text, unit) else {
                    let why = t!(self.ui_state.i18n, ids::PCB_BOARD_GRID_INVALID).into_string();
                    self.say_panel_refused(why);
                    return;
                };
                self.edit_active_board_panel(|panel| Ok(field.set_length(panel, mm)));
            }
            PanelFieldKind::Count => {
                let Ok(count) = text.trim().parse::<u32>() else {
                    let why = t!(self.ui_state.i18n, ids::BOARD_PANEL_COUNT_INVALID).into_string();
                    self.say_panel_refused(why);
                    return;
                };
                self.edit_active_board_panel(|panel| Ok(field.set_count(panel, count)));
            }
            PanelFieldKind::Separation => {
                let text = text.to_string();
                self.edit_active_board_panel(|panel| field.set_text(panel, &text));
            }
        }
    }

    fn say_panel_refused(&self, why: String) {
        let line = t!(self.ui_state.i18n, ids::BOARD_PANEL_REFUSED, "why" => why);
        crate::diagnostics::log_warning(line.into_string());
    }
}
