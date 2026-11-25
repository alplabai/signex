//! Mockup row 179: the Sketch solve section leads with a DOF card (the
//! number, the state, the constraint count and the solve time) and one row
//! per free motion.

use signex_i18n::I18n;
use signex_sketch::id::SketchEntityId;
use signex_sketch::solver::free_motion::MotionKind;

use super::{dof_card, motion_label};
use crate::panels::{FootprintSolveSummary, FreeMotionSummary, SketchDof};
use crate::styles::PillTone;

fn plain(text: impl Into<String>) -> String {
    text.into().replace(['\u{2068}', '\u{2069}'], "")
}

fn solve(dof: usize, conflicts: usize) -> FootprintSolveSummary {
    FootprintSolveSummary {
        iterations: 4,
        elapsed_ms: 3,
        final_residual_norm: 0.0,
        over_constraint_count: conflicts,
        over_constraints: Vec::new(),
        redundant_constraints: Vec::new(),
        dof,
        free_motions: Vec::new(),
    }
}

fn card_texts(
    dof: usize,
    conflicts: usize,
    constraints: usize,
) -> (String, PillTone, String, String) {
    let state = SketchDof::of(conflicts, false, dof);
    let card = dof_card(state, &solve(dof, conflicts), constraints, &I18n::en_us());
    (
        plain(card.number.into_string()),
        card.tone,
        plain(card.state.into_string()),
        plain(card.detail.into_string()),
    )
}

#[test]
fn an_under_constrained_sketch_shows_its_dof_count_and_solve() {
    assert_eq!(
        card_texts(2, 0, 46),
        (
            "2".to_string(),
            PillTone::Info,
            "Under-constrained · 2 DOF".to_string(),
            "46 constraints · solved in 3 ms".to_string(),
        )
    );
}

#[test]
fn a_fully_constrained_sketch_shows_zero() {
    let (number, tone, state, detail) = card_texts(0, 0, 48);
    assert_eq!((number.as_str(), tone), ("0", PillTone::Ok));
    assert_eq!(state, "Fully constrained");
    assert_eq!(detail, "48 constraints · solved in 3 ms");
}

#[test]
fn an_over_constrained_sketch_shows_its_conflicts_and_keeps_the_last_solve() {
    let (number, tone, state, detail) = card_texts(0, 1, 49);
    assert_eq!((number.as_str(), tone), ("\u{2212}1", PillTone::Err));
    assert_eq!(state, "Over-constrained · 1 conflict");
    assert_eq!(detail, "49 constraints · last good solve kept");
}

#[test]
fn a_rejected_edit_shows_the_over_constrained_card_over_a_free_solve() {
    let state = SketchDof::of(0, true, 4);
    let card = dof_card(state, &solve(4, 0), 7, &I18n::en_us());
    assert_eq!(
        (plain(card.number.into_string()).as_str(), card.tone),
        ("\u{2212}1", PillTone::Err)
    );
    assert_eq!(
        plain(card.state.into_string()),
        "Over-constrained · 1 conflict"
    );
    assert_eq!(
        plain(card.detail.into_string()),
        "7 constraints · last good solve kept"
    );
}

/// #2855: a sketch whose QR colours read Over while the row reduction finds
/// no conflict and no free motion. The chip and the card read the row
/// reduction and say Fully constrained; the canvas colour keeps Over.
#[test]
fn the_chip_and_the_card_read_full_where_the_qr_colours_read_over() {
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
    use signex_sketch::sketch::SketchData;
    use signex_sketch::solver::FullSolveOutput;
    use signex_sketch::solver::dof::DofColor;
    use signex_sketch::solver::lm::SolveResult;
    use signex_sketch::solver::state::pack;

    use crate::library::editor::standalone::dof_chip::{dof_state, sketch_dof};

    let plane = PlaneId::new();
    let point = SketchEntityId::new();
    let sketch = SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        entities: vec![Entity::new(
            point,
            plane,
            EntityKind::Point { x: 0.0, y: 0.0 },
        )],
        ..SketchData::default()
    };
    let packed = pack(&sketch);
    let out = FullSolveOutput {
        result: SolveResult {
            state: packed.vector,
            index: packed.index,
            iterations: 4,
            final_residual_norm: 0.0,
            elapsed_ms: 3,
        },
        colours: std::iter::once((point, DofColor::Over)).collect(),
        over_constraints: Vec::new(),
        redundant_constraints: Vec::new(),
        warnings: Vec::new(),
        redundancy_pending: false,
        jacobian: Vec::new(),
        free_motions: Default::default(),
        params: Default::default(),
    };
    let mut fp = signex_library::Footprint::empty("near-dependent");
    fp.sketch = Some(sketch);
    let mut editor = crate::app::FootprintDocument::new(
        std::path::PathBuf::from("near-dependent.snxfpt"),
        signex_library::FootprintFile::from_footprint(fp),
    );
    editor.state.last_solve = Some(out);

    let (tone, id) = dof_state(&editor);
    assert_eq!(tone, PillTone::Ok);
    assert_eq!(
        plain(signex_i18n::t!(I18n::en_us_ref(), id).into_string()),
        "Fully constrained · DOF 0"
    );

    let state = sketch_dof(&editor).expect("a solve to read");
    assert_eq!(state, SketchDof::Full);
    let out = editor.state.last_solve.as_ref().expect("the solve");
    let summary = FootprintSolveSummary {
        over_constraint_count: out.over_constraints.len(),
        dof: out.free_motions.dof,
        ..solve(0, 0)
    };
    let card = dof_card(state, &summary, 0, &I18n::en_us());
    assert_eq!(
        (plain(card.number.into_string()).as_str(), card.tone),
        ("0", PillTone::Ok)
    );
    assert_eq!(plain(card.state.into_string()), "Fully constrained");
    assert_eq!(out.colours.get(&point), Some(&DofColor::Over));
}

fn label(kind: MotionKind, entity_count: usize) -> String {
    let motion = FreeMotionSummary {
        focus_entity_id: SketchEntityId::new(),
        entity_count,
        kind,
    };
    plain(motion_label(&motion, &I18n::en_us()).into_string())
}

#[test]
fn each_free_motion_says_what_moves_and_which_way() {
    let slide = |dx, dy| MotionKind::Slide { dx, dy };
    assert_eq!(label(slide(1.0, 0.0), 4), "4 points slide along X");
    assert_eq!(label(slide(-1.0, 0.0), 1), "One point slides along X");
    assert_eq!(label(slide(0.0, 1.0), 2), "2 points slide along Y");
    let diagonal = std::f64::consts::FRAC_1_SQRT_2;
    assert_eq!(
        label(slide(diagonal, diagonal), 1),
        "One point slides at 45.0°"
    );
    assert_eq!(
        label(MotionKind::AnyDirection, 1),
        "One point moves in any direction"
    );
    assert_eq!(label(MotionKind::Reshape, 3), "3 entities can change shape");
}
