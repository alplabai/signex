//! Pad rendering + Pads-mode multi-click gesture preview.

use iced::widget::canvas::{self, Path, Stroke};
use iced::{Color, Point};

use super::super::state::{EditorPad, FootprintEditorState, PadsTool, PlaceArcPending};
use super::FootprintCanvasState;

/// Render a single pad — copper outline, drilled hole, and pad number
/// (when zoomed in enough to read).
pub(super) fn draw_pad(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    pad: &EditorPad,
    is_selected: bool,
) {
    use signex_library::PadShape as PS;
    let layer = pad.primary_layer();
    let color = layer.color();
    let (x0, y0, x1, y1) = pad.bbox_mm();
    let p0 = cstate.world_to_screen((x0, y0));
    let p1 = cstate.world_to_screen((x1, y1));
    let size = iced::Size::new(p1.x - p0.x, p1.y - p0.y);
    let centre = cstate.world_to_screen(pad.position_mm);
    let half_w = size.width / 2.0;
    let half_h = size.height / 2.0;

    // v0.20 — branch on pad.shape to render the actual copper
    // outline. Round/Oval use a stretched circle; RoundRect uses
    // rect with arc corners; Chamfered uses a 6/8-vertex polygon;
    // Custom falls back to its provided polygon points.
    let shape_path = match &pad.shape {
        PS::Round | PS::Oval => Path::new(|b| {
            let segments = 36;
            for i in 0..=segments {
                let t = i as f32 / segments as f32 * std::f32::consts::TAU;
                let x = centre.x + half_w * t.cos();
                let y = centre.y + half_h * t.sin();
                if i == 0 {
                    b.move_to(Point::new(x, y));
                } else {
                    b.line_to(Point::new(x, y));
                }
            }
            b.close();
        }),
        PS::RoundRect { radius_ratio } => {
            let r = (half_w.min(half_h) * (*radius_ratio as f32 * 2.0)).max(0.5);
            Path::new(|b| {
                b.move_to(Point::new(centre.x - half_w + r, centre.y - half_h));
                b.line_to(Point::new(centre.x + half_w - r, centre.y - half_h));
                b.arc_to(
                    Point::new(centre.x + half_w, centre.y - half_h),
                    Point::new(centre.x + half_w, centre.y - half_h + r),
                    r,
                );
                b.line_to(Point::new(centre.x + half_w, centre.y + half_h - r));
                b.arc_to(
                    Point::new(centre.x + half_w, centre.y + half_h),
                    Point::new(centre.x + half_w - r, centre.y + half_h),
                    r,
                );
                b.line_to(Point::new(centre.x - half_w + r, centre.y + half_h));
                b.arc_to(
                    Point::new(centre.x - half_w, centre.y + half_h),
                    Point::new(centre.x - half_w, centre.y + half_h - r),
                    r,
                );
                b.line_to(Point::new(centre.x - half_w, centre.y - half_h + r));
                b.arc_to(
                    Point::new(centre.x - half_w, centre.y - half_h),
                    Point::new(centre.x - half_w + r, centre.y - half_h),
                    r,
                );
                b.close();
            })
        }
        PS::Chamfered { chamfer_ratio, corners } => {
            let c = (half_w.min(half_h) * (*chamfer_ratio as f32 * 2.0)).max(0.5);
            Path::new(|b| {
                let tl = Point::new(centre.x - half_w, centre.y - half_h);
                let tr = Point::new(centre.x + half_w, centre.y - half_h);
                let br = Point::new(centre.x + half_w, centre.y + half_h);
                let bl = Point::new(centre.x - half_w, centre.y + half_h);

                b.move_to(Point::new(tl.x + c, tl.y));
                if corners.top_right {
                    b.line_to(Point::new(tr.x - c, tr.y));
                    b.line_to(Point::new(tr.x, tr.y + c));
                } else {
                    b.line_to(tr);
                }
                if corners.bottom_right {
                    b.line_to(Point::new(br.x, br.y - c));
                    b.line_to(Point::new(br.x - c, br.y));
                } else {
                    b.line_to(br);
                }
                if corners.bottom_left {
                    b.line_to(Point::new(bl.x + c, bl.y));
                    b.line_to(Point::new(bl.x, bl.y - c));
                } else {
                    b.line_to(bl);
                }
                if corners.top_left {
                    b.line_to(Point::new(tl.x, tl.y + c));
                    b.line_to(Point::new(tl.x + c, tl.y));
                } else {
                    b.line_to(tl);
                    b.line_to(Point::new(tl.x + c, tl.y));
                }
                b.close();
            })
        }
        PS::Custom(poly) => Path::new(|b| {
            if let Some((first, rest)) = poly.points.split_first() {
                let p0 = cstate.world_to_screen((
                    pad.position_mm.0 + first[0],
                    pad.position_mm.1 + first[1],
                ));
                b.move_to(p0);
                for pt in rest {
                    let p = cstate.world_to_screen((
                        pad.position_mm.0 + pt[0],
                        pad.position_mm.1 + pt[1],
                    ));
                    b.line_to(p);
                }
                b.close();
            }
        }),
        _ => Path::rectangle(p0, size),
    };

    frame.fill(&shape_path, Color { a: 0.85, ..color });
    let outline_color = if is_selected {
        Color::from_rgb(1.0, 1.0, 1.0)
    } else {
        Color { a: 1.0, ..color }
    };
    frame.stroke(
        &shape_path,
        Stroke::default()
            .with_width(if is_selected { 1.6 } else { 0.8 })
            .with_color(outline_color),
    );

    // v0.23 — Pad hole. Through-hole / NPT pads carry a positive
    // `drill_diameter_mm`; render it as a black "punched" disc on
    // top of the copper.
    if let Some(d_mm) = pad.drill_diameter_mm.filter(|d| *d > 1e-6) {
        let drill_r = (d_mm * 0.5) as f32 * cstate.scale;
        let hole_path = Path::new(|b| {
            let segments = 24;
            for i in 0..=segments {
                let t = i as f32 / segments as f32 * std::f32::consts::TAU;
                let x = centre.x + drill_r * t.cos();
                let y = centre.y + drill_r * t.sin();
                if i == 0 {
                    b.move_to(Point::new(x, y));
                } else {
                    b.line_to(Point::new(x, y));
                }
            }
            b.close();
        });
        frame.fill(&hole_path, Color::BLACK);
        frame.stroke(
            &hole_path,
            Stroke::default()
                .with_width(0.8)
                .with_color(if is_selected {
                    Color::WHITE
                } else {
                    Color::from_rgba(0.0, 0.0, 0.0, 0.85)
                }),
        );
    }

    // Pad number — only when zoomed in enough to read.
    if cstate.scale >= 25.0 && !pad.number.is_empty() {
        let centre = cstate.world_to_screen(pad.position_mm);
        let text_size = (cstate.scale * 0.35).clamp(8.0, 16.0);
        frame.fill_text(canvas::Text {
            content: pad.number.clone(),
            position: Point::new(centre.x, centre.y - text_size / 2.0),
            size: text_size.into(),
            color: Color::from_rgb(0.05, 0.05, 0.05),
            align_x: iced::alignment::Horizontal::Center.into(),
            align_y: iced::alignment::Vertical::Top,
            ..canvas::Text::default()
        });
    }
}

