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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PadKind {
    /// Surface-mount.
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PadShape {
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    #[serde(default)]
    pub solder_mask_margin: Option<f64>,
    #[serde(default)]
    pub paste_margin: Option<f64>,
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
    },
}

/// One footprint silkscreen / fab graphic.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FpGraphic {
    pub kind: FpGraphicKind,
    /// Stroke width in mm (0.0 = renderer default).
    #[serde(default)]
    pub stroke_width: f64,
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

/// v0.18.2 — multi-footprint container for `.snxfpt` files.
///
/// Wire format: TOML envelope with one `[[footprints]]` array entry
/// per Footprint. Mirrors the `SymbolFile` envelope from
/// `primitive/symbol.rs` but ships TOML-first per the architectural
/// spec (`docs/internal/docs/ARCHITECTURE.md` "TOML envelope + TSV
/// bulk-block pattern matches `.snxlib`/`.snxsym`/`.snxfpt`").
///
/// Reader auto-detects JSON vs TOML so v0.16.5-and-earlier files
/// continue to load unchanged. New files are written as TOML.
///
/// Pads are emitted inline within each footprint's `[[footprints]]`
/// entry. The TSV bulk-block optimisation (separate
/// `[footprints.0.pads]` table with `tsv = '''…'''`) is queued for
/// v0.18.3 — pure TOML works as the v0.18.2 ship target and keeps
/// the serde derive surface flat.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FootprintFile {
    /// Schema sentinel — current emitters write `"snxfpt/1"`.
    /// Legacy JSON files don't carry this field; the loader detects
    /// them via the JSON-vs-TOML format probe and wraps the bare
    /// `Footprint` into a one-element container.
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

fn default_footprint_format() -> String {
    "snxfpt/1".to_string()
}

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

    /// Auto-detecting parse for `.snxfpt` bytes. Probes the first
    /// non-whitespace byte: `{` → JSON (legacy v0.16.5 format), any
    /// other character → TOML (v0.18.2+). Legacy JSON files are
    /// wrapped into a one-element container.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, FootprintFileError> {
        let first_non_ws = bytes
            .iter()
            .find(|b| !b.is_ascii_whitespace())
            .copied()
            .ok_or(FootprintFileError::Empty)?;
        if first_non_ws == b'{' {
            // Legacy JSON path — accept either a bare Footprint OR a
            // pre-existing FootprintFile JSON (defensive — earlier
            // session experiments may have written a JSON envelope).
            #[derive(Deserialize)]
            #[serde(untagged)]
            enum JsonForm {
                Container(FootprintFile),
                Legacy(Footprint),
            }
            match serde_json::from_slice::<JsonForm>(bytes)? {
                JsonForm::Container(file) => Ok(file),
                JsonForm::Legacy(fp) => {
                    let now = Utc::now();
                    Ok(Self {
                        format: default_footprint_format(),
                        file_uuid: fp.uuid,
                        display_name: fp.name.clone(),
                        created: fp.created,
                        updated: now,
                        footprints: vec![fp],
                    })
                }
            }
        } else {
            let text = std::str::from_utf8(bytes)?;
            Self::from_toml_str(text)
        }
    }

    /// Parse the TOML wire format. The format-token check pins us to
    /// `snxfpt/1`; mismatched files surface
    /// [`FootprintFileError::UnsupportedFormat`].
    pub fn from_toml_str(text: &str) -> Result<Self, FootprintFileError> {
        let parsed: Self = toml::from_str(text)?;
        if parsed.format != "snxfpt/1" {
            return Err(FootprintFileError::UnsupportedFormat {
                got: parsed.format,
            });
        }
        Ok(parsed)
    }

    /// Serialise to canonical TOML. Output is deterministic — re-
    /// parsing returns a value equal to `self`.
    pub fn to_toml_string(&self) -> Result<String, FootprintFileError> {
        toml::to_string_pretty(self).map_err(FootprintFileError::TomlSerialize)
    }

    /// Locate a footprint by UUID within this file.
    pub fn get_footprint(&self, uuid: Uuid) -> Option<&Footprint> {
        self.footprints.iter().find(|f| f.uuid == uuid)
    }

    pub fn get_footprint_mut(&mut self, uuid: Uuid) -> Option<&mut Footprint> {
        self.footprints.iter_mut().find(|f| f.uuid == uuid)
    }
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
    #[error("JSON deserialise failed: {0}")]
    JsonDeserialize(#[from] serde_json::Error),
    #[error("unsupported .snxfpt format token {got:?}; this build supports \"snxfpt/1\"")]
    UnsupportedFormat { got: String },
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
    fn footprint_file_from_bytes_auto_detects_legacy_json() {
        // v0.16.5 stock library files contain a bare Footprint JSON.
        // from_bytes should accept these and wrap them.
        let legacy = Footprint::empty("Legacy-Pad");
        let bytes = serde_json::to_vec_pretty(&legacy).unwrap();
        let wrapped = FootprintFile::from_bytes(&bytes).expect("parse");
        assert_eq!(wrapped.footprints.len(), 1);
        assert_eq!(wrapped.footprints[0].name, "Legacy-Pad");
        assert_eq!(wrapped.format, "snxfpt/1");
        assert_eq!(wrapped.file_uuid, legacy.uuid);
    }

    #[test]
    fn footprint_file_from_bytes_auto_detects_toml_envelope() {
        let mut file = FootprintFile::from_footprint(Footprint::empty("TOML-Test"));
        file.footprints.push(Footprint::empty("Second"));
        let toml_bytes = file.to_toml_string().unwrap().into_bytes();
        let back = FootprintFile::from_bytes(&toml_bytes).expect("parse");
        assert_eq!(back.footprints.len(), 2);
        assert_eq!(back.footprints[0].name, "TOML-Test");
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
        assert_eq!(file.get_footprint(a_uuid).map(|f| f.name.as_str()), Some("A"));
        assert_eq!(file.get_footprint(b_uuid).map(|f| f.name.as_str()), Some("B"));
        assert!(file.get_footprint(Uuid::now_v7()).is_none());
    }
}
