//! The sketch fixes the Properties panel offers in the editor in view:
//! Remove and Convert to reference on a conflict row (Caner 2026-11-20),
//! and Close gap on an open outline's check line (mockup row 190).

use signex_sketch::id::{ConstraintId, SketchEntityId};

/// The user picked a fix for the sketch in view.
#[derive(Debug, Clone, PartialEq)]
pub enum SketchFixMsg {
    /// Remove on a conflict row: delete that constraint, one undo step.
    ConflictRemoveClicked { constraint: ConstraintId },
    /// Convert to reference on a conflict row of a dimension (mockup row
    /// 187, item E4): that dimension measures from now on, one undo step.
    ConflictConvertClicked(ConstraintId),
    /// Close gap on an open outline's check line: join the two open ends,
    /// one undo step.
    CloseGapClicked {
        keep: SketchEntityId,
        merge: SketchEntityId,
    },
    /// Add fillets on the failing inside corners (mockup row 179d), one undo
    /// step.
    AddFilletsClicked,
    /// A role picked on a row of the Roles section (mockup row 190): the
    /// closed profile through `seed` takes it, one undo step.
    ProfileRolePicked {
        seed: SketchEntityId,
        role: crate::library::messages::RoleTag,
    },
    /// Bake preview in the Roles section.
    BakePreviewClicked,
    /// Bake in the Roles section.
    BakeClicked,
    /// Edit in Sketch on the Bake outcome (mockup row 190).
    EditInSketchClicked,
}
