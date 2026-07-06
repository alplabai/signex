//! `Footprint` primitive — PCB-side reusable shape.
//!
//! Per `v0.9-refactor-2-plan.md` §2.2, a `Footprint` carries:
//! - typed pad list,
//! - courtyard polygon,
//! - silk / fab graphics for both copper sides,
//! - an embedded [`Body3D`] (drives Signex's procedural 3D render),
//! - an optional [`StepAttachment`] (mech-CAD STEP file, content-hashed).
//!
//! Two MPNs sharing a SOIC-8 footprint reference the same `Footprint` UUID
//! via `Component::footprint_ref` — the geometry lives once and accumulates
//! fixes over time.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::param::ParamMap;

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
    #[serde(default, skip_serializing_if = "signex_sketch::attr::PadFeature::is_none")]
    pub feature_top: signex_sketch::attr::PadFeature,
    /// Bottom-side surface feature.
    #[serde(default, skip_serializing_if = "signex_sketch::attr::PadFeature::is_none")]
    pub feature_bottom: signex_sketch::attr::PadFeature,
    /// Test-point participation (top/bottom × assembly/fab).
    #[serde(default, skip_serializing_if = "signex_sketch::attr::TestpointFlags::is_default")]
    pub testpoint: signex_sketch::attr::TestpointFlags,
    /// Altium-parity electrical-type flag (Load/Source/Terminator).
    #[serde(default, skip_serializing_if = "signex_sketch::attr::ElectricalType::is_default")]
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

/// Footprint graphic kinds — silkscreen / fab outline primitives.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FpGraphicKind {
    Line {
        from: [f64; 2],
        to: [f64; 2],
    },
    Rectangle {
        from: [f64; 2],
        to: [f64; 2],
    },
    Circle {
        center: [f64; 2],
        radius: f64,
    },
    Arc {
        center: [f64; 2],
        radius: f64,
        start_deg: f64,
        end_deg: f64,
    },
    Text {
        position: [f64; 2],
        content: String,
        size: f64,
        /// Optional bounding-box (width, height) in mm. `None` = point
        /// text (legacy). Rendering aligns/clips the string inside it.
        #[serde(default)]
        frame: Option<(f32, f32)>,
    },
    /// v0.18.17 — closed-loop polygon outlined or filled. The
    /// vertex list is closed implicitly (`vertices[N-1]` connects
    /// back to `vertices[0]` at render / bake time).
    Polygon {
        vertices: Vec<[f64; 2]>,
    },
    /// v0.18.17 — closed-loop polygon outlined or filled. The
    /// vertex list is closed implicitly (`vertices[N-1]` connects
    /// back to `vertices[0]` at render / bake time).
    Polygon {
        vertices: Vec<[f64; 2]>,
    },
}

/// One footprint silkscreen / fab graphic.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FpGraphic {
    pub kind: FpGraphicKind,
    /// Stroke width in mm (0.0 = renderer default).
    #[serde(default)]
    pub stroke_width: f64,
    /// v0.18.17 — fill flag. `true` = render as a solid colour fill
    /// (Altium "Place Region" / "Place Fill"); `false` = outline
    /// only (the v0.16.5 behaviour, which all pre-v0.18.17 files
    /// still load as via `#[serde(default)]`). Only meaningful for
    /// `Polygon` / `Rectangle` / `Circle` / `Arc` (closed shapes).
    #[serde(default)]
    pub filled: bool,
}

/// 3D body shape — drives the procedural render (no STEP required).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub enum BodyShape {
    /// Extrude the fab outline (or `Body3D::outline` override) up by `height_mm`.
    #[default]
    Extrude,
    /// Spherical-cap dome — common for LEDs / TO-92.
    Dome,
    /// Cylindrical body — through-hole electrolytics, crystals.
    Cylinder,
    /// Custom shape provided by the renderer (currently a stub).
    Custom,
}

impl BodyShape {
    pub const ALL: &'static [BodyShape] = &[
        BodyShape::Extrude,
        BodyShape::Dome,
        BodyShape::Cylinder,
        BodyShape::Custom,
    ];
}

/// Embedded 3D body description. Lives on [`Footprint`] so two MPNs that share
/// a footprint also share the same procedural 3D render.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Body3D {
    pub shape: BodyShape,
    pub height_mm: f32,
    /// Body sits this far above the PCB surface.
    pub offset_z_mm: f32,
    /// RGBA top color (typically the body face).
    pub top_color: [f32; 4],
    /// RGBA side color.
    pub side_color: [f32; 4],
    /// Optional outline override; defaults to `fab_f` convex hull when `None`.
    #[serde(default)]
    pub outline: Option<Polygon>,
}

impl Default for Body3D {
    fn default() -> Self {
        Self {
            shape: BodyShape::Extrude,
            height_mm: 1.0,
            offset_z_mm: 0.0,
            // Mid-grey defaults so an empty body renders visibly.
            top_color: [0.20, 0.20, 0.20, 1.0],
            side_color: [0.30, 0.30, 0.30, 1.0],
            outline: None,
        }
    }
}

/// Net reference — string for now, will become a UUID once nets are
/// first-class library citizens (v0.16+). v0.14 introduces this as a
/// thin wrapper so future migration can be one-shot.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NetRef(pub Option<String>);

impl NetRef {
    pub fn named(s: impl Into<String>) -> Self {
        Self(Some(s.into()))
    }
}

/// Pour fill mode — drives the polygon raster fill at PCB-render time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PourFillType {
    #[default]
    Solid,
    Hatched,
    None,
}

/// Thermal-relief connection style for pads inside a pour. v0.14
/// records the choice; the actual relief geometry is generated at
/// pour-render time (v0.15).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ThermalReliefStyle {
    Direct,
    #[default]
    Spoke,
    None,
}

/// Copper pour / region. Fill generation lives in v0.15 — v0.14 stores
/// the boundary + metadata only.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FpPour {
    pub boundary: Polygon,
    pub layer: LayerId,
    #[serde(default)]
    pub net: NetRef,
    #[serde(default)]
    pub fill_type: PourFillType,
    #[serde(default)]
    pub thermal_relief: ThermalReliefStyle,
    #[serde(default)]
    pub clearance: f64,
    #[serde(default)]
    pub min_thickness: f64,
    #[serde(default)]
    pub priority: u8,
}

/// What a keepout zone forbids. DRC enforcement lands in v0.15.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum KeepoutForbid {
    #[default]
    All,
    Tracks,
    Pads,
    Vias,
    Copper,
}

/// DRC keepout zone. v0.14 stores the polygon + layer + forbid kind.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FpKeepout {
    pub boundary: Polygon,
    pub layer: LayerId,
    #[serde(default)]
    pub forbids: KeepoutForbid,
}

