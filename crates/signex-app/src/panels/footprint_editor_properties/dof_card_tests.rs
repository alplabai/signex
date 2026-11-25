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
