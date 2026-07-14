use iced::Task;

use super::super::super::*;
use super::pre_placement_shape;

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
            && let Some(hit) =
                crate::schematic_runtime::hit_test::hit_test(snapshot, world_x, world_y)
            && hit.kind == signex_types::schematic::SelectedKind::ChildSheet
        {
            snapshot
                .child_sheets
                .iter()
                .find(|c| c.uuid == hit.uuid)
                .map(|child_sheet| child_sheet.filename.clone())
        } else {
            None
        };

        if let Some(child_filename) = child_filename_to_open {
            self.open_or_focus_child_sheet(child_filename.as_str());
            return Task::none();
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
        match self.interaction_state.current_tool {
            Tool::Line => {
                let p = signex_types::schematic::Point::new(world_x, world_y);
                if let Some(start) = self.interaction_state.shape_anchor.take() {
                    let drawing = signex_types::schematic::SchDrawing::Line {
                        uuid: uuid::Uuid::new_v4(),
                        start,
                        end: p,
                        width: pre_placement_shape(&self.document_state).0,
                        stroke_color: None,
                    };
                    self.apply_engine_command(
                        signex_engine::Command::PlaceSchDrawing { drawing },
                        false,
                        false,
                    );
                    self.interaction_state.shape_anchor = Some(p);
                    self.interaction_state.active_canvas_mut().shape_anchor =
                        Some((p, crate::canvas::ShapePreviewKind::Line));
                    self.interaction_state
                        .active_canvas_mut()
                        .clear_overlay_cache();
                }
                return Task::none();
            }
            Tool::Rectangle => {
                let (pp_w, pp_fill) = pre_placement_shape(&self.document_state);
                let p = signex_types::schematic::Point::new(world_x, world_y);
                if let Some(start) = self.interaction_state.shape_anchor.take() {
                    let drawing = signex_types::schematic::SchDrawing::Rect {
                        uuid: uuid::Uuid::new_v4(),
                        start,
                        end: p,
                        width: pp_w,
                        fill: pp_fill,
                        stroke_color: None,
                    };
                    self.apply_engine_command(
                        signex_engine::Command::PlaceSchDrawing { drawing },
                        false,
                        false,
                    );
                    self.interaction_state.active_canvas_mut().shape_anchor = None;
                    self.interaction_state
                        .active_canvas_mut()
                        .clear_overlay_cache();
                }
                return Task::none();
            }
            Tool::Circle => {
                let (pp_w, pp_fill) = pre_placement_shape(&self.document_state);
                let p = signex_types::schematic::Point::new(world_x, world_y);
                if let Some(center) = self.interaction_state.shape_anchor.take() {
                    let dx = p.x - center.x;
                    let dy = p.y - center.y;
                    let radius = (dx * dx + dy * dy).sqrt();
                    if radius > 0.01 {
                        let drawing = signex_types::schematic::SchDrawing::Circle {
                            uuid: uuid::Uuid::new_v4(),
                            center,
                            radius,
                            width: pp_w,
                            fill: pp_fill,
                            stroke_color: None,
                        };
                        self.apply_engine_command(
                            signex_engine::Command::PlaceSchDrawing { drawing },
                            false,
                            false,
                        );
                    }
                    self.interaction_state.active_canvas_mut().shape_anchor = None;
                    self.interaction_state
                        .active_canvas_mut()
                        .clear_overlay_cache();
                }
                return Task::none();
            }
            Tool::Arc => {
                let (pp_w, pp_fill) = pre_placement_shape(&self.document_state);
                let p = signex_types::schematic::Point::new(world_x, world_y);
                self.interaction_state.arc_points.push(p);
                if self.interaction_state.arc_points.len() >= 3 {
                    let pts = std::mem::take(&mut self.interaction_state.arc_points);
                    let drawing = signex_types::schematic::SchDrawing::Arc {
                        uuid: uuid::Uuid::new_v4(),
                        start: pts[0],
                        mid: pts[1],
                        end: pts[2],
                        width: pp_w,
                        fill: pp_fill,
                        stroke_color: None,
                    };
                    self.apply_engine_command(
                        signex_engine::Command::PlaceSchDrawing { drawing },
                        false,
                        false,
                    );
                    self.interaction_state
                        .active_canvas_mut()
                        .arc_points
                        .clear();
                } else {
                    self.interaction_state.active_canvas_mut().arc_points =
                        self.interaction_state.arc_points.clone();
                }
                self.interaction_state
                    .active_canvas_mut()
                    .clear_overlay_cache();
                return Task::none();
            }
            _ => {}
        }

        // Polyline closes on double-click. The canvas widget
        // intercepts the 2nd click and publishes DoubleClicked
        // INSTEAD OF Clicked, so the buffer has only one
        // vertex at this point. Append the cursor as the
        // final vertex, then commit if we now have >= 2.
        if self.interaction_state.current_tool == Tool::Polyline {
            let p = signex_types::schematic::Point::new(world_x, world_y);
            // Avoid appending a duplicate of the anchor when
            // the user double-clicks on the same spot.
            let is_dup = self
                .interaction_state
                .polyline_points
                .last()
                .map(|last| (last.x - p.x).abs() < 0.01 && (last.y - p.y).abs() < 0.01)
                .unwrap_or(false);
            if !is_dup {
                self.interaction_state.polyline_points.push(p);
            }
            if self.interaction_state.polyline_points.len() >= 2 {
                let (pp_w, pp_fill) = pre_placement_shape(&self.document_state);
                let pts = std::mem::take(&mut self.interaction_state.polyline_points);
                let drawing = signex_types::schematic::SchDrawing::Polyline {
                    uuid: uuid::Uuid::new_v4(),
                    points: pts,
                    width: pp_w,
                    fill: pp_fill,
                    stroke_color: None,
                };
                self.apply_engine_command(
                    signex_engine::Command::PlaceSchDrawing { drawing },
                    false,
                    false,
                );
            }
            self.interaction_state
                .active_canvas_mut()
                .polyline_points
                .clear();
            self.interaction_state
                .active_canvas_mut()
                .clear_overlay_cache();
            return Task::none();
        }
        if self.interaction_state.wire_drawing {
            self.interaction_state.wire_drawing = false;
            self.interaction_state.wire_points.clear();
            self.interaction_state
                .active_canvas_mut()
                .wire_preview
                .clear();
            self.interaction_state.active_canvas_mut().drawing_mode = false;
        } else if let Some(snapshot) = self.active_render_snapshot() {
            use signex_types::schematic::SelectedKind;
            if let Some(hit) =
                crate::schematic_runtime::hit_test::hit_test(snapshot, world_x, world_y)
            {
                use crate::schematic_runtime::text::expand_char_escapes;
                let edit_info =
                    match hit.kind {
                        SelectedKind::Label => snapshot
                            .labels
                            .iter()
                            .find(|l| l.uuid == hit.uuid)
                            .map(|l| {
                                (
                                    l.text.clone(),
                                    SelectedKind::Label,
                                    l.position.x,
                                    l.position.y,
                                )
                            }),
                        SelectedKind::TextNote => snapshot
                            .text_notes
                            .iter()
                            .find(|t| t.uuid == hit.uuid)
                            .map(|t| {
                                (
                                    t.text.clone(),
                                    SelectedKind::TextNote,
                                    t.position.x,
                                    t.position.y,
                                )
                            }),
                        _ => None,
                    };
                if let Some((raw_text, kind, wx, wy)) = edit_info {
                    // Show the user the visible form (e.g. "/OE"), not
                    // the Standard-escaped storage form ("{slash}OE").
                    let display_text = expand_char_escapes(&raw_text);
                    self.interaction_state.editing_text = Some(TextEditState {
                        uuid: hit.uuid,
                        kind,
                        original_text: display_text.clone(),
                        text: display_text,
                        world_x: wx,
                        world_y: wy,
                    });
                }
            }
        }
        Task::none()
    }
}
