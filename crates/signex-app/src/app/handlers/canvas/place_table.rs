//! The Table tool (#1300): one click places a typed table, three empty
//! columns by three rows, its top-left corner at the click. The tool then
//! returns to Select, as a text note does.

use iced::Task;
use signex_types::schematic::{Point, SchDrawing, SchTable};

use super::super::super::*;

impl Signex {
    pub(super) fn place_table_click(&mut self, p: Point) {
        let table = SchTable::new_typed(uuid::Uuid::now_v7(), p);
        let placed = self.apply_engine_command(
            signex_engine::Command::PlaceSchDrawing {
                drawing: SchDrawing::Table(table),
            },
            false,
            false,
        );
        if placed {
            self.interaction_state.current_tool = Tool::Select;
            self.interaction_state.active_canvas_mut().tool_preview = None;
        }
    }

    /// A double-click on a row of a table whose rows open sheets (the
    /// contents table): open that row's sheet. `None` when the point is on
    /// no such row.
    pub(super) fn table_row_double_clicked(&mut self, at: Point) -> Option<Task<Message>> {
        let sheet = self.document_state.active_engine()?.document();
        let link = sheet.drawings.iter().rev().find_map(|d| match d {
            SchDrawing::Table(t) => t
                .cell_at(at)
                .and_then(|cell| cell.row)
                .and_then(|row| t.link(row))
                .map(str::to_string),
            _ => None,
        })?;
        let path = self.resolve_child_sheet_path(&link)?;
        if self.focus_open_sheet(&path) {
            return Some(Task::none());
        }
        Some(self.open_sheet(path, super::SheetOpen::plain()))
    }
}
