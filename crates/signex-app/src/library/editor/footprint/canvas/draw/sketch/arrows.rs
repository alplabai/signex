//! DOF direction-arrow overlay — for every under-constrained Point,
//! draws a short arrow along its free degree of freedom.

use iced::widget::canvas::{self, Path, Stroke};
use iced::{Color, Point};

use crate::library::editor::footprint::canvas::FootprintCanvasState;
use crate::library::editor::footprint::state::FootprintEditorState;

/// v0.22 Phase E2 — DOF direction-arrow overlay for under-constrained
/// Points. For every Point with `DofColor::Under`, draws a 10-px-long
/// 1-px-wide cyan arrow pointing in the direction of least constraint
/// sensitivity — i.e. the direction in which moving the Point
/// increases the constraint residual the least. Visually answers the
/// "if I drag this blue Point, which way will it go freely?"
/// question Fusion users expect.
///
/// Math: for a Point with Jacobian columns `c_x`, `c_y` (each column
/// is the partial derivative of every residual w.r.t. that state
/// var), the direction of greatest constraint sensitivity is the
/// eigenvector of
///   `M = [[||c_x||², c_x·c_y], [c_x·c_y, ||c_y||²]]`
/// associated with the LARGEST eigenvalue. The free-DoF direction is
/// the perpendicular (smallest-eigenvalue eigenvector).
///
/// Closed-form for a 2×2 symmetric matrix:
/// - λ_min = (a+d)/2 − √(((a-d)/2)² + b²)
/// - eigenvector for λ_min:
///     - if |b| > ε: (b, λ_min − a), normalized
///     - else (already diagonal): pick whichever column is smaller
/// - if all of a, b, d ≈ 0 (Point isn't touched by any constraint):
///   default to (1, 0) so the arrow still gives visual feedback.
///
/// Hides itself entirely when `state.last_solve` is `None` or the
/// jacobian is empty.
pub(in crate::library::editor::footprint::canvas::draw) fn draw_dof_direction_arrows(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    sketch: &signex_sketch::SketchData,
    state: &FootprintEditorState,
) {
    use signex_sketch::entity::EntityKind;
    use signex_sketch::solver::dof::DofColor;

    let solve = match state.last_solve.as_ref() {
        Some(s) => s,
        None => return,
    };
    if solve.jacobian.is_empty() {
        // No constraints yet — would draw an arrow on every Point.
        // Silent skip; the user reads "no constraints" from the DOF
        // counter in the inspector.
        return;
    }

    const ARROW_LEN_PX: f32 = 10.0;
    const HEAD_LEN_PX: f32 = 3.0;
    const HEAD_SPREAD_RAD: f64 = 0.5; // ~28°
    // v0.27 — DOF arrow shifted to a darker cyan so it reads
    // against the white sketch canvas without competing with the
    // blue under-constrained DOF dot underneath.
    let cyan = Color::from_rgba(0.05, 0.55, 0.80, 1.00);
    let stroke = Stroke::default().with_width(1.0).with_color(cyan);

    let m_rows = solve.jacobian.len();

    for entity in &sketch.entities {
        let pt_id = match entity.kind {
            EntityKind::Point { .. } => entity.id,
            _ => continue,
        };
        if !matches!(solve.colours.get(&pt_id), Some(DofColor::Under)) {
            continue;
        }
        let (xi, yi) = match solve.result.index.points.get(&pt_id) {
            Some(t) => *t,
            None => continue, // Fixed Point — has no state column.
        };
        // Compute a, d, b from columns xi, yi.
        let mut a = 0.0_f64;
        let mut d = 0.0_f64;
        let mut b = 0.0_f64;
        for r in 0..m_rows {
            let row = &solve.jacobian[r];
            if xi >= row.len() || yi >= row.len() {
                continue;
            }
            let cx = row[xi];
            let cy = row[yi];
            a += cx * cx;
            d += cy * cy;
            b += cx * cy;
        }
        let (mut dirx, mut diry) = if a.abs() < 1e-12 && d.abs() < 1e-12 && b.abs() < 1e-12 {
            (1.0, 0.0)
        } else {
            let half = (a + d) * 0.5;
            let radicand = ((a - d) * 0.5).powi(2) + b * b;
            let lam_min = half - radicand.sqrt();
            if b.abs() > 1e-12 {
                (b, lam_min - a)
            } else if a <= d {
                (1.0, 0.0)
            } else {
                (0.0, 1.0)
            }
        };
        let mag = (dirx * dirx + diry * diry).sqrt();
        if mag < 1e-12 {
            dirx = 1.0;
            diry = 0.0;
        } else {
            dirx /= mag;
            diry /= mag;
        }

        // Resolve world position via the solved state (preferring) or
        // the authored entity coords.
        let world = if let Some(p) = signex_sketch::solver::state::point_xy(
            pt_id,
            &solve.result.state,
            &solve.result.index,
            sketch,
        ) {
            p
        } else {
            match entity.kind {
                EntityKind::Point { x, y } => (x, y),
                _ => continue,
            }
        };
        let p_screen = cstate.world_to_screen(world);

        // Screen-space arrow. Y is flipped on screen so we negate
        // diry to match the world convention (positive y is up in
        // world but down in screen).
        let dx_s = dirx as f32 * ARROW_LEN_PX;
        let dy_s = -(diry as f32) * ARROW_LEN_PX;
        let tip = Point::new(p_screen.x + dx_s, p_screen.y + dy_s);
        let shaft = Path::line(p_screen, tip);
        frame.stroke(&shaft, stroke);

        // Arrow head: two short strokes at ±HEAD_SPREAD_RAD from the
        // shaft direction.
        let dir_angle = (dy_s as f64).atan2(dx_s as f64);
        for sign in [-1.0_f64, 1.0_f64] {
            let a = dir_angle + std::f64::consts::PI - sign * HEAD_SPREAD_RAD;
            let head_end = Point::new(
                tip.x + (a.cos() as f32) * HEAD_LEN_PX,
                tip.y + (a.sin() as f32) * HEAD_LEN_PX,
            );
            frame.stroke(&Path::line(tip, head_end), stroke);
        }
    }
}
