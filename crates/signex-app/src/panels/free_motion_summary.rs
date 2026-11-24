//! One free motion of a sketch solve, as the Properties DOF card lists it
//! (mockup row 179).

/// One free motion of the solve: what moves, and Show's target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FreeMotionSummary {
    /// The entity Show selects: the first the motion moves.
    pub focus_entity_id: signex_sketch::id::SketchEntityId,
    /// How many entities move.
    pub entity_count: usize,
    pub kind: signex_sketch::solver::free_motion::MotionKind,
}
