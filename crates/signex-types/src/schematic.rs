use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Point -- KiCad mm-space coordinate (f64). Copy-friendly.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const ZERO: Point = Point { x: 0.0, y: 0.0 };

    pub const fn new(x: f64, y: f64) -> Self {
        Point { x, y }
    }
}

impl Default for Point {
    fn default() -> Self {
        Point::ZERO
    }
}

// ---------------------------------------------------------------------------
// Alignment enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HAlign {
    Left,
    #[default]
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VAlign {
    Top,
    #[default]
    Center,
    Bottom,
}

// ---------------------------------------------------------------------------
// Fill
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FillType {
    #[default]
    None,
    Outline,
    Background,
}

// ---------------------------------------------------------------------------
// Pin types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PinElectricalType {
    Input,
    Output,
    Bidirectional,
    TriState,
    Passive,
    Free,
    Unspecified,
    PowerIn,
    PowerOut,
    OpenCollector,
    OpenEmitter,
    NotConnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PinShape {
    Line,
    Inverted,
    Clock,
    InvertedClock,
    InputLow,
    ClockLow,
    OutputLow,
    EdgeClockHigh,
    NonLogic,
}

// ---------------------------------------------------------------------------
// Label type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelType {
    Net,
    Global,
    Hierarchical,
    Power,
}

// ---------------------------------------------------------------------------
// Text property (for reference/value fields)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextProp {
    pub position: Point,
    pub rotation: f64,
    pub font_size: f64,
    #[serde(default)]
    pub justify_h: HAlign,
    #[serde(default)]
    pub justify_v: VAlign,
    #[serde(default)]
    pub hidden: bool,
}

// ---------------------------------------------------------------------------
// LibSymbol & graphics
// ---------------------------------------------------------------------------

/// A graphic primitive inside a library symbol, tagged with unit and body-style
/// so the renderer can filter to only draw the correct unit for each instance.
///
/// - `unit == 0`       → common to ALL units (always rendered)
/// - `unit == N`       → only rendered for symbol instances with `unit = N`
/// - `body_style == 0` → common to all body styles (normal + De Morgan)
/// - `body_style == 1` → normal body style (default)
/// - `body_style == 2` → De Morgan body style
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibGraphic {
    #[serde(default)]
    pub unit: u32,
    #[serde(default = "default_body_style")]
    pub body_style: u32,
    pub graphic: Graphic,
}

/// A pin inside a library symbol, tagged with unit and body-style.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibPin {
    #[serde(default)]
    pub unit: u32,
    #[serde(default = "default_body_style")]
    pub body_style: u32,
    pub pin: Pin,
}

fn default_body_style() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibSymbol {
    pub id: String,
    #[serde(default)]
    pub graphics: Vec<LibGraphic>,
    #[serde(default)]
    pub pins: Vec<LibPin>,
    #[serde(default = "default_true")]
    pub show_pin_numbers: bool,
    #[serde(default = "default_true")]
    pub show_pin_names: bool,
    #[serde(default)]
    pub pin_name_offset: f64,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Graphic {
    Polyline {
        points: Vec<Point>,
        #[serde(default)]
        width: f64,
        #[serde(default)]
        fill: FillType,
    },
    Rectangle {
        start: Point,
        end: Point,
        #[serde(default)]
        width: f64,
        #[serde(default)]
        fill: FillType,
    },
    Circle {
        center: Point,
        radius: f64,
        #[serde(default)]
        width: f64,
        #[serde(default)]
        fill: FillType,
    },
    Arc {
        start: Point,
        mid: Point,
        end: Point,
        #[serde(default)]
        width: f64,
        #[serde(default)]
        fill: FillType,
    },
    Text {
        text: String,
        position: Point,
        #[serde(default)]
        rotation: f64,
        #[serde(default)]
        font_size: f64,
        #[serde(default)]
        bold: bool,
        #[serde(default)]
        italic: bool,
        #[serde(default)]
        justify_h: HAlign,
        #[serde(default)]
        justify_v: VAlign,
    },
    TextBox {
        text: String,
        position: Point,
        #[serde(default)]
        rotation: f64,
        size: Point,
        #[serde(default)]
        font_size: f64,
        #[serde(default)]
        bold: bool,
        #[serde(default)]
        italic: bool,
        #[serde(default)]
        width: f64,
        #[serde(default)]
        fill: FillType,
    },
}

// ---------------------------------------------------------------------------
// Pin
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pin {
    pub pin_type: PinElectricalType,
    pub shape: PinShape,
    pub position: Point,
    #[serde(default)]
    pub rotation: f64,
    #[serde(default)]
    pub length: f64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub number: String,
    #[serde(default = "default_true")]
    pub name_visible: bool,
    #[serde(default = "default_true")]
    pub number_visible: bool,
}

// ---------------------------------------------------------------------------
// Symbol instance
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub uuid: Uuid,
    pub lib_id: String,
    #[serde(default)]
    pub reference: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub footprint: String,
    pub position: Point,
    #[serde(default)]
    pub rotation: f64,
    #[serde(default)]
    pub mirror_x: bool,
    #[serde(default)]
    pub mirror_y: bool,
    #[serde(default = "default_unit")]
    pub unit: u32,
    #[serde(default)]
    pub is_power: bool,
    pub ref_text: Option<TextProp>,
    pub val_text: Option<TextProp>,
    #[serde(default)]
    pub fields_autoplaced: bool,
    #[serde(default)]
    pub dnp: bool,
    #[serde(default = "default_true")]
    pub in_bom: bool,
    #[serde(default = "default_true")]
    pub on_board: bool,
    #[serde(default)]
    pub exclude_from_sim: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub fields: HashMap<String, String>,
}