/// Board cutout — subtracts from the PCB outline (mounting hole, slot,
/// edge cutout). Outline subtraction itself runs at PCB-export time.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FpCutout {
    pub boundary: Polygon,
    /// v0.15 — fillet radius applied to every corner of the cutout
    /// at fab-export time. `0.0` (default) = sharp corners.
    #[serde(default)]
    pub edge_radius_mm: f64,
    /// v0.15 — `true` (default) = full-depth through-hole cutout;
    /// `false` = partial-depth pocket / blind cutout. Fab tooling
    /// reads this to choose between routing and milling.
    #[serde(default = "default_true")]
    pub through: bool,
}

fn default_true() -> bool {
    true
}

/// Which side of the PCB the V-score is cut into. v0.15.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum VScoreSide {
    #[default]
    Both,
    Top,
    Bottom,
}

/// V-score panelisation hint — straight-line score on the PCB surface.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FpVScore {
    /// Score line endpoints (mm).
    pub line: [[f64; 2]; 2],
    /// Score depth in mm.
    #[serde(default)]
    pub depth: f64,
    /// v0.15 — which board side the score is cut into.
    #[serde(default)]
    pub side: VScoreSide,
    /// v0.15 — minimum web thickness in mm; the panelisation
    /// consumer enforces this lower bound when evaluating depth /
    /// nominal-board-thickness ratios. `0.0` (default) = no minimum.
    #[serde(default)]
    pub min_web_mm: f64,
}

/// Solder-mask opening (cutout) — copper without solder mask covering.
/// Distinct from a pad's mask margin: this is a standalone profile for
/// e.g. an exposed copper region or a panel-level marker.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FpMaskOpening {
    pub boundary: Polygon,
    pub layer: LayerId,
}

/// Solder-mask exclusion (cover) — solder mask explicitly extended over
/// copper. Same shape as `FpMaskOpening`; semantics flip.
pub type FpMaskExclude = FpMaskOpening;

/// Standalone paste aperture — a paste opening separate from any pad
/// (used for thermal-pad split-aperture patterns and panel fiducials).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FpPasteAperture {
    pub boundary: Polygon,
    pub layer: LayerId,
}

/// Optional STEP/WRL attachment for mech-CAD export. Content-hashed so two
/// MPNs with identical STEP geometry de-duplicate to one file in
/// `mylib.snxlib/step/<sha256>.step`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StepAttachment {
    /// SHA-256 hex of the .step file contents — also the on-disk filename stem.
    pub content_hash: String,
    /// Original filename hint for UX ("LM358.step").
    pub filename: String,
    /// Model placement offset relative to the footprint origin (mm).
    pub offset_xyz: [f64; 3],
    /// Model rotation in degrees, X / Y / Z.
    pub rotation_xyz: [f64; 3],
}

/// Reusable PCB primitive. Bound by `Component::footprint_ref`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Footprint {
    pub uuid: Uuid,
    /// Human-facing name ("SOIC-8") — independent of any binding component's
    /// `internal_pn`.
    pub name: String,
    #[serde(default)]
    pub anchor: [f64; 2],
    pub pads: Vec<Pad>,
    /// Courtyard outline (F.CrtYd / B.CrtYd geometry, single polygon).
    #[serde(default)]
    pub courtyard: Polygon,
    /// Front silkscreen graphics (F.SilkS).
    #[serde(default)]
    pub silk_f: Vec<FpGraphic>,
    /// Back silkscreen graphics (B.SilkS).
    #[serde(default)]
    pub silk_b: Vec<FpGraphic>,
    /// Front fab outline (F.Fab) — drives the body_3d outline default.
    #[serde(default)]
    pub fab_f: Vec<FpGraphic>,
    /// Back fab outline (B.Fab).
    #[serde(default)]
    pub fab_b: Vec<FpGraphic>,
    /// Embedded procedural 3D body description.
    #[serde(default)]
    pub body_3d: Body3D,
    /// Optional content-hashed STEP attachment for mech-CAD export.
    #[serde(default)]
    pub step_attachment: Option<StepAttachment>,
    /// Default PCB-side parameter values that flow to the binding component.
    #[serde(default)]
    pub pcb_params: ParamMap,
    /// Semver-style revision string. Stage 14 of
    /// `v0.9-snxlib-as-file-plan.md`: footprints version
    /// independently of the bound symbols and component rows.
    /// Defaults to `"0.0.1"` for new + legacy primitives.
    #[serde(default = "default_footprint_version")]
    pub version: String,
    /// Released-flag: locks edit-in-place under Team mode.
    #[serde(default)]
    pub released: bool,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    /// Schema version — bump on breaking format change. Defaults to 2
    /// when missing (legacy v1 footprints had no `schema_version`).
    #[serde(default = "default_schema_v2")]
    pub schema_version: u32,
    /// Optional 2D parametric sketch — drives pad layout via the
    /// signex-sketch solver in v0.13+. v1 footprints (no
    /// `schema_version`) deserialise with `sketch == None` and
    /// preserve their literal pad-list authoring.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sketch: Option<signex_sketch::SketchData>,
    /// Copper pour / region polygons. v0.14+; fill generation is v0.15.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pours: Vec<FpPour>,
    /// DRC keepout zones. v0.14 records; v0.15 enforces.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keepouts: Vec<FpKeepout>,
    /// Board cutouts. v0.14 records; outline subtraction at PCB-export.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cutouts: Vec<FpCutout>,
    /// V-score panelisation hints. v0.14 records; panelisation tool
    /// consumes them in v0.16+.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub v_scores: Vec<FpVScore>,
    /// Standalone solder-mask openings (separate from pad mask margins).
    /// v0.14+.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mask_openings: Vec<FpMaskOpening>,
    /// Standalone solder-mask exclusions (mask cover). v0.14+.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mask_excludes: Vec<FpMaskExclude>,
    /// Standalone paste apertures (separate from pad paste). v0.14+.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paste_apertures: Vec<FpPasteAperture>,
    /// v0.21 — Altium-parity component-level fields surfaced when
    /// nothing is selected in the editor. `description` is the
    /// human-readable summary; `default_designator` is the auto-
    /// numbering template (`R?`, `U?`, …); `component_type` drives
    /// BOM / Net-Tie / Jumper semantics; `height_mm` is the
    /// component's overall height for collision checking.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub default_designator: String,
    #[serde(default, skip_serializing_if = "ComponentType::is_default")]
    pub component_type: ComponentType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height_mm: Option<f64>,
}

/// v0.21 — Altium-parity component type. Drives whether the part
/// appears in the BOM and whether its pads can short different nets
/// (Net Tie / Jumper).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ComponentType {
    #[default]
    Standard,
    StandardNoBom,
    Mechanical,
    Graphical,
    NetTie,
    NetTieInBom,
    Jumper,
}

