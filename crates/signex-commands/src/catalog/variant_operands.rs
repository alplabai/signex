//! The operands of the Variant Manager's part edits (#3038). `document` is
//! the project or any of its documents; a part is named by `designator` as
//! the Variant Manager lists it (`CH2_R5` for a channel's copy).

use super::operands::{Operand, OperandKind, optional, required};

/// `project_set_variant_fit`: the part fitted (`true`) or not in `variant`;
/// no `fitted` is as on its sheets. `scope` is `part` (the default) or
/// `channels`, every channel's copy of a part on a repeated sheet.
pub const PROJECT_VARIANT_FIT: &[Operand] = &[
    required("document", OperandKind::Document),
    required("variant", OperandKind::Text),
    required("designator", OperandKind::Text),
    optional("fitted", OperandKind::Bool),
    optional("scope", OperandKind::Text),
];
/// `project_set_variant_alternate`: the part in `variant` replaced by the
/// part numbered `mpn`, with its `value` and `footprint`; none is the
/// part's own again.
pub const PROJECT_VARIANT_ALTERNATE: &[Operand] = &[
    required("document", OperandKind::Document),
    required("variant", OperandKind::Text),
    required("designator", OperandKind::Text),
    optional("mpn", OperandKind::Text),
    optional("value", OperandKind::Text),
    optional("footprint", OperandKind::Text),
];
/// `project_set_variant_override`: the part's `field` (`Value`, `Footprint`
/// or a field name) in `variant`; no `value` is the part's own again.
pub const PROJECT_VARIANT_OVERRIDE: &[Operand] = &[
    required("document", OperandKind::Document),
    required("variant", OperandKind::Text),
    required("designator", OperandKind::Text),
    required("field", OperandKind::Text),
    optional("value", OperandKind::Text),
];
/// `project_duplicate_variant`: a copy of the variant `name` (`Base` copies
/// Base) named `value`, by default the first free "<name> copy".
pub const PROJECT_DUPLICATE_VARIANT: &[Operand] = &[
    required("document", OperandKind::Document),
    required("name", OperandKind::Text),
    optional("value", OperandKind::Text),
];
