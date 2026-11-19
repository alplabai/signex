//! The Snap & Grid page illustration: the active grid around the origin
//! (fine and coarse, as lines or dots), the snap distance circle at a
//! sample cursor and the grid point it snaps to, and the axis band while
//! snapping to axes is on. Callouts for Snap Distance, Axis Snap Range and
//! the grid Step follow their fields.

use iced::{Point, Rectangle, Size};
use signex_i18n::{I18n, ids, t};
use signex_widgets::property_illustration::{
    Anchor, Callout, Illustration, Ink, Paint, Part, Primitive, Style, TextLabel, TextRole, Tint,
    Weight,
};

use super::fields;
use crate::library::editor::footprint::state::{GridDisplay, SnappingMode};
use crate::panels::FootprintEditorPanelContext;

const VIEW_W: f32 = 264.0;
const VIEW_H: f32 = 140.0;
/// One fine grid cell, in view units.
const CELL: f32 = 12.0;
/// The footprint origin in the view.
const ORIGIN: Point = Point::new(108.0, 84.0);
/// The sample cursor, in fine cells right of and above the origin.
const CURSOR_CELLS: (f32, f32) = (2.4, -2.6);
/// Radius of a grid dot, in view units.
const DOT_RADIUS: f32 = 0.7;
const COARSE_DOT_RADIUS: f32 = 1.1;
/// Half the arm of the origin and cursor crosses.
const CROSS_ARM: f32 = 5.0;
/// Bounds of a drawn radius, so a tiny or huge setting still reads.
const MIN_RADIUS: f32 = 3.0;
const MAX_SNAP_RADIUS: f32 = 34.0;
const MAX_AXIS_HALF: f32 = 18.0;
/// Where the Step callout and the axis callout sit.
const STEP_Y: f32 = VIEW_H - 8.0;
const AXIS_CALLOUT_X: f32 = VIEW_W - 12.0;
const LABEL_GAP: f32 = 4.0;
/// The coarse grid is drawn only for multipliers in this range.
const COARSE_RANGE: std::ops::RangeInclusive<u32> = 2..=12;

/// The snap and grid illustration of `fp`.
pub(super) fn illustration(fp: &FootprintEditorPanelContext, i18n: &I18n) -> Illustration {
    let opts = &fp.snap_options;
    let is_snapping = fp.snapping_mode != SnappingMode::Off;
    let (fine, coarse, multiplier) = match fp.grids.get(fp.active_grid_idx) {
        Some(grid) => (
            grid.fine_display,
            grid.coarse_display,
            grid.coarse_multiplier,
        ),
        None => (
            opts.fine_grid_display,
            opts.coarse_grid_display,
            opts.coarse_multiplier,
        ),
    };
    let step = opts.grid_step_mm.max(f64::EPSILON);
    let to_view = |mm: f64, max: f32| ((mm / step) as f32 * CELL).clamp(MIN_RADIUS, max);

    let mut ill = Illustration::new(VIEW_W, VIEW_H)
        .feature(fields::SNAPPING_ON, is_snapping)
        .feature(fields::AXES, opts.snap_to_axes);
    ill = ill.parts(grid_parts(fine, CELL, DOT_RADIUS, Weight::Hairline));
    if COARSE_RANGE.contains(&multiplier) {
        let cell = CELL * multiplier as f32;
        ill = ill.parts(grid_parts(coarse, cell, COARSE_DOT_RADIUS, Weight::Thin));
    }
    ill = ill.parts(cross(ORIGIN, Ink::Secondary));
    ill = ill.part(step_callout(i18n));
    ill = ill.parts(axis_parts(
        to_view(opts.axis_snap_range_mm, MAX_AXIS_HALF),
        i18n,
    ));
    ill = ill.parts(snap_parts(
        to_view(opts.snap_distance_mm, MAX_SNAP_RADIUS),
        opts.snap_to_grids,
        i18n,
    ));
    if !is_snapping {
        ill = ill.part(Part::label(
            TextLabel {
                at: Point::new(LABEL_GAP, LABEL_GAP * 2.0),
                text: t!(i18n, ids::EDITORS_FP_PROPS_ILLUSTRATION_SNAPPING_OFF).into(),
                anchor: Anchor::Start,
                role: TextRole::Note,
            },
            Ink::Faint,
        ));
    }
    ill
}

/// Grid lines or dots every `cell` units through the origin.
fn grid_parts(display: GridDisplay, cell: f32, dot: f32, weight: Weight) -> Vec<Part> {
    let xs = steps(ORIGIN.x, cell, VIEW_W);
    let ys = steps(ORIGIN.y, cell, VIEW_H);
    match display {
        GridDisplay::Hidden => Vec::new(),
        GridDisplay::Lines => {
            let style = Style::outline(Ink::Rule, weight);
            let vertical = xs.iter().map(|&x| Primitive::Line {
                from: Point::new(x, 0.0),
                to: Point::new(x, VIEW_H),
            });
            let horizontal = ys.iter().map(|&y| Primitive::Line {
                from: Point::new(0.0, y),
                to: Point::new(VIEW_W, y),
            });
            vertical
                .chain(horizontal)
                .map(|line| Part::new(line, style))
                .collect()
        }
        GridDisplay::Dots => {
            let style = Style::filled(Paint::Ink(Ink::Rule), Tint::Solid);
            xs.iter()
                .flat_map(|&x| ys.iter().map(move |&y| Point::new(x, y)))
                .map(|center| {
                    Part::new(
                        Primitive::Circle {
                            center,
                            radius: dot,
                        },
                        style,
                    )
                })
                .collect()
        }
    }
}