fn default_unit() -> u32 {
    1
}

// ---------------------------------------------------------------------------
// Wiring primitives
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wire {
    pub uuid: Uuid,
    pub start: Point,
    pub end: Point,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Junction {
    pub uuid: Uuid,
    pub position: Point,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub uuid: Uuid,
    pub text: String,
    pub position: Point,
    #[serde(default)]
    pub rotation: f64,
    pub label_type: LabelType,
    #[serde(default)]
    pub shape: String,
    #[serde(default)]
    pub font_size: f64,
    #[serde(default)]
    pub justify: HAlign,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoConnect {
    pub uuid: Uuid,
    pub position: Point,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextNote {
    pub uuid: Uuid,
    pub text: String,
    pub position: Point,
    #[serde(default)]
    pub rotation: f64,
    #[serde(default)]
    pub font_size: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bus {
    pub uuid: Uuid,
    pub start: Point,
    pub end: Point,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusEntry {
    pub uuid: Uuid,
    pub position: Point,
    pub size: (f64, f64),
}

// ---------------------------------------------------------------------------
// Hierarchical sheets
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetPin {
    pub uuid: Uuid,
    pub name: String,
    #[serde(default)]
    pub direction: String,
    pub position: Point,
    #[serde(default)]
    pub rotation: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildSheet {
    pub uuid: Uuid,
    pub name: String,
    pub filename: String,
    pub position: Point,
    pub size: (f64, f64),
    #[serde(default)]
    pub pins: Vec<SheetPin>,
}

// ---------------------------------------------------------------------------
// Schematic drawing primitives
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SchDrawing {
    Line {
        uuid: Uuid,
        start: Point,
        end: Point,
        #[serde(default)]
        width: f64,
    },
    Rect {
        uuid: Uuid,
        start: Point,
        end: Point,
        #[serde(default)]
        width: f64,
        #[serde(default)]
        fill: FillType,
    },
    Circle {
        uuid: Uuid,
        center: Point,
        radius: f64,
        #[serde(default)]
        width: f64,
        #[serde(default)]
        fill: FillType,
    },
    Arc {
        uuid: Uuid,
        start: Point,
        mid: Point,
        end: Point,
        #[serde(default)]
        width: f64,
    },
    Polyline {
        uuid: Uuid,
        points: Vec<Point>,
        #[serde(default)]
        width: f64,
        #[serde(default)]
        fill: bool,
    },
}

// ---------------------------------------------------------------------------
// Top-level sheet
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchematicSheet {
    pub uuid: Uuid,
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub generator: String,
    #[serde(default)]
    pub generator_version: String,
    #[serde(default)]
    pub paper_size: String,
    #[serde(default)]
    pub symbols: Vec<Symbol>,
    #[serde(default)]
    pub wires: Vec<Wire>,
    #[serde(default)]
    pub junctions: Vec<Junction>,
    #[serde(default)]
    pub labels: Vec<Label>,
    #[serde(default)]
    pub child_sheets: Vec<ChildSheet>,
    #[serde(default)]
    pub no_connects: Vec<NoConnect>,
    #[serde(default)]
    pub text_notes: Vec<TextNote>,
    #[serde(default)]
    pub buses: Vec<Bus>,
    #[serde(default)]
    pub bus_entries: Vec<BusEntry>,
    #[serde(default)]
    pub drawings: Vec<SchDrawing>,
    #[serde(default)]
    pub no_erc_directives: Vec<NoConnect>,
    #[serde(default)]
    pub title_block: HashMap<String, String>,
    #[serde(default)]
    pub lib_symbols: HashMap<String, LibSymbol>,
}