impl ComponentType {
    pub const ALL: &'static [ComponentType] = &[
        ComponentType::Standard,
        ComponentType::StandardNoBom,
        ComponentType::Mechanical,
        ComponentType::Graphical,
        ComponentType::NetTie,
        ComponentType::NetTieInBom,
        ComponentType::Jumper,
    ];
    pub fn label(self) -> &'static str {
        match self {
            ComponentType::Standard => "Standard",
            ComponentType::StandardNoBom => "Standard (No BOM)",
            ComponentType::Mechanical => "Mechanical",
            ComponentType::Graphical => "Graphical",
            ComponentType::NetTie => "Net Tie (No BOM)",
            ComponentType::NetTieInBom => "Net Tie (in BOM)",
            ComponentType::Jumper => "Jumper",
        }
    }
    pub fn is_default(&self) -> bool {
        matches!(self, ComponentType::Standard)
    }
}

impl std::fmt::Display for ComponentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

fn default_footprint_version() -> String {
    "0.0.1".to_string()
}

fn default_schema_v2() -> u32 {
    2
}

/// Schema version emitted by `Footprint::empty()`. v3 (the current
/// version) adds optional pour / keepout / cutout / v_score /
/// mask_opening / mask_exclude / paste_aperture fields. v2 files load
/// cleanly as v2 (the new fields default to empty Vecs); the version
/// number is bumped only so consumers can see at a glance which fields
/// the file may use.
pub const FOOTPRINT_SCHEMA_VERSION: u32 = 3;

/// Multi-footprint container for `.snxfpt` files — Altium PCB
/// Library parity. One file holds many footprints; each footprint
/// still has its own UUID for `PrimitiveRef` resolution.
///
/// Wire format (v0.18.4): TOML manifest header + one `[[footprints]]`
/// array entry per Footprint. Each entry's bulk pad list is embedded
/// as a TSV literal multi-line string (`pads_tsv = '''…'''`) — line-
/// diffable in git, editable in any spreadsheet. Graphics
/// (silk/fab/courtyard), 3D body, sketch, pours, keepouts, cutouts,
/// v-scores, mask openings/excludes, and paste apertures stay as
/// inline TOML since they're variant-shaped or sparse.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FootprintFile {
    /// Schema sentinel — current emitters write `"snxfpt/1"`.
    #[serde(default = "default_footprint_format")]
    pub format: String,
    /// File-level UUID — distinct from any contained footprint's uuid.
    /// Used as the file-rename-stable handle.
    pub file_uuid: Uuid,
    /// Human-facing library name shown in the Footprint Library
    /// panel header. Defaults to the file stem when empty.
    #[serde(default)]
    pub display_name: String,
    /// All footprints in this file. Order is the Footprint Library
    /// panel order.
    pub footprints: Vec<Footprint>,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
}

const FOOTPRINT_FILE_FORMAT_TOKEN: &str = "snxfpt/1";

/// Stable column layout for the per-footprint `pads_tsv` block.
/// Adding or reordering columns is a wire-format break — bump
/// [`FOOTPRINT_FILE_FORMAT_TOKEN`].
const PAD_TSV_COLUMNS: &[&str] = &[
    "number",
    "kind",
    "shape",
    "size_x",
    "size_y",
    "pos_x",
    "pos_y",
    "rotation",
    "layers",
    "drill_diameter",
    "drill_slot_length",
    "solder_mask_margin",
    "paste_margin",
];

/// Sentinel string substituted for each footprint's `pads_tsv` field
/// before TOML serialise; replaced post-emit with the literal multi-
/// line `'''…'''` block.
const PADS_TSV_PLACEHOLDER_PREFIX: &str = "__SIGNEX_PADS_TSV_a1b2c3d4_";

fn default_footprint_format() -> String {
    FOOTPRINT_FILE_FORMAT_TOKEN.to_string()
}

/// On-disk wire shape. Mirrors [`FootprintFile`] but each
/// [`Footprint`]'s `pads` Vec is replaced with a `pads_tsv: String`
/// carrying the TSV-encoded payload.
#[derive(Serialize, Deserialize)]
struct FootprintFileWire {
    format: String,
    file_uuid: Uuid,
    #[serde(default)]
    display_name: String,
    created: DateTime<Utc>,
    updated: DateTime<Utc>,
    #[serde(default)]
    footprints: Vec<FootprintWire>,
}

#[derive(Serialize, Deserialize)]
struct FootprintWire {
    uuid: Uuid,
    name: String,
    #[serde(default)]
    anchor: [f64; 2],
    /// TSV-encoded pad list — header row + one row per pad.
    pads_tsv: String,
    #[serde(default)]
    courtyard: Polygon,
    #[serde(default)]
    silk_f: Vec<FpGraphic>,
    #[serde(default)]
    silk_b: Vec<FpGraphic>,
    #[serde(default)]
    fab_f: Vec<FpGraphic>,
    #[serde(default)]
    fab_b: Vec<FpGraphic>,
    #[serde(default)]
    body_3d: Body3D,
    #[serde(default)]
    step_attachment: Option<StepAttachment>,
    #[serde(default)]
    pcb_params: ParamMap,
    #[serde(default = "default_footprint_version")]
    version: String,
    #[serde(default)]
    released: bool,
    created: DateTime<Utc>,
    updated: DateTime<Utc>,
    #[serde(default = "default_schema_v2")]
    schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sketch: Option<signex_sketch::SketchData>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pours: Vec<FpPour>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    keepouts: Vec<FpKeepout>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    cutouts: Vec<FpCutout>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    v_scores: Vec<FpVScore>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    mask_openings: Vec<FpMaskOpening>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    mask_excludes: Vec<FpMaskExclude>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    paste_apertures: Vec<FpPasteAperture>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    description: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    default_designator: String,
    #[serde(default, skip_serializing_if = "ComponentType::is_default")]
    component_type: ComponentType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    height_mm: Option<f64>,
    // FootprintWire (per-footprint) wire fields below; the
    // FootprintFile (file-level) wire ends here.
}

// ── FootprintWire mirrors Footprint for the multi-footprint TOML
//    wire format. Adding the four v0.21 fields here too.

impl FootprintFile {
    /// Build a new container holding a single footprint — what the
    /// `Add New ▸ Footprint Library` flow seeds.
    pub fn from_footprint(footprint: Footprint) -> Self {
        let now = Utc::now();
        Self {
            format: default_footprint_format(),
            file_uuid: Uuid::now_v7(),
            display_name: footprint.name.clone(),
            footprints: vec![footprint],
            created: now,
            updated: now,
        }
    }

    /// Decode bytes as UTF-8 and parse via [`FootprintFile::from_toml_str`].
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, FootprintFileError> {
        if bytes.iter().all(u8::is_ascii_whitespace) {
            return Err(FootprintFileError::Empty);
        }
        let text = std::str::from_utf8(bytes)?;
        Self::from_toml_str(text)
    }