/// Every position `from + k * cell` inside `0..=end`.
fn steps(from: f32, cell: f32, end: f32) -> Vec<f32> {
    let first = from - (from / cell).floor() * cell;
    (0..)
        .map(|k| first + k as f32 * cell)
        .take_while(|v| *v <= end)
        .collect()
}

fn cross(at: Point, ink: Ink) -> Vec<Part> {
    let style = Style::outline(ink, Weight::Regular);
    [
        (
            Point::new(at.x - CROSS_ARM, at.y),
            Point::new(at.x + CROSS_ARM, at.y),
        ),
        (
            Point::new(at.x, at.y - CROSS_ARM),
            Point::new(at.x, at.y + CROSS_ARM),
        ),
    ]
    .into_iter()
    .map(|(from, to)| Part::new(Primitive::Line { from, to }, style))
    .collect()
}

/// The Step callout over one fine cell right of the origin.
fn step_callout(i18n: &I18n) -> Part {
    let from = Point::new(ORIGIN.x, STEP_Y);
    let to = Point::new(ORIGIN.x + CELL, STEP_Y);
    Part::callout(
        Callout::new(from, to)
            .label(t!(i18n, ids::EDITORS_FP_PROPS_GRID_STEP))
            .label_at(Point::new(to.x + LABEL_GAP, STEP_Y), Anchor::Start),
    )
    .linked(fields::GRID_STEP)
}

/// The axis band through the origin and its range callout; only while
/// snapping to axes is on.
fn axis_parts(half: f32, i18n: &I18n) -> Vec<Part> {
    let band = Rectangle::new(
        Point::new(0.0, ORIGIN.y - half),
        Size::new(VIEW_W, 2.0 * half),
    );
    let from = Point::new(AXIS_CALLOUT_X, ORIGIN.y - half);
    let to = Point::new(AXIS_CALLOUT_X, ORIGIN.y + half);
    let callout = Callout::new(from, to)
        .label(t!(i18n, ids::EDITORS_FP_PROPS_AXIS_SNAP_RANGE))
        .label_at(
            Point::new(AXIS_CALLOUT_X, ORIGIN.y + half + LABEL_GAP * 2.0),
            Anchor::End,
        );
    vec![
        Part::new(
            Primitive::Rect { rect: band },
            Style::filled(Paint::Ink(Ink::Accent), Tint::Wash),
        )
        .linked(fields::AXIS_SNAP_RANGE)
        .requires(fields::AXES),
        Part::callout(callout)
            .linked(fields::AXIS_SNAP_RANGE)
            .requires(fields::AXES),
    ]
}

/// The sample cursor, its snap distance circle and callout, and the grid
/// point it snaps to; the circle and the target only while snapping.
fn snap_parts(radius: f32, snaps_to_grid: bool, i18n: &I18n) -> Vec<Part> {
    let cursor = Point::new(
        ORIGIN.x + CURSOR_CELLS.0 * CELL,
        ORIGIN.y + CURSOR_CELLS.1 * CELL,
    );
    let mut parts = cross(cursor, Ink::Accent);
    parts.push(
        Part::new(
            Primitive::Circle {
                center: cursor,
                radius,
            },
            Style::dashed(),
        )
        .linked(fields::SNAP_DISTANCE)
        .requires(fields::SNAPPING_ON),
    );
    let edge = Point::new(cursor.x + radius, cursor.y);
    parts.push(
        Part::callout(
            Callout::new(cursor, edge)
                .label(t!(i18n, ids::EDITORS_FP_PROPS_SNAP_DISTANCE))
                .label_at(Point::new(edge.x + LABEL_GAP, cursor.y), Anchor::Start),
        )
        .linked(fields::SNAP_DISTANCE)
        .requires(fields::SNAPPING_ON),
    );
    let target = Point::new(
        ORIGIN.x + CURSOR_CELLS.0.round() * CELL,
        ORIGIN.y + CURSOR_CELLS.1.round() * CELL,
    );
    let in_reach = cursor.distance(target) <= radius;
    if snaps_to_grid && in_reach {
        parts.push(
            Part::new(
                Primitive::Circle {
                    center: target,
                    radius: COARSE_DOT_RADIUS * 2.0,
                },
                Style::outline(Ink::Accent, Weight::Regular)
                    .with_fill(Paint::Ink(Ink::Accent), Tint::Strong),
            )
            .requires(fields::SNAPPING_ON),
        );
    }
    parts
}
