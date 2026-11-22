//! The instances of a pattern of circles, drawn at rest (mockup row 179e): a
//! hole pattern shows every hole it bakes, dashed, at the bake's own slots
//! (`signex_bake::pattern_slots`), so an edited spacing moves them.

use iced::widget::canvas::{self, LineDash, Path, Stroke};
use signex_sketch::SketchData;
use signex_sketch::array::Array;
use signex_sketch::entity::EntityKind;
use signex_sketch::solver::FullSolveOutput;
use signex_sketch::solver::state::point_xy;

use super::SELECTION;
use crate::library::editor::footprint::canvas::FootprintCanvasState;
use crate::library::editor::footprint::sketch_mode::value_edit::MAX_DRAWN_SLOTS;
use crate::library::editor::footprint::state::FootprintEditorState;

const STROKE_PX: f32 = 1.2;
const DASH: [f32; 2] = [4.0, 3.0];
const INK_ALPHA: f32 = 0.7;

/// Where `array` of a Circle source places its other circles, with their
/// radius: the source itself and suppressed slots are left out. Empty for
/// any other source (a pad pattern draws as baked pads).
pub(in crate::library::editor::footprint::canvas) fn instance_circles(
    array: &Array,
    sketch: &SketchData,
    solve: &FullSolveOutput,
) -> Vec<((f64, f64), f64)> {
    let source = array.kind.source();
    let Some(EntityKind::Circle { center, radius }) = sketch
        .entities
        .iter()
        .find(|e| e.id == source)
        .map(|e| e.kind.clone())
    else {
        return Vec::new();
    };
    let Some(at) = point_xy(center, &solve.result.state, &solve.result.index, sketch) else {
        return Vec::new();
    };
    signex_bake::pattern_slots(sketch, solve, array.id, at, MAX_DRAWN_SLOTS)
        .unwrap_or_default()
        .into_iter()
        .filter(|slot| (slot.i, slot.j) != (0, 0) && !slot.suppressed)
        .map(|slot| (slot.at, radius))
        .collect()
}

pub(in crate::library::editor::footprint::canvas::draw) fn draw_pattern_instances(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    sketch: &SketchData,
    state: &FootprintEditorState,
) {
    let Some(solve) = state.last_solve.as_ref() else {
        return;
    };
    let stroke = Stroke {
        line_dash: LineDash {
            segments: &DASH,
            offset: 0,
        },
        ..Stroke::default().with_width(STROKE_PX).with_color(iced::Color {
            a: INK_ALPHA,
            ..SELECTION
        })
    };
    for array in &sketch.arrays {
        for (at, radius) in instance_circles(array, sketch, solve) {
            let circle = Path::circle(cstate.world_to_screen(at), radius as f32 * cstate.scale);
            frame.stroke(&circle, stroke);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_sketch::Solver;
    use signex_sketch::array::{ArrayId, ArrayKind, NumberingScheme};
    use signex_sketch::entity::Entity;
    use signex_sketch::id::SketchEntityId;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

    fn circle_grid() -> (SketchData, Array) {
        let plane = PlaneId::new();
        let mut sketch = SketchData::default();
        sketch.planes.push(Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        });
        let (center, hole) = (SketchEntityId::new(), SketchEntityId::new());
        sketch.entities.push(Entity::new(
            center,
            plane,
            EntityKind::Point { x: 1.0, y: 2.0 },
        ));
        sketch.entities.push(Entity::new(
            hole,
            plane,
            EntityKind::Circle {
                center,
                radius: 0.5,
            },
        ));
        let array = Array {
            id: ArrayId::new(),
            kind: ArrayKind::Grid {
                source: hole,
                nx_expr: "2".into(),
                ny_expr: "2".into(),
                dx_expr: "10mm".into(),
                dy_expr: "5mm".into(),
                depopulation: None,
            },
            numbering: NumberingScheme::default(),
        };
        sketch.arrays.push(array.clone());
        (sketch, array)
    }

    /// Mockup row 179e: a 2 x 2 hole pattern draws its three other holes
    /// at the spacing, each with the source's radius.
    #[test]
    fn a_circle_pattern_places_its_other_instances() {
        let (sketch, array) = circle_grid();
        let params = signex_sketch::parameter::resolve(&sketch.parameters).expect("resolves");
        let solve = Solver::default().solve(&sketch, &params).expect("solves");
        let circles = instance_circles(&array, &sketch, &solve);
        let mut centres: Vec<(f64, f64)> = circles.iter().map(|(at, _)| *at).collect();
        centres.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        let want = [(1.0, 7.0), (11.0, 2.0), (11.0, 7.0)];
        assert_eq!(centres.len(), want.len(), "{centres:?}");
        for (got, want) in centres.iter().zip(want) {
            assert!(
                (got.0 - want.0).abs() < 1e-6 && (got.1 - want.1).abs() < 1e-6,
                "{got:?} is not {want:?}"
            );
        }
        assert!(circles.iter().all(|(_, r)| (*r - 0.5).abs() < 1e-9));
    }
}
