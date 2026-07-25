//! Live ghost preview for the multi-click sketch drawing tools — the
//! dashed rubber-band shape, the Fusion-style dimension pills, and the
//! modeless numeric placement-input overlay.

use iced::widget::canvas::{self, Path, Stroke};
use iced::{Color, Point};

use crate::library::editor::footprint::canvas::FootprintCanvasState;
use crate::library::editor::footprint::state::FootprintEditorState;

/// v0.14.2 — live ghost preview for the multi-click sketch drawing
/// tools. Reads `state.tool_pending` + `state.cursor_mm` and draws a
/// dashed semi-transparent overlay showing where the next click would
/// land:
///
/// - **Line tool, after click 1** → ghost line from first endpoint
///   to cursor.
/// - **Circle tool, after click 1** → ghost circle centred on click 1
///   with radius = distance(centre, cursor).
/// - **Arc tool, after click 1** → ghost line from centre to cursor
///   (cursor will become the start endpoint).
/// - **Arc tool, after click 2** → ghost arc from start through the
///   cursor angle, around the centre.
/// v0.27 — Fusion-style dimension pill chrome. Centred at
/// `centre` (screen coords), draws a soft grey rounded-look
/// rectangle behind a centred white label. Used by the live
/// dimension overlays during sketch tool placement so the user
/// sees the running length / width / height / angle as they
/// move the cursor.
fn draw_dim_pill(frame: &mut canvas::Frame, centre: Point, label: &str) {
    draw_dim_pill_styled(frame, centre, label, false);
}

/// v0.14-footprint — dimension-pill chrome with an explicit `focused`
/// state. `focused` paints the accent-bordered "active field" variant
/// used while the user Tab-cycles the Line length / angle inputs; the
/// inactive field keeps the muted grey plate every passive live
/// dimension uses.
fn draw_dim_pill_styled(frame: &mut canvas::Frame, centre: Point, label: &str, focused: bool) {
    // Label text first so we can size the background plate from
    // its rough advance width. Iosevka 11px averages ≈ 6 px per
    // character; pad ±5 px on each side and 3 px top/bottom.
    let glyph_w = 6.5_f32;
    let pad_x = 5.0_f32;
    let pad_y = 2.0_f32;
    let body_w = glyph_w * (label.chars().count() as f32) + pad_x * 2.0;
    let body_h = 14.0_f32 + pad_y * 2.0;
    let plate_origin = Point::new(centre.x - body_w / 2.0, centre.y - body_h / 2.0);
    // Focused field: bluish plate + bright accent border so the eye
    // lands on the input Tab currently targets. Inactive field: the
    // muted grey plate shared by every passive live-dimension pill.
    let (plate, border, border_w, text) = if focused {
        (
            Color::from_rgba(0.10, 0.20, 0.34, 0.95),
            Color::from_rgba(0.40, 0.70, 1.00, 0.95),
            1.3_f32,
            Color::from_rgba(1.00, 1.00, 1.00, 1.0),
        )
    } else {
        (
            Color::from_rgba(0.20, 0.22, 0.26, 0.92),
            Color::from_rgba(0.10, 0.12, 0.15, 1.0),
            0.8_f32,
            Color::from_rgba(0.95, 0.95, 0.97, 1.0),
        )
    };
    frame.fill_rectangle(plate_origin, iced::Size::new(body_w, body_h), plate);
    frame.stroke(
        &Path::rectangle(plate_origin, iced::Size::new(body_w, body_h)),
        Stroke::default().with_width(border_w).with_color(border),
    );
    frame.fill_text(canvas::Text {
        content: label.to_string(),
        position: centre,
        color: text,
        size: iced::Pixels(11.0),
        align_x: iced::alignment::Horizontal::Center.into(),
        align_y: iced::alignment::Vertical::Center,
        ..canvas::Text::default()
    });
}

/// v0.14-footprint — a placement-input field's raw buffer by kind,
/// searched across the focused slot then the parked fields. The live
/// dimension pills use it so each shows the user's typed digits
/// verbatim (never reformatted mid-type).
fn placement_field_buf(
    state: &FootprintEditorState,
    kind: crate::library::editor::footprint::state::PlacementInputKind,
) -> Option<&str> {
    std::iter::once(state.placement_input.as_ref())
        .chain(state.placement_input_others.iter().map(Some))
        .flatten()
        .find(|p| p.kind == kind)
        .map(|p| p.buffer.as_str())
}

