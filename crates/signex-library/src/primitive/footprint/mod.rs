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

mod format_token;
#[cfg(test)]
mod format_token_tests;
#[cfg(test)]
mod format_version_tests;
mod generated;
mod load_repair;
mod models;
#[cfg(test)]
mod models_tests;
mod mount_style;
mod pad;
mod pad_columns;
mod pad_shape_check;
#[cfg(test)]
mod pad_thermal_tests;
mod pitch;
#[cfg(test)]
mod pour_corner_format_tests;
mod serde_tsv;
mod silk_marker;
#[cfg(test)]
mod tests;
mod textconv;
#[cfg(test)]
mod textconv_tests;
mod wire;

#[cfg(test)]
use format_token::FOOTPRINT_FILE_VERSION;
use format_token::SNXFPT_FORMAT_V1_1;
pub use generated::{
    DEFAULT_COURTYARD_LINE_WIDTH_MM, DEFAULT_PAD_CLEARANCE_MM, DEFAULT_SILK_LINE_WIDTH_MM,
    GeneratedFeatures,
};
pub use load_repair::LoadRepair;
pub use models::{
    EmbeddedModel, EmbeddedModels, MODEL_DEFLATE_LEVEL, MODEL_ENCODING, ModelError, ModelFault,
    step_model_hash,
};
pub use mount_style::MountStyle;
pub use pad::*;
pub use serde_tsv::FootprintFileError;
pub(crate) use serde_tsv::{pads_from_tsv, pads_to_tsv};
pub use textconv::textconv_footprint_text;
#[cfg(test)]
use wire::PADS_TSV_PLACEHOLDER_PREFIX;

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
    /// Circular arc about `center`.
    ///
    /// **Sweep convention:** the arc runs from `start_deg` to `end_deg` in
    /// the direction of *increasing* angle (angles measured from +X with
    /// `atan2(dy, dx)`; CCW in a y-up frame, which reads clockwise on the
    /// y-down footprint canvas), wrapping through 360 when `start_deg > end_deg`.
    /// So `300 -> 60` is the 120 degree arc through 0, not the 240 degree
    /// one; `10 -> 80` is the 70 degree arc. Angles may be given outside
    /// `[0, 360)` (the placement tool stores `atan2` output in
    /// `(-180, 180]`). A sweep of 360 or more (`|end - start| >= 360`) is a
    /// full circle and equal angles are a zero-length arc; see
    /// [`FpGraphicKind::arc_sweep_deg`].
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
}

impl FpGraphicKind {
    /// CCW sweep in degrees of an [`FpGraphicKind::Arc`], in `[0, 360]`.
    ///
    /// `0.0` for a degenerate zero-length arc, `360.0` for a full circle
    /// (`|end - start| >= 360`), otherwise `(end - start).rem_euclid(360)`.
    pub fn arc_sweep_deg(start_deg: f64, end_deg: f64) -> f64 {
        let raw = (end_deg - start_deg).abs();
        if raw < 1e-6 {
            0.0
        } else if raw >= 359.999 {
            360.0
        } else {
            (end_deg - start_deg).rem_euclid(360.0)
        }
    }
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
    /// The sketch entity a bake made this entry from; `None` = drawn by
    /// hand. A re-bake replaces only marked entries. See
    /// `docs/FOOTPRINT_SKETCH.md`, "Bake ownership".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sketch_entity: Option<signex_sketch::id::SketchEntityId>,
}

impl FpGraphic {
    /// A hand-drawn graphic: no sketch entity owns it.
    pub fn drawn(kind: FpGraphicKind, stroke_width: f64, filled: bool) -> Self {
        Self {
            kind,
            stroke_width,
            filled,
            sketch_entity: None,
        }
    }

