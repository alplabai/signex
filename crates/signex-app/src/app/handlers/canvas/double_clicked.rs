use iced::Task;

use super::super::super::*;
use super::shape_tool::ClickKind;

impl Signex {
    pub(super) fn handle_canvas_double_clicked(
        &mut self,
        world_x: f64,
        world_y: f64,
    ) -> Task<Message> {
        if self.is_annotation_pause_of_another(None) {
            return Task::none();
        }
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
            return self.open_sheet_through_symbol(sheet_symbol, child_filename.as_str());
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
        // A contents table's row opens its sheet (#1300).
        if self.interaction_state.current_tool == Tool::Select
            && let Some(open) =
                self.table_row_double_clicked(signex_types::schematic::Point::new(world_x, world_y))
        {
            return open;
        }
        match self.interaction_state.current_tool {
            Tool::Sheet => {
                return self.place_sheet_click(world_x, world_y);
            }
            // #1280: a double-click stands for the connector's second click.
            Tool::HarnessConnector => {
                if self.interaction_state.shape_anchor.is_some() {
                    let p = signex_types::schematic::Point::new(snapped_x, snapped_y);
                    self.harness_connector_click(p);
                }
                return Task::none();
            }
            tool @ (Tool::Blanket | Tool::CompileMask) => {
                let p = signex_types::schematic::Point::new(snapped_x, snapped_y);
                self.directive_region_point(tool, p, ClickKind::DoubleClick);
                return Task::none();
            }
            // A quick next click of the Bezier, curve or dimension tool (#1267).
            tool @ (Tool::Bezier
            | Tool::Ellipse
            | Tool::EllipticalArc
            | Tool::RoundRectangle
            | Tool::LinearDimension
            | Tool::AlignedDimension) => {
                // The single-click path has this gate too: a pause takes no point.
                if self.interaction_state.active_canvas().placement_paused {
                    return Task::none();
                }
                let p = signex_types::schematic::Point::new(snapped_x, snapped_y);
                self.annotation_tool_point(tool, p);
                return Task::none();
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
