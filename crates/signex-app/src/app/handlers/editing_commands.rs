use super::super::*;

impl Signex {
    pub(crate) fn handle_selection_delete_requested(&mut self) {
        if let Some(engine) = self.document_state.engine.as_ref()
            && engine.has_selected_items(&self.interaction_state.canvas.selected)
            && self.apply_engine_command(
                signex_engine::Command::DeleteSelection {
                    items: self.interaction_state.canvas.selected.clone(),
                },
                true,
                true,
            )
        {
            self.interaction_state.canvas.selected.clear();
        }
    }

    pub(crate) fn handle_undo_requested(&mut self) {
        // Net-colour floods aren't persisted to the KiCad document so
        // they don't enter the engine's history. Check the app-level
        // net_color_undo stack first; only fall through to the engine
        // when no net-colour action is pending.
        if let Some(prev) = self.ui_state.net_color_undo.pop() {
            self.ui_state.wire_color_overrides = prev.clone();
            self.interaction_state.canvas.wire_color_overrides = prev;
            self.interaction_state.canvas.clear_content_cache();
            return;
        }
        let undone = self.apply_engine_undo(true);

        if undone {
            self.interaction_state.canvas.selected.clear();
        }
    }

    pub(crate) fn handle_redo_requested(&mut self) {
        let redone = self.apply_engine_redo(true);

        if redone {
            self.interaction_state.canvas.selected.clear();
        }
    }

    pub(crate) fn handle_selection_rotate_requested(&mut self) {
        if let Some(engine) = self.document_state.engine.as_ref()
            && engine.selection_is_single_symbol(&self.interaction_state.canvas.selected)
        {
            self.apply_engine_command(
                signex_engine::Command::RotateSelection {
                    items: self.interaction_state.canvas.selected.clone(),
                    angle_degrees: 90.0,
                },
                true,
                true,
            );
        }
    }

    pub(crate) fn handle_selection_mirror_x_requested(&mut self) {
        if let Some(engine) = self.document_state.engine.as_ref()
            && engine.selection_is_single_symbol(&self.interaction_state.canvas.selected)
        {
            self.apply_engine_command(
                signex_engine::Command::MirrorSelection {
                    items: self.interaction_state.canvas.selected.clone(),
                    axis: signex_engine::MirrorAxis::Vertical,
                },
                true,
                true,
            );
        }
    }

    pub(crate) fn handle_selection_mirror_y_requested(&mut self) {
        if let Some(engine) = self.document_state.engine.as_ref()
            && engine.selection_is_single_symbol(&self.interaction_state.canvas.selected)
        {
            self.apply_engine_command(
                signex_engine::Command::MirrorSelection {
                    items: self.interaction_state.canvas.selected.clone(),
                    axis: signex_engine::MirrorAxis::Horizontal,
                },
                true,
                true,
            );
        }
    }

    pub(crate) fn handle_update_drawing_field(
        &mut self,
        target_uuid: uuid::Uuid,
        edit: crate::app::contracts::DrawingFieldEdit,
    ) -> iced::Task<crate::app::Message> {
        use signex_types::schematic::SchDrawing;
        let Some(engine) = self.document_state.engine.as_ref() else {
            return iced::Task::none();
        };
        let doc = engine.document();
        let Some(current) = doc
            .drawings
            .iter()
            .find(|d| {
                let u = match d {
                    SchDrawing::Line { uuid, .. }
                    | SchDrawing::Rect { uuid, .. }
                    | SchDrawing::Circle { uuid, .. }
                    | SchDrawing::Arc { uuid, .. }
                    | SchDrawing::Polyline { uuid, .. } => *uuid,
                };
                u == target_uuid
            })
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
/// KiCad's stored (start, mid, end) triple.
fn apply_drawing_edit(
    current: signex_types::schematic::SchDrawing,
    edit: crate::app::contracts::DrawingFieldEdit,
) -> Option<signex_types::schematic::SchDrawing> {
    use crate::app::contracts::DrawingFieldEdit as E;
    use signex_types::schematic::{Point, SchDrawing};
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
            *start = Point::new(v, start.y.min(end.y));
            *end = Point::new(v + w_mm, start.y.max(end.y));
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
                    circumcircle_points(*start, *mid, *end).unwrap_or((start.x, start.y, 1.0));
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

fn circumcircle_points(
    a: signex_types::schematic::Point,
    b: signex_types::schematic::Point,
    c: signex_types::schematic::Point,
) -> Option<(f64, f64, f64)> {
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    if d.abs() < 1e-9 {
        return None;
    }
    let ux = ((a.x * a.x + a.y * a.y) * (b.y - c.y)
        + (b.x * b.x + b.y * b.y) * (c.y - a.y)
        + (c.x * c.x + c.y * c.y) * (a.y - b.y))
        / d;
    let uy = ((a.x * a.x + a.y * a.y) * (c.x - b.x)
        + (b.x * b.x + b.y * b.y) * (a.x - c.x)
        + (c.x * c.x + c.y * c.y) * (b.x - a.x))
        / d;
    let r = ((a.x - ux) * (a.x - ux) + (a.y - uy) * (a.y - uy)).sqrt();
    Some((ux, uy, r))
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
