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

mod pad;
mod serde_tsv;
#[cfg(test)]
mod tests;

pub use pad::*;
pub use serde_tsv::FootprintFileError;
pub(crate) use serde_tsv::{pads_from_tsv, pads_to_tsv};

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
