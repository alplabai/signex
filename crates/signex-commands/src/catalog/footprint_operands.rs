//! The operands of the board footprint placement edits (#2936) and of the
//! footprint assignment commands (#2937). A footprint, or a part, is named
//! by `id` (a uuid), `designator` or `ids` (a list), exactly one of them.

use super::operands::{Operand, OperandKind, optional, required};

/// `set_footprint_position`: the footprint goes to `at` (`x,y` in mm from
/// the board origin, Y up); with `ids`, `points` holds one `x,y` per id,
/// `;` separated, and the list is one undo step.
pub const FOOTPRINT_POSITION: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("id", OperandKind::Text),
    optional("designator", OperandKind::Text),
    optional("ids", OperandKind::Targets),
    optional("at", OperandKind::Point),
    optional("points", OperandKind::Text),
];
/// `set_footprint_rotation`: each named footprint turned to `rotation`
/// about its own origin.
pub const FOOTPRINT_ROTATION: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("id", OperandKind::Text),
    optional("designator", OperandKind::Text),
    optional("ids", OperandKind::Targets),
    required("rotation", OperandKind::Angle),
];
/// `set_footprint_side`: each named footprint on `side`, `top` or `bottom`.
pub const FOOTPRINT_SIDE: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("id", OperandKind::Text),
    optional("designator", OperandKind::Text),
    optional("ids", OperandKind::Targets),
    required("side", OperandKind::Text),
];
/// `assign_footprint` on the sheet `document`: the parts named take
/// `footprint`, `<library>:<footprint>` (each by id or name).
pub const FOOTPRINT_CHOICE: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("id", OperandKind::Text),
    optional("designator", OperandKind::Text),
    optional("ids", OperandKind::Targets),
    required("footprint", OperandKind::Text),
];
/// `change_footprint` on the board `document`: the footprints named take
/// `footprint`, as for [`FOOTPRINT_CHOICE`]; `unmatched` is what an old pad
/// no new pad takes over does: `refuse` the swap (the default) or `keep`
/// swapping and list it, as the footprint picker does.
pub const FOOTPRINT_SWAP: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("id", OperandKind::Text),
    optional("designator", OperandKind::Text),
    optional("ids", OperandKind::Targets),
    required("footprint", OperandKind::Text),
    optional("unmatched", OperandKind::Text),
];
