//! Editor-side `Pad` mirror, pad-stack overrides, side enum, and
//! courtyard rect. Pure data types — no canvas state, no tool state.

use signex_library::{LayerId, Pad, PadKind, PadShape};
use signex_sketch::attr::{ElectricalType, PadFeature, TestpointFlags};

use super::super::layers::FpLayer;

/// Default new-pad size in mm.
pub(super) const NEW_PAD_SIZE_MM: f64 = 1.0;

/// One pad in the editor canvas. A subset of [`signex_library::Pad`] —
/// we only carry the fields the canvas renders or hit-tests. Extra
/// fields on `Pad` (drill, mask/paste margins, etc.) round-trip via
/// [`super::FootprintEditorState::sync_pads_to_primitive`] without a UI yet.
#[derive(Debug, Clone, PartialEq)]
pub struct EditorPad {
    pub number: String,
    pub position_mm: (f64, f64),
    pub size_mm: (f64, f64),
    pub kind: PadKind,
    pub shape: PadShape,
    /// Layers the pad lives on — first entry is treated as the
    /// primary layer for hit-test/visibility gating.
    pub layers: Vec<LayerId>,
    /// v0.15 — bidirectional sketch ↔ pads link.
    pub sketch_entity_id: Option<signex_sketch::id::SketchEntityId>,
    /// v0.16 — outline-corner Points minted when the pad enters Sketch
    /// mode. Order: `[ne, se, sw, nw]`. Construction-flagged.
    pub corner_entity_ids: Option<[signex_sketch::id::SketchEntityId; 4]>,
    /// v0.16.6 — pad rotation in degrees.
    pub rotation_deg: f64,
    /// v0.18.12 — drill diameter (mm) for through-hole / NPT pads.
    pub drill_diameter_mm: Option<f64>,
    /// v0.20 — Altium-parity pad-stack overrides.
    pub stack: PadStackUi,
    /// v0.20 — top-side surface treatment.
    pub feature_top: PadFeature,
    /// v0.20 — bottom-side surface treatment.
    pub feature_bottom: PadFeature,
    /// v0.20 — test-point participation flags.
    pub testpoint: TestpointFlags,
    /// v0.20 — pad-template name. Empty = no template.
    pub template: String,
    /// v0.20 — pad-template library reference. Empty = local.
    pub template_library: String,
    /// v0.20 — Altium-parity electrical-type.
    pub electrical_type: ElectricalType,
    /// v0.20 — net assignment. Empty = unassigned.
    pub net: String,
    /// v0.20 — locked flag.
    pub locked: bool,
    /// v0.20 — Pad Hole tolerance ± (mm).
    pub hole_tolerance_plus_mm: Option<f64>,
    pub hole_tolerance_minus_mm: Option<f64>,
    /// v0.20 — Pad Hole rotation (Slot/Rectangular orientation).
    pub hole_rotation_deg: Option<f64>,
    /// v0.20 — Copper offset relative to hole centre.
    pub copper_offset_x_mm: Option<f64>,
    pub copper_offset_y_mm: Option<f64>,
    /// v0.24 Phase 1 (Track A stub) — Per-pad parametric handles.
    pub shape_params: ShapeParamMap,
}

/// v0.24 Phase 1 — per-pad parametric handle map. Type alias keeps
/// the field flexible for Phase 2 to swap in a dedicated struct if
/// the linked/unlinked semantics get richer.
pub type ShapeParamMap = std::collections::HashMap<String, String>;

/// v0.20 — UI-side mirror of `Pad`'s pad-stack override fields. All
/// values in mm (already evaluated). `None` on a margin override
/// means "use the rule-driven / global value"; `true` on a tented
/// flag means "skip the mask opening on that side".
#[derive(Debug, Clone, PartialEq)]
pub struct PadStackUi {
    pub paste_margin_top: Option<f64>,
    pub paste_margin_bottom: Option<f64>,
    pub paste_enabled_top: bool,
    pub paste_enabled_bottom: bool,
    pub mask_margin_top: Option<f64>,
    pub mask_margin_bottom: Option<f64>,
    pub mask_tented_top: bool,
    pub mask_tented_bottom: bool,
    pub thermal_relief: bool,
    pub corner_radius_pct: Option<f64>,
}

