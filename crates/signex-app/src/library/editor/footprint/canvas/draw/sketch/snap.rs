//! Inferred-constraint snap glyph — the badge drawn at the cursor while
//! a placement tool is active, hinting the auto-constraint the next
//! click will land — and the words beside it naming the target
//! (Graphite mockup #188).

use iced::widget::canvas::{self, Path, Stroke};
use iced::{Color, Point, Size};
use signex_sketch::SketchData;
use signex_sketch::id::SketchEntityId;
use signex_types::theme::{ThemeId, theme_tokens};

use crate::library::editor::footprint::canvas::FootprintCanvasState;
use crate::library::editor::footprint::snap::SnapKind;
use crate::library::editor::footprint::snap_board::{self, BoardFeatureKind};
use crate::library::editor::footprint::state::FootprintEditorState;

/// The snap glyph's cyan, darkened to read on the dark board ground and
/// on a white sketch ground alike (the look it has had since v0.27).
const SNAP_INK: Color = Color::from_rgba(0.10, 0.60, 0.90, 1.00);
/// The badge pill's corner, down and right of the snap point, in px.
const BADGE_OFFSET_PX: f32 = 12.0;
/// The badge pill's corner radius in px.
const BADGE_RADIUS_PX: f32 = 4.0;
/// Type steps at the default size; the canvas has no live scale.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;

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
/// - `◐` / `⊙` — `SnapKind::Midpoint` / `SnapKind::Centre` (#188): the
///   click lands a Midpoint, or a Coincident on the centre.
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
    let c = SNAP_INK;
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
        SnapKind::BoardFeature(_) => {
            // A hollow square: a place on the board, no constraint.
            let path = Path::rectangle(Point::new(p.x - 6.0, p.y - 6.0), Size::new(12.0, 12.0));
            frame.stroke(&path, stroke);
        }
        SnapKind::Midpoint(_) => {
            // ◐: a ring with its left half filled (mockup #188).
            let r = 6.0;
            let half = Path::new(|b| {
                b.arc(canvas::path::Arc {
                    center: Point::new(p.x, p.y),
                    radius: r,
                    start_angle: iced::Radians(std::f32::consts::FRAC_PI_2),
                    end_angle: iced::Radians(3.0 * std::f32::consts::FRAC_PI_2),
                });
                b.close();
            });
            frame.fill(&half, c);
            frame.stroke(&Path::circle(Point::new(p.x, p.y), r), stroke);
        }
        SnapKind::Centre(_) => {
            // ⊙: a ring around a dot (mockup #188).
            frame.stroke(&Path::circle(Point::new(p.x, p.y), 6.0), stroke);
            frame.fill(&Path::circle(Point::new(p.x, p.y), 2.0), c);
        }
        SnapKind::Guide | SnapKind::Grid | SnapKind::Raw => {}
    }
}

/// The words a snap badge is built from, rendered by the app from its
/// catalog into [`FootprintEditorState::canvas_text`]; the canvas cannot
/// read the catalog itself.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SnapBadgeWords<'a> {
    pub coincident: &'a str,
    pub horizontal: &'a str,
    pub vertical: &'a str,
    pub angle: &'a str,
    pub intersection: &'a str,
    pub guide: &'a str,
    /// The target of a Point snap on a plain sketch point.
    pub point: &'a str,
    /// Put before a pad's number: "pad 1".
    pub pad: &'a str,
    pub board_outline: &'a str,
    pub board_pad: &'a str,
    pub board_hole: &'a str,
    pub board_via: &'a str,
    pub board_track: &'a str,
    pub board_courtyard: &'a str,
    pub midpoint: &'a str,
    pub centre: &'a str,
    /// The target of a Midpoint snap on a Line of no pad.
    pub line: &'a str,
    /// The target of a Centre snap on a Circle or an Arc of no pad.
    pub circle: &'a str,
    pub arc: &'a str,
}

impl<'a> SnapBadgeWords<'a> {
    fn of(state: &'a FootprintEditorState) -> Self {
        let t = &state.canvas_text;
        Self {
            coincident: &t.snap_coincident,
            horizontal: &t.snap_horizontal,
            vertical: &t.snap_vertical,
            angle: &t.snap_angle,
            intersection: &t.snap_intersection,
            guide: &t.snap_guide,
            point: &t.snap_point,
            pad: &t.snap_pad,
            board_outline: &t.snap_board_outline,
            board_pad: &t.snap_board_pad,
            board_hole: &t.snap_board_hole,
            board_via: &t.snap_board_via,
            board_track: &t.snap_board_track,
            board_courtyard: &t.snap_board_courtyard,
            midpoint: &t.snap_midpoint,
            centre: &t.snap_centre,
            line: &t.snap_line,
            circle: &t.snap_circle,
            arc: &t.snap_arc,
        }
    }
}