/// v0.18.16 — Pads-mode multi-click gesture preview. Reads the
/// in-flight tool state (`track_first` / `place_arc_pending` /
/// `place_polygon_vertices`) plus `cursor_mm` and draws a ghost
/// preview of what the next click will commit.
pub(super) fn draw_pads_tool_preview(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    state: &FootprintEditorState,
) {
    let Some(cursor) = state.cursor_mm else {
        return;
    };
    let ghost_colour = Color::from_rgba(1.0, 1.0, 1.0, 0.55);
    let stroke_px = 1.2_f32;
    let stroke = || {
        Stroke::default()
            .with_width(stroke_px)
            .with_color(ghost_colour)
    };

    match state.pads_tool {
        PadsTool::PlaceTrack => {
            if let Some((sx, sy)) = state.track_first {
                let p0 = cstate.world_to_screen((sx, sy));
                let p1 = cstate.world_to_screen(cursor);
                frame.stroke(&Path::line(p0, p1), stroke());
                let dot = Path::circle(p0, 3.0);
                frame.fill(&dot, ghost_colour);
            }
        }
        PadsTool::PlaceArc => match state.place_arc_pending {
            PlaceArcPending::Idle => {}
            PlaceArcPending::Center { center: (cx, cy) } => {
                let c = cstate.world_to_screen((cx, cy));
                let cur = cstate.world_to_screen(cursor);
                frame.stroke(&Path::line(c, cur), stroke());
                frame.fill(&Path::circle(c, 3.0), ghost_colour);
            }
            PlaceArcPending::Start {
                center: (cx, cy),
                start: (sx, sy),
            } => {
                let c = cstate.world_to_screen((cx, cy));
                let radius_world = ((sx - cx).powi(2) + (sy - cy).powi(2)).sqrt();
                let r_px = (radius_world as f32) * cstate.scale;
                let start_rad = ((sy - cy).atan2(sx - cx)) as f32;
                let end_rad = ((cursor.1 - cy).atan2(cursor.0 - cx)) as f32;
                let sweep = end_rad - start_rad;
                let segments = 64;
                let path = Path::new(|builder| {
                    let p0_x = c.x + r_px * start_rad.cos();
                    let p0_y = c.y + r_px * start_rad.sin();
                    builder.move_to(Point::new(p0_x, p0_y));
                    for i in 1..=segments {
                        let t = (i as f32) / (segments as f32);
                        let a = start_rad + sweep * t;
                        let p_x = c.x + r_px * a.cos();
                        let p_y = c.y + r_px * a.sin();
                        builder.line_to(Point::new(p_x, p_y));
                    }
                });
                frame.stroke(&path, stroke());
                frame.fill(&Path::circle(c, 3.0), ghost_colour);
            }
        },
        // v0.14 — Place Text Frame press-drag-release ghost (item
        // ③). Reads the live `cstate.drag` anchor (set on press for
        // any empty-canvas click) rather than a `state` pending
        // field, since the gesture commits in one message on
        // release — there's no cross-click state to persist.
        PadsTool::PlaceTextFrame => {
            if let Some(drag) = cstate.drag
                && drag.pad_idx == usize::MAX
            {
                let p0 = cstate.world_to_screen(drag.grab_offset_mm);
                let p1 = cstate.world_to_screen(cursor);
                let rect = Path::rectangle(
                    Point::new(p0.x.min(p1.x), p0.y.min(p1.y)),
                    iced::Size::new((p1.x - p0.x).abs(), (p1.y - p0.y).abs()),
                );
                frame.stroke(&rect, stroke());
                frame.fill(&Path::circle(p0, 3.0), ghost_colour);
            }
        }
        PadsTool::PlacePolygon | PadsTool::PlaceRegion => {
            let verts = &state.place_polygon_vertices;
            if verts.is_empty() {
                return;
            }
            let path = Path::new(|builder| {
                let first = cstate.world_to_screen(verts[0]);
                builder.move_to(first);
                for v in verts.iter().skip(1) {
                    builder.line_to(cstate.world_to_screen(*v));
                }
                let cur = cstate.world_to_screen(cursor);
                builder.line_to(cur);
                if verts.len() >= 2 {
                    builder.line_to(first);
                }
            });
            frame.stroke(&path, stroke());
            for v in verts {
                let p = cstate.world_to_screen(*v);
                frame.fill(&Path::circle(p, 3.0), ghost_colour);
            }
        }
        _ => {}
    }
}