pub(in crate::library::editor::footprint::canvas::draw) fn draw_sketch_tool_preview(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    sketch: &signex_sketch::SketchData,
    state: &FootprintEditorState,
) {
    use crate::library::editor::footprint::state::{PlacementInputKind, ToolPending};
    use signex_sketch::entity::EntityKind;
    use signex_sketch::id::SketchEntityId;

    let cursor = match state.cursor_mm {
        Some(c) => c,
        None => return,
    };

    // v0.14-footprint — TAB-pause hides the live ghost + dimension
    // overlay for sketch tools, mirroring how the Pads-mode placement
    // ghost hides on pause. Without this the dashed preview keeps
    // tracking the cursor after TAB, which reads as "still placing"
    // even though clicks are suppressed — the "shows the pause but
    // doesn't pause" feedback. Suppressing the preview makes the
    // paused state unambiguous.
    if state.placement_paused {
        return;
    }

    let resolve_point = |id: SketchEntityId| -> Option<(f64, f64)> {
        if let Some(solve) = state.last_solve.as_ref() {
            if let Some((x, y)) = signex_sketch::solver::state::point_xy(
                id,
                &solve.result.state,
                &solve.result.index,
                sketch,
            ) {
                return Some((x, y));
            }
        }
        sketch
            .entities
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| match e.kind {
                EntityKind::Point { x, y } => Some((x, y)),
                _ => None,
            })
    };

    // Ghost colour — accent at low alpha so it reads as preview, not
    // committed geometry. Dashed stroke for the same reason.
    let ghost = Color::from_rgba(0.40, 0.70, 1.00, 0.85);
    let stroke = Stroke::default().with_width(1.5).with_color(ghost);

    let dashed = |frame: &mut canvas::Frame, p0: Point, p1: Point| {
        let dx = p1.x - p0.x;
        let dy = p1.y - p0.y;
        let len = (dx * dx + dy * dy).sqrt();
        if len <= 0.5 {
            return;
        }
        let dash_len = 8.0_f32;
        let n = ((len / dash_len).ceil() as i32).max(2);
        for i in (0..n).step_by(2) {
            let t0 = i as f32 / n as f32;
            let t1 = ((i + 1) as f32 / n as f32).min(1.0);
            let q0 = Point::new(p0.x + dx * t0, p0.y + dy * t0);
            let q1 = Point::new(p0.x + dx * t1, p0.y + dy * t1);
            frame.stroke(&Path::line(q0, q1), stroke);
        }
    };

    let cursor_screen = cstate.world_to_screen(cursor);

    // v0.27 — suppress the bright cyan cursor pip; the dark-blue
    // snap reticle (drawn later in canvas/mod.rs) already marks the
    // snap target. The pip + reticle stacking read as "too bright"
    // on the white sketch canvas.

    match state.tool_pending {
        ToolPending::Idle => {}
        ToolPending::LineFirst { first } => {
            let Some(first_world) = resolve_point(first) else {
                return;
            };
            let p0 = cstate.world_to_screen(first_world);
            // v0.14-footprint — the Line tool carries TWO live
            // dimension fields (length + angle); collect each typed
            // buffer from whichever slot holds it (focused or parked)
            // so the preview reflects exactly what the second click
            // will commit.
            let len_buf = placement_field_buf(state, PlacementInputKind::LineLength);
            let ang_buf = placement_field_buf(state, PlacementInputKind::LineAngle);
            let typed_len = len_buf
                .and_then(|b| b.parse::<f64>().ok())
                .filter(|v| *v > 0.0);
            let typed_ang = ang_buf.and_then(|b| b.parse::<f64>().ok());
            // Cursor-relative azimuth + distance (world space) — the
            // fallback when a field hasn't been typed. Mirrors the
            // second-click commit arm in dispatch/library.rs so the
            // preview and the committed segment always agree.
            let dxw = cursor.0 - first_world.0;
            let dyw = cursor.1 - first_world.1;
            let cursor_len = (dxw * dxw + dyw * dyw).sqrt();
            let cursor_ang = if cursor_len > 1e-9 {
                dyw.atan2(dxw)
            } else {
                0.0
            };
            let eff_len = typed_len.unwrap_or(cursor_len);
            let eff_ang = typed_ang.map(f64::to_radians).unwrap_or(cursor_ang);
            // Effective endpoint = first + (eff_len @ eff_ang). Draw the
            // ghost to THIS point (not the raw cursor) so a typed
            // length/angle visibly snaps the rubber-band into place.
            let end_world = (
                first_world.0 + eff_len * eff_ang.cos(),
                first_world.1 + eff_len * eff_ang.sin(),
            );
            let p_end = cstate.world_to_screen(end_world);
            dashed(frame, p0, p_end);
            // Length pill beside the segment midpoint (perpendicular
            // offset); angle pill off the endpoint. Each echoes the raw
            // typed buffer verbatim while that field is being edited
            // (so keystrokes never get reformatted mid-type — see
            // reference_erasable_numeric_input) and the live computed
            // value otherwise. The Tab-focused field is highlighted.
            let mid = Point::new((p0.x + p_end.x) / 2.0, (p0.y + p_end.y) / 2.0);
            let sdx = p_end.x - p0.x;
            let sdy = p_end.y - p0.y;
            let seg = (sdx * sdx + sdy * sdy).sqrt().max(1.0);
            let nx = -sdy / seg;
            let ny = sdx / seg;
            let len_pos = Point::new(mid.x + nx * 18.0, mid.y + ny * 18.0);
            let ang_pos = Point::new(p_end.x + 22.0, p_end.y + 22.0);
            let focused = state.placement_input.as_ref().map(|p| p.kind);
            let len_text = match len_buf {
                Some(b) if !b.is_empty() => format!("{b} mm"),
                _ => format!("{eff_len:.3} mm"),
            };
            let ang_text = match ang_buf {
                Some(b) if !b.is_empty() => format!("{b} deg"),
                _ => format!("{:.1} deg", eff_ang.to_degrees()),
            };
            draw_dim_pill_styled(
                frame,
                len_pos,
                &len_text,
                focused == Some(PlacementInputKind::LineLength),
            );
            draw_dim_pill_styled(
                frame,
                ang_pos,
                &ang_text,
                focused == Some(PlacementInputKind::LineAngle),
            );
        }
        ToolPending::RectangleFirst { first } => {
            // v0.15 — preview the axis-aligned rectangle from the first
            // corner. v0.14-footprint: typed width/height (the Tab
            // fields) override the cursor on each axis so the ghost box
            // and pills match what the second click commits.
            let Some(first_world) = resolve_point(first) else {
                return;
            };
            let w_buf = placement_field_buf(state, PlacementInputKind::RectWidth);
            let h_buf = placement_field_buf(state, PlacementInputKind::RectHeight);
            let typed_w = w_buf
                .and_then(|b| b.parse::<f64>().ok())
                .filter(|v| *v > 0.0);
            let typed_h = h_buf
                .and_then(|b| b.parse::<f64>().ok())
                .filter(|v| *v > 0.0);
            let sx = if cursor.0 < first_world.0 { -1.0 } else { 1.0 };
            let sy = if cursor.1 < first_world.1 { -1.0 } else { 1.0 };
            let ex = typed_w.map(|w| first_world.0 + sx * w).unwrap_or(cursor.0);
            let ey = typed_h.map(|h| first_world.1 + sy * h).unwrap_or(cursor.1);
            let p0 = cstate.world_to_screen(first_world);
            let p2 = cstate.world_to_screen((ex, ey));
            let p1 = Point::new(p2.x, p0.y);
            let p3 = Point::new(p0.x, p2.y);
            dashed(frame, p0, p1);
            dashed(frame, p1, p2);
            dashed(frame, p2, p3);
            dashed(frame, p3, p0);
            // Width pill on the top edge, height pill on the left edge.
            // Each echoes the raw typed buffer while editing, else the
            // live size; the Tab-focused field is highlighted.
            let focused = state.placement_input.as_ref().map(|p| p.kind);
            let width_eff = (ex - first_world.0).abs();
            let height_eff = (ey - first_world.1).abs();
            let top_mid = Point::new((p0.x + p1.x) / 2.0, p0.y - 18.0);
            let left_mid = Point::new(p0.x - 18.0, (p0.y + p3.y) / 2.0);
            let w_text = match w_buf {
                Some(b) if !b.is_empty() => format!("{b} mm"),
                _ => format!("{width_eff:.3} mm"),
            };
            let h_text = match h_buf {
                Some(b) if !b.is_empty() => format!("{b} mm"),
                _ => format!("{height_eff:.3} mm"),
            };
            draw_dim_pill_styled(
                frame,
                top_mid,
                &w_text,
                focused == Some(PlacementInputKind::RectWidth),
            );
            draw_dim_pill_styled(
                frame,
                left_mid,
                &h_text,
                focused == Some(PlacementInputKind::RectHeight),
            );
        }
        ToolPending::RoundedRectangleFirst { first } => {
            // v0.16 — preview the rounded rectangle. Compute the bbox
            // from the first corner + cursor, derive a clamped
            // corner radius from the dimension input (default 0.5
            // mm), and stroke 4 dashed line segments + 4 dashed
            // 90° arcs.
            let Some(first_world) = resolve_point(first) else {
                return;
            };
            // v0.14-footprint — typed width/height override the cursor
            // on each axis; the box grows into the cursor's quadrant.
            let w_buf = placement_field_buf(state, PlacementInputKind::RectWidth);
            let h_buf = placement_field_buf(state, PlacementInputKind::RectHeight);
            let rr_buf = placement_field_buf(state, PlacementInputKind::RRectRadius);
            let typed_w = w_buf
                .and_then(|b| b.parse::<f64>().ok())
                .filter(|v| *v > 0.0);
            let typed_h = h_buf
                .and_then(|b| b.parse::<f64>().ok())
                .filter(|v| *v > 0.0);
            let sgnx = if cursor.0 < first_world.0 { -1.0 } else { 1.0 };
            let sgny = if cursor.1 < first_world.1 { -1.0 } else { 1.0 };
            let cx_corner = typed_w
                .map(|w| first_world.0 + sgnx * w)
                .unwrap_or(cursor.0);
            let cy_corner = typed_h
                .map(|h| first_world.1 + sgny * h)
                .unwrap_or(cursor.1);
            let x0 = first_world.0.min(cx_corner);
            let y0 = first_world.1.min(cy_corner);
            let x1 = first_world.0.max(cx_corner);
            let y1 = first_world.1.max(cy_corner);
            let half_w = (x1 - x0) / 2.0;
            let half_h = (y1 - y0) / 2.0;
            // Corner radius: typed RRectRadius wins, then the legacy
            // dimension_input text, else 0.5 mm (matches the commit).
            let r_input = rr_buf
                .and_then(|b| b.parse::<f64>().ok())
                .or_else(|| state.dimension_input.trim().parse::<f64>().ok())
                .unwrap_or(0.5);
            let r_max = half_w.min(half_h).max(0.05);
            let r = r_input.clamp(0.05, r_max);
            // Line endpoints in world coords.
            let tl_right = (x0 + r, y0);
            let tr_left = (x1 - r, y0);
            let tr_top = (x1, y0 + r);
            let br_top = (x1, y1 - r);
            let br_right = (x1 - r, y1);
            let bl_left = (x0 + r, y1);
            let bl_bot = (x0, y1 - r);
            let tl_bot = (x0, y0 + r);
            // Lines.
            for (a, b) in [
                (tl_right, tr_left),
                (tr_top, br_top),
                (br_right, bl_left),
                (bl_bot, tl_bot),
            ] {
                dashed(frame, cstate.world_to_screen(a), cstate.world_to_screen(b));
            }
            // Arc centres.
            let centres = [
                ((x1 - r, y0 + r), tr_left, tr_top),
                ((x1 - r, y1 - r), br_top, br_right),
                ((x0 + r, y1 - r), bl_left, bl_bot),
                ((x0 + r, y0 + r), tl_bot, tl_right),
            ];
            for (c_world, s_world, e_world) in centres {
                let a0 = (s_world.1 - c_world.1).atan2(s_world.0 - c_world.0);
                let a1 = (e_world.1 - c_world.1).atan2(e_world.0 - c_world.0);
                let mut delta = a1 - a0;
                while delta < 0.0 {
                    delta += std::f64::consts::TAU;
                }
                let segs = 12;
                let mut prev = cstate.world_to_screen(s_world);
                for i in 1..=segs {
                    if i % 2 == 0 {
                        let t = (i as f64) / (segs as f64);
                        let a = a0 + delta * t;
                        let p = (c_world.0 + r * a.cos(), c_world.1 + r * a.sin());
                        let q = cstate.world_to_screen(p);
                        frame.stroke(&Path::line(prev, q), stroke);
                        prev = q;
                    } else {
                        let t = (i as f64) / (segs as f64);
                        let a = a0 + delta * t;
                        let p = (c_world.0 + r * a.cos(), c_world.1 + r * a.sin());
                        prev = cstate.world_to_screen(p);
                    }
                }
            }
            // v0.14-footprint — width / height / radius pills echo the
            // raw typed buffer while editing, else the live value; the
            // Tab-focused field is highlighted.
            let focused = state.placement_input.as_ref().map(|p| p.kind);
            let p0_screen = cstate.world_to_screen(first_world);
            let corner_screen = cstate.world_to_screen((cx_corner, cy_corner));
            let width_eff = (cx_corner - first_world.0).abs();
            let height_eff = (cy_corner - first_world.1).abs();
            let top_mid = Point::new((p0_screen.x + corner_screen.x) / 2.0, p0_screen.y - 18.0);
            let left_mid = Point::new(p0_screen.x - 18.0, (p0_screen.y + corner_screen.y) / 2.0);
            let w_text = match w_buf {
                Some(b) if !b.is_empty() => format!("{b} mm"),
                _ => format!("{width_eff:.3} mm"),
            };
            let h_text = match h_buf {
                Some(b) if !b.is_empty() => format!("{b} mm"),
                _ => format!("{height_eff:.3} mm"),
            };
            let r_text = match rr_buf {
                Some(b) if !b.is_empty() => format!("r {b} mm"),
                _ => format!("r {r:.3} mm"),
            };
            draw_dim_pill_styled(
                frame,
                top_mid,
                &w_text,
                focused == Some(PlacementInputKind::RectWidth),
            );
            draw_dim_pill_styled(
                frame,
                left_mid,
                &h_text,
                focused == Some(PlacementInputKind::RectHeight),
            );
            let r_label = Point::new(corner_screen.x + 24.0, corner_screen.y + 24.0);
            draw_dim_pill_styled(
                frame,
                r_label,
                &r_text,
                focused == Some(PlacementInputKind::RRectRadius),
            );
        }
        ToolPending::CircleCenter { center } => {
            let Some(c_world) = resolve_point(center) else {
                return;
            };
            let c_screen = cstate.world_to_screen(c_world);
            let r_world = ((cursor.0 - c_world.0).powi(2) + (cursor.1 - c_world.1).powi(2)).sqrt();
            let r_screen = (r_world as f32) * cstate.scale;
            // Approximate dashed circle with 32-segment polyline.
            let segments = 32;
            for i in (0..segments).step_by(2) {
                let t0 = i as f32 / segments as f32;
                let t1 = (i + 1) as f32 / segments as f32;
                let a0 = t0 * std::f32::consts::TAU;
                let a1 = t1 * std::f32::consts::TAU;
                let q0 = Point::new(
                    c_screen.x + r_screen * a0.cos(),
                    c_screen.y + r_screen * a0.sin(),
                );
                let q1 = Point::new(
                    c_screen.x + r_screen * a1.cos(),
                    c_screen.y + r_screen * a1.sin(),
                );
                frame.stroke(&Path::line(q0, q1), stroke);
            }
            // Radial guide from centre to cursor.
            dashed(frame, c_screen, cursor_screen);
            // v0.27 — radius pill near the cursor.
            let mid = Point::new(
                (c_screen.x + cursor_screen.x) / 2.0,
                (c_screen.y + cursor_screen.y) / 2.0,
            );
            // Perpendicular offset so the pill sits beside the
            // radial guide, not on top of it.
            let dx_s = cursor_screen.x - c_screen.x;
            let dy_s = cursor_screen.y - c_screen.y;
            let len_s = (dx_s * dx_s + dy_s * dy_s).sqrt().max(1.0);
            let nx = -dy_s / len_s;
            let ny = dx_s / len_s;
            let label = Point::new(mid.x + nx * 18.0, mid.y + ny * 18.0);
            draw_dim_pill(frame, label, &format!("r {:.3} mm", r_world));
        }
        ToolPending::ArcCenter { center } => {
            // Centre placed; cursor will become the start point. Show
            // a dashed radial line from centre to cursor.
            let Some(c_world) = resolve_point(center) else {
                return;
            };
            let c_screen = cstate.world_to_screen(c_world);
            dashed(frame, c_screen, cursor_screen);
            // v0.27 — radius pill on the radial midpoint.
            let r_world = ((cursor.0 - c_world.0).powi(2) + (cursor.1 - c_world.1).powi(2)).sqrt();
            let mid = Point::new(
                (c_screen.x + cursor_screen.x) / 2.0,
                (c_screen.y + cursor_screen.y) / 2.0,
            );
            let dx_s = cursor_screen.x - c_screen.x;
            let dy_s = cursor_screen.y - c_screen.y;
            let len_s = (dx_s * dx_s + dy_s * dy_s).sqrt().max(1.0);
            let nx = -dy_s / len_s;
            let ny = dx_s / len_s;
            let label = Point::new(mid.x + nx * 18.0, mid.y + ny * 18.0);
            draw_dim_pill(frame, label, &format!("r {:.3} mm", r_world));
        }
        ToolPending::ArcStart { center, start } => {
            // Centre + start placed; cursor will become the end. Draw
            // a dashed CCW arc from start to cursor angle.
            let Some(c_world) = resolve_point(center) else {
                return;
            };
            let Some(s_world) = resolve_point(start) else {
                return;
            };
            let c_screen = cstate.world_to_screen(c_world);
            let r_world =
                ((s_world.0 - c_world.0).powi(2) + (s_world.1 - c_world.1).powi(2)).sqrt();
            let r_screen = (r_world as f32) * cstate.scale;
            let start_angle = (s_world.1 - c_world.1).atan2(s_world.0 - c_world.0) as f32;
            let end_angle = (cursor.1 - c_world.1).atan2(cursor.0 - c_world.0) as f32;
            // CCW sweep — wrap end above start by 2π if needed.
            let mut delta = end_angle - start_angle;
            while delta < 0.0 {
                delta += std::f32::consts::TAU;
            }
            let segments = 32;
            for i in (0..segments).step_by(2) {
                let t0 = i as f32 / segments as f32;
                let t1 = (i + 1) as f32 / segments as f32;
                let a0 = start_angle + delta * t0;
                let a1 = start_angle + delta * t1;
                let q0 = Point::new(
                    c_screen.x + r_screen * a0.cos(),
                    c_screen.y + r_screen * a0.sin(),
                );
                let q1 = Point::new(
                    c_screen.x + r_screen * a1.cos(),
                    c_screen.y + r_screen * a1.sin(),
                );
                frame.stroke(&Path::line(q0, q1), stroke);
            }
            // Radial guides for both endpoints + cursor.
            let s_screen = cstate.world_to_screen(s_world);
            dashed(frame, c_screen, s_screen);
            dashed(frame, c_screen, cursor_screen);
            // v0.27 — sweep angle pill (deg). Radius is fixed by
            // the start endpoint so we surface the sweep, which is
            // what changes as the cursor moves.
            let sweep_deg = (delta.to_degrees().rem_euclid(360.0)) as f64;
            let label = Point::new(cursor_screen.x + 24.0, cursor_screen.y + 24.0);
            draw_dim_pill(frame, label, &format!("{:.1} deg", sweep_deg));
        }
        // #467 — Edge Arc, start placed. Cursor previews where `end`
        // will land; a plain dashed segment, since there's no
        // radius/sweep to show until the third "point on arc" click
        // supplies it.
        ToolPending::EdgeArcStart { start } => {
            let Some(s_world) = resolve_point(start) else {
                return;
            };
            let s_screen = cstate.world_to_screen(s_world);
            dashed(frame, s_screen, cursor_screen);
        }
        // #467 — Edge Arc, start + end placed. Cursor previews the
        // third "point on arc" pick — mirror the dispatcher's
        // circumcircle geometry (#461/#483) so the ghost arc matches
        // exactly what a click would commit. Falls back to the plain
        // start-end chord while the cursor sits on the collinear line
        // (no solvable circle yet).
        ToolPending::EdgeArcEnd { start, end } => {
            let (Some(s_world), Some(e_world)) = (resolve_point(start), resolve_point(end)) else {
                return;
            };
            use signex_types::schematic::{Point as SchPoint, circumcircle};
            match circumcircle(
                SchPoint::new(s_world.0, s_world.1),
                SchPoint::new(cursor.0, cursor.1),
                SchPoint::new(e_world.0, e_world.1),
            ) {
                Some((cx, cy, r)) => {
                    use signex_sketch::geom::{Sign, orient2d};
                    let sweep_ccw = match orient2d(s_world.into(), cursor.into(), e_world.into()) {
                        Sign::Negative => false,
                        Sign::Positive | Sign::Zero => true,
                    };
                    let c_screen = cstate.world_to_screen((cx, cy));
                    let r_screen = (r as f32) * cstate.scale;
                    let start_angle = (s_world.1 - cy).atan2(s_world.0 - cx) as f32;
                    let end_angle = (e_world.1 - cy).atan2(e_world.0 - cx) as f32;
                    let mut delta = end_angle - start_angle;
                    if sweep_ccw {
                        while delta < 0.0 {
                            delta += std::f32::consts::TAU;
                        }
                    } else {
                        while delta > 0.0 {
                            delta -= std::f32::consts::TAU;
                        }
                    }
                    let segments = 32;
                    for i in (0..segments).step_by(2) {
                        let t0 = i as f32 / segments as f32;
                        let t1 = (i + 1) as f32 / segments as f32;
                        let a0 = start_angle + delta * t0;
                        let a1 = start_angle + delta * t1;
                        let q0 = Point::new(
                            c_screen.x + r_screen * a0.cos(),
                            c_screen.y + r_screen * a0.sin(),
                        );
                        let q1 = Point::new(
                            c_screen.x + r_screen * a1.cos(),
                            c_screen.y + r_screen * a1.sin(),
                        );
                        frame.stroke(&Path::line(q0, q1), stroke);
                    }
                    let label = Point::new(cursor_screen.x + 24.0, cursor_screen.y + 24.0);
                    draw_dim_pill(frame, label, &format!("r {:.3} mm", r));
                }
                None => {
                    let s_screen = cstate.world_to_screen(s_world);
                    let e_screen = cstate.world_to_screen(e_world);
                    dashed(frame, s_screen, e_screen);
                }
            }
        }
        // v0.23 — Polar centre re-pick has no preview shape; the
        // cursor PIP at the top of this match is the only visual cue.
        ToolPending::RepickPolarCenter { .. } => {}
        // v0.24 Track C — Tangent Arc, first endpoint placed.
        // Mirror the dispatcher's geometry: locate a Line ending at
        // `first`, compute the tangent-circle centre on the line's
        // perpendicular through `first` that passes through the
        // cursor, and stroke a dashed ghost arc from `first` to the
        // cursor along that circle.
        //
        // Without an incident line, fall back to a dashed straight
        // segment (matches the LineFirst preview) so the user still
        // gets visual feedback while the dispatcher will publish a
        // placeholder warning on commit.
        ToolPending::TangentArcFirst { first } => {
            let Some(first_world) = resolve_point(first) else {
                return;
            };
            // Find a Line ending at `first` (most recent first, same
            // priority the dispatcher uses).
            let incident_line: Option<(f64, f64)> =
                sketch.entities.iter().rev().find_map(|e| match e.kind {
                    EntityKind::Line { start, end } if end == first => resolve_point(start),
                    EntityKind::Line { start, end } if start == first => resolve_point(end),
                    _ => None,
                });
            let p0 = cstate.world_to_screen(first_world);
            match incident_line {
                Some(line_other) => {
                    // Line direction (line_other -> first).
                    let lx = first_world.0 - line_other.0;
                    let ly = first_world.1 - line_other.1;
                    let llen_sq = lx * lx + ly * ly;
                    if llen_sq <= 1e-12 {
                        dashed(frame, p0, cursor_screen);
                        return;
                    }
                    let llen = llen_sq.sqrt();
                    // Perpendicular to the line at `first`.
                    let nx = -ly / llen;
                    let ny = lx / llen;
                    // Solve for the centre (see dispatcher comment).
                    let dx = first_world.0 - cursor.0;
                    let dy = first_world.1 - cursor.1;
                    let denom = 2.0 * (dx * nx + dy * ny);
                    let chord_sq = dx * dx + dy * dy;
                    if denom.abs() <= 1e-9 || chord_sq <= 1e-9 {
                        // Cursor on the tangent line — preview the
                        // straight segment until the cursor pulls off
                        // axis.
                        dashed(frame, p0, cursor_screen);
                        return;
                    }
                    let t = -chord_sq / denom;
                    let cx = first_world.0 + t * nx;
                    let cy = first_world.1 + t * ny;
                    // Radius (use the start-side distance — both
                    // sides should match within solver tolerance).
                    let rx = first_world.0 - cx;
                    let ry = first_world.1 - cy;
                    let r_world = (rx * rx + ry * ry).sqrt();
                    if r_world <= 1e-9 {
                        dashed(frame, p0, cursor_screen);
                        return;
                    }
                    // Sweep direction — match the dispatcher's logic.
                    let ex = cursor.0 - first_world.0;
                    let ey = cursor.1 - first_world.1;
                    let sweep_ccw = lx * ey - ly * ex >= 0.0;
                    // Stroke the dashed arc.
                    let c_screen = cstate.world_to_screen((cx, cy));
                    let r_screen = (r_world as f32) * cstate.scale;
                    let start_angle = (first_world.1 - cy).atan2(first_world.0 - cx) as f32;
                    let end_angle = (cursor.1 - cy).atan2(cursor.0 - cx) as f32;
                    let mut delta = end_angle - start_angle;
                    if sweep_ccw {
                        while delta < 0.0 {
                            delta += std::f32::consts::TAU;
                        }
                    } else {
                        while delta > 0.0 {
                            delta -= std::f32::consts::TAU;
                        }
                    }
                    let segments = 32;
                    for i in (0..segments).step_by(2) {
                        let t0 = i as f32 / segments as f32;
                        let t1 = (i + 1) as f32 / segments as f32;
                        let a0 = start_angle + delta * t0;
                        let a1 = start_angle + delta * t1;
                        let q0 = Point::new(
                            c_screen.x + r_screen * a0.cos(),
                            c_screen.y + r_screen * a0.sin(),
                        );
                        let q1 = Point::new(
                            c_screen.x + r_screen * a1.cos(),
                            c_screen.y + r_screen * a1.sin(),
                        );
                        frame.stroke(&Path::line(q0, q1), stroke);
                    }
                }
                None => {
                    // No incident line — show a dashed chord so the
                    // user gets a visual cue. Dispatcher publishes
                    // the "no incident line" warning on commit.
                    dashed(frame, p0, cursor_screen);
                }
            }
        }
        // v0.27 — Fillet, first Line picked. The tool is waiting for
        // the user's second-line click; no shape preview to draw,
        // the cursor reticle is the sole visual cue.
        ToolPending::FilletFirst { .. } => {}
    }

    // v0.24 Track D — modeless live numeric placement-input overlay.
    // Renders the user-typed buffer at the cursor whenever
    // `placement_input` is `Some`, regardless of which `ToolPending`
    // is current (the kind picker decided which tool's gesture mints
    // it; the dispatcher tolerates unrelated tool changes by clearing
    // on commit + on Esc). Position: 4 px right and 8 px below the
    // cursor, with a translucent rounded background so the buffer
    // reads against any canvas content.
    //
    // v0.14-footprint — multi-field tools (Line len/angle, Rectangle
    // w/h, Rounded-Rect w/h/radius) render their typed values via the
    // highlighted dimension pills above, so skip the generic overlay
    // for them to avoid a double readout. Single-field tools (radius /
    // sweep / offset / fillet) still use this overlay as their sole
    // typed-value display.
    if let Some(input) = state
        .placement_input
        .as_ref()
        .filter(|p| !p.kind.is_tab_switchable())
    {
        let label = input.kind.label();
        let body = if input.buffer.is_empty() {
            format!("{label}: _")
        } else {
            format!("{label}: {}", input.buffer)
        };
        let origin = Point::new(cursor_screen.x + 4.0, cursor_screen.y + 8.0);
        // Approximate a one-line text bbox so the background plate
        // sits behind the glyphs. Iosevka 11px averages ~6 px per
        // character at the canvas's default rendering; a 4 px pad
        // around the label keeps the chrome readable.
        let glyph_w = 6.5_f32;
        let pad_x = 5.0_f32;
        let pad_y = 3.0_f32;
        let body_w = glyph_w * (body.chars().count() as f32) + pad_x * 2.0;
        let body_h = 16.0_f32 + pad_y * 2.0;
        let plate_origin = Point::new(origin.x - pad_x, origin.y - pad_y);
        // Background plate.
        frame.fill_rectangle(
            plate_origin,
            iced::Size::new(body_w, body_h),
            Color::from_rgba(0.05, 0.07, 0.10, 0.85),
        );
        // Accent border.
        frame.stroke(
            &Path::rectangle(plate_origin, iced::Size::new(body_w, body_h)),
            Stroke::default()
                .with_width(1.0)
                .with_color(Color::from_rgba(0.40, 0.70, 1.00, 0.95)),
        );
        // Buffer text.
        frame.fill_text(canvas::Text {
            content: body,
            position: origin,
            color: Color::from_rgba(0.95, 0.95, 0.97, 1.00),
            size: iced::Pixels(13.0),
            ..canvas::Text::default()
        });
    }
}