/// The badge text of a snap, `<kind> · <target>` (Graphite mockup #188:
/// "the badge names the target"): "Coincident · pad 1", "Horizontal",
/// "Angle 45°", "Pad · J1-1". `target` is what [`snap_badge_target`]
/// found; a Point snap without one names a plain point. Grid and Raw say
/// nothing.
pub(crate) fn snap_badge_label(
    kind: SnapKind,
    target: Option<&str>,
    words: &SnapBadgeWords<'_>,
) -> Option<String> {
    let named = |word: &str| match target {
        Some(name) => format!("{word} · {name}"),
        None => word.to_owned(),
    };
    match kind {
        SnapKind::Point(_) => Some(format!(
            "{} · {}",
            words.coincident,
            target.unwrap_or(words.point)
        )),
        SnapKind::Horizontal => Some(words.horizontal.to_owned()),
        SnapKind::Vertical => Some(words.vertical.to_owned()),
        SnapKind::Angle(rad) => Some(format!("{} {}°", words.angle, screen_angle_deg(rad))),
        SnapKind::Intersection => Some(words.intersection.to_owned()),
        SnapKind::Guide => Some(words.guide.to_owned()),
        SnapKind::BoardFeature(feature) => Some(named(match feature {
            BoardFeatureKind::Outline => words.board_outline,
            BoardFeatureKind::Pad => words.board_pad,
            BoardFeatureKind::Hole => words.board_hole,
            BoardFeatureKind::Via => words.board_via,
            BoardFeatureKind::TrackEnd => words.board_track,
            BoardFeatureKind::Courtyard => words.board_courtyard,
        })),
        SnapKind::Midpoint(_) => Some(named(words.midpoint)),
        SnapKind::Centre(_) => Some(named(words.centre)),
        SnapKind::Grid | SnapKind::Raw => None,
    }
}

/// A snapped line's slope as the user sees it: degrees counter-clockwise
/// on screen (the canvas's y runs down), in `0..180`, whole degrees.
pub(crate) fn screen_angle_deg(rad: f64) -> i64 {
    ((-rad).to_degrees().rem_euclid(180.0).round() as i64) % 180
}

/// What a snap landed on, for the badge: a sketch point that is a pad, or
/// belongs to one, names it ("pad 1"); a board feature names its pad or
/// component. `None` when there is nothing to name.
pub(crate) fn snap_badge_target(
    kind: SnapKind,
    pos: (f64, f64),
    sketch: &SketchData,
    state: &FootprintEditorState,
    words: &SnapBadgeWords<'_>,
) -> Option<String> {
    match kind {
        SnapKind::Point(id) => {
            pad_number_of(sketch, id).map(|number| format!("{} {number}", words.pad))
        }
        SnapKind::BoardFeature(feature) => state
            .board_backdrop
            .as_deref()
            .and_then(|board| snap_board::feature_name(board, feature, pos)),
        SnapKind::Midpoint(id) | SnapKind::Centre(id) => Some(curve_name(sketch, id, words)),
        _ => None,
    }
}

/// A Midpoint or Centre target: the pad the curve belongs to ("pad 1"),
/// else what it is ("line", "circle", "arc").
fn curve_name(sketch: &SketchData, id: SketchEntityId, words: &SnapBadgeWords<'_>) -> String {
    if let Some(number) = pad_number_of(sketch, id) {
        return format!("{} {number}", words.pad);
    }
    use signex_sketch::entity::EntityKind;
    let kind = sketch.entities.iter().find(|e| e.id == id).map(|e| &e.kind);
    let word = match kind {
        Some(EntityKind::Circle { .. }) => words.circle,
        Some(EntityKind::Arc { .. }) => words.arc,
        _ => words.line,
    };
    word.to_owned()
}