    /// Whether a sketch bake made this entry (it carries a marker).
    pub fn is_baked(&self) -> bool {
        self.sketch_entity.is_some()
    }
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
/// first-class library citizens. v0.14 introduced this as a
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

/// Copper pour / region. Stores the boundary + metadata only; fill
/// generation lands with the PCB Routing plan (Copper Pour).
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

/// What a keepout zone forbids. DRC enforcement lands with the Advanced PCB plan
/// (Routing Polish & Geometry).
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

/// DRC keepout zone. Stores the polygon + layer + forbid kind.
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

/// Optional STEP attachment for mech-CAD export. The model bytes live in the
/// same file's `[[models]]` entry under `content_hash`; a library written
/// before #3107 keeps them in `step/<sha256>.step`, read as a fallback.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StepAttachment {
    /// SHA-256 hex of the STEP bytes: the `[[models]]` entry's `hash`.
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
    /// Changes [`Footprint::repair_stored_sketch`] made to the sketch when
    /// this footprint was read from storage (arc ends moved, dangling
    /// references removed). Transient: never
    /// serialised. The editor that opens the footprint reports and clears it.
    #[serde(skip)]
    pub load_repairs: Vec<LoadRepair>,
    /// Copper pour / region polygons. v0.14+; fill generation lands with
    /// the PCB Routing plan (Copper Pour).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pours: Vec<FpPour>,
    /// DRC keepout zones. Recorded only; enforcement lands with the
    /// Advanced PCB plan.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keepouts: Vec<FpKeepout>,
    /// Board cutouts. v0.14 records; outline subtraction at PCB-export.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cutouts: Vec<FpCutout>,
    /// V-score panelisation hints. Recorded only; the panelisation tool
    /// (Advanced Output plan, Panelization) consumes them.
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
    /// Generated silkscreen, pin 1 and assembly features (`snxfpt/1.3`).
    #[serde(default, skip_serializing_if = "GeneratedFeatures::is_default")]
    pub generated: GeneratedFeatures,
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
    /// The format token the file was read with; a save writes
    /// [`FootprintFile::format_token`].
    #[serde(default = "default_footprint_format")]
    pub format: String,
    /// File-level UUID — distinct from any contained footprint's uuid.
    /// Used as the file-rename-stable handle.
    pub file_uuid: Uuid,
    /// Human-facing library name shown in the Footprint Library
    /// panel header. Defaults to the file stem when empty.
    #[serde(default)]
    pub display_name: String,
    /// The `file_uuid` of the stock file this one was copied from (#813);
    /// see `SymbolFile::copied_from`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copied_from: Option<Uuid>,
    /// All footprints in this file. Order is the Footprint Library
    /// panel order.
    pub footprints: Vec<Footprint>,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    /// STEP models the footprints attach, by hash (`[[models]]`, #3107).
    #[serde(default, skip_serializing_if = "EmbeddedModels::is_empty")]
    pub models: EmbeddedModels,
}

/// Base column layout for the per-footprint `pads_tsv` block; these
/// lead every header. Reordering them is a wire-format break — bump
/// the `.snxfpt` token (`format_token`). The remaining `Pad` fields are
/// optional trailing columns, see `pad_columns`.
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

fn default_footprint_format() -> String {
    SNXFPT_FORMAT_V1_1.to_string()
}

impl FootprintFile {
    /// Build a new container holding a single footprint — what the
    /// `Add New ▸ Footprint Library` flow seeds.
    pub fn from_footprint(footprint: Footprint) -> Self {
        Self::with_identity(footprint, Uuid::now_v7(), Utc::now())
    }

    /// Like [`FootprintFile::from_footprint`] but with caller-supplied
    /// identity, so a generator can emit byte-identical files on every
    /// run (the stock-library emitter pins both).
    pub fn with_identity(footprint: Footprint, file_uuid: Uuid, timestamp: DateTime<Utc>) -> Self {
        Self {
            format: default_footprint_format(),
            file_uuid,
            display_name: footprint.name.clone(),
            copied_from: None,
            footprints: vec![footprint],
            created: timestamp,
            updated: timestamp,
            models: EmbeddedModels::default(),
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

    /// Locate a footprint by UUID within this file.
    pub fn get_footprint(&self, uuid: Uuid) -> Option<&Footprint> {
        self.footprints.iter().find(|f| f.uuid == uuid)
    }
}

impl Footprint {
    /// Empty footprint with no pads — what the New Component flow seeds.
    pub fn empty(name: impl Into<String>) -> Self {
        Self::with_identity(name, Uuid::now_v7(), Utc::now())
    }

    /// Empty footprint with a caller-supplied `uuid` and
    /// `created`/`updated` timestamp instead of freshly minted ones.
    pub fn with_identity(name: impl Into<String>, uuid: Uuid, timestamp: DateTime<Utc>) -> Self {
        Self {
            uuid,
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
            created: timestamp,
            updated: timestamp,
            schema_version: FOOTPRINT_SCHEMA_VERSION,
            sketch: None,
            load_repairs: Vec::new(),
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
            generated: GeneratedFeatures::default(),
        }
    }
}
