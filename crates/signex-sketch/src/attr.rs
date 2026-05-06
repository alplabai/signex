use serde::{Deserialize, Serialize};
use signex_types::layer::SignexLayer;

use crate::id::SketchEntityId;

/// Pad mounting style.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum PadKind {
    #[default]
    Smd,
    Tht,
    NptHole,
    ConnectorPad,
    /// Fiducial / vision-alignment marker. Single side, no paste,
    /// no drill. Default mask margin is 1.0 mm when
    /// `mask_margin_expr` is `None`.
    Fiducial,
    /// Castellated half-hole — plated through-hole on the board edge,
    /// halved by the parent PCB outline router. Bakes as Tht in v0.13.
    Castellated,
}

/// Pad copper side. Drives the layer set assembled at bake time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum PadSide {
    #[default]
    Top,
    Bottom,
    /// Both copper sides. Required for THT and NPT pads.
    All,
}

/// Drill specification for THT / NPT pads.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DrillSpec {
    /// Drill diameter — expression evaluated to a length (mm).
    pub diameter_expr: String,
    /// `None` = round drill; `Some(expr)` = oval slot whose
    /// long-axis length evaluates from `expr`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot_length_expr: Option<String>,
    /// Plated through-hole. `false` = NPT (non-plated mounting hole).
    #[serde(default = "default_true")]
    pub plated: bool,
}

fn default_true() -> bool {
    true
}

/// Pad-level electrical-type flag. Altium's PCB pad Properties shows
/// only three values (`Load / Source / Terminator`) — distinct from
/// the schematic Pin's eight-value enum. Used by Signal-Integrity
/// rules and the testpoint auto-classifier; defaults to `Load`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ElectricalType {
    #[default]
    Load,
    Source,
    Terminator,
}

impl ElectricalType {
    pub const ALL: &'static [ElectricalType] = &[
        ElectricalType::Load,
        ElectricalType::Source,
        ElectricalType::Terminator,
    ];
    pub fn label(self) -> &'static str {
        match self {
            ElectricalType::Load => "Load",
            ElectricalType::Source => "Source",
            ElectricalType::Terminator => "Terminator",
        }
    }
    pub fn is_default(&self) -> bool {
        matches!(self, ElectricalType::Load)
    }
}

impl std::fmt::Display for ElectricalType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Mechanical feature applied to a pad on the named side. Altium's
/// Pad Features section exposes this exactly:
///   - `None`        — bare copper / no machining.
///   - `Counterbore` — flat-bottom recess machined around the hole
///                     to seat a fastener head flush.
///   - `Countersink` — conical recess machined around the hole for
///                     a flat-head fastener.
/// The earlier "Solder Bumps / Glue Dots / Adhesive Beads" variants
/// were factually wrong — those are mechanical-layer purposes
/// (Glue Points / Coating / Gold Plating) authored as separate
/// primitives on dedicated mechanical layers, not pad fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum PadFeature {
    #[default]
    None,
    Counterbore,
    Countersink,
}

impl PadFeature {
    pub const ALL: &'static [PadFeature] = &[
        PadFeature::None,
        PadFeature::Counterbore,
        PadFeature::Countersink,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PadFeature::None => "None",
            PadFeature::Counterbore => "Counterbore",
            PadFeature::Countersink => "Countersink",
        }
    }

    pub fn is_none(&self) -> bool {
        matches!(self, PadFeature::None)
    }
}

impl std::fmt::Display for PadFeature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Test-point participation flags. Each bool selects one of the four
/// test-point columns in Altium's Testpoint sub-section
/// (Top Assembly / Top Fab / Bottom Assembly / Bottom Fab).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestpointFlags {
    #[serde(default)]
    pub top_assembly: bool,
    #[serde(default)]
    pub top_fab: bool,
    #[serde(default)]
    pub bottom_assembly: bool,
    #[serde(default)]
    pub bottom_fab: bool,
}

impl TestpointFlags {
    pub fn is_default(&self) -> bool {
        !self.top_assembly && !self.top_fab && !self.bottom_assembly && !self.bottom_fab
    }
}

