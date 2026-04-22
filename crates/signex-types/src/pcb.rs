use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::property::PcbProperty;

pub use crate::schematic::Point;

// ---------------------------------------------------------------------------
// PCB domain constants
// ---------------------------------------------------------------------------

/// Standard PCB board thickness in mm (1.6 mm = 63 mil).
pub const PCB_DEFAULT_THICKNESS_MM: f64 = 1.6;

/// Default PCB grid size in mm (1.27 mm = 50 mil).
pub const PCB_GRID_MM: f64 = 1.27;

/// Default trace / segment width in mm (0.25 mm = ~10 mil).
pub const PCB_DEFAULT_TRACE_WIDTH_MM: f64 = 0.25;

/// Minimum trace width fallback in mm (0.1 mm = ~4 mil).
pub const PCB_TRACK_MIN_MM: f64 = 0.1;

/// Default via finished-hole diameter in mm (0.6 mm = ~24 mil).
pub const PCB_DEFAULT_VIA_DIAMETER_MM: f64 = 0.6;

/// Default via drill diameter in mm (0.3 mm = ~12 mil).
pub const PCB_DEFAULT_VIA_DRILL_MM: f64 = 0.3;

/// Minimum via diameter fallback in mm (0.4 mm = ~16 mil).
pub const PCB_VIA_MIN_DIAMETER_MM: f64 = 0.4;

/// Minimum via drill fallback in mm (0.2 mm = ~8 mil).
pub const PCB_VIA_MIN_DRILL_MM: f64 = 0.2;

/// Default design-rule clearance in mm (0.2 mm = ~8 mil).
pub const PCB_DEFAULT_CLEARANCE_MM: f64 = 0.2;

/// Default copper-zone minimum fill thickness in mm (0.254 mm = 10 mil).
pub const PCB_ZONE_MIN_THICKNESS_MM: f64 = 0.254;

/// Default copper-zone thermal relief gap in mm (0.508 mm = 20 mil).
pub const PCB_THERMAL_GAP_MM: f64 = 0.508;

/// Default copper-zone thermal bridge width in mm (0.254 mm = 10 mil).
pub const PCB_THERMAL_BRIDGE_MM: f64 = 0.254;

/// Default graphic / silkscreen line width in mm (0.1 mm = ~4 mil).
pub const PCB_DEFAULT_GRAPHIC_WIDTH_MM: f64 = 0.1;

/// Default pad size (width and height) in mm (1.0 mm = ~39 mil).
pub const PCB_DEFAULT_PAD_SIZE_MM: f64 = 1.0;

/// Default PCB text / fp_text font size in mm (1.0 mm = ~39 mil).
pub const PCB_DEFAULT_TEXT_SIZE_MM: f64 = 1.0;

/// Stroke thickness for PCB text in mm (0.15 mm = ~6 mil).
pub const PCB_TEXT_THICKNESS_MM: f64 = 0.15;

/// Vertical offset for auto-placed Reference / Value fp_text in mm (2.0 mm = ~79 mil).
pub const PCB_FP_TEXT_OFFSET_MM: f64 = 2.0;

/// Point-coincidence epsilon for outline deduplication (0.01 mm).
pub const PCB_COINCIDENCE_EPS: f64 = 0.01;

