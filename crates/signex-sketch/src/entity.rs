use serde::{Deserialize, Serialize};

use crate::attr::{
    BoardCutoutAttr, CourtyardAttr, KeepoutAttr, MaskExcludeAttr, MaskOpeningAttr, PadAttr,
    PasteApertureAttr, PourAttr, SilkAttr, VScoreHintAttr,
};
use crate::id::SketchEntityId;
use crate::plane::PlaneId;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    pub id: SketchEntityId,
    pub plane: PlaneId,
    #[serde(default)]
    pub construction: bool,
    #[serde(flatten)]
    pub kind: EntityKind,

    // ─── Bake attributes ───
    // Only entities that produce baked output carry these. v0.13
    // honours `pad` (including kind=Fiducial); the rest round-trip
    // only and bake in v0.14 (silk/courtyard/mask/paste/pour/keepout-
    // boundary) or v0.15 (pour-fill, keepout-DRC).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pad: Option<PadAttr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub silk: Option<SilkAttr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub courtyard: Option<CourtyardAttr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask_opening: Option<MaskOpeningAttr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask_exclude: Option<MaskExcludeAttr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paste_aperture: Option<PasteApertureAttr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pour: Option<PourAttr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keepout: Option<KeepoutAttr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board_cutout: Option<BoardCutoutAttr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v_score: Option<VScoreHintAttr>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "PascalCase")]
pub enum EntityKind {
    /// Point in plane-local coordinates (mm).
    Point { x: f64, y: f64 },

    /// Line — both endpoints reference Point entities by ID.
    Line {
        start: SketchEntityId,
        end: SketchEntityId,
    },

    /// Arc — center is a Point, start/end are Points.
    /// `sweep_ccw = true` means CCW from start to end.
    Arc {
        center: SketchEntityId,
        start: SketchEntityId,
        end: SketchEntityId,
        #[serde(default = "default_sweep_ccw")]
        sweep_ccw: bool,
    },

    /// Circle — center is a Point, radius is a literal.
    Circle { center: SketchEntityId, radius: f64 },
}

fn default_sweep_ccw() -> bool {
    true
}

impl Entity {
    /// Construct a bare entity with no bake attributes attached.
    pub fn new(id: SketchEntityId, plane: PlaneId, kind: EntityKind) -> Self {
        Self {
            id,
            plane,
            construction: false,
            kind,
            pad: None,
            silk: None,
            courtyard: None,
            mask_opening: None,
            mask_exclude: None,
            paste_aperture: None,
            pour: None,
            keepout: None,
            board_cutout: None,
            v_score: None,
        }
    }

    /// Point endpoints reachable from this entity. Used by the
    /// solver to discover entity → state-vector mapping.
    pub fn point_refs(&self) -> Vec<SketchEntityId> {
        match self.kind {
            EntityKind::Point { .. } => vec![self.id],
            EntityKind::Line { start, end } => vec![start, end],
            EntityKind::Arc {
                center, start, end, ..
            } => vec![center, start, end],
            EntityKind::Circle { center, .. } => vec![center],
        }
    }
}