    /// Parse the TOML+TSV wire format. The format-token check pins us
    /// to [`FOOTPRINT_FILE_FORMAT_TOKEN`]; mismatched files surface
    /// [`FootprintFileError::UnsupportedFormat`].
    pub fn from_toml_str(text: &str) -> Result<Self, FootprintFileError> {
        let wire: FootprintFileWire = toml::from_str(text)?;
        if wire.format != FOOTPRINT_FILE_FORMAT_TOKEN {
            return Err(FootprintFileError::UnsupportedFormat { got: wire.format });
        }
        let mut footprints = Vec::with_capacity(wire.footprints.len());
        for fw in wire.footprints {
            let pads = pads_from_tsv(&fw.pads_tsv)?;
            footprints.push(Footprint {
                uuid: fw.uuid,
                name: fw.name,
                anchor: fw.anchor,
                pads,
                courtyard: fw.courtyard,
                silk_f: fw.silk_f,
                silk_b: fw.silk_b,
                fab_f: fw.fab_f,
                fab_b: fw.fab_b,
                body_3d: fw.body_3d,
                step_attachment: fw.step_attachment,
                pcb_params: fw.pcb_params,
                version: fw.version,
                released: fw.released,
                created: fw.created,
                updated: fw.updated,
                schema_version: fw.schema_version,
                sketch: fw.sketch,
                pours: fw.pours,
                keepouts: fw.keepouts,
                cutouts: fw.cutouts,
                v_scores: fw.v_scores,
                mask_openings: fw.mask_openings,
                mask_excludes: fw.mask_excludes,
                paste_apertures: fw.paste_apertures,
                description: fw.description,
                default_designator: fw.default_designator,
                component_type: fw.component_type,
                height_mm: fw.height_mm,
            });
        }
        Ok(FootprintFile {
            format: wire.format,
            file_uuid: wire.file_uuid,
            display_name: wire.display_name,
            created: wire.created,
            updated: wire.updated,
            footprints,
        })
    }

    /// Serialise to canonical TOML+TSV. Pad lists become
    /// `pads_tsv = '''\n<header>\n<rows>\n'''` literal multi-line
    /// strings so the bulk data is line-diffable in git output.
    pub fn to_toml_string(&self) -> Result<String, FootprintFileError> {
        let mut tsv_payloads: Vec<String> = Vec::with_capacity(self.footprints.len());
        let mut wire_footprints: Vec<FootprintWire> = Vec::with_capacity(self.footprints.len());
        for (idx, fp) in self.footprints.iter().enumerate() {
            tsv_payloads.push(pads_to_tsv(&fp.pads)?);
            wire_footprints.push(FootprintWire {
                uuid: fp.uuid,
                name: fp.name.clone(),
                anchor: fp.anchor,
                pads_tsv: format!("{PADS_TSV_PLACEHOLDER_PREFIX}{idx}__"),
                courtyard: fp.courtyard.clone(),
                silk_f: fp.silk_f.clone(),
                silk_b: fp.silk_b.clone(),
                fab_f: fp.fab_f.clone(),
                fab_b: fp.fab_b.clone(),
                body_3d: fp.body_3d.clone(),
                step_attachment: fp.step_attachment.clone(),
                pcb_params: fp.pcb_params.clone(),
                version: fp.version.clone(),
                released: fp.released,
                created: fp.created,
                updated: fp.updated,
                schema_version: fp.schema_version,
                sketch: fp.sketch.clone(),
                pours: fp.pours.clone(),
                keepouts: fp.keepouts.clone(),
                cutouts: fp.cutouts.clone(),
                v_scores: fp.v_scores.clone(),
                mask_openings: fp.mask_openings.clone(),
                mask_excludes: fp.mask_excludes.clone(),
                paste_apertures: fp.paste_apertures.clone(),
                description: fp.description.clone(),
                default_designator: fp.default_designator.clone(),
                component_type: fp.component_type,
                height_mm: fp.height_mm,
            });
        }
        let wire = FootprintFileWire {
            format: self.format.clone(),
            file_uuid: self.file_uuid,
            display_name: self.display_name.clone(),
            created: self.created,
            updated: self.updated,
            footprints: wire_footprints,
        };
        let mut out = toml::to_string_pretty(&wire).map_err(FootprintFileError::TomlSerialize)?;
        for (idx, payload) in tsv_payloads.iter().enumerate() {
            let needle = format!("\"{PADS_TSV_PLACEHOLDER_PREFIX}{idx}__\"");
            let replacement = format!("'''\n{payload}'''");
            out = out.replace(&needle, &replacement);
        }
        Ok(out)
    }

    /// Locate a footprint by UUID within this file.
    pub fn get_footprint(&self, uuid: Uuid) -> Option<&Footprint> {
        self.footprints.iter().find(|f| f.uuid == uuid)
    }

    pub fn get_footprint_mut(&mut self, uuid: Uuid) -> Option<&mut Footprint> {
        self.footprints.iter_mut().find(|f| f.uuid == uuid)
    }
}

// ---- Pad TSV codec --------------------------------------------------

fn pad_kind_token(k: PadKind) -> &'static str {
    match k {
        PadKind::Smd => "Smd",
        PadKind::Tht => "Tht",
        PadKind::NptHole => "NptHole",
        PadKind::ConnectorPad => "ConnectorPad",
        PadKind::Castellated => "Castellated",
        PadKind::Fiducial => "Fiducial",
    }
}

fn pad_kind_from_token(s: &str) -> Result<PadKind, FootprintFileError> {
    Ok(match s {
        "Smd" => PadKind::Smd,
        "Tht" => PadKind::Tht,
        "NptHole" => PadKind::NptHole,
        "ConnectorPad" => PadKind::ConnectorPad,
        "Castellated" => PadKind::Castellated,
        "Fiducial" => PadKind::Fiducial,
        other => {
            return Err(FootprintFileError::UnknownEnumToken {
                kind: "PadKind",
                got: other.to_string(),
            });
        }
    })
}

/// HI-10: see [`crate::primitive::symbol::fmt_f64`] — same NaN/inf guard.
fn fmt_f64_fp(v: f64) -> String {
    if v == 0.0 {
        "0".to_string()
    } else if !v.is_finite() {
        debug_assert!(v.is_finite(), "fmt_f64_fp called with non-finite {v}");
        String::new()
    } else {
        format!("{v}")
    }
}

fn fmt_opt_f64_fp(v: Option<f64>) -> String {
    v.map(fmt_f64_fp).unwrap_or_default()
}

