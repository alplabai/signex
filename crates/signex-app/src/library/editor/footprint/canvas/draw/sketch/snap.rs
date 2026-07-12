//! Inferred-constraint snap glyph — the badge drawn at the cursor while
//! a placement tool is active, hinting the auto-constraint the next
//! click will land.

use iced::widget::canvas::{self, Path, Stroke};
use iced::{Color, Point};

use crate::library::editor::footprint::canvas::FootprintCanvasState;
use crate::library::editor::footprint::snap::{self, SnapKind, SnapResult};
use crate::library::editor::footprint::state::FootprintEditorState;

/// v0.22 Phase A6 — Inferred-constraint snap glyph at the cursor.
/// Rendered AFTER the entity overlay so the badge sits on top of the
/// underlying geometry. Drives off `cstate.last_snap` which the
/// cursor-moved handler refreshes via `snap::snap_cursor`. Visible
/// only while a placement tool is active — Select doesn't draw a
/// hint because no entity is about to land. Glyphs:
/// - `●` (filled circle in cyan) — `SnapKind::Point` — auto-Coincident
///   target; clicking lands a new Point coincident with this one.
/// - `─` (horizontal cyan bar) — `SnapKind::Horizontal` — auto-H
///   constraint will land on the new Line.
/// - `│` (vertical cyan bar) — `SnapKind::Vertical` — auto-V
///   constraint will land on the new Line.
/// - `◇` (cyan diamond) — `SnapKind::Angle` — angle-snapped to the
///   nearest 15° increment.
/// - Guide / Grid / Raw — silent (Guide already paints its line;
///   Grid + Raw aren't actionable hints).
pub(in crate::library::editor::footprint::canvas::draw) fn draw_sketch_snap_glyph(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    state: &FootprintEditorState,
) {
    use crate::library::editor::footprint::state::SketchTool;

    if matches!(state.active_tool, SketchTool::Select) {
        return;
    }
    let snap = match cstate.last_snap {
        Some(s) => s,
        None => return,
    };
    let p = cstate.world_to_screen(snap.pos);
    // v0.27 — slightly darkened cyan so the snap badge reads
    // against both the dark Pads-mode canvas and the white
    // Sketch-mode canvas without competing.
    let c = Color::from_rgba(0.10, 0.60, 0.90, 1.00);
    let fill = Color { a: 0.30, ..c };
    let stroke = Stroke::default().with_width(1.5).with_color(c);

    match snap.kind {
        SnapKind::Point(_) => {
            let path = Path::circle(Point::new(p.x, p.y), 7.0);
            frame.fill(&path, fill);
            frame.stroke(&path, stroke);
        }
        SnapKind::Horizontal => {
            frame.stroke(
                &Path::line(Point::new(p.x - 10.0, p.y), Point::new(p.x + 10.0, p.y)),
                stroke,
            );
        }
        SnapKind::Vertical => {
            frame.stroke(
                &Path::line(Point::new(p.x, p.y - 10.0), Point::new(p.x, p.y + 10.0)),
                stroke,
            );
        }
        SnapKind::Angle(_) => {
            let r = 6.0;
            frame.stroke(
                &Path::line(Point::new(p.x, p.y - r), Point::new(p.x + r, p.y)),
                stroke,
            );
            frame.stroke(
                &Path::line(Point::new(p.x + r, p.y), Point::new(p.x, p.y + r)),
                stroke,
            );
            frame.stroke(
                &Path::line(Point::new(p.x, p.y + r), Point::new(p.x - r, p.y)),
                stroke,
            );
            frame.stroke(
                &Path::line(Point::new(p.x - r, p.y), Point::new(p.x, p.y - r)),
                stroke,
            );
        }
        SnapKind::Intersection => {
            // v0.27 — small "+" badge marks an intersection snap.
            let r = 6.0;
            frame.stroke(
                &Path::line(Point::new(p.x - r, p.y), Point::new(p.x + r, p.y)),
                stroke,
            );
            frame.stroke(
                &Path::line(Point::new(p.x, p.y - r), Point::new(p.x, p.y + r)),
                stroke,
            );
            let path = Path::circle(Point::new(p.x, p.y), 2.5);
            frame.fill(&path, c);
        }
        SnapKind::Guide | SnapKind::Grid | SnapKind::Raw => {}
    }
}
