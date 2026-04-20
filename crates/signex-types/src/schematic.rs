use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::property::SchematicProperty;

// ---------------------------------------------------------------------------
// Schematic text constants
// ---------------------------------------------------------------------------

/// Default schematic text height: 1.27 mm = 50 mils = 10 Altium pt.
pub const SCHEMATIC_TEXT_MM: f64 = 1.27;

/// Altium schematic point → mm: 1 pt = 0.127 mm (10 pt = 1.27 mm).
pub const SCHEMATIC_PT_TO_MM: f64 = 0.127;

/// KiCad schematic grid step: 2.54 mm = 100 mils. Used as pin length,
/// bus-entry size, and any other default that snaps to the coarse grid.
pub const GRID_MM: f64 = 2.54;

/// KiCad default pin line length in mm (one grid step).
pub const PIN_LENGTH_MM: f64 = GRID_MM;

/// KiCad default offset from pin body-end to pin name text anchor.
pub const PIN_NAME_OFFSET_MM: f64 = 0.508;

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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SymbolInstance {
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub reference: String,
    #[serde(default = "default_unit")]
    pub unit: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SheetInstance {
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub page: String,
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
    pub reference: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub footprint: String,
    #[serde(default)]
    pub datasheet: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub keywords: String,
    #[serde(default)]
    pub fp_filters: String,
    #[serde(default = "default_true")]
    pub in_bom: bool,
    #[serde(default = "default_true")]
    pub on_board: bool,
    #[serde(default = "default_true")]
    pub in_pos_files: bool,
    #[serde(default)]
    pub duplicate_pin_numbers_are_jumpers: bool,
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
    /// Cubic bezier: control points [p0, c1, c2, p3]
    Bezier {
        /// Exactly 4 control points: start, cp1, cp2, end
        points: Vec<Point>,
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
    pub visible: bool,
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
    #[serde(default)]
    pub datasheet: String,
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
    #[serde(default)]
    pub custom_properties: Vec<SchematicProperty>,
    #[serde(default)]
    pub pin_uuids: HashMap<String, Uuid>,
    #[serde(default)]
    pub instances: Vec<SymbolInstance>,
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
    /// Stroke width in mm. 0.0 = use schematic default (~0.15mm).
    #[serde(default)]
    pub stroke_width: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Junction {
    pub uuid: Uuid,
    pub position: Point,
    /// 0.0 means use the theme default size.
    #[serde(default)]
    pub diameter: f64,
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
    #[serde(default)]
    pub justify_h: HAlign,
    #[serde(default)]
    pub justify_v: VAlign,
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
    pub stroke_width: f64,
    #[serde(default)]
    pub fill: FillType,
    #[serde(default)]
    pub fields_autoplaced: bool,
    #[serde(default)]
    pub pins: Vec<SheetPin>,
    #[serde(default)]
    pub instances: Vec<SheetInstance>,
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
        #[serde(default)]
        fill: FillType,
    },
    Polyline {
        uuid: Uuid,
        points: Vec<Point>,
        #[serde(default)]
        width: f64,
        #[serde(default)]
        fill: FillType,
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
    #[serde(default = "default_root_sheet_page")]
    pub root_sheet_page: String,
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

fn default_root_sheet_page() -> String {
    "1".to_string()
}

// ---------------------------------------------------------------------------
// Selection -- identifies what the user has selected on the canvas
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SelectedKind {
    Symbol,
    Wire,
    Bus,
    BusEntry,
    Junction,
    NoConnect,
    Label,
    TextNote,
    ChildSheet,
    Drawing,
    /// Symbol reference field ("C39", "R1", …). UUID = symbol UUID.
    SymbolRefField,
    /// Symbol value field ("100n", "10k", …). UUID = symbol UUID.
    SymbolValField,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SelectedItem {
    pub uuid: Uuid,
    pub kind: SelectedKind,
}

impl SelectedItem {
    pub fn new(uuid: Uuid, kind: SelectedKind) -> Self {
        Self { uuid, kind }
    }
}

// ---------------------------------------------------------------------------
// Bounding box helpers
// ---------------------------------------------------------------------------

/// Axis-aligned bounding box in world (mm) coordinates.
#[derive(Debug, Clone, Copy)]
pub struct Aabb {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl Aabb {
    pub fn new(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        Self {
            min_x: x1.min(x2),
            min_y: y1.min(y2),
            max_x: x1.max(x2),
            max_y: y1.max(y2),
        }
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.min_x && x <= self.max_x && y >= self.min_y && y <= self.max_y
    }

    pub fn expand(&self, margin: f64) -> Self {
        Self {
            min_x: self.min_x - margin,
            min_y: self.min_y - margin,
            max_x: self.max_x + margin,
            max_y: self.max_y + margin,
        }
    }

    pub fn union(&self, other: &Aabb) -> Self {
        Self {
            min_x: self.min_x.min(other.min_x),
            min_y: self.min_y.min(other.min_y),
            max_x: self.max_x.max(other.max_x),
            max_y: self.max_y.max(other.max_y),
        }
    }

    pub fn width(&self) -> f64 {
        self.max_x - self.min_x
    }

    pub fn height(&self) -> f64 {
        self.max_y - self.min_y
    }
}

impl SchematicSheet {
    /// Compute the bounding box of all elements in the sheet.
    pub fn content_bounds(&self) -> Option<Aabb> {
        let mut aabb: Option<Aabb> = None;

        let mut extend = |x: f64, y: f64| {
            aabb = Some(match aabb {
                Some(a) => Aabb {
                    min_x: a.min_x.min(x),
                    min_y: a.min_y.min(y),
                    max_x: a.max_x.max(x),
                    max_y: a.max_y.max(y),
                },
                None => Aabb::new(x, y, x, y),
            });
        };

        for s in &self.symbols {
            extend(s.position.x, s.position.y);
        }
        for w in &self.wires {
            extend(w.start.x, w.start.y);
            extend(w.end.x, w.end.y);
        }
        for b in &self.buses {
            extend(b.start.x, b.start.y);
            extend(b.end.x, b.end.y);
        }
        for j in &self.junctions {
            extend(j.position.x, j.position.y);
        }
        for l in &self.labels {
            extend(l.position.x, l.position.y);
        }
        for n in &self.no_connects {
            extend(n.position.x, n.position.y);
        }
        for t in &self.text_notes {
            extend(t.position.x, t.position.y);
        }
        for c in &self.child_sheets {
            extend(c.position.x, c.position.y);
            extend(c.position.x + c.size.0, c.position.y + c.size.1);
        }

        // Add margin around content
        aabb.map(|a| a.expand(10.0))
    }
}

/// Distance from a point to a line segment.
pub fn point_to_segment_dist(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    let dx = bx - ax;
    let dy = by - ay;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 1e-12 {
        return ((px - ax).powi(2) + (py - ay).powi(2)).sqrt();
    }
    let t = ((px - ax) * dx + (py - ay) * dy) / len_sq;
    let t = t.clamp(0.0, 1.0);
    let proj_x = ax + t * dx;
    let proj_y = ay + t * dy;
    ((px - proj_x).powi(2) + (py - proj_y).powi(2)).sqrt()
}