impl Default for PadStackUi {
    fn default() -> Self {
        Self {
            paste_margin_top: None,
            paste_margin_bottom: None,
            paste_enabled_top: true,
            paste_enabled_bottom: true,
            mask_margin_top: None,
            mask_margin_bottom: None,
            mask_tented_top: false,
            mask_tented_bottom: false,
            thermal_relief: true,
            corner_radius_pct: None,
        }
    }
}

impl EditorPad {
    pub fn new_default(number: String, position_mm: (f64, f64)) -> Self {
        Self {
            number,
            position_mm,
            size_mm: (NEW_PAD_SIZE_MM, NEW_PAD_SIZE_MM),
            kind: PadKind::Smd,
            shape: PadShape::Rect,
            layers: vec![
                LayerId::new("F.Cu"),
                LayerId::new("F.Mask"),
                LayerId::new("F.Paste"),
            ],
            sketch_entity_id: None,
            corner_entity_ids: None,
            rotation_deg: 0.0,
            drill_diameter_mm: None,
            stack: PadStackUi::default(),
            feature_top: PadFeature::None,
            feature_bottom: PadFeature::None,
            testpoint: TestpointFlags::default(),
            template: String::new(),
            template_library: String::new(),
            electrical_type: ElectricalType::Load,
            net: String::new(),
            locked: false,
            hole_tolerance_plus_mm: None,
            hole_tolerance_minus_mm: None,
            hole_rotation_deg: None,
            copper_offset_x_mm: None,
            copper_offset_y_mm: None,
            shape_params: ShapeParamMap::new(),
        }
    }

    /// v0.18.12 — non-plated through hole. No copper / mask / paste
    /// layers; the drill is the visible footprint feature.
    pub fn new_npt_hole(number: String, position_mm: (f64, f64), drill_mm: f64) -> Self {
        let d = drill_mm.max(0.05);
        Self {
            number,
            position_mm,
            size_mm: (d, d),
            kind: PadKind::NptHole,
            shape: PadShape::Round,
            layers: Vec::new(),
            sketch_entity_id: None,
            corner_entity_ids: None,
            rotation_deg: 0.0,
            drill_diameter_mm: Some(d),
            stack: PadStackUi::default(),
            feature_top: PadFeature::None,
            feature_bottom: PadFeature::None,
            testpoint: TestpointFlags::default(),
            template: String::new(),
            template_library: String::new(),
            electrical_type: ElectricalType::Load,
            net: String::new(),
            locked: false,
            hole_tolerance_plus_mm: None,
            hole_tolerance_minus_mm: None,
            hole_rotation_deg: None,
            copper_offset_x_mm: None,
            copper_offset_y_mm: None,
            shape_params: ShapeParamMap::new(),
        }
    }

    /// Layer the pad lives on for hit-testing / toggle gating.
    pub fn primary_layer(&self) -> FpLayer {
        self.layers
            .first()
            .and_then(|name| FpLayer::from_standard_name(name.as_str()))
            .unwrap_or(FpLayer::FCu)
    }

    /// Bounding box (min_x, min_y, max_x, max_y) in mm.
    pub fn bbox_mm(&self) -> (f64, f64, f64, f64) {
        let (cx, cy) = self.position_mm;
        let (w, h) = self.size_mm;
        (cx - w / 2.0, cy - h / 2.0, cx + w / 2.0, cy + h / 2.0)
    }

    /// AABB containment check.
    pub fn contains_mm(&self, x: f64, y: f64) -> bool {
        let (xmin, ymin, xmax, ymax) = self.bbox_mm();
        x >= xmin && x <= xmax && y >= ymin && y <= ymax
    }

