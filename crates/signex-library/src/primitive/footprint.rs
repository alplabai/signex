//! `Footprint` primitive — PCB-side reusable shape.
//!
//! Per `v0.9-library-refactor-plan.md` §2.2, a `Footprint` carries:
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
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
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
            created: now,
            updated: now,
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
            created: Utc::now(),
            updated: Utc::now(),
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
}