/// Per-side mask + paste expansions and the tented flag. Each Option
/// expression overrides the rule-driven default at bake time. `None`
/// here means "use the rule-driven value" — same convention as
/// Altium's "Rule Expansion" cell.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PadStackOverrides {
    /// Override for top-side paste expansion (mm). `None` = rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paste_top_expr: Option<String>,
    /// Override for top-side paste expansion (% of pad). Mutually
    /// exclusive with `paste_top_expr` — UI keeps one populated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paste_top_pct: Option<f64>,
    /// Override for bottom-side paste expansion (mm).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paste_bottom_expr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paste_bottom_pct: Option<f64>,
    /// Per-side paste enable. Default `true`. `false` skips paste
    /// stencil for that side.
    #[serde(default = "default_true")]
    pub paste_top_enabled: bool,
    #[serde(default = "default_true")]
    pub paste_bottom_enabled: bool,
    /// Override for top-side mask expansion (mm). `None` = rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask_top_expr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask_bottom_expr: Option<String>,
    /// Per-side tented flag. `true` skips the mask opening entirely.
    #[serde(default)]
    pub mask_top_tented: bool,
    #[serde(default)]
    pub mask_bottom_tented: bool,
    /// Thermal-relief style on copper — `false` = direct connect.
    #[serde(default = "default_true")]
    pub thermal_relief: bool,
    /// Corner-radius percentage (0..50) for `PadShape::RoundRect`.
    /// Mirrored to / from `PadShape::RoundRect.radius_ratio_expr`
    /// when the shape is round-rect; preserved here for the Altium
    /// "Round Rectangle" UI even when the shape is changed and back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corner_radius_pct: Option<f64>,
}

impl PadStackOverrides {
    pub fn is_default(&self) -> bool {
        self.paste_top_expr.is_none()
            && self.paste_top_pct.is_none()
            && self.paste_bottom_expr.is_none()
            && self.paste_bottom_pct.is_none()
            && self.paste_top_enabled
            && self.paste_bottom_enabled
            && self.mask_top_expr.is_none()
            && self.mask_bottom_expr.is_none()
            && !self.mask_top_tented
            && !self.mask_bottom_tented
            && self.thermal_relief
            && self.corner_radius_pct.is_none()
    }
}

/// Attribute attached to a Real Point on a `BoardTopPlane` to
/// indicate that this point bakes to a footprint pad.
///
/// `Default` exists so existing literal constructors can omit the
/// pad-stack / feature / testpoint fields via `..PadAttr::default()`.
/// Default values place a 1×1 mm SMD round pad on the top side with
/// no overrides — the canonical "blank" pad.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PadAttr {
    pub number: String,
    #[serde(default)]
    pub kind: PadKind,
    #[serde(default)]
    pub side: PadSide,
    pub shape: PadShape,
    pub size_x_expr: String,
    pub size_y_expr: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation_expr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset_x_expr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset_y_expr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drill: Option<DrillSpec>,
    /// Global (both sides) mask expansion fallback. Per-side overrides
    /// live in `stack.mask_top_expr` / `stack.mask_bottom_expr`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask_margin_expr: Option<String>,
    /// Global paste expansion fallback. Per-side overrides live in
    /// `stack.paste_*`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paste_margin_expr: Option<String>,
    #[serde(default)]
    pub paste_apertures: PasteAperturePattern,
    /// Optional pad-template name. Empty = no template. Pad templates
    /// pre-package shape + hole + paste/mask defaults so a footprint
    /// can re-use a stack across many pads.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub template: String,
    /// Optional library-of-record reference for the pad template.
    /// Empty = local / template stored in this footprint.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub library: String,
    /// Per-side pad-stack overrides — paste/mask/tented + thermal +
    /// corner radius.
    #[serde(default, skip_serializing_if = "PadStackOverrides::is_default")]
    pub stack: PadStackOverrides,
    /// Top-side surface feature.
    #[serde(default, skip_serializing_if = "PadFeature::is_none")]
    pub feature_top: PadFeature,
    /// Bottom-side surface feature.
    #[serde(default, skip_serializing_if = "PadFeature::is_none")]
    pub feature_bottom: PadFeature,
    /// Test-point participation.
    #[serde(default, skip_serializing_if = "TestpointFlags::is_default")]
    pub testpoint: TestpointFlags,
    /// Altium-parity electrical-type flag (Load/Source/Terminator).
    #[serde(default, skip_serializing_if = "ElectricalType::is_default")]
    pub electrical_type: ElectricalType,
    /// Net assignment (PCB-level). Empty = unassigned. Persists on
    /// the pad so footprint-level edits don't lose net info on
    /// component sync.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub net: String,
    /// Locked flag — pads with `locked == true` resist accidental
    /// drag / delete / move-by-arrow-keys. The dispatcher MUST
    /// honour this on every mutating gesture.
    #[serde(default, skip_serializing_if = "is_false")]
    pub locked: bool,
    /// Pad Hole tolerance (mm, plus side). Reporting-only — used by
    /// IPC-356 / drill-table export, not by any DRC rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hole_tolerance_plus_mm: Option<f64>,
    /// Pad Hole tolerance (mm, minus side).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hole_tolerance_minus_mm: Option<f64>,
    /// Pad Hole rotation in degrees — orients Slot / Rectangular
    /// holes around the pad centre. Round holes ignore.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hole_rotation_deg: Option<f64>,
    /// Copper offset X (mm) — shifts the copper pad relative to the
    /// hole centre. Used for stress relief or finger-pad designs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copper_offset_x_mm: Option<f64>,
    /// Copper offset Y (mm).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copper_offset_y_mm: Option<f64>,
}