    pub(super) fn from_pad(p: &Pad) -> Self {
        Self {
            number: p.number.clone(),
            position_mm: (p.position[0], p.position[1]),
            size_mm: (p.size[0], p.size[1]),
            kind: p.kind,
            shape: p.shape.clone(),
            layers: p.layers.clone(),
            sketch_entity_id: None,
            corner_entity_ids: None,
            rotation_deg: p.rotation,
            drill_diameter_mm: p.drill.as_ref().map(|d| d.diameter),
            stack: PadStackUi {
                paste_margin_top: p.paste_margin_top,
                paste_margin_bottom: p.paste_margin_bottom,
                paste_enabled_top: p.paste_enabled_top,
                paste_enabled_bottom: p.paste_enabled_bottom,
                mask_margin_top: p.mask_margin_top,
                mask_margin_bottom: p.mask_margin_bottom,
                mask_tented_top: p.mask_tented_top,
                mask_tented_bottom: p.mask_tented_bottom,
                thermal_relief: p.thermal_relief,
                corner_radius_pct: p.corner_radius_pct,
            },
            feature_top: p.feature_top,
            feature_bottom: p.feature_bottom,
            testpoint: p.testpoint,
            template: p.template.clone(),
            template_library: p.template_library.clone(),
            electrical_type: p.electrical_type,
            net: p.net.clone(),
            locked: p.locked,
            hole_tolerance_plus_mm: p.hole_tolerance_plus_mm,
            hole_tolerance_minus_mm: p.hole_tolerance_minus_mm,
            hole_rotation_deg: p.hole_rotation_deg,
            copper_offset_x_mm: p.copper_offset_x_mm,
            copper_offset_y_mm: p.copper_offset_y_mm,
            shape_params: ShapeParamMap::new(),
        }
    }

    pub(super) fn to_pad(&self) -> Pad {
        let drill = self.drill_diameter_mm.map(|d| signex_library::Drill {
            diameter: d,
            slot_length: None,
        });
        Pad {
            number: self.number.clone(),
            kind: self.kind,
            shape: self.shape.clone(),
            size: [self.size_mm.0, self.size_mm.1],
            position: [self.position_mm.0, self.position_mm.1],
            rotation: self.rotation_deg,
            layers: self.layers.clone(),
            drill,
            solder_mask_margin: None,
            paste_margin: None,
            template: self.template.clone(),
            template_library: self.template_library.clone(),
            paste_margin_top: self.stack.paste_margin_top,
            paste_margin_bottom: self.stack.paste_margin_bottom,
            paste_enabled_top: self.stack.paste_enabled_top,
            paste_enabled_bottom: self.stack.paste_enabled_bottom,
            mask_margin_top: self.stack.mask_margin_top,
            mask_margin_bottom: self.stack.mask_margin_bottom,
            mask_tented_top: self.stack.mask_tented_top,
            mask_tented_bottom: self.stack.mask_tented_bottom,
            thermal_relief: self.stack.thermal_relief,
            corner_radius_pct: self.stack.corner_radius_pct,
            feature_top: self.feature_top,
            feature_bottom: self.feature_bottom,
            testpoint: self.testpoint,
            electrical_type: self.electrical_type,
            net: self.net.clone(),
            locked: self.locked,
            hole_tolerance_plus_mm: self.hole_tolerance_plus_mm,
            hole_tolerance_minus_mm: self.hole_tolerance_minus_mm,
            hole_rotation_deg: self.hole_rotation_deg,
            copper_offset_x_mm: self.copper_offset_x_mm,
            copper_offset_y_mm: self.copper_offset_y_mm,
        }
    }
}

/// Pad copper side mirror — UI-side label-bearing enum. The sketch
/// crate has the same shape at `signex_sketch::attr::PadSide`; this
/// type wraps it for the app's panel/dispatcher boundary so the panel
/// doesn't pull in the sketch crate's constraint-residual surface.
///
/// HI-24: variants MUST stay in lockstep with `signex_sketch::attr::PadSide`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PadSide {
    #[default]
    Top,
    Bottom,
    All,
}

impl PadSide {
    pub const ALL_OPTIONS: &'static [PadSide] = &[PadSide::Top, PadSide::Bottom, PadSide::All];
    pub fn label(self) -> &'static str {
        match self {
            PadSide::Top => "Top Layer",
            PadSide::Bottom => "Bottom Layer",
            PadSide::All => "Multi-Layer",
        }
    }
}

impl std::fmt::Display for PadSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

impl From<signex_sketch::attr::PadSide> for PadSide {
    fn from(value: signex_sketch::attr::PadSide) -> Self {
        match value {
            signex_sketch::attr::PadSide::Top => PadSide::Top,
            signex_sketch::attr::PadSide::Bottom => PadSide::Bottom,
            signex_sketch::attr::PadSide::All => PadSide::All,
        }
    }
}

