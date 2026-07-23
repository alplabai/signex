//! Pad geometry types (layers, shapes, drills, chamfers) for the footprint primitive.

use super::*;

/// PCB layer identifier — minimal subset surfaced by the library layer.
///
/// The PCB editor (signex-types::LayerId) carries the full Altium taxonomy.
/// This crate only needs to express which copper / mask / paste layers a pad
/// participates in; we keep a string-typed wrapper rather than importing
/// signex-types here so this crate stays leaf-level.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LayerId(pub String);

impl LayerId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for LayerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Pad mounting style.
///
/// Variant names persist in PascalCase to preserve v1 / v2 fixture
/// compatibility — adding `rename_all = "snake_case"` would break
/// every existing footprint TOML.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PadKind {
    /// Surface-mount.
    #[default]
    Smd,
    /// Through-hole, plated.
    Tht,
    /// Non-plated mounting hole.
    NptHole,
    /// Edge / mezzanine connector pad.
    ConnectorPad,
    /// Castellated edge pad — half-hole on the board edge. Bake emits
    /// drill semantics + an outline-edge truncation hint so gerber
    /// outline export can identify the halved hole. v0.14+.
    Castellated,
    /// Fiducial vision marker — copper + mask only, no paste, no drill.
    /// v0.14+.
    Fiducial,
}

/// Which corners of a chamfered-rectangle pad are cut.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ChamferedCorners {
    #[serde(default)]
    pub top_left: bool,
    #[serde(default)]
    pub top_right: bool,
    #[serde(default)]
    pub bottom_left: bool,
    #[serde(default)]
    pub bottom_right: bool,
}

impl ChamferedCorners {
    pub const fn all() -> Self {
        Self {
            top_left: true,
            top_right: true,
            bottom_left: true,
            bottom_right: true,
        }
    }
}

/// Pad geometry shape.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PadShape {
    #[default]
    Round,
    Rect,
    RoundRect {
        /// Corner-radius ratio (0.0 = sharp rect, 0.5 = full pill).
        radius_ratio: f64,
    },
    Oval,
    /// Chamfered-corner rectangle. v0.14+.
    Chamfered {
        /// Chamfer extent as a ratio of pad min-dimension (0.0 = no
        /// chamfer, 0.5 = full diagonal cut).
        chamfer_ratio: f64,
        /// Per-corner enable flags.
        corners: ChamferedCorners,
    },
    /// Custom outline polygon — points relative to pad centre, mm.
    Custom(Polygon),
}

/// Closed polygon — points in mm. Used for courtyards, custom pads, etc.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Polygon {
    pub points: Vec<[f64; 2]>,
}

impl Polygon {
    pub fn new(points: Vec<[f64; 2]>) -> Self {
        Self { points }
    }
}

/// Drill specification for through-hole / mounting pads.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Drill {
    pub diameter: f64,
    /// Slot length — `None` = round drill, `Some(len)` = oval slot of length `len`.
    #[serde(default)]
    pub slot_length: Option<f64>,
}

/// One PCB pad.
///
/// `Default` exists so existing literal constructors can omit the
/// pad-stack / feature / testpoint fields via `..Pad::default()`.
/// Default values place a 0×0 mm round SMD pad at the origin with
/// no overrides — the canonical "blank" pad. Real callers always
/// override the geometry fields explicitly.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Pad {
    /// Pad number — pin-map binding key ("1", "EP", "MOUNT1").
    pub number: String,
    pub kind: PadKind,
    pub shape: PadShape,
    /// Pad outer dimensions in mm.
    pub size: [f64; 2],
    /// Position of the pad centre in footprint-local mm coordinates.
    pub position: [f64; 2],
    /// Rotation in degrees.
    #[serde(default)]
    pub rotation: f64,
    /// Layers this pad lives on — copper + mask + paste as appropriate.
    pub layers: Vec<LayerId>,
    /// Drill (None for SMD).
    #[serde(default)]
    pub drill: Option<Drill>,
    /// Global mask margin fallback (mm). Per-side overrides live in
    /// `mask_margin_top` / `mask_margin_bottom`.
    #[serde(default)]
    pub solder_mask_margin: Option<f64>,
    /// Global paste margin fallback. Per-side overrides live in
    /// `paste_margin_top` / `paste_margin_bottom`.
    #[serde(default)]
    pub paste_margin: Option<f64>,
    /// Optional pad-template name. Empty = no template.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub template: String,
    /// Optional library-of-record reference for the pad template.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub template_library: String,
    /// Per-side paste-margin overrides (mm). `None` = use the global
    /// `paste_margin`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paste_margin_top: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paste_margin_bottom: Option<f64>,
    /// Per-side paste-stencil enable. Default `true`.
    #[serde(default = "default_true_bool")]
    pub paste_enabled_top: bool,
    #[serde(default = "default_true_bool")]
    pub paste_enabled_bottom: bool,
    /// Per-side mask-margin overrides (mm).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask_margin_top: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask_margin_bottom: Option<f64>,
    /// Per-side tented flag — `true` skips the mask opening entirely.
    #[serde(default)]
    pub mask_tented_top: bool,
    #[serde(default)]
    pub mask_tented_bottom: bool,
    /// Thermal-relief style on copper. `false` = direct connect.
    #[serde(default = "default_true_bool")]
    pub thermal_relief: bool,
    /// Corner-radius percentage (0..50) for `PadShape::RoundRect`.
    /// Mirror of `PadShape::RoundRect.radius_ratio` but persisted
    /// independently so the Altium "Round Rectangle" UI value
    /// survives a shape switch and back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corner_radius_pct: Option<f64>,
    /// Top-side surface feature (Altium "Pad Features → Top Side").
    #[serde(
        default,
        skip_serializing_if = "signex_sketch::attr::PadFeature::is_none"
    )]
    pub feature_top: signex_sketch::attr::PadFeature,
    /// Bottom-side surface feature.
    #[serde(
        default,
        skip_serializing_if = "signex_sketch::attr::PadFeature::is_none"
    )]
    pub feature_bottom: signex_sketch::attr::PadFeature,
    /// Test-point participation (top/bottom × assembly/fab).
    #[serde(
        default,
        skip_serializing_if = "signex_sketch::attr::TestpointFlags::is_default"
    )]
    pub testpoint: signex_sketch::attr::TestpointFlags,
    /// Altium-parity electrical-type flag (Load/Source/Terminator).
    #[serde(
        default,
        skip_serializing_if = "signex_sketch::attr::ElectricalType::is_default"
    )]
    pub electrical_type: signex_sketch::attr::ElectricalType,
    /// Net assignment. Empty = unassigned.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub net: String,
    /// Lock flag — resists accidental drag/move/delete.
    #[serde(default, skip_serializing_if = "is_false")]
    pub locked: bool,
    /// Pad Hole tolerance ± in mm (reporting only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hole_tolerance_plus_mm: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hole_tolerance_minus_mm: Option<f64>,
    /// Pad Hole rotation (Slot/Rectangular orientation).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hole_rotation_deg: Option<f64>,
    /// Copper offset relative to hole centre.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copper_offset_x_mm: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copper_offset_y_mm: Option<f64>,
}

fn is_false(v: &bool) -> bool {
    !v
}

/// Helper for `#[serde(default = "...")]` on bool fields that should
/// default to `true`. `bool::default()` is `false`, so this is needed
/// for fields where omission means "yes / enabled".
fn default_true_bool() -> bool {
    true
}
