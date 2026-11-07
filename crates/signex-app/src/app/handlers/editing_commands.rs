use super::super::*;

impl Signex {
    /// Delete the selection. `true` when something was deleted.
    pub(crate) fn handle_selection_delete_requested(&mut self) -> bool {
        if let Some(engine) = self.document_state.active_engine()
            && engine.has_selected_items(&self.interaction_state.active_canvas().selected)
            && self.apply_engine_command(
                signex_engine::Command::DeleteSelection {
                    items: self.interaction_state.active_canvas().selected.clone(),
                },
                true,
                true,
            )
        {
            self.interaction_state.active_canvas_mut().selected.clear();
            return true;
        }
        false
    }

    /// Undo in the main window: whatever its tab shows (see
    /// [`Signex::undo_target`]).
    pub(crate) fn handle_undo_requested(&mut self) {
        self.close_text_edit("the document was undone");
        let target = self.undo_target(None);
        self.step_undo_target(&target, true);
    }

    /// Redo in the main window, see [`Self::handle_undo_requested`].
    pub(crate) fn handle_redo_requested(&mut self) {
        self.close_text_edit("the document was redone");
        let target = self.undo_target(None);
        self.step_undo_target(&target, false);
    }

    /// One undo of the active sheet: its newest net-class pick or engine edit
    /// (net colours are engine edits), whichever came last. Whether a step ran.
    pub(crate) fn undo_active_sheet(&mut self) -> bool {
        if self.undo_net_class_pick() {
            return true;
        }
        let undone = self.apply_engine_undo(true);

        if undone {
            self.interaction_state.active_canvas_mut().selected.clear();
            // A multi-click buffer anchors at the end of the segment the
            // undo just removed; drawing on from it would leave an orphan
            // (#1831). The tool stays armed, the next click starts afresh.
            self.clear_drawing_buffers();
        }
        undone
    }

    /// One redo of the active sheet, see [`Self::undo_active_sheet`].
    pub(crate) fn redo_active_sheet(&mut self) -> bool {
        if self.redo_net_class_pick() {
            return true;
        }
        let redone = self.apply_engine_redo(true);

        if redone {
            self.interaction_state.active_canvas_mut().selected.clear();
        }
        redone
    }

    pub(crate) fn handle_update_drawing_field(
        &mut self,
        target_uuid: uuid::Uuid,
        edit: crate::app::contracts::DrawingFieldEdit,
    ) -> iced::Task<crate::app::Message> {
        if let crate::app::contracts::DrawingFieldEdit::Placement(placement) = edit {
            self.apply_engine_command(
                signex_engine::Command::SetDrawingPlacement {
                    ids: vec![target_uuid],
                    placement,
                },
                true,
                true,
            );
            return iced::Task::none();
        }
        let Some(engine) = self.document_state.active_engine() else {
            return iced::Task::none();
        };
        let doc = engine.document();
        let Some(current) = doc
            .drawings
            .iter()
            .find(|d| d.uuid() == target_uuid)
            .cloned()
        else {
            return iced::Task::none();
        };
        let next = apply_drawing_edit(current, edit);
        if let Some(next) = next {
            self.apply_engine_command(
                signex_engine::Command::UpdateSchDrawing { drawing: next },
                true,
                true,
            );
        }
        iced::Task::none()
    }
}