fn pad_shape_to_token(shape: &PadShape) -> Result<String, FootprintFileError> {
    Ok(match shape {
        PadShape::Round => "round".to_string(),
        PadShape::Rect => "rect".to_string(),
        PadShape::Oval => "oval".to_string(),
        PadShape::RoundRect { radius_ratio } => {
            format!("round_rect:{}", fmt_f64_fp(*radius_ratio))
        }
        PadShape::Chamfered {
            chamfer_ratio,
            corners,
        } => {
            let bits = format!(
                "{}{}{}{}",
                bool_bit(corners.top_left),
                bool_bit(corners.top_right),
                bool_bit(corners.bottom_left),
                bool_bit(corners.bottom_right),
            );
            format!("chamfered:{}:{}", fmt_f64_fp(*chamfer_ratio), bits)
        }
        PadShape::Custom(poly) => {
            let mut parts: Vec<String> = Vec::with_capacity(poly.points.len());
            for p in &poly.points {
                parts.push(format!("{},{}", fmt_f64_fp(p[0]), fmt_f64_fp(p[1])));
            }
            format!("custom:{}", parts.join("|"))
        }
    })
}

fn bool_bit(b: bool) -> char {
    if b { '1' } else { '0' }
}

fn pad_shape_from_token(s: &str) -> Result<PadShape, FootprintFileError> {
    let invalid = || FootprintFileError::InvalidPadShape(s.to_string());
    if s == "round" {
        return Ok(PadShape::Round);
    }
    if s == "rect" {
        return Ok(PadShape::Rect);
    }
    if s == "oval" {
        return Ok(PadShape::Oval);
    }
    if let Some(rest) = s.strip_prefix("round_rect:") {
        let radius_ratio: f64 = rest.parse().map_err(|_| invalid())?;
        return Ok(PadShape::RoundRect { radius_ratio });
    }
    if let Some(rest) = s.strip_prefix("chamfered:") {
        let mut parts = rest.splitn(2, ':');
        let ratio_str = parts.next().ok_or_else(invalid)?;
        let bits_str = parts.next().ok_or_else(invalid)?;
        let chamfer_ratio: f64 = ratio_str.parse().map_err(|_| invalid())?;
        let bits: Vec<char> = bits_str.chars().collect();
        if bits.len() != 4 || bits.iter().any(|c| *c != '0' && *c != '1') {
            return Err(invalid());
        }
        let corners = ChamferedCorners {
            top_left: bits[0] == '1',
            top_right: bits[1] == '1',
            bottom_left: bits[2] == '1',
            bottom_right: bits[3] == '1',
        };
        return Ok(PadShape::Chamfered {
            chamfer_ratio,
            corners,
        });
    }
    if let Some(rest) = s.strip_prefix("custom:") {
        let points: Vec<[f64; 2]> = if rest.is_empty() {
            Vec::new()
        } else {
            let mut points = Vec::new();
            for p in rest.split('|') {
                let mut xy = p.split(',');
                let x_str = xy.next().ok_or_else(invalid)?;
                let y_str = xy.next().ok_or_else(invalid)?;
                if xy.next().is_some() {
                    return Err(invalid());
                }
                let x: f64 = x_str.parse().map_err(|_| invalid())?;
                let y: f64 = y_str.parse().map_err(|_| invalid())?;
                points.push([x, y]);
            }
            points
        };
        return Ok(PadShape::Custom(Polygon::new(points)));
    }
    Err(invalid())
}

fn layers_to_token(layers: &[LayerId]) -> Result<String, FootprintFileError> {
    for layer in layers {
        if layer.as_str().contains('|') {
            return Err(FootprintFileError::InvalidTsvCell {
                column: "layers",
                value: layer.as_str().to_string(),
            });
        }
    }
    Ok(layers
        .iter()
        .map(|l| l.as_str())
        .collect::<Vec<&str>>()
        .join("|"))
}

fn layers_from_token(s: &str) -> Vec<LayerId> {
    if s.is_empty() {
        Vec::new()
    } else {
        s.split('|').map(LayerId::new).collect()
    }
}

fn parse_f64_cell_fp(col: &'static str, s: &str) -> Result<f64, FootprintFileError> {
    s.parse()
        .map_err(|_| FootprintFileError::InvalidNumericCell {
            column: col,
            value: s.to_string(),
        })
}

fn parse_opt_f64_cell_fp(col: &'static str, s: &str) -> Result<Option<f64>, FootprintFileError> {
    if s.is_empty() {
        Ok(None)
    } else {
        Ok(Some(parse_f64_cell_fp(col, s)?))
    }
}

fn pad_to_tsv_row(pad: &Pad) -> Result<String, FootprintFileError> {
    let shape_cell = pad_shape_to_token(&pad.shape)?;
    let layers_cell = layers_to_token(&pad.layers)?;
    let drill_diameter_cell = pad
        .drill
        .as_ref()
        .map(|d| fmt_f64_fp(d.diameter))
        .unwrap_or_default();
    let drill_slot_cell = pad
        .drill
        .as_ref()
        .and_then(|d| d.slot_length)
        .map(fmt_f64_fp)
        .unwrap_or_default();
    let cells: [String; 13] = [
        pad.number.clone(),
        pad_kind_token(pad.kind).to_string(),
        shape_cell,
        fmt_f64_fp(pad.size[0]),
        fmt_f64_fp(pad.size[1]),
        fmt_f64_fp(pad.position[0]),
        fmt_f64_fp(pad.position[1]),
        fmt_f64_fp(pad.rotation),
        layers_cell,
        drill_diameter_cell,
        drill_slot_cell,
        fmt_opt_f64_fp(pad.solder_mask_margin),
        fmt_opt_f64_fp(pad.paste_margin),
    ];
    for (col, cell) in PAD_TSV_COLUMNS.iter().zip(cells.iter()) {
        if cell.contains('\t') || cell.contains('\n') || cell.contains("'''") {
            return Err(FootprintFileError::InvalidTsvCell {
                column: col,
                value: cell.clone(),
            });
        }
    }
    Ok(cells.join("\t"))
}

/// Encode a slice of pads as TSV — header row first, then one row
/// per pad. Empty slice still emits the header row.
pub(crate) fn pads_to_tsv(pads: &[Pad]) -> Result<String, FootprintFileError> {
    let mut out = String::new();
    out.push_str(&PAD_TSV_COLUMNS.join("\t"));
    out.push('\n');
    for pad in pads {
        out.push_str(&pad_to_tsv_row(pad)?);
        out.push('\n');
    }
    Ok(out)
}

