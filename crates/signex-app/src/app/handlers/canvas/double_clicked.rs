use iced::Task;

use super::super::super::*;
use super::shape_tool::ClickKind;

impl Signex {
    pub(super) fn handle_canvas_double_clicked(
        &mut self,
        world_x: f64,
        world_y: f64,
    ) -> Task<Message> {
        // In Select mode, double-clicking a child-sheet symbol
        // navigates into the referenced schematic.
        let child_filename_to_open = if self.interaction_state.current_tool == Tool::Select
            && let Some(snapshot) = self.active_render_snapshot()
            && let Some(hit) = crate::schematic_runtime::hit_test::hit_test_styled(
                snapshot,
                &self.interaction_state.active_canvas().designators,
                world_x,
                world_y,
                self.active_wire_widths(),
            )
            && hit.kind == signex_types::schematic::SelectedKind::ChildSheet
        {
            snapshot
                .child_sheets
                .iter()
                .find(|c| c.uuid == hit.uuid)
                .map(|child_sheet| (child_sheet.uuid, child_sheet.filename.clone()))
        } else {
            None
        };

        if let Some((sheet_symbol, child_filename)) = child_filename_to_open {
            return self.descend_through_sheet_symbol(sheet_symbol, child_filename.as_str());
        }

        // Lasso already commits on the second single-click
        // (see CanvasEvent::Clicked above), so by the time
        // a DoubleClicked fires the polygon is already
        // consumed. Fall through to the wire-drawing /
        // inline-edit branches below.
        //
        // The canvas detects double-clicks (300ms / 3mm) and
        // publishes DoubleClicked INSTEAD of Clicked for the
        // second press. For multi-click shape tools that
        // means the 2nd click would otherwise be eaten. Route
        // DoubleClicked into the same commit path as Clicked.
        // Shape commits snap to the grid exactly like the single-click
        // path (`handle_canvas_clicked`); hit-testing keeps the raw cursor.
        let (snapped_x, snapped_y) = self.snap_to_grid(world_x, world_y);
        match self.interaction_state.current_tool {
            Tool::Sheet => {
                return self.place_sheet_click(world_x, world_y);
            }
            tool => {
                let p = signex_types::schematic::Point::new(snapped_x, snapped_y);
                if self.shape_tool_point(tool, p, ClickKind::DoubleClick) {
                    return Task::none();
                }
            }
        }

        if self.interaction_state.wire_drawing {
            self.interaction_state.wire_drawing = false;
            self.interaction_state.wire_points.clear();
            self.interaction_state
                .active_canvas_mut()
                .wire_preview
                .clear();
            self.interaction_state.active_canvas_mut().drawing_mode = false;
        } else if let Some(snapshot) = self.active_render_snapshot()
            && let Some(hit) = crate::schematic_runtime::hit_test::hit_test_styled(
                snapshot,
                &self.interaction_state.active_canvas().designators,
                world_x,
                world_y,
                self.active_wire_widths(),
            )
            && let Some(focus) = self.open_text_edit_on_double_click(hit)
        {
            return focus;
        }
        Task::none()
    }
}