fn is_false(v: &bool) -> bool {
    !v
}

/// Pad copper outline.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "shape", rename_all = "PascalCase")]
pub enum PadShape {
    #[default]
    Round,
    Rect,
    /// Rounded rectangle — `radius_ratio_expr` ∈ 0..0.5.
    RoundRect {
        radius_ratio_expr: String,
    },
    Oval,
    /// Rectangle with chamfered corners.
    Chamfered {
        chamfer_ratio_expr: String,
        corners: ChamferedCorners,
    },
    /// Arbitrary polygon outline.
    Custom(CustomPadShape),
}

/// Which corners of a Chamfered pad are beveled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChamferedCorners {
    #[serde(default)]
    pub top_left: bool,
    #[serde(default)]
    pub top_right: bool,
    #[serde(default)]
    pub bottom_right: bool,
    #[serde(default)]
    pub bottom_left: bool,
}

impl ChamferedCorners {
    pub const ALL: Self = Self {
        top_left: true,
        top_right: true,
        bottom_right: true,
        bottom_left: true,
    };
}

/// Source of a `PadShape::Custom` polygon.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "PascalCase")]
pub enum CustomPadShape {
    /// Static polygon — points relative to pad center, in mm.
    StaticPoints { points: Vec<[f64; 2]> },
    /// Sketch-driven polygon — closed-loop set of sketch entities.
    /// v0.13 round-trips; bake falls back to bbox-rect with warning.
    SketchProfile { source: Vec<SketchEntityId> },
}

/// Solder-paste aperture layout for a pad.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "PascalCase")]
pub enum PasteAperturePattern {
    Single,
    Grid {
        nx_expr: String,
        ny_expr: String,
        coverage_expr: String,
    },
    Custom {
        source: Vec<SketchEntityId>,
    },
}

impl Default for PasteAperturePattern {
    fn default() -> Self {
        Self::Single
    }
}

/// Closed sketch profile bakes as a silkscreen line/arc set.
/// v0.13 round-trips; v0.14 bakes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SilkAttr {
    pub layer: SignexLayer,
}

/// Closed sketch profile bakes as the courtyard polygon.
/// v0.13 round-trips; v0.14 bakes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CourtyardAttr;

/// Closed sketch profile bakes as a mask opening (cutout).
/// v0.13 round-trips; v0.14 bakes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MaskOpeningAttr {
    pub layer: SignexLayer,
}