/// Patch a `SchDrawing` with a single field edit. Mutates a cloned
/// copy in place to avoid rebuilding every SchDrawing variant — and
/// to preserve every future field (stroke_color et al) automatically.
/// Returns `None` when the edit is incompatible with the drawing
/// variant (e.g. `ArcRadius` on a Rect). Arc edits convert the
/// Altium-style (center, radius, start/end angle) fields back to
/// Standard's stored (start, mid, end) triple.
fn apply_drawing_edit(
    current: signex_types::schematic::SchDrawing,
    edit: crate::app::contracts::DrawingFieldEdit,
) -> Option<signex_types::schematic::SchDrawing> {
    use crate::app::contracts::DrawingFieldEdit as E;
    use signex_types::schematic::{Point, SchDrawing, circumcircle};
    let mut next = current;
    match (&mut next, edit) {
        // Stroke width applies to every variant.
        (
            SchDrawing::Line { width, .. }
            | SchDrawing::Rect { width, .. }
            | SchDrawing::Circle { width, .. }
            | SchDrawing::Arc { width, .. }
            | SchDrawing::Polyline { width, .. },
            E::Width(w),
        ) => {
            *width = w.max(0.0);
        }
        // Fill applies to filled variants only.
        (
            SchDrawing::Rect { fill, .. }
            | SchDrawing::Circle { fill, .. }
            | SchDrawing::Polyline { fill, .. },
            E::Fill(f),
        ) => {
            *fill = f;
        }
        // Stroke colour: every variant carries an Option<StrokeColor>.
        (
            SchDrawing::Line { stroke_color, .. }
            | SchDrawing::Rect { stroke_color, .. }
            | SchDrawing::Circle { stroke_color, .. }
            | SchDrawing::Arc { stroke_color, .. }
            | SchDrawing::Polyline { stroke_color, .. },
            E::StrokeColor(c),
        ) => {
            *stroke_color = c;
        }
        // Line endpoints
        (SchDrawing::Line { start, .. }, E::LineStartX(v)) => start.x = v,
        (SchDrawing::Line { start, .. }, E::LineStartY(v)) => start.y = v,
        (SchDrawing::Line { end, .. }, E::LineEndX(v)) => end.x = v,
        (SchDrawing::Line { end, .. }, E::LineEndY(v)) => end.y = v,
        // Rect position / size — preserved by repositioning start/end.
        (SchDrawing::Rect { start, end, .. }, E::RectStartX(v)) => {
            let w_mm = (start.x.max(end.x)) - (start.x.min(end.x));
            let y0 = start.y.min(end.y);
            let y1 = start.y.max(end.y);
            *start = Point::new(v, y0);
            *end = Point::new(v + w_mm, y1);
        }
        (SchDrawing::Rect { start, end, .. }, E::RectStartY(v)) => {
            let h_mm = (end.y - start.y).abs();
            let x0 = start.x.min(end.x);
            let x1 = start.x.max(end.x);
            *start = Point::new(x0, v);
            *end = Point::new(x1, v + h_mm);
        }
        (SchDrawing::Rect { start, end, .. }, E::RectWidthMm(v)) => {
            let x0 = start.x.min(end.x);
            let y0 = start.y.min(end.y);
            let y1 = start.y.max(end.y);
            *start = Point::new(x0, y0);
            *end = Point::new(x0 + v.max(0.01), y1);
        }
        (SchDrawing::Rect { start, end, .. }, E::RectHeightMm(v)) => {
            let x0 = start.x.min(end.x);
            let x1 = start.x.max(end.x);
            let y0 = start.y.min(end.y);
            *start = Point::new(x0, y0);
            *end = Point::new(x1, y0 + v.max(0.01));
        }
        // Circle
        (SchDrawing::Circle { center, .. }, E::CircleCenterX(v)) => center.x = v,
        (SchDrawing::Circle { center, .. }, E::CircleCenterY(v)) => center.y = v,
        (SchDrawing::Circle { radius, .. }, E::CircleRadius(v)) => *radius = v.max(0.01),
        // Arc — needs full (start,mid,end) reconstruction so it's
        // handled separately (original values live inside `next`).
        (
            SchDrawing::Arc { .. },
            edit @ (E::ArcCenterX(_)
            | E::ArcCenterY(_)
            | E::ArcRadius(_)
            | E::ArcStartAngle(_)
            | E::ArcEndAngle(_)),
        ) => {
            if let SchDrawing::Arc {
                start, mid, end, ..
            } = &mut next
            {
                let (cx, cy, r) =
                    circumcircle(*start, *mid, *end).unwrap_or((start.x, start.y, 1.0));
                let mut ncx = cx;
                let mut ncy = cy;
                let mut nr = r;
                let mut nsa = (start.y - cy).atan2(start.x - cx);
                let mut nea = (end.y - cy).atan2(end.x - cx);
                match edit {
                    E::ArcCenterX(v) => ncx = v,
                    E::ArcCenterY(v) => ncy = v,
                    E::ArcRadius(v) => nr = v.max(0.01),
                    E::ArcStartAngle(deg) => nsa = deg.to_radians(),
                    E::ArcEndAngle(deg) => nea = deg.to_radians(),
                    _ => {}
                }
                // Preserve original sweep proportion for mid.
                let orig_sweep = {
                    let s = normalize_rad((start.y - cy).atan2(start.x - cx));
                    let m = normalize_rad((mid.y - cy).atan2(mid.x - cx));
                    let e = normalize_rad((end.y - cy).atan2(end.x - cx));
                    let s_to_m = norm_ccw(s, m);
                    let s_to_e = norm_ccw(s, e);
                    if s_to_e.abs() < 1e-9 {
                        0.5
                    } else {
                        s_to_m / s_to_e
                    }
                };
                let ccw = norm_ccw(normalize_rad(nsa), normalize_rad(nea));
                let mid_angle = nsa + ccw * orig_sweep;
                *start = Point::new(ncx + nr * nsa.cos(), ncy + nr * nsa.sin());
                *mid = Point::new(ncx + nr * mid_angle.cos(), ncy + nr * mid_angle.sin());
                *end = Point::new(ncx + nr * nea.cos(), ncy + nr * nea.sin());
            }
        }
        _ => return None,
    }
    Some(next)
}

fn normalize_rad(a: f64) -> f64 {
    use std::f64::consts::TAU;
    let mut t = a % TAU;
    if t < 0.0 {
        t += TAU;
    }
    t
}

fn norm_ccw(a: f64, b: f64) -> f64 {
    use std::f64::consts::TAU;
    let d = b - a;
    if d < 0.0 { d + TAU } else { d }
}

#[cfg(test)]
pub(crate) mod net_color_driver;
#[cfg(test)]
mod net_color_guard_tests;
#[cfg(test)]
mod net_color_persist_tests;
#[cfg(test)]
mod net_color_timeline_tests;
#[cfg(test)]
mod net_color_undo_order_tests;
mod net_colors;
#[cfg(test)]
mod rect_edit_tests;
#[cfg(test)]
mod undo_save_point_tests;