// ---------------------------------------------------------------------------
// Pad / Via enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PadType {
    Thru,
    Smd,
    Connect,
    NpThru,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PadShape {
    Circle,
    Rect,
    Oval,
    Trapezoid,
    RoundRect,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViaType {
    Through,
    Blind,
    Micro,
}

// ---------------------------------------------------------------------------
// Layer definition
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerDef {
    pub id: u8,
    pub name: String,
    pub layer_type: String,
}

// ---------------------------------------------------------------------------
// PCB setup
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PcbSetup {
    #[serde(default)]
    pub grid_size: f64,
    #[serde(default)]
    pub trace_width: f64,
    #[serde(default)]
    pub via_diameter: f64,
    #[serde(default)]
    pub via_drill: f64,
    #[serde(default)]
    pub clearance: f64,
    #[serde(default)]
    pub track_min_width: f64,
    #[serde(default)]
    pub via_min_diameter: f64,
    #[serde(default)]
    pub via_min_drill: f64,
}

// ---------------------------------------------------------------------------
// Net definition
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetDef {
    pub number: u32,
    pub name: String,
}

// ---------------------------------------------------------------------------
// Drill definition
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrillDef {
    pub diameter: f64,
    #[serde(default)]
    pub shape: String,
}

// ---------------------------------------------------------------------------
// Pad net reference
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PadNet {
    pub number: u32,
    pub name: String,
}

// ---------------------------------------------------------------------------
// Pad
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pad {
    pub uuid: Uuid,
    #[serde(default)]
    pub number: String,
    pub pad_type: PadType,
    pub shape: PadShape,
    pub position: Point,
    pub size: Point,
    pub drill: Option<DrillDef>,
    #[serde(default)]
    pub layers: Vec<String>,
    pub net: Option<PadNet>,
    #[serde(default)]
    pub roundrect_ratio: f64,
}

// ---------------------------------------------------------------------------
// Footprint graphic
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FpGraphic {
    #[serde(default)]
    pub graphic_type: String,
    #[serde(default)]
    pub layer: String,
    #[serde(default)]
    pub width: f64,
    pub start: Option<Point>,
    pub end: Option<Point>,
    pub center: Option<Point>,
    pub mid: Option<Point>,
    #[serde(default)]
    pub radius: f64,
    #[serde(default)]
    pub points: Vec<Point>,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub font_size: f64,
    pub position: Option<Point>,
    #[serde(default)]
    pub rotation: f64,
    #[serde(default)]
    pub fill: String,
}

// ---------------------------------------------------------------------------
// Footprint
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Footprint {
    pub uuid: Uuid,
    #[serde(default)]
    pub reference: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub footprint_id: String,
    pub position: Point,
    #[serde(default)]
    pub rotation: f64,
    #[serde(default)]
    pub layer: String,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub pads: Vec<Pad>,
    #[serde(default)]
    pub graphics: Vec<FpGraphic>,
    #[serde(default)]
    pub properties: Vec<PcbProperty>,
}

// ---------------------------------------------------------------------------
// Traces / routing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub uuid: Uuid,
    pub start: Point,
    pub end: Point,
    #[serde(default)]
    pub width: f64,
    #[serde(default)]
    pub layer: String,
    #[serde(default)]
    pub net: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Via {
    pub uuid: Uuid,
    pub position: Point,
    #[serde(default)]
    pub diameter: f64,
    #[serde(default)]
    pub drill: f64,
    #[serde(default)]
    pub layers: Vec<String>,
    #[serde(default)]
    pub net: u32,
    #[serde(default = "default_via_type")]
    pub via_type: ViaType,
}

fn default_via_type() -> ViaType {
    ViaType::Through
}

// ---------------------------------------------------------------------------
// Zone
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Zone {
    pub uuid: Uuid,
    #[serde(default)]
    pub net: u32,
    #[serde(default)]
    pub net_name: String,
    #[serde(default)]
    pub layer: String,
    #[serde(default)]
    pub outline: Vec<Point>,
    #[serde(default)]
    pub priority: u32,
    #[serde(default)]
    pub fill_type: String,
    #[serde(default)]
    pub thermal_relief: bool,
    #[serde(default)]
    pub thermal_gap: f64,
    #[serde(default)]
    pub thermal_width: f64,
    #[serde(default)]
    pub clearance: f64,
    #[serde(default)]
    pub min_thickness: f64,
}

// ---------------------------------------------------------------------------
// Board-level graphics and text
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardGraphic {
    #[serde(default)]
    pub graphic_type: String,
    #[serde(default)]
    pub layer: String,
    #[serde(default)]
    pub width: f64,
    pub start: Option<Point>,
    pub end: Option<Point>,
    pub center: Option<Point>,
    #[serde(default)]
    pub radius: f64,
    #[serde(default)]
    pub points: Vec<Point>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardText {
    pub uuid: Uuid,
    #[serde(default)]
    pub text: String,
    pub position: Point,
    #[serde(default)]
    pub layer: String,
    #[serde(default)]
    pub font_size: f64,
    #[serde(default)]
    pub rotation: f64,
}

// ---------------------------------------------------------------------------
// Top-level board
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PcbBoard {
    pub uuid: Uuid,
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub generator: String,
    #[serde(default)]
    pub thickness: f64,
    #[serde(default)]
    pub outline: Vec<Point>,
    #[serde(default)]
    pub layers: Vec<LayerDef>,
    pub setup: Option<PcbSetup>,
    #[serde(default)]
    pub nets: Vec<NetDef>,
    #[serde(default)]
    pub footprints: Vec<Footprint>,
    #[serde(default)]
    pub segments: Vec<Segment>,
    #[serde(default)]
    pub vias: Vec<Via>,
    #[serde(default)]
    pub zones: Vec<Zone>,
    #[serde(default)]
    pub graphics: Vec<BoardGraphic>,
    #[serde(default)]
    pub texts: Vec<BoardText>,
}
