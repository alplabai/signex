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

/// Patch a `SchDrawing` with a single field edit. Returns `None`
/// when the edit is incompatible with the drawing variant (e.g.
/// `ArcRadius` on a Rect). Arc edits convert the Altium-style
/// (center, radius, start/end angle) fields back to KiCad's stored
/// (start, mid, end) triple.
fn apply_drawing_edit(
    current: signex_types::schematic::SchDrawing,
    edit: crate::app::contracts::DrawingFieldEdit,
) -> Option<signex_types::schematic::SchDrawing> {
    use crate::app::contracts::DrawingFieldEdit as E;
    use signex_types::schematic::{Point, SchDrawing};
    match (current, edit) {
        (
            SchDrawing::Line {
                uuid,
                start,
                end,
                width: _,
            },
            E::Width(w),
        ) => Some(SchDrawing::Line {
            uuid,
            start,
            end,
            width: w.max(0.0),
        }),
        (
            SchDrawing::Line {
                uuid,
                mut start,
                end,
                width,
            },
            E::LineStartX(v),
        ) => {
            start.x = v;
            Some(SchDrawing::Line {
                uuid,
                start,
                end,
                width,
            })
        }
        (
            SchDrawing::Line {
                uuid,
                mut start,
                end,
                width,
            },
            E::LineStartY(v),
        ) => {
            start.y = v;
            Some(SchDrawing::Line {
                uuid,
                start,
                end,
                width,
            })
        }
        (
            SchDrawing::Line {
                uuid,
                start,
                mut end,
                width,
            },
            E::LineEndX(v),
        ) => {
            end.x = v;
            Some(SchDrawing::Line {
                uuid,
                start,
                end,
                width,
            })
        }
        (
            SchDrawing::Line {
                uuid,
                start,
                mut end,
                width,
            },
            E::LineEndY(v),
        ) => {
            end.y = v;
            Some(SchDrawing::Line {
                uuid,
                start,
                end,
                width,
            })
        }
        (
            SchDrawing::Rect {
                uuid,
                start,
                end,
                width: _,
                fill,
            },
            E::Width(w),
        ) => Some(SchDrawing::Rect {
            uuid,
            start,
            end,
            width: w.max(0.0),
            fill,
        }),
        (
            SchDrawing::Rect {
                uuid,
                start,
                end,
                width,
                fill: _,
            },
            E::Fill(f),
        ) => Some(SchDrawing::Rect {
            uuid,
            start,
            end,
            width,
            fill: f,
        }),
        (
            SchDrawing::Rect {
                uuid,
                start,
                end,
                width,
                fill,
            },
            E::RectStartX(v),
        ) => {
            let (x0, x1) = (start.x.min(end.x), start.x.max(end.x));
            let w_mm = x1 - x0;
            Some(SchDrawing::Rect {
                uuid,
                start: Point::new(v, start.y.min(end.y)),
                end: Point::new(v + w_mm, start.y.max(end.y)),
                width,
                fill,
            })
        }
        (
            SchDrawing::Rect {
                uuid,
                start,
                end,
                width,
                fill,
            },
            E::RectStartY(v),
        ) => {
            let h_mm = (end.y - start.y).abs();
            Some(SchDrawing::Rect {
                uuid,
                start: Point::new(start.x.min(end.x), v),
                end: Point::new(start.x.max(end.x), v + h_mm),
                width,
                fill,
            })
        }
        (
            SchDrawing::Rect {
                uuid,
                start,
                end,
                width,
                fill,
            },
            E::RectWidthMm(v),
        ) => {
            let x0 = start.x.min(end.x);
            let y0 = start.y.min(end.y);
            let y1 = start.y.max(end.y);
            Some(SchDrawing::Rect {
                uuid,
                start: Point::new(x0, y0),
                end: Point::new(x0 + v.max(0.01), y1),
                width,
                fill,
            })
        }
        (
            SchDrawing::Rect {
                uuid,
                start,
                end,
                width,
                fill,
            },
            E::RectHeightMm(v),
        ) => {
            let x0 = start.x.min(end.x);
            let x1 = start.x.max(end.x);
            let y0 = start.y.min(end.y);
            Some(SchDrawing::Rect {
                uuid,
                start: Point::new(x0, y0),
                end: Point::new(x1, y0 + v.max(0.01)),
                width,
                fill,
            })
        }
        (
            SchDrawing::Circle {
                uuid,
                center,
                radius,
                width: _,
                fill,
            },
            E::Width(w),
        ) => Some(SchDrawing::Circle {
            uuid,
            center,
            radius,
            width: w.max(0.0),
            fill,
        }),
        (
            SchDrawing::Circle {
                uuid,
                center,
                radius,
                width,
                fill: _,
            },
            E::Fill(f),
        ) => Some(SchDrawing::Circle {
            uuid,
            center,
            radius,
            width,
            fill: f,
        }),
        (
            SchDrawing::Circle {
                uuid,
                mut center,
                radius,
                width,
                fill,
            },
            E::CircleCenterX(v),
        ) => {
            center.x = v;
            Some(SchDrawing::Circle {
                uuid,
                center,
                radius,
                width,
                fill,
            })
        }
        (
            SchDrawing::Circle {
                uuid,
                mut center,
                radius,
                width,
                fill,
            },
            E::CircleCenterY(v),
        ) => {
            center.y = v;
            Some(SchDrawing::Circle {
                uuid,
                center,
                radius,
                width,
                fill,
            })
        }
        (
            SchDrawing::Circle {
                uuid,
                center,
                radius: _,
                width,
                fill,
            },
            E::CircleRadius(v),
        ) => Some(SchDrawing::Circle {
            uuid,
            center,
            radius: v.max(0.01),
            width,
            fill,
        }),
        (
            SchDrawing::Arc {
                uuid,
                start,
                mid,
                end,
                width: _,
                fill,
            },
            E::Width(w),
        ) => Some(SchDrawing::Arc {
            uuid,
            start,
            mid,
            end,
            width: w.max(0.0),
            fill,
        }),
        (
            SchDrawing::Arc {
                uuid,
                start,
                mid,
                end,
                width,
                fill,
            },
            edit @ (E::ArcCenterX(_)
            | E::ArcCenterY(_)
            | E::ArcRadius(_)
            | E::ArcStartAngle(_)
            | E::ArcEndAngle(_)),
        ) => {
            // Convert KiCad (start,mid,end) → Altium (cx,cy,r,sa,ea),
            // apply the edit, then reconstruct the three points. Arcs
            // that can't form a circle (colinear three points) get
            // their radius synthesised from the drag anchor.
            let (cx, cy, r) =
                circumcircle_points(start, mid, end).unwrap_or((start.x, start.y, 1.0));
            let mut ncx = cx;
            let mut ncy = cy;
            let mut nr = r;
            let mut nsa = (start.y - cy).atan2(start.x - cx);
            let mut nea = (end.y - cy).atan2(end.x - cx);
            let nma = (mid.y - cy).atan2(mid.x - cx);
            match edit {
                E::ArcCenterX(v) => ncx = v,
                E::ArcCenterY(v) => ncy = v,
                E::ArcRadius(v) => nr = v.max(0.01),
                E::ArcStartAngle(deg) => nsa = deg.to_radians(),
                E::ArcEndAngle(deg) => nea = deg.to_radians(),
                _ => {}
            }
            // Choose the mid point so the arc sweep direction is
            // preserved: keep mid angle proportional between new
            // start/end based on where it sat in the original sweep.
            let orig_sweep = {
                let s = normalize_rad((start.y - cy).atan2(start.x - cx));
                let m = normalize_rad(nma);
                let e = normalize_rad((end.y - cy).atan2(end.x - cx));
                let s_to_m = norm_ccw(s, m);
                let s_to_e = norm_ccw(s, e);
                if s_to_e.abs() < 1e-9 {
                    0.5
                } else {
                    s_to_m / s_to_e
                }
            };
            let (sa_unwrapped, ea_unwrapped) = (nsa, nea);
            let ccw = norm_ccw(normalize_rad(sa_unwrapped), normalize_rad(ea_unwrapped));
            let mid_angle = sa_unwrapped + ccw * orig_sweep;
            let new_start =
                Point::new(ncx + nr * sa_unwrapped.cos(), ncy + nr * sa_unwrapped.sin());
            let new_mid = Point::new(ncx + nr * mid_angle.cos(), ncy + nr * mid_angle.sin());
            let new_end = Point::new(ncx + nr * ea_unwrapped.cos(), ncy + nr * ea_unwrapped.sin());
            Some(SchDrawing::Arc {
                uuid,
                start: new_start,
                mid: new_mid,
                end: new_end,
                width,
                fill,
            })
        }
        (
            SchDrawing::Polyline {
                uuid,
                points,
                width: _,
                fill,
            },
            E::Width(w),
        ) => Some(SchDrawing::Polyline {
            uuid,
            points,
            width: w.max(0.0),
            fill,
        }),
        (
            SchDrawing::Polyline {
                uuid,
                points,
                width,
                fill: _,
            },
            E::Fill(f),
        ) => Some(SchDrawing::Polyline {
            uuid,
            points,
            width,
            fill: f,
        }),
        _ => None,
    }
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
