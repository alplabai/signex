//! Why a board sketch did not bake.

use signex_sketch::SolveError;
use signex_sketch::baked_kinds::BakedKind;
use signex_sketch::expr::ExprError;
use signex_sketch::id::SketchEntityId;

/// A board sketch that cannot become board geometry. Every tagged entity
/// must bake: a dropped keepout or hole would pass DRC unseen, so the board
/// bake refuses where the footprint bake only warns.
#[derive(Debug, Clone, thiserror::Error)]
pub enum BoardBakeError {
    #[error("the sketch parameters do not resolve: {0}")]
    Parameters(ExprError),
    #[error("the board sketch does not solve: {0}")]
    Solve(SolveError),
    #[error("the {} through edge {edge} is not a closed profile", .kind.label())]
    OpenProfile {
        kind: BakedKind,
        edge: SketchEntityId,
    },
    #[error("the {} through edge {edge} branches at a vertex", .kind.label())]
    BranchingProfile {
        kind: BakedKind,
        edge: SketchEntityId,
    },
    #[error("the {} through edge {edge} cannot be traced", .kind.label())]
    UntraceableProfile {
        kind: BakedKind,
        edge: SketchEntityId,
    },
    #[error("{} entity {entity} is not a line, arc or circle", .kind.label())]
    NotAnEdge {
        kind: BakedKind,
        entity: SketchEntityId,
    },
    #[error("point {point} of the {} has no solved position", .kind.label())]
    MissingPoint {
        kind: BakedKind,
        point: SketchEntityId,
    },
    #[error("the board outline is {count} closed profiles; it must be one")]
    OutlineCount { count: usize },
    #[error("mounting hole {entity} is not a circle")]
    HoleNotACircle { entity: SketchEntityId },
    #[error("mounting hole {entity} has a drill of {drill} mm; it must be above 0")]
    HoleDrill { entity: SketchEntityId, drill: f64 },
    #[error("mounting hole {entity} names net {net:?}, which the board does not have")]
    UnknownNet { entity: SketchEntityId, net: String },
    #[error("V-score {entity} is not a line")]
    VScoreNotALine { entity: SketchEntityId },
    #[error("the {what} of entity {entity} does not evaluate: {message}")]
    Expression {
        entity: SketchEntityId,
        what: &'static str,
        message: String,
    },
}