/// Parse a `pads_tsv` payload back into `Vec<Pad>`. The first non-
/// empty line is the header and must equal [`PAD_TSV_COLUMNS`]; each
/// subsequent line is a pad row.
pub(crate) fn pads_from_tsv(tsv: &str) -> Result<Vec<Pad>, FootprintFileError> {
    let trimmed = tsv.trim_matches('\n');
    if trimmed.is_empty() {
        return Err(FootprintFileError::EmptyPadsTsv);
    }
    let mut lines = trimmed.split('\n');
    let header = lines.next().ok_or(FootprintFileError::EmptyPadsTsv)?;
    let header_cols: Vec<&str> = header.split('\t').collect();
    if header_cols.len() != PAD_TSV_COLUMNS.len()
        || header_cols
            .iter()
            .zip(PAD_TSV_COLUMNS.iter())
            .any(|(g, e)| g != e)
    {
        return Err(FootprintFileError::PadsTsvSchemaMismatch {
            got: header_cols.iter().map(|s| (*s).to_string()).collect(),
        });
    }
    let mut pads = Vec::new();
    for (row_idx, line) in lines.enumerate() {
        let cells: Vec<&str> = line.split('\t').collect();
        if cells.len() != PAD_TSV_COLUMNS.len() {
            return Err(FootprintFileError::PadsTsvCellCountMismatch {
                row_index: row_idx,
                got: cells.len(),
                expected: PAD_TSV_COLUMNS.len(),
            });
        }
        pads.push(pad_from_tsv_row(&cells)?);
    }
    Ok(pads)
}

fn pad_from_tsv_row(cells: &[&str]) -> Result<Pad, FootprintFileError> {
    let drill = if cells[9].is_empty() {
        if !cells[10].is_empty() {
            return Err(FootprintFileError::InvalidNumericCell {
                column: "drill_slot_length",
                value: format!(
                    "drill_slot_length set ({:?}) without a drill_diameter",
                    cells[10]
                ),
            });
        }
        None
    } else {
        Some(Drill {
            diameter: parse_f64_cell_fp("drill_diameter", cells[9])?,
            slot_length: parse_opt_f64_cell_fp("drill_slot_length", cells[10])?,
        })
    };
    Ok(Pad {
        number: cells[0].to_string(),
        kind: pad_kind_from_token(cells[1])?,
        shape: pad_shape_from_token(cells[2])?,
        size: [
            parse_f64_cell_fp("size_x", cells[3])?,
            parse_f64_cell_fp("size_y", cells[4])?,
        ],
        position: [
            parse_f64_cell_fp("pos_x", cells[5])?,
            parse_f64_cell_fp("pos_y", cells[6])?,
        ],
        rotation: parse_f64_cell_fp("rotation", cells[7])?,
        layers: layers_from_token(cells[8]),
        drill,
        solder_mask_margin: parse_opt_f64_cell_fp("solder_mask_margin", cells[11])?,
        paste_margin: parse_opt_f64_cell_fp("paste_margin", cells[12])?,
        ..Pad::default()
    })
}

