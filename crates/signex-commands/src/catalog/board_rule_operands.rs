//! Operands of the board rule edits: the teardrop rule, a pad's own
//! teardrop setting and a pour's fill priority and island removal. Kept
//! apart from `operands` so that table stays small.

use super::operands::{Operand, OperandKind, optional, required};

/// `set_teardrop_rule`: the board and any of the rule's fields; a field
/// left out keeps its value. `width_share` in `(0, 1]` and `length_share`
/// in `(0, 4]` are plain numbers (shares of the pad's inscribed radius).
pub const SET_TEARDROP_RULE: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("pads", OperandKind::Bool),
    optional("vias", OperandKind::Bool),
    optional("width_share", OperandKind::Text),
    optional("length_share", OperandKind::Text),
];

/// `set_pad_teardrop`: pads by uuid and `teardrop`, `on` (always), `off`
/// (never) or `rule` (as the board's teardrop rule says).
pub const SET_PAD_TEARDROP: &[Operand] = &[
    required("document", OperandKind::Document),
    required("ids", OperandKind::Targets),
    required("teardrop", OperandKind::Text),
];

/// `set_pour_properties`: pours by uuid and any of `priority` (a whole
/// number from 0; a higher one fills first), `islands_below` (mm², 0 keeps
/// every island) and `dead_copper` (remove islands touching no copper of
/// the pour's net). At least one is needed.
pub const SET_POUR_PROPERTIES: &[Operand] = &[
    required("document", OperandKind::Document),
    required("ids", OperandKind::Targets),
    optional("priority", OperandKind::Text),
    optional("islands_below", OperandKind::Text),
    optional("dead_copper", OperandKind::Bool),
];