impl From<PadSide> for signex_sketch::attr::PadSide {
    fn from(value: PadSide) -> Self {
        match value {
            PadSide::Top => signex_sketch::attr::PadSide::Top,
            PadSide::Bottom => signex_sketch::attr::PadSide::Bottom,
            PadSide::All => signex_sketch::attr::PadSide::All,
        }
    }
}

/// v0.16.3 — author-controlled defaults for the next placed pad.
#[derive(Debug, Clone, PartialEq)]
pub struct NextPadDefaults {
    pub designator_override: Option<String>,
    pub size_x_mm: f64,
    pub size_y_mm: f64,
    pub side: PadSide,
    pub rotation_deg: f64,
    pub stack: PadStackUi,
    pub feature_top: PadFeature,
    pub feature_bottom: PadFeature,
    pub testpoint: TestpointFlags,
    pub template: String,
    pub template_library: String,
    pub drill_diameter_mm: Option<f64>,
    pub drill_slot_length_mm: Option<f64>,
    pub shape: signex_library::PadShape,
    pub kind: signex_library::PadKind,
    pub electrical_type: ElectricalType,
    pub net: String,
    pub locked: bool,
    pub hole_tolerance_plus_mm: Option<f64>,
    pub hole_tolerance_minus_mm: Option<f64>,
    pub hole_rotation_deg: Option<f64>,
    pub copper_offset_x_mm: Option<f64>,
    pub copper_offset_y_mm: Option<f64>,
}

impl Default for NextPadDefaults {
    fn default() -> Self {
        Self {
            designator_override: None,
            size_x_mm: NEW_PAD_SIZE_MM,
            size_y_mm: NEW_PAD_SIZE_MM,
            side: PadSide::Top,
            rotation_deg: 0.0,
            stack: PadStackUi::default(),
            feature_top: PadFeature::None,
            feature_bottom: PadFeature::None,
            testpoint: TestpointFlags::default(),
            template: String::new(),
            template_library: String::new(),
            drill_diameter_mm: None,
            drill_slot_length_mm: None,
            shape: signex_library::PadShape::Rect,
            kind: signex_library::PadKind::Smd,
            electrical_type: ElectricalType::Load,
            net: String::new(),
            locked: false,
            hole_tolerance_plus_mm: None,
            hole_tolerance_minus_mm: None,
            hole_rotation_deg: None,
            copper_offset_x_mm: None,
            copper_offset_y_mm: None,
        }
    }
}

/// Auto-fit courtyard rectangle in mm. Built by
/// [`super::FootprintEditorState::recompute_courtyard`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CourtyardRect {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

/// v0.14 — active-bar Align / Distribute / Spacing operations. Carried
/// by [`crate::library::messages::FootprintEditorMsg::AlignPads`].
/// Pure data — the geometry lives in the dispatcher's `align_pads`
/// helper. Align variants act on ≥2 selected pads; the two Distribute
/// variants need ≥3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignOp {
    /// Move every selected pad so its centre sits at the minimum X
    /// (leftmost centre) among the selection.
    Left,
    /// Centres → maximum X (rightmost centre).
    Right,
    /// Centres → minimum Y (topmost centre, screen-down +Y).
    Top,
    /// Centres → maximum Y (bottommost centre).
    Bottom,
    /// Centres → mean X of the selection (horizontal centring).
    CenterH,
    /// Centres → mean Y of the selection (vertical centring).
    CenterV,
    /// Keep the extreme-X pads fixed; space the in-between pads at
    /// equal centre-to-centre gaps (Altium "Distribute Horizontally").
    DistributeH,
    /// Keep the extreme-Y pads fixed; equalise vertical gaps.
    DistributeV,
    /// Expand horizontal centre-to-centre gaps by one grid step,
    /// pivoting about the selection's mean X.
    IncreaseHSpacing,
    /// Contract horizontal gaps by one grid step (never past overlap).
    DecreaseHSpacing,
    /// Expand vertical gaps by one grid step about the mean Y.
    IncreaseVSpacing,
    /// Contract vertical gaps by one grid step.
    DecreaseVSpacing,
}
