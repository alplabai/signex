//! Which document a sketch belongs to, and what that host offers (#2518).
//!
//! One sketch editor serves two hosts: a footprint and a board. The tools,
//! constraints, DOF row and parameters are the same on both; the roles an
//! entity can take, the fields a bake writes and a few actions differ. This
//! module is the one place that says how. See `docs/PCB_SKETCH.md`, "Hosts".

use crate::sketch::baked_kinds::BakedKind;
use crate::sketch::entity::EntityKind;

/// The document a sketch is edited for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SketchHost {
    /// A footprint's sketch: it bakes into the footprint's own fields.
    Footprint,
    /// A board's sketch: it bakes into the board outline and features.
    Board,
}

/// What a sketch entity becomes when the sketch bakes: the value of the
/// role picker. Each role but [`SketchRole::Unassigned`] writes one
/// [`BakedKind`] (see [`SketchRole::target`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SketchRole {
    /// No role: the entity is drawing aid only.
    Unassigned,
    /// A pad. A point places it; on a line the picker means "make this
    /// loop a pad".
    Pad,
    SilkTop,
    SilkBottom,
    Courtyard,
    Keepout,
    /// A board cutout through the board.
    Cutout,
    MaskOpeningTop,
    MaskOpeningBottom,
    MaskExcludeTop,
    MaskExcludeBottom,
    PourTop,
    PourBottom,
    PasteApertureTop,
    PasteApertureBottom,
    /// An edge of the board outline.
    BoardOutline,
    /// A circle that is a mounting hole.
    MountingHole,
    /// A line that is a V-score path.
    VScore,
}

/// An action of the sketch editor that only some hosts offer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SketchAction {
    /// Modify ▸ Make Pad from Profile: a board has no pads.
    MakePadFromProfile,
}

impl SketchHost {
    /// The roles the role picker offers, in the picker's order.
    pub const fn roles(self) -> &'static [SketchRole] {
        match self {
            Self::Footprint => FOOTPRINT_ROLES,
            Self::Board => BOARD_ROLES,
        }
    }

    /// The fields a bake on this host writes, in display order. A board
    /// lists only what `signex_bake::bake_board` produces today.
    pub const fn bake_targets(self) -> &'static [BakedKind] {
        match self {
            Self::Footprint => FOOTPRINT_TARGETS,
            Self::Board => BOARD_TARGETS,
        }
    }

    /// Whether this host offers `action`.
    pub const fn offers(self, action: SketchAction) -> bool {
        match action {
            SketchAction::MakePadFromProfile => matches!(self, Self::Footprint),
        }
    }

    /// Whether the picker on this host offers `role`.
    pub fn offers_role(self, role: SketchRole) -> bool {
        self.roles().contains(&role)
    }
}

const FOOTPRINT_ROLES: &[SketchRole] = &[
    SketchRole::Unassigned,
    SketchRole::Pad,
    SketchRole::SilkTop,
    SketchRole::SilkBottom,
    SketchRole::Courtyard,
    SketchRole::Keepout,
    SketchRole::Cutout,
    SketchRole::MaskOpeningTop,
    SketchRole::MaskOpeningBottom,
    SketchRole::MaskExcludeTop,
    SketchRole::MaskExcludeBottom,
    SketchRole::PourTop,
    SketchRole::PourBottom,
    SketchRole::PasteApertureTop,
    SketchRole::PasteApertureBottom,
];

/// The board picker's order follows the board sketch design: outline,
/// cutout, keepout, V-score, mounting hole.
const BOARD_ROLES: &[SketchRole] = &[
    SketchRole::Unassigned,
    SketchRole::BoardOutline,
    SketchRole::Cutout,
    SketchRole::Keepout,
    SketchRole::VScore,
    SketchRole::MountingHole,
];

const FOOTPRINT_TARGETS: &[BakedKind] = &[
    BakedKind::Pads,
    BakedKind::SilkF,
    BakedKind::SilkB,
    BakedKind::Courtyard,
    BakedKind::MaskOpenings,
    BakedKind::MaskExcludes,
    BakedKind::PasteApertures,
    BakedKind::Pours,
    BakedKind::Keepouts,
    BakedKind::Cutouts,
    BakedKind::VScores,
];

const BOARD_TARGETS: &[BakedKind] = &[
    BakedKind::BoardOutline,
    BakedKind::Cutouts,
    BakedKind::Keepouts,
    BakedKind::VScores,
    BakedKind::MountingHoles,
];

impl SketchRole {
    /// The field this role bakes into; `None` for [`Self::Unassigned`].
    pub const fn target(self) -> Option<BakedKind> {
        Some(match self {
            Self::Unassigned => return None,
            Self::Pad => BakedKind::Pads,
            Self::SilkTop => BakedKind::SilkF,
            Self::SilkBottom => BakedKind::SilkB,
            Self::Courtyard => BakedKind::Courtyard,
            Self::Keepout => BakedKind::Keepouts,
            Self::Cutout => BakedKind::Cutouts,
            Self::MaskOpeningTop | Self::MaskOpeningBottom => BakedKind::MaskOpenings,
            Self::MaskExcludeTop | Self::MaskExcludeBottom => BakedKind::MaskExcludes,
            Self::PourTop | Self::PourBottom => BakedKind::Pours,
            Self::PasteApertureTop | Self::PasteApertureBottom => BakedKind::PasteApertures,
            Self::BoardOutline => BakedKind::BoardOutline,
            Self::MountingHole => BakedKind::MountingHoles,
            Self::VScore => BakedKind::VScores,
        })
    }

    /// Whether an entity of `kind` can take this role. The board roles
    /// need their shape (an outline an edge, a hole a circle, a V-score a
    /// line); a footprint role takes any entity, as the picker always did
    /// (Pad on a line is the make-a-pad shorthand).
    pub fn fits(self, kind: &EntityKind) -> bool {
        match self {
            Self::BoardOutline => !matches!(kind, EntityKind::Point { .. }),
            Self::MountingHole => matches!(kind, EntityKind::Circle { .. }),
            Self::VScore => matches!(kind, EntityKind::Line { .. }),
            _ => true,
        }
    }
}

#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;