/// The number of the pad `id` is the centre of, or one of whose owned
/// entities it is; `None` for a point of no pad or a pad with no number.
pub(crate) fn pad_number_of(sketch: &SketchData, id: SketchEntityId) -> Option<&str> {
    let own = sketch
        .entities
        .iter()
        .find(|e| e.id == id)
        .and_then(|e| e.pad.as_ref());
    let pad = own.or_else(|| {
        sketch
            .entities
            .iter()
            .filter_map(|e| e.pad.as_ref())
            .find(|pad| pad.owned.contains(&id))
    })?;
    Some(pad.number.as_str()).filter(|n| !n.is_empty())
}

/// The snap badge's words, in a pill beside the glyph, while a placement
/// tool is armed (the glyph's rule). Pill colours are the Graphite Dark
/// chrome tokens: the board ground is Graphite Dark in both themes.
pub(in crate::library::editor::footprint::canvas::draw) fn draw_sketch_snap_badge(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    sketch: &SketchData,
    state: &FootprintEditorState,
) {
    use crate::library::editor::footprint::state::SketchTool;

    if matches!(state.active_tool, SketchTool::Select) {
        return;
    }
    let Some(snap) = cstate.last_snap else {
        return;
    };
    let words = SnapBadgeWords::of(state);
    let target = snap_badge_target(snap.kind, snap.pos, sketch, state, &words);
    let Some(label) = snap_badge_label(snap.kind, target.as_deref(), &words) else {
        return;
    };
    let tokens = theme_tokens(ThemeId::GraphiteDark);
    let resolved = crate::styles::tokens::resolve(&tokens);
    let size = TS.caption;
    let p = cstate.world_to_screen(snap.pos);
    let height = size + 7.0;
    let width = label.chars().count() as f32 * size * 0.6 + 12.0;
    let pill = Path::rounded_rectangle(
        Point::new(p.x + BADGE_OFFSET_PX, p.y + BADGE_OFFSET_PX),
        Size::new(width, height),
        iced::border::Radius::from(BADGE_RADIUS_PX),
    );
    frame.fill(&pill, resolved.raised);
    frame.stroke(
        &pill,
        Stroke::default()
            .with_width(1.0)
            .with_color(resolved.line_strong),
    );
    frame.fill_text(canvas::Text {
        content: label,
        position: Point::new(
            p.x + BADGE_OFFSET_PX + 6.0,
            p.y + BADGE_OFFSET_PX + height / 2.0,
        ),
        color: crate::styles::ti(tokens.text),
        size: size.into(),
        align_y: iced::alignment::Vertical::Center,
        ..canvas::Text::default()
    });
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
    use signex_types::sketch::attr::PadAttr;

    use super::*;
    use crate::library::editor::footprint::state::{BackdropPad, BoardBackdrop, PadCopper};

    /// The en-US words, as the catalog renders them for the canvas.
    const WORDS: SnapBadgeWords<'static> = SnapBadgeWords {
        coincident: "Coincident",
        horizontal: "Horizontal",
        vertical: "Vertical",
        angle: "Angle",
        intersection: "Intersection",
        guide: "Guide",
        point: "point",
        pad: "pad",
        board_outline: "Board outline",
        board_pad: "Pad",
        board_hole: "Hole",
        board_via: "Via",
        board_track: "Track end",
        board_courtyard: "Courtyard",
        midpoint: "Midpoint",
        centre: "Centre",
        line: "line",
        circle: "circle",
        arc: "arc",
    };

    fn label(kind: SnapKind, target: Option<&str>) -> Option<String> {
        snap_badge_label(kind, target, &WORDS)
    }

    /// A sketch with pad 1's centre, one of its corners and a plain point.
    fn sketch() -> (SketchData, [SketchEntityId; 3]) {
        let mut sketch = SketchData::default();
        let plane = Plane {
            id: PlaneId::new(),
            kind: PlaneKind::BoardTop,
        };
        sketch.planes.push(plane.clone());
        let ids = [
            SketchEntityId::new(),
            SketchEntityId::new(),
            SketchEntityId::new(),
        ];
        let point = |id| Entity::new(id, plane.id, EntityKind::Point { x: 0.0, y: 0.0 });
        let mut centre = point(ids[0]);
        centre.pad = Some(PadAttr {
            number: "1".into(),
            owned: vec![ids[1]],
            ..PadAttr::default()
        });
        sketch
            .entities
            .extend([centre, point(ids[1]), point(ids[2])]);
        (sketch, ids)
    }

    #[test]
    fn a_point_snap_names_its_pad_or_says_point() {
        let (sketch, [centre, corner, plain]) = sketch();
        let state = FootprintEditorState::empty();
        let target =
            |id| snap_badge_target(SnapKind::Point(id), (0.0, 0.0), &sketch, &state, &WORDS);
        assert_eq!(target(centre).as_deref(), Some("pad 1"));
        assert_eq!(
            target(corner).as_deref(),
            Some("pad 1"),
            "a pad's own corner"
        );
        assert_eq!(target(plain), None);
        assert_eq!(
            label(SnapKind::Point(centre), Some("pad 1")).as_deref(),
            Some("Coincident · pad 1")
        );
        assert_eq!(
            label(SnapKind::Point(plain), None).as_deref(),
            Some("Coincident · point")
        );
    }

    #[test]
    fn inference_snaps_name_their_kind_and_grid_says_nothing() {
        assert_eq!(
            label(SnapKind::Horizontal, None).as_deref(),
            Some("Horizontal")
        );
        assert_eq!(label(SnapKind::Vertical, None).as_deref(), Some("Vertical"));
        assert_eq!(
            label(SnapKind::Intersection, None).as_deref(),
            Some("Intersection")
        );
        assert_eq!(label(SnapKind::Guide, None).as_deref(), Some("Guide"));
        assert_eq!(label(SnapKind::Grid, None), None);
        assert_eq!(label(SnapKind::Raw, None), None);
    }

    #[test]
    fn an_angle_snap_reads_its_slope_on_screen() {
        // The canvas's y runs down: up and to the right is -45° in world.
        let up_right = (-45.0_f64).to_radians();
        assert_eq!(
            label(SnapKind::Angle(up_right), None).as_deref(),
            Some("Angle 45°")
        );
        assert_eq!(screen_angle_deg(45.0_f64.to_radians()), 135);
        assert_eq!(screen_angle_deg(-180.0_f64.to_radians()), 0);
        assert_eq!(screen_angle_deg((-30.0_f64).to_radians()), 30);
    }

    #[test]
    fn a_board_feature_names_its_kind_and_its_part() {
        let board = BoardBackdrop {
            pads: vec![BackdropPad {
                centre: (3.0, 4.0),
                size: (1.0, 1.0),
                rotation_deg: 0.0,
                round: true,
                copper: PadCopper::Through,
                designator: "J1-1".into(),
            }],
            ..BoardBackdrop::default()
        };
        let mut state = FootprintEditorState::empty();
        state.board_backdrop = Some(Arc::new(board));
        let (sketch, _) = sketch();
        let pad = SnapKind::BoardFeature(BoardFeatureKind::Pad);
        let target = snap_badge_target(pad, (3.0, 4.0), &sketch, &state, &WORDS);
        assert_eq!(target.as_deref(), Some("J1-1"));
        assert_eq!(label(pad, target.as_deref()).as_deref(), Some("Pad · J1-1"));
        let edge = SnapKind::BoardFeature(BoardFeatureKind::Outline);
        assert_eq!(
            snap_badge_target(edge, (0.0, 0.0), &sketch, &state, &WORDS),
            None
        );
        assert_eq!(label(edge, None).as_deref(), Some("Board outline"));
    }

    #[test]
    fn midpoint_and_centre_name_their_curve_or_its_pad() {
        let (mut sketch, [centre, corner, plain]) = sketch();
        let plane = sketch.planes[0].id;
        let mut add = |kind| {
            let id = SketchEntityId::new();
            sketch.entities.push(Entity::new(id, plane, kind));
            id
        };
        let line = add(EntityKind::Line {
            start: corner,
            end: plain,
        });
        let circle = add(EntityKind::Circle {
            center: plain,
            radius: 1.0,
        });
        let arc = add(EntityKind::Arc {
            center: plain,
            start: corner,
            end: centre,
            sweep_ccw: true,
        });
        if let Some(pad) = sketch.entities[0].pad.as_mut() {
            pad.owned.push(arc);
        }
        let state = FootprintEditorState::empty();
        let named = |kind| {
            let target = snap_badge_target(kind, (0.0, 0.0), &sketch, &state, &WORDS);
            label(kind, target.as_deref())
        };
        assert_eq!(
            named(SnapKind::Midpoint(line)).as_deref(),
            Some("Midpoint · line")
        );
        assert_eq!(
            named(SnapKind::Centre(circle)).as_deref(),
            Some("Centre · circle")
        );
        assert_eq!(
            named(SnapKind::Centre(arc)).as_deref(),
            Some("Centre · pad 1"),
            "an arc of pad 1 names the pad"
        );
    }
}