/// Closed sketch profile bakes as an explicit mask cover.
/// v0.13 round-trips; v0.14 bakes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MaskExcludeAttr {
    pub layer: SignexLayer,
}

/// Closed sketch profile bakes as a stencil paste aperture.
/// v0.13 round-trips; v0.14 bakes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PasteApertureAttr {
    pub layer: SignexLayer,
}

/// Closed sketch profile bakes as a copper-fill region.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PourAttr {
    pub layer: SignexLayer,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub net: Option<String>,
    #[serde(default)]
    pub fill_type: PourFillType,
    #[serde(default)]
    pub thermal_relief: ThermalRelief,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clearance_expr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_thickness_expr: Option<String>,
    #[serde(default)]
    pub priority: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum PourFillType {
    #[default]
    Solid,
    Hatched,
    Outline,
}

impl PourFillType {
    /// Display order used by Properties-panel pick_lists. Mirrors the
    /// declaration order on the enum.
    pub const ALL: &'static [PourFillType] = &[
        PourFillType::Solid,
        PourFillType::Hatched,
        PourFillType::Outline,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PourFillType::Solid => "Solid",
            PourFillType::Hatched => "Hatched",
            PourFillType::Outline => "Outline",
        }
    }
}

impl std::fmt::Display for PourFillType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Thermal relief — how the pour connects to same-net pads.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThermalRelief {
    #[serde(default = "default_thermal_enabled")]
    pub enabled: bool,
    #[serde(default = "default_thermal_gap")]
    pub gap_expr: String,
    #[serde(default = "default_thermal_spoke")]
    pub spoke_width_expr: String,
    #[serde(default = "default_thermal_spoke_count")]
    pub spoke_count: u8,
}

impl Default for ThermalRelief {
    fn default() -> Self {
        Self {
            enabled: true,
            gap_expr: "0.508mm".into(),
            spoke_width_expr: "0.254mm".into(),
            spoke_count: 4,
        }
    }
}

fn default_thermal_enabled() -> bool {
    true
}
fn default_thermal_gap() -> String {
    "0.508mm".into()
}
fn default_thermal_spoke() -> String {
    "0.254mm".into()
}
fn default_thermal_spoke_count() -> u8 {
    4
}

/// Closed sketch profile bakes as a keepout region.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeepoutAttr {
    pub layer: SignexLayer,
    pub kinds: KeepoutKinds,
}

/// Keepout category bitfield.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeepoutKinds {
    #[serde(default)]
    pub no_routing: bool,
    #[serde(default)]
    pub no_components: bool,
    #[serde(default)]
    pub no_copper: bool,
    #[serde(default)]
    pub no_vias: bool,
    #[serde(default)]
    pub no_drilling: bool,
    #[serde(default)]
    pub no_pours: bool,
}

impl KeepoutKinds {
    /// "No copper of any kind" — common preset.
    pub const ALL_COPPER: Self = Self {
        no_routing: true,
        no_components: false,
        no_copper: true,
        no_vias: true,
        no_drilling: false,
        no_pours: true,
    };

    /// Antenna keep-clear — alias for ALL_COPPER.
    pub const ANTENNA: Self = Self::ALL_COPPER;

    /// "No traces under this part" — typical for crystals, magnetics.
    pub const NO_ROUTING: Self = Self {
        no_routing: true,
        no_components: false,
        no_copper: false,
        no_vias: true,
        no_drilling: false,
        no_pours: false,
    };
}

/// Closed sketch profile on a `BoardTopPlane` bakes as a board cutout.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BoardCutoutAttr {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge_radius_expr: Option<String>,
    #[serde(default = "default_through")]
    pub through: bool,
}

fn default_through() -> bool {
    true
}

/// Line entity attribute: this line is a V-score path.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VScoreHintAttr {
    #[serde(default = "default_v_depth")]
    pub depth_fraction_expr: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_web_expr: Option<String>,
    #[serde(default)]
    pub side: VScoreSide,
}

fn default_v_depth() -> String {
    "0.333".into()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum VScoreSide {
    #[default]
    Both,
    Top,
    Bottom,
}
