//! `snxpcb/1.8` for a board sketch whose pattern repeats a baked target
//! (#2517) or whose array has a numbering sweep (mockup row 126): a build
//! that knows only 1.7 opens it read-only. Without them the board keeps its
//! lower token, byte for byte. Policy: `docs/FORMAT_VERSIONS.md`.

use super::tests::empty_board;
use super::*;
use crate::pcb::PcbBoard;
use crate::pcb_sketch::BoardSketch;
use crate::sketch::SketchData;
use crate::sketch::array::{Array, ArrayId, ArrayKind, GridSweep, NumberingScheme};
use crate::sketch::attr::MountingHoleAttr;
use crate::sketch::board_attr::MechanicalAttr;
use crate::sketch::entity::{Entity, EntityKind};
use crate::sketch::id::SketchEntityId;
use crate::sketch::pattern_links::PatternMirror;
use crate::sketch::plane::{Plane, PlaneId, PlaneKind};

/// A mounting-hole circle, a mechanical line and a free point.
struct Fixture {
    sketch: SketchData,
    hole: SketchEntityId,
    hole_centre: SketchEntityId,
    drawing_end: SketchEntityId,
    free_point: SketchEntityId,
}

fn fixture() -> Fixture {
    let plane = PlaneId::new();
    let mut sketch = SketchData::default();
    sketch.planes.push(Plane {
        id: plane,
        kind: PlaneKind::BoardTop,
    });
    let mut point = |x, y| {
        let id = SketchEntityId::new();
        sketch
            .entities
            .push(Entity::new(id, plane, EntityKind::Point { x, y }));
        id
    };
    let hole_centre = point(5.0, 5.0);
    let drawing_start = point(0.0, 20.0);
    let drawing_end = point(10.0, 20.0);
    let free_point = point(30.0, 30.0);
    let hole = SketchEntityId::new();
    let mut circle = Entity::new(
        hole,
        plane,
        EntityKind::Circle {
            center: hole_centre,
            radius: 1.5,
        },
    );
    circle.mounting_hole = Some(MountingHoleAttr::default());
    sketch.entities.push(circle);
    Fixture {
        sketch,
        hole,
        hole_centre,
        drawing_end,
        free_point,
    }
    .with_drawing(drawing_start, plane)
}

impl Fixture {
    fn with_drawing(mut self, start: SketchEntityId, plane: PlaneId) -> Self {
        let mut line = Entity::new(
            SketchEntityId::new(),
            plane,
            EntityKind::Line {
                start,
                end: self.drawing_end,
            },
        );
        line.mechanical = Some(MechanicalAttr::default());
        self.sketch.entities.push(line);
        self
    }
}

fn linear(source: SketchEntityId) -> Array {
    Array {
        id: ArrayId::new(),
        kind: ArrayKind::Linear {
            source,
            count_expr: "4".into(),
            dx_expr: "5mm".into(),
            dy_expr: "0mm".into(),
        },
        numbering: NumberingScheme::default(),
    }
}

fn swept(source: SketchEntityId, sweep: GridSweep) -> Array {
    Array {
        numbering: NumberingScheme::LinearIncrement {
            start_expr: "1".into(),
            step_expr: "1".into(),
            sweep,
        },
        ..linear(source)
    }
}

fn board_of(sketch: SketchData) -> PcbBoard {
    let mut board = empty_board();
    board.sketch = Some(BoardSketch {
        sketch,
        ..BoardSketch::default()
    });
    board
}

/// A board whose sketch holds one untagged point and `array` over it.
fn board_without_targets(array: impl FnOnce(SketchEntityId) -> Array) -> PcbBoard {
    let plane = PlaneId::new();
    let id = SketchEntityId::new();
    let mut sketch = SketchData::default();
    sketch.planes.push(Plane {
        id: plane,
        kind: PlaneKind::BoardTop,
    });
    sketch
        .entities
        .push(Entity::new(id, plane, EntityKind::Point { x: 1.0, y: 1.0 }));
    sketch.arrays.push(array(id));
    board_of(sketch)
}

fn write(board: &PcbBoard) -> String {
    SnxPcb::new(board.clone()).write_string().expect("write")
}