/// Error variants raised by [`FootprintFile`] parsers + serialisers.
#[derive(Debug, thiserror::Error)]
pub enum FootprintFileError {
    #[error("empty .snxfpt file")]
    Empty,
    #[error("invalid UTF-8 in TOML payload: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("TOML deserialise failed: {0}")]
    TomlDeserialize(#[from] toml::de::Error),
    #[error("TOML serialise failed: {0}")]
    TomlSerialize(toml::ser::Error),
    #[error("unsupported .snxfpt format token {got:?}; this build supports \"snxfpt/1\"")]
    UnsupportedFormat { got: String },
    #[error(
        "TSV cell in column {column:?} contains a tab, newline, or triple-quote: \
         {value:?}; cells must be free of \\t, \\n, and the literal \"'''\""
    )]
    InvalidTsvCell { column: &'static str, value: String },
    #[error("pads_tsv block is empty (no header row)")]
    EmptyPadsTsv,
    #[error("pads_tsv header does not match the expected schema; got columns {got:?}")]
    PadsTsvSchemaMismatch { got: Vec<String> },
    #[error("pads_tsv row {row_index} has {got} cells; header declares {expected}")]
    PadsTsvCellCountMismatch {
        row_index: usize,
        got: usize,
        expected: usize,
    },
    #[error("unknown {kind} token {got:?} in pads_tsv cell")]
    UnknownEnumToken { kind: &'static str, got: String },
    #[error("invalid pad shape token {0:?}")]
    InvalidPadShape(String),
    #[error("invalid numeric cell in column {column:?}: {value:?}")]
    InvalidNumericCell { column: &'static str, value: String },
}

impl Footprint {
    /// Empty footprint with no pads — what the New Component flow seeds.
    pub fn empty(name: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            uuid: Uuid::now_v7(),
            name: name.into(),
            anchor: [0.0, 0.0],
            pads: Vec::new(),
            courtyard: Polygon::default(),
            silk_f: Vec::new(),
            silk_b: Vec::new(),
            fab_f: Vec::new(),
            fab_b: Vec::new(),
            body_3d: Body3D::default(),
            step_attachment: None,
            pcb_params: ParamMap::new(),
            version: default_footprint_version(),
            released: false,
            created: now,
            updated: now,
            schema_version: FOOTPRINT_SCHEMA_VERSION,
            sketch: None,
            pours: Vec::new(),
            keepouts: Vec::new(),
            cutouts: Vec::new(),
            v_scores: Vec::new(),
            mask_openings: Vec::new(),
            mask_excludes: Vec::new(),
            paste_apertures: Vec::new(),
            description: String::new(),
            default_designator: String::new(),
            component_type: ComponentType::Standard,
            height_mm: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_pad(num: &str) -> Pad {
        Pad {
            number: num.into(),
            kind: PadKind::Smd,
            shape: PadShape::Rect,
            size: [1.025, 1.4],
            position: [0.0, 0.0],
            rotation: 0.0,
            layers: vec![LayerId::new("F.Cu"), LayerId::new("F.Mask")],
            drill: None,
            solder_mask_margin: None,
            paste_margin: None,
            ..Pad::default()
        }
    }

    #[test]
    fn footprint_json_roundtrip_with_body3d() {
        let fp = Footprint {
            uuid: Uuid::now_v7(),
            name: "SOIC-8".into(),
            anchor: [0.0, 0.0],
            pads: vec![fixture_pad("1"), fixture_pad("2")],
            courtyard: Polygon::new(vec![[-2.5, -2.5], [2.5, -2.5], [2.5, 2.5], [-2.5, 2.5]]),
            silk_f: vec![FpGraphic {
                kind: FpGraphicKind::Line {
                    from: [-1.0, 0.0],
                    to: [1.0, 0.0],
                },
                stroke_width: 0.12,
                filled: false,
            }],
            silk_b: Vec::new(),
            fab_f: Vec::new(),
            fab_b: Vec::new(),
            body_3d: Body3D {
                shape: BodyShape::Extrude,
                height_mm: 1.6,
                offset_z_mm: 0.1,
                top_color: [0.10, 0.10, 0.10, 1.0],
                side_color: [0.20, 0.20, 0.20, 1.0],
                outline: None,
            },
            step_attachment: Some(StepAttachment {
                content_hash: "abcdef0123456789".into(),
                filename: "SOIC-8.step".into(),
                offset_xyz: [0.0, 0.0, 0.5],
                rotation_xyz: [0.0, 0.0, 90.0],
            }),
            pcb_params: ParamMap::new(),
            version: "0.0.1".into(),
            released: false,
            created: Utc::now(),
            updated: Utc::now(),
            schema_version: 2,
            sketch: None,
            pours: Vec::new(),
            keepouts: Vec::new(),
            cutouts: Vec::new(),
            v_scores: Vec::new(),
            mask_openings: Vec::new(),
            mask_excludes: Vec::new(),
            paste_apertures: Vec::new(),
            description: String::new(),
            default_designator: String::new(),
            component_type: ComponentType::Standard,
            height_mm: None,
        };
        let json = serde_json::to_string(&fp).unwrap();
        let back: Footprint = serde_json::from_str(&json).unwrap();
        assert_eq!(fp, back);
    }

    #[test]
    fn body3d_default_is_grey_extrude_at_zero_offset() {
        let b = Body3D::default();
        assert_eq!(b.shape, BodyShape::Extrude);
        assert_eq!(b.offset_z_mm, 0.0);
        assert!(b.outline.is_none());
        // Round-trip must succeed even without explicit fields.
        let json = serde_json::to_string(&b).unwrap();
        let back: Body3D = serde_json::from_str(&json).unwrap();
        assert_eq!(b, back);
    }

    #[test]
    fn pad_kind_round_trip_all_variants() {
        for k in [
            PadKind::Smd,
            PadKind::Tht,
            PadKind::NptHole,
            PadKind::ConnectorPad,
        ] {
            let json = serde_json::to_string(&k).unwrap();
            let back: PadKind = serde_json::from_str(&json).unwrap();
            assert_eq!(k, back);
        }
    }

    #[test]
    fn pad_shape_round_trip_each_variant() {
        let cases = [
            PadShape::Round,
            PadShape::Rect,
            PadShape::RoundRect { radius_ratio: 0.25 },
            PadShape::Oval,
            PadShape::Custom(Polygon::new(vec![[0.0, 0.0], [1.0, 0.0], [0.5, 1.0]])),
        ];
        for s in cases {
            let json = serde_json::to_string(&s).unwrap();
            let back: PadShape = serde_json::from_str(&json).unwrap();
            assert_eq!(s, back);
        }
    }

    #[test]
    fn body_shape_round_trip_all_variants() {
        for s in BodyShape::ALL {
            let json = serde_json::to_string(s).unwrap();
            let back: BodyShape = serde_json::from_str(&json).unwrap();
            assert_eq!(*s, back);
        }
    }

    #[test]
    fn empty_footprint_has_no_pads() {
        let fp = Footprint::empty("test");
        assert_eq!(fp.name, "test");
        assert!(fp.pads.is_empty());
        assert_eq!(fp.body_3d, Body3D::default());
    }

    #[test]
    fn step_attachment_round_trip() {
        let s = StepAttachment {
            content_hash: "0123456789abcdef".into(),
            filename: "Test.step".into(),
            offset_xyz: [1.0, 2.0, 3.0],
            rotation_xyz: [10.0, 20.0, 30.0],
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: StepAttachment = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    // ---- v0.18.2 — FootprintFile TOML envelope round-trip + JSON ----

    #[test]
    fn footprint_file_toml_round_trip_empty() {
        let fp = Footprint::empty("SOIC-8");
        let original = FootprintFile::from_footprint(fp.clone());
        let toml_text = original.to_toml_string().expect("serialise");
        let back = FootprintFile::from_toml_str(&toml_text).expect("parse");
        assert_eq!(back.footprints.len(), 1);
        assert_eq!(back.footprints[0].name, "SOIC-8");
        assert_eq!(back.format, "snxfpt/1");
        assert_eq!(back.file_uuid, original.file_uuid);
    }

    #[test]
    fn footprint_file_toml_round_trip_with_pads() {
        let mut fp = Footprint::empty("R0805");
        fp.pads.push(fixture_pad("1"));
        fp.pads.push(fixture_pad("2"));
        let original = FootprintFile::from_footprint(fp);
        let toml_text = original.to_toml_string().expect("serialise");
        let back = FootprintFile::from_toml_str(&toml_text).expect("parse");
        assert_eq!(back.footprints[0].pads.len(), 2);
        assert_eq!(back.footprints[0].pads[0].number, "1");
        assert_eq!(back.footprints[0].pads[1].number, "2");
    }

    #[test]
    fn footprint_file_toml_round_trip_multi() {
        let mut file = FootprintFile::from_footprint(Footprint::empty("SOIC-8"));
        file.footprints.push(Footprint::empty("QFN-16"));
        file.footprints.push(Footprint::empty("R0805"));
        file.display_name = "Reference parts".into();
        let toml_text = file.to_toml_string().expect("serialise");
        let back = FootprintFile::from_toml_str(&toml_text).expect("parse");
        assert_eq!(back.footprints.len(), 3);
        let names: Vec<&str> = back.footprints.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["SOIC-8", "QFN-16", "R0805"]);
    }

    #[test]
    fn footprint_file_from_bytes_decodes_toml_envelope() {
        let mut file = FootprintFile::from_footprint(Footprint::empty("TOML-Test"));
        file.footprints.push(Footprint::empty("Second"));
        let toml_bytes = file.to_toml_string().unwrap().into_bytes();
        let back = FootprintFile::from_bytes(&toml_bytes).expect("parse");
        assert_eq!(back.footprints.len(), 2);
        assert_eq!(back.footprints[0].name, "TOML-Test");
    }

    #[test]
    fn footprint_file_from_bytes_rejects_empty_payload() {
        match FootprintFile::from_bytes(b"   \n  \t\n") {
            Err(FootprintFileError::Empty) => {}
            other => panic!("expected Empty, got {other:?}"),
        }
    }

    // ---- v0.18.4 — pad TSV codec ------------------------------------

    #[test]
    fn pad_kind_token_round_trip_all_variants() {
        for k in [
            PadKind::Smd,
            PadKind::Tht,
            PadKind::NptHole,
            PadKind::ConnectorPad,
            PadKind::Castellated,
            PadKind::Fiducial,
        ] {
            let token = pad_kind_token(k);
            let back = pad_kind_from_token(token).unwrap();
            assert_eq!(k, back);
        }
    }

    #[test]
    fn pad_shape_token_round_trip_each_variant() {
        let cases = [
            PadShape::Round,
            PadShape::Rect,
            PadShape::Oval,
            PadShape::RoundRect { radius_ratio: 0.25 },
            PadShape::Chamfered {
                chamfer_ratio: 0.4,
                corners: ChamferedCorners {
                    top_left: true,
                    top_right: false,
                    bottom_left: true,
                    bottom_right: false,
                },
            },
            PadShape::Custom(Polygon::new(vec![[0.0, 0.0], [1.5, 0.0], [0.75, 1.0]])),
            PadShape::Custom(Polygon::new(Vec::new())),
        ];
        for s in cases {
            let token = pad_shape_to_token(&s).unwrap();
            let back = pad_shape_from_token(&token).unwrap();
            assert_eq!(s, back, "round-trip failed via token {token:?}");
        }
    }

    #[test]
    fn pads_to_tsv_empty_emits_header_only() {
        let tsv = pads_to_tsv(&[]).expect("serialise");
        assert_eq!(tsv, format!("{}\n", PAD_TSV_COLUMNS.join("\t")));
    }

    #[test]
    fn pads_to_tsv_rejects_tab_in_cell() {
        let mut pad = fixture_pad("1");
        pad.number = "1\t2".into();
        match pads_to_tsv(std::slice::from_ref(&pad)) {
            Err(FootprintFileError::InvalidTsvCell { column, .. }) => {
                assert_eq!(column, "number");
            }
            other => panic!("expected InvalidTsvCell, got {other:?}"),
        }
    }

    #[test]
    fn pads_from_tsv_rejects_schema_mismatch() {
        let bad = "foo\tbar\n1\t2\n";
        match pads_from_tsv(bad) {
            Err(FootprintFileError::PadsTsvSchemaMismatch { got }) => {
                assert_eq!(got, vec!["foo", "bar"]);
            }
            other => panic!("expected PadsTsvSchemaMismatch, got {other:?}"),
        }
    }

    #[test]
    fn pads_from_tsv_rejects_drill_slot_without_diameter() {
        // 13 cells: drill_diameter (col 9) empty, drill_slot_length
        // (col 10) non-empty → invariant violation.
        let header = PAD_TSV_COLUMNS.join("\t");
        let row = "1\tSmd\trect\t1.5\t1.5\t0\t0\t0\tF.Cu\t\t2.0\t\t";
        let body = format!("{header}\n{row}\n");
        match pads_from_tsv(&body) {
            Err(FootprintFileError::InvalidNumericCell { column, .. }) => {
                assert_eq!(column, "drill_slot_length");
            }
            other => panic!("expected InvalidNumericCell, got {other:?}"),
        }
    }

    /// All-fields round-trip — every Pad field gets a non-default
    /// value (chamfered shape, non-trivial drill, multiple layers,
    /// solder/paste margins) so the TSV cell encoders / decoders are
    /// exercised end-to-end.
    #[test]
    fn footprint_file_round_trip_with_full_pad_payload() {
        let pad = Pad {
            number: "EP".into(),
            kind: PadKind::Tht,
            shape: PadShape::Chamfered {
                chamfer_ratio: 0.3,
                corners: ChamferedCorners {
                    top_left: false,
                    top_right: true,
                    bottom_left: true,
                    bottom_right: false,
                },
            },
            size: [2.5, 1.6],
            position: [-0.75, 1.25],
            rotation: 45.0,
            layers: vec![
                LayerId::new("F.Cu"),
                LayerId::new("F.Mask"),
                LayerId::new("F.Paste"),
            ],
            drill: Some(Drill {
                diameter: 0.8,
                slot_length: Some(2.4),
            }),
            solder_mask_margin: Some(0.05),
            paste_margin: Some(-0.025),
            ..Pad::default()
        };
        let mut fp = Footprint::empty("CUSTOM");
        fp.pads = vec![pad.clone()];
        let file = FootprintFile::from_footprint(fp);
        let toml_text = file.to_toml_string().expect("serialise");
        let back = FootprintFile::from_toml_str(&toml_text).expect("parse");
        assert_eq!(back.footprints[0].pads.len(), 1);
        assert_eq!(back.footprints[0].pads[0], pad);
    }

    #[test]
    fn footprint_file_round_trip_with_custom_polygon_pad() {
        let pad = Pad {
            number: "1".into(),
            kind: PadKind::Smd,
            shape: PadShape::Custom(Polygon::new(vec![
                [0.0, 0.0],
                [1.0, 0.0],
                [1.5, 0.5],
                [1.0, 1.0],
                [0.0, 1.0],
            ])),
            size: [1.5, 1.0],
            position: [0.0, 0.0],
            rotation: 0.0,
            layers: vec![LayerId::new("F.Cu")],
            drill: None,
            solder_mask_margin: None,
            paste_margin: None,
            ..Pad::default()
        };
        let mut fp = Footprint::empty("CUSTOM");
        fp.pads = vec![pad.clone()];
        let file = FootprintFile::from_footprint(fp);
        let toml_text = file.to_toml_string().expect("serialise");
        let back = FootprintFile::from_toml_str(&toml_text).expect("parse");
        assert_eq!(back.footprints[0].pads[0], pad);
    }

    #[test]
    fn footprint_file_to_toml_emits_pads_as_literal_multiline() {
        let mut fp = Footprint::empty("Demo");
        fp.pads.push(fixture_pad("1"));
        let toml_text = FootprintFile::from_footprint(fp).to_toml_string().unwrap();
        assert!(
            toml_text.contains("pads_tsv = '''"),
            "expected literal multi-line opener; got:\n{toml_text}"
        );
        assert!(
            !toml_text.contains(PADS_TSV_PLACEHOLDER_PREFIX),
            "placeholder should be fully replaced; got:\n{toml_text}"
        );
    }

    #[test]
    fn footprint_file_unsupported_format_token_is_rejected() {
        // Any token other than "snxfpt/1" must surface
        // FootprintFileError::UnsupportedFormat.
        let bad = r#"
format = "snxfpt/99"
file_uuid = "00000000-0000-0000-0000-000000000000"
display_name = ""
created = "2026-05-04T00:00:00Z"
updated = "2026-05-04T00:00:00Z"
footprints = []
"#;
        match FootprintFile::from_toml_str(bad) {
            Err(FootprintFileError::UnsupportedFormat { got }) => {
                assert_eq!(got, "snxfpt/99");
            }
            other => panic!("expected UnsupportedFormat, got {other:?}"),
        }
    }

    #[test]
    fn footprint_file_get_by_uuid() {
        let a = Footprint::empty("A");
        let b = Footprint::empty("B");
        let a_uuid = a.uuid;
        let b_uuid = b.uuid;
        let mut file = FootprintFile::from_footprint(a);
        file.footprints.push(b);
        assert_eq!(
            file.get_footprint(a_uuid).map(|f| f.name.as_str()),
            Some("A")
        );
        assert_eq!(
            file.get_footprint(b_uuid).map(|f| f.name.as_str()),
            Some("B")
        );
        assert!(file.get_footprint(Uuid::now_v7()).is_none());
    }
}
