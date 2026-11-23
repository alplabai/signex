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
        let path = self.table_link_path(&link)?;
        if self.focus_open_sheet(&path) {
            return Some(Task::none());
        }
        Some(self.open_sheet(path, super::SheetOpen::plain()))
    }

    /// The sheet a contents row's link names, from the folder of the sheet
    /// holding the table. A link may climb out of that folder (`../power/x`)
    /// only to a file inside the project or one its last compile read.
    pub(super) fn table_link_path(&self, link: &str) -> Option<std::path::PathBuf> {
        use crate::app::project_sheets::{
            lexically_normalize, resolve_child_reference, resolve_child_reference_quiet,
        };
        let Some(carrier) = self.active_tab_path() else {
            return self.resolve_child_sheet_path(link);
        };
        let Some(dir) = carrier.parent() else {
            return self.resolve_child_sheet_path(link);
        };
        if let Ok(path) = resolve_child_reference_quiet(dir, link) {
            return Some(path);
        }
        let trimmed = link.trim();
        if trimmed.is_empty() {
            return None;
        }
        let target = lexically_normalize(&dir.join(trimmed));
        let docs = &self.document_state;
        let known = docs.document_project(&carrier).is_some_and(|project| {
            let fold = crate::app::state::project_case::fold_of(Some(project));
            super::relative_within(project.dir(), &target, fold).is_some()
                || docs
                    .compiled_design
                    .results
                    .get(&project.id)
                    .is_some_and(|result| result.covers(&target))
        });
        if known {
            Some(target)
        } else {
            resolve_child_reference(dir, link)
        }
    }
}
