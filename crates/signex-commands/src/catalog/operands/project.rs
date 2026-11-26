//! Project commands' operands from the command-line batch (#2943).

use super::{Operand, OperandKind, optional, required};

/// `project_set_rail` (#2943): the rail `net`'s voltage and tolerance as
/// typed (an empty tolerance clears it), or `remove` to take it out.
pub const PROJECT_SET_RAIL: &[Operand] = &[
    required("document", OperandKind::Document),
    required("net", OperandKind::Text),
    optional("voltage", OperandKind::Text),
    optional("tolerance_percent", OperandKind::Text),
    optional("remove", OperandKind::Bool),
];
