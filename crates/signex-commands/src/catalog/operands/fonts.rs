//! Operands of the Project tab's Fonts commands (#1178, mockup row 205).

use super::{Operand, OperandKind, required};

/// `project_add_font`: the font file at `path`, copied into the project's
/// `assets/fonts/` and listed under its families.
pub const PROJECT_ADD_FONT: &[Operand] = &[
    required("document", OperandKind::Document),
    required("path", OperandKind::Path),
];

/// `project_remove_font`: the project font by its `family` name.
pub const PROJECT_FONT: &[Operand] = &[
    required("document", OperandKind::Document),
    required("family", OperandKind::Text),
];

/// `project_set_font_license`: the `family`'s license as typed in `value`
/// (an SPDX id or a license file name); empty clears it.
pub const PROJECT_FONT_LICENSE: &[Operand] = &[
    required("document", OperandKind::Document),
    required("family", OperandKind::Text),
    required("value", OperandKind::Text),
];
