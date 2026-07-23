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

    /// Un-rotated, axis-aligned half-extent box (min_x, min_y, max_x,
    /// max_y) in mm.
    ///
    /// This is the PAD-LOCAL frame — `rotation_deg` is deliberately
    /// ignored. Only callers that reason in the pad's own frame want
    /// this (the chamfer / round-rect anchor derivation in
    /// `pad_to_sketch::solve`). Anything asking "where does this pad
    /// actually sit on the board" wants [`Self::rotated_aabb_mm`] or
    /// [`Self::rotated_corners_mm`] instead — reading the un-rotated
    /// box is what left hit-test, courtyard, rubber-band and the pad
    /// renderer all disagreeing with the drawn copper.
    pub fn bbox_mm(&self) -> (f64, f64, f64, f64) {
        let (cx, cy) = self.position_mm;
        let (w, h) = self.size_mm;
        (cx - w / 2.0, cy - h / 2.0, cx + w / 2.0, cy + h / 2.0)
    }

    /// Rotate a free VECTOR (a delta — no translation applied) from the
    /// pad's own frame into world mm.
    pub fn rotate_delta_to_world_mm(&self, dx: f64, dy: f64) -> (f64, f64) {
        if self.rotation_deg == 0.0 {
            return (dx, dy);
        }
        let (sin, cos) = self.rotation_deg.to_radians().sin_cos();
        (dx * cos - dy * sin, dx * sin + dy * cos)
    }

    /// Inverse of [`Self::rotate_delta_to_world_mm`].
    pub fn rotate_delta_to_local_mm(&self, dx: f64, dy: f64) -> (f64, f64) {
        if self.rotation_deg == 0.0 {
            return (dx, dy);
        }
        let (sin, cos) = self.rotation_deg.to_radians().sin_cos();
        (dx * cos + dy * sin, -dx * sin + dy * cos)
    }

    /// Map a POINT given in the pad's own frame — the frame
    /// [`Self::bbox_mm`] is expressed in — into world mm.
    ///
    /// Every position derived off `bbox_mm` (round-rect arc anchors,
    /// chamfer anchors, oval arc centres, the resized-edge corner
    /// targets) has to come back through here, or the derived geometry
    /// stays axis-aligned while the corners it is supposed to join turn
    /// with the pad, and the outline no longer closes.
    pub fn local_to_world_mm(&self, x: f64, y: f64) -> (f64, f64) {
        let (cx, cy) = self.position_mm;
        let (dx, dy) = self.rotate_delta_to_world_mm(x - cx, y - cy);
        (cx + dx, cy + dy)
    }

    /// Inverse of [`Self::local_to_world_mm`] — takes a world point into
    /// the pad's own frame, where the axis-aligned reasoning that
    /// `bbox_mm` supports is valid again.
    pub fn world_to_local_mm(&self, x: f64, y: f64) -> (f64, f64) {
        let (cx, cy) = self.position_mm;
        let (dx, dy) = self.rotate_delta_to_local_mm(x - cx, y - cy);
        (cx + dx, cy + dy)
    }

    /// The four half-extent corners rotated about `position_mm` by
    /// `rotation_deg`, in `[ne, se, sw, nw]` order — the order the
    /// sketch-mirror corner code already assumes.
    pub fn rotated_corners_mm(&self) -> [(f64, f64); 4] {
        let (xmin, ymin, xmax, ymax) = self.bbox_mm();
        // [ne, se, sw, nw].
        [(xmax, ymin), (xmax, ymax), (xmin, ymax), (xmin, ymin)]
            .map(|(x, y)| self.local_to_world_mm(x, y))
    }

    /// Axis-aligned bounding box of the ROTATED pad, in mm. Equals
    /// [`Self::bbox_mm`] at zero rotation and grows to enclose the
    /// turned copper otherwise.
    pub fn rotated_aabb_mm(&self) -> (f64, f64, f64, f64) {
        if self.rotation_deg == 0.0 {
            return self.bbox_mm();
        }
        let corners = self.rotated_corners_mm();
        corners.iter().skip(1).fold(
            (corners[0].0, corners[0].1, corners[0].0, corners[0].1),
            |(x0, y0, x1, y1), &(x, y)| (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
        )
    }

    /// Mirror every mirror-sensitive field about the pad's OWN
    /// vertical axis (local `x → -x`). This is what moving a pad to
    /// the other side of the board does to its copper.
    ///
    /// `signex_bake::pad` consumes each of these verbatim with no
    /// side-based mirroring of its own, so the stored data IS the
    /// baked geometry. Mirroring only a subset bakes a shape that is
    /// neither the front nor the back one — a Chamfered pad flipped
    /// with its angle negated but its corner flags left alone keeps
    /// the chamfer on the wrong corner and the part will not seat.
    /// Every field that changes under `x → -x` therefore moves here
    /// together, or none of them do.
    ///
    /// Pad POSITION is deliberately untouched: this mirrors each pad
    /// in place. Mirroring the layout about the footprint origin is a
    /// different operation and does not exist yet.
    pub fn mirror_about_own_vertical_axis(&mut self) {
        self.rotation_deg = (-self.rotation_deg).rem_euclid(360.0);
        self.hole_rotation_deg = self.hole_rotation_deg.map(|d| (-d).rem_euclid(360.0));
        self.copper_offset_x_mm = self.copper_offset_x_mm.map(|v| -v);
        match &mut self.shape {
            PadShape::Chamfered { corners, .. } => {
                std::mem::swap(&mut corners.top_left, &mut corners.top_right);
                std::mem::swap(&mut corners.bottom_left, &mut corners.bottom_right);
            }
            PadShape::Custom(poly) => {
                for p in poly.points.iter_mut() {
                    p[0] = -p[0];
                }
            }
            // Round / Rect / RoundRect / Oval are symmetric about
            // their own vertical axis — nothing to mirror.
            PadShape::Round | PadShape::Rect | PadShape::RoundRect { .. } | PadShape::Oval => {}
        }
    }

    /// Point-in-pad containment, rotation-aware. Inverse-rotates the
    /// probe into the pad's own frame and compares against the half
    /// extents, so a turned pad is hit on its real copper rather than
    /// on the axis-aligned box it would occupy unrotated.
    pub fn contains_mm(&self, x: f64, y: f64) -> bool {
        let (cx, cy) = self.position_mm;
        let (hw, hh) = (self.size_mm.0 / 2.0, self.size_mm.1 / 2.0);
        let (lx, ly) = self.rotate_delta_to_local_mm(x - cx, y - cy);
        lx.abs() <= hw && ly.abs() <= hh
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

/// Carry the three session-volatile link fields from the pre-refresh
/// pad list onto the freshly-rebuilt one, matching by pad number.
///
/// Only where the number identifies exactly ONE pad on each side.
/// Numbers are not unique in signex, and a last-wins number map hands
/// several pads the same `sketch_entity_id` — after which a Pads-mode
/// delete of one pad runs the delete mirror over another's geometry
/// and that pad's copper silently disappears from the bake. An
/// ambiguous number is left unlinked here and picked up by
/// [`relink_pads_to_sketch`], which disambiguates by exact position
/// and refuses if even that ties.
pub(super) fn carry_links_by_unique_number(old: &[EditorPad], new_pads: &mut [EditorPad]) {
    use std::collections::HashMap;
    type Link<'a> = (
        Option<signex_sketch::id::SketchEntityId>,
        Option<[signex_sketch::id::SketchEntityId; 4]>,
        &'a ShapeParamMap,
    );

    // `None` = the number is claimed by more than one old pad.
    let mut old_links: HashMap<&str, Option<Link<'_>>> = HashMap::new();
    for p in old {
        old_links
            .entry(p.number.as_str())
            .and_modify(|slot| *slot = None)
            .or_insert(Some((
                p.sketch_entity_id,
                p.corner_entity_ids,
                &p.shape_params,
            )));
    }
    let mut new_counts: HashMap<&str, usize> = HashMap::new();
    for p in new_pads.iter() {
        *new_counts.entry(p.number.as_str()).or_default() += 1;
    }
    // Resolved up front: `new_counts` borrows `new_pads` immutably.
    let resolved: Vec<Option<Link<'_>>> = new_pads
        .iter()
        .map(|p| match new_counts.get(p.number.as_str()) {
            Some(1) => old_links.get(p.number.as_str()).copied().flatten(),
            _ => None,
        })
        .collect();
    for (p, link) in new_pads.iter_mut().zip(resolved) {
        if let Some((sid, cids, params)) = link {
            p.sketch_entity_id = sid;
            p.corner_entity_ids = cids;
            p.shape_params = params.clone();
        }
    }
}

/// Re-attach each pad's `sketch_entity_id` from the sketch itself, by
/// matching the `PadAttr.number` the centre `Point` carries.
///
/// `EditorPad::from_pad` cannot restore it — the link has no home on
/// `Pad`, so every pad rebuilt from the primitive comes back with
/// `sketch_entity_id: None`. That is silent data loss on the next
/// edit, not a cosmetic gap: `mirror_move_pad_in_sketch` and
/// `mirror_delete_pad_from_sketch` both early-return on a `None` link,
/// so after a save + reopen a Pads-mode move left the pad's whole
/// outline stranded at its old position (the bake then emits copper
/// from the stranded geometry) and a Pads-mode delete left the outline
/// AND its `PadAttr`-carrying centre behind, resurrecting the deleted
/// pad on the next bake.
///
/// The sketch is the durable side of the link, so it is the side the
/// link is rebuilt from. Shared by [`super::FootprintEditorState::from_footprint`]
/// (open / reopen) and `refresh_pads_from_primitive` (post-bake
/// refresh) so the two loaders cannot drift apart.
///
/// # Pad numbers are NOT unique
///
/// Nothing in signex enforces a unique pad number — the Properties
/// field takes any string and `next_pad_defaults.designator_override`
/// stamps one number onto every pad placed after it, which is how a
/// shared-designator row / thermal / shield pad set is authored. Each
/// such pad still mints its OWN `PadAttr`-bearing centre, so a plain
/// number → id map is last-wins and hands several pads the SAME
/// centre. Aliased that way, a Pads-mode delete of one pad runs the
/// delete mirror against ANOTHER pad's geometry and its `PadAttr`
/// ledger: that pad's whole outline goes, the pad itself stays in the
/// list, and since the bake resolves copper from the sketch its copper
/// silently vanishes from the export. A move aliases the same way.
///
/// So the number is only the first half of the key. Positions
/// disambiguate a collision — exactly, never by epsilon, and the pad
/// centre's `Point` is written from `pad.position_mm` verbatim. If a
/// collision survives that (two pads sharing a number AND a position),
/// the link is REFUSED: an unlinked pad makes both mirrors early-return,
/// which is the pre-fix #142 behaviour — stale geometry, but never
/// another pad's geometry destroyed.
pub(super) fn relink_pads_to_sketch(pads: &mut [EditorPad], fp: &signex_library::Footprint) {
    use std::collections::HashMap;

    let Some(sketch) = fp.sketch.as_ref() else {
        return;
    };
    let mut by_number: HashMap<&str, Vec<(PosKey, signex_sketch::id::SketchEntityId)>> =
        HashMap::new();
    for e in &sketch.entities {
        if let (Some(attr), signex_sketch::entity::EntityKind::Point { x, y }) =
            (e.pad.as_ref(), &e.kind)
        {
            by_number
                .entry(attr.number.as_str())
                .or_default()
                .push((pos_key(*x, *y), e.id));
        }
    }
    // Two pads that share a number are ambiguous from the pad side
    // too, and two that share a number AND a position are ambiguous
    // even after the tie-break. Counted up front so the loop below can
    // refuse instead of aliasing.
    let mut number_claims: HashMap<String, usize> = HashMap::new();
    let mut exact_claims: HashMap<(String, PosKey), usize> = HashMap::new();
    for pad in pads.iter() {
        *number_claims.entry(pad.number.clone()).or_default() += 1;
        *exact_claims
            .entry((pad.number.clone(), pad_pos_key(pad)))
            .or_default() += 1;
    }
    for pad in pads.iter_mut() {
        if pad.sketch_entity_id.is_some() {
            continue;
        }
        let Some(candidates) = by_number.get(pad.number.as_str()) else {
            continue;
        };
        pad.sketch_entity_id = resolve_link(pad, candidates, &number_claims, &exact_claims);
    }
}

/// Bit-exact position key. `f64::to_bits` because "same point" in this
/// repo is exact equality — an epsilon compare here would be the same
/// aliasing bug with extra steps.
type PosKey = (u64, u64);

fn pos_key(x: f64, y: f64) -> PosKey {
    (x.to_bits(), y.to_bits())
}

fn pad_pos_key(pad: &EditorPad) -> PosKey {
    pos_key(pad.position_mm.0, pad.position_mm.1)
}

/// Pick `pad`'s centre out of the candidates sharing its number, or
/// `None` when the answer is not unambiguous. See the collision section
/// on [`relink_pads_to_sketch`] for why `None` is the safe answer.
fn resolve_link(
    pad: &EditorPad,
    candidates: &[(PosKey, signex_sketch::id::SketchEntityId)],
    number_claims: &std::collections::HashMap<String, usize>,
    exact_claims: &std::collections::HashMap<(String, PosKey), usize>,
) -> Option<signex_sketch::id::SketchEntityId> {
    let key = pad_pos_key(pad);
    // Fast path: the number identifies exactly one pad and exactly one
    // centre. No position involved, so a solver-nudged centre still
    // relinks.
    if candidates.len() == 1 && number_claims.get(&pad.number).copied().unwrap_or(0) == 1 {
        return Some(candidates[0].1);
    }
    if exact_claims
        .get(&(pad.number.clone(), key))
        .copied()
        .unwrap_or(0)
        != 1
    {
        return None;
    }
    let mut hits = candidates.iter().filter(|(k, _)| *k == key);
    match (hits.next(), hits.next()) {
        (Some((_, id)), None) => Some(*id),
        _ => None,
    }
}