fn token_of(text: &str) -> &str {
    text.lines()
        .next()
        .and_then(|line| line.strip_prefix("format = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or_default()
}

#[test]
fn an_array_over_a_mounting_hole_writes_1_8() {
    for on_centre in [false, true] {
        let mut f = fixture();
        let source = if on_centre { f.hole_centre } else { f.hole };
        f.sketch.arrays.push(linear(source));
        assert_eq!(token_of(&write(&board_of(f.sketch))), SNXPCB_FORMAT_V1_8);
    }
}

/// A target that alone writes 1.7 still goes to 1.8 once it is patterned.
#[test]
fn an_array_over_a_mechanical_drawing_writes_1_8_not_1_7() {
    let mut f = fixture();
    let end = f.drawing_end;
    assert_eq!(
        token_of(&write(&board_of(f.sketch.clone()))),
        SNXPCB_FORMAT_BOARD_SKETCH_TARGETS
    );
    f.sketch.arrays.push(linear(end));
    assert_eq!(token_of(&write(&board_of(f.sketch))), SNXPCB_FORMAT_V1_8);
}

#[test]
fn a_mirror_of_a_patterned_target_writes_1_8() {
    let mut f = fixture();
    let array = linear(f.hole);
    let line = f.sketch.entities.last().expect("the drawing").id;
    f.sketch.pattern_mirrors.push(PatternMirror {
        id: ArrayId::new(),
        pattern: array.id,
        line,
    });
    f.sketch.arrays.push(array);
    assert_eq!(token_of(&write(&board_of(f.sketch))), SNXPCB_FORMAT_V1_8);
}

/// A construction circle bakes nothing, so a pattern of it needs no token.
#[test]
fn an_array_over_a_construction_hole_keeps_the_lower_token() {
    let mut f = fixture();
    let hole = f.hole;
    f.sketch.entities.retain(|e| e.mechanical.is_none());
    if let Some(circle) = f.sketch.entities.iter_mut().find(|e| e.id == hole) {
        circle.construction = true;
    }
    f.sketch.arrays.push(linear(hole));
    assert_eq!(token_of(&write(&board_of(f.sketch))), SNXPCB_FORMAT_V1);
}

#[test]
fn an_array_over_a_free_point_keeps_its_token_and_bytes() {
    let mut f = fixture();
    let free_point = f.free_point;
    f.sketch.entities.retain(|e| e.mechanical.is_none());
    f.sketch.arrays.push(linear(free_point));
    let text = write(&board_of(f.sketch));
    assert_eq!(token_of(&text), SNXPCB_FORMAT_V1, "{text}");
    let back = SnxPcb::parse(&text).expect("parse");
    assert_eq!(back.write_string().expect("write"), text);
}

#[test]
fn a_numbering_sweep_writes_1_8_and_going_back_lowers_it() {
    for sweep in [GridSweep::ColumnByColumn, GridSweep::DownThenUp] {
        let board = board_without_targets(|id| swept(id, sweep));
        let text = write(&board);
        assert_eq!(token_of(&text), SNXPCB_FORMAT_V1_8, "{sweep:?}");
        let back = SnxPcb::parse(&text).expect("parse").board;
        assert_eq!(back.sketch, board.sketch, "the sweep round-trips");
    }
    let row_by_row = board_without_targets(|id| swept(id, GridSweep::RowByRow));
    assert_eq!(token_of(&write(&row_by_row)), SNXPCB_FORMAT_V1);
}

#[test]
fn removing_the_pattern_lowers_the_token_again() {
    let mut f = fixture();
    f.sketch.entities.retain(|e| e.mechanical.is_none());
    f.sketch.arrays.push(linear(f.hole));
    let mut board = board_of(f.sketch);
    assert_eq!(token_of(&write(&board)), SNXPCB_FORMAT_V1_8);
    if let Some(stored) = board.sketch.as_mut() {
        stored.sketch.arrays.clear();
    }
    assert_eq!(token_of(&write(&board)), SNXPCB_FORMAT_V1);
}

#[test]
fn a_build_that_knows_only_1_7_opens_a_patterned_board_read_only() {
    let one_seven = FormatVersion {
        current: SNXPCB_FORMAT_BOARD_SKETCH_TARGETS,
        readable: &[
            "snxpcb/1",
            "snxpcb/1.1",
            "snxpcb/1.2",
            "snxpcb/1.3",
            SNXPCB_FORMAT_V1,
            SNXPCB_FORMAT_LAYER_COLORS,
            SNXPCB_FORMAT_ROOMS,
            SNXPCB_FORMAT_BOARD_SKETCH_TARGETS,
        ],
        ..SNXPCB_VERSION
    };
    let mut f = fixture();
    f.sketch.arrays.push(linear(f.hole));
    let text = write(&board_of(f.sketch));
    let token = token_of(&text);
    assert!(one_seven.is_newer_minor(token), "{token}");
    assert!(one_seven.refuse_save(token).is_err(), "{token}");
    assert!(!SNXPCB_VERSION.is_newer_minor(token), "this build reads it");
}
