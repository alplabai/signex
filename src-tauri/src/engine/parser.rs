use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use super::sexpr::{self, SExpr};

// UUID generator for elements missing UUIDs.
// Uses process start time + atomic counter to avoid collisions across sessions.
static COUNTER: AtomicU64 = AtomicU64::new(1);
static SESSION_SEED: std::sync::LazyLock<u64> = std::sync::LazyLock::new(|| {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x1234_5678_9abc_def0)
});
fn rand_u32() -> u32 {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mixed = n.wrapping_mul(0x517cc1b727220a95) ^ *SESSION_SEED;
    (mixed >> 16) as u32
}
fn rand_u16() -> u16 {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mixed = n.wrapping_mul(0x517cc1b727220a95) ^ *SESSION_SEED;
    (mixed >> 32) as u16
}
fn rand_u48() -> u64 {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mixed = n.wrapping_mul(0x517cc1b727220a95) ^ *SESSION_SEED;
    mixed & 0xFFFF_FFFF_FFFF
}

// --- Data structures ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchematicSheet {
    pub uuid: String,
    pub version: String,
    pub generator: String,
    pub generator_version: String,
    pub paper_size: String,
    pub symbols: Vec<Symbol>,
    pub wires: Vec<Wire>,
    pub junctions: Vec<Junction>,
    pub labels: Vec<Label>,
    pub child_sheets: Vec<ChildSheet>,
    pub no_connects: Vec<NoConnect>,
    pub text_notes: Vec<TextNote>,
    pub rectangles: Vec<SchRectangle>,
    pub buses: Vec<Bus>,
    pub bus_entries: Vec<BusEntry>,
    pub drawings: Vec<SchDrawing>,
    pub no_erc_directives: Vec<NoConnect>, // Reuse NoConnect struct (uuid + position)
    pub title_block: HashMap<String, String>,
    pub lib_symbols: HashMap<String, LibSymbol>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibSymbol {
    pub id: String,
    pub graphics: Vec<Graphic>,
    pub pins: Vec<Pin>,
    pub show_pin_numbers: bool,
    pub show_pin_names: bool,
    pub pin_name_offset: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Graphic {
    Polyline {
        points: Vec<Point>,
        width: f64,
        fill_type: String,
    },
    Rectangle {
        start: Point,
        end: Point,
        width: f64,
        fill_type: String,
    },
    Circle {
        center: Point,
        radius: f64,
        width: f64,
        fill_type: String,
    },
    Arc {
        start: Point,
        mid: Point,
        end: Point,
        width: f64,
        fill_type: String,
    },
    Text {
        text: String,
        position: Point,
        rotation: f64,
        font_size: f64,
        bold: bool,
        italic: bool,
        justify_h: String,
        justify_v: String,
    },
    TextBox {
        text: String,
        position: Point,
        rotation: f64,
        size: Point,
        font_size: f64,
        bold: bool,
        italic: bool,
        width: f64,
        fill_type: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pin {
    pub pin_type: String,
    pub shape: String,
    pub position: Point,
    pub rotation: f64,
    pub length: f64,
    pub name: String,
    pub number: String,
    pub name_visible: bool,
    pub number_visible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub uuid: String,
    pub lib_id: String,
    pub reference: String,
    pub value: String,
    pub footprint: String,
    pub position: Point,
    pub rotation: f64,
    pub mirror_x: bool,
    pub mirror_y: bool,
    pub unit: u32,
    pub is_power: bool,
    pub ref_text: TextProp,
    pub val_text: TextProp,
    pub fields_autoplaced: bool,
    // KiCad 10 fields
    pub dnp: bool,
    pub in_bom: bool,
    pub on_board: bool,
    pub exclude_from_sim: bool,
    pub locked: bool,
    pub fields: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextProp {
    pub position: Point,
    pub rotation: f64,
    pub font_size: f64,
    pub justify_h: String,
    pub justify_v: String,
    pub hidden: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wire {
    pub uuid: String,
    pub start: Point,
    pub end: Point,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Junction {
    pub uuid: String,
    pub position: Point,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub uuid: String,
    pub text: String,
    pub position: Point,
    pub rotation: f64,
    pub label_type: LabelType,
    pub shape: String,
    pub font_size: f64,
    pub justify: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LabelType {
    Net,
    Global,
    Hierarchical,
    Power,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoConnect {
    pub uuid: String,
    pub position: Point,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextNote {
    pub uuid: String,
    pub text: String,
    pub position: Point,
    pub rotation: f64,
    pub font_size: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchRectangle {
    pub uuid: String,
    pub start: Point,
    pub end: Point,
    pub stroke_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bus {
    pub uuid: String,
    pub start: Point,
    pub end: Point,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusEntry {
    pub uuid: String,
    pub position: Point,
    pub size: (f64, f64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetPin {
    pub uuid: String,
    pub name: String,
    pub direction: String,
    pub position: Point,
    pub rotation: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildSheet {
    pub uuid: String,
    pub name: String,
    pub filename: String,
    pub position: Point,
    pub size: (f64, f64),
    pub pins: Vec<SheetPin>,
}

/// User-drawn schematic graphics (not inside symbols)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SchDrawing {
    Line {
        uuid: String,
        start: Point,
        end: Point,
        width: f64,
    },
    Rect {
        uuid: String,
        start: Point,
        end: Point,
        width: f64,
        fill: bool,
    },
    Circle {
        uuid: String,
        center: Point,
        radius: f64,
        width: f64,
        fill: bool,
    },
    Arc {
        uuid: String,
        start: Point,
        mid: Point,
        end: Point,
        width: f64,
    },
    Polyline {
        uuid: String,
        points: Vec<Point>,
        width: f64,
        fill: bool,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectData {
    pub name: String,
    pub dir: String,
    pub schematic_root: Option<String>,
    pub pcb_file: Option<String>,
    pub sheets: Vec<SheetEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetEntry {
    pub name: String,
    pub filename: String,
    pub symbols_count: usize,
    pub wires_count: usize,
    pub labels_count: usize,
}

// --- Helper functions ---

fn parse_at(node: &SExpr) -> (Point, f64) {
    match node.find("at") {
        Some(at) => {
            let x = at.arg_f64(0).unwrap_or(0.0);
            let y = at.arg_f64(1).unwrap_or(0.0);
            let rot = at.arg_f64(2).unwrap_or(0.0);
            (Point { x, y }, rot)
        }
        None => (Point { x: 0.0, y: 0.0 }, 0.0),
    }
}

fn parse_text_prop(prop_node: &SExpr, _fallback_pos: Point) -> TextProp {
    let (position, rotation) = parse_at(prop_node);
    let effects = prop_node.find("effects");

    let font_size = effects
        .and_then(|e| e.find("font"))
        .and_then(|f| f.find("size"))
        .and_then(|s| s.arg_f64(0))
        .unwrap_or(1.27);

    let hidden = effects
        .and_then(|e| e.find("hide"))
        .and_then(|h| h.first_arg())
        .map(|v| v == "yes")
        .unwrap_or(false);

    // Parse justify: (justify left bottom), (justify right), (justify center), etc.
    let justify = effects.and_then(|e| e.find("justify"));
    let mut justify_h = "center".to_string();
    let mut justify_v = "center".to_string();
    if let Some(j) = justify {
        for child in j.children() {
            if let super::sexpr::SExpr::Atom(s) = child {
                match s.as_str() {
                    "left" => justify_h = "left".to_string(),
                    "right" => justify_h = "right".to_string(),
                    "top" => justify_v = "top".to_string(),
                    "bottom" => justify_v = "bottom".to_string(),
                    "mirror" => {} // ignore mirror for now
                    _ => {}
                }
            }
        }
    }

    TextProp {
        position,
        rotation,
        font_size,
        justify_h,
        justify_v,
        hidden,
    }
}

fn parse_uuid(node: &SExpr) -> String {
    node.find("uuid")
        .and_then(|u| u.first_arg())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            // Generate a fresh UUID rather than returning a duplicate "unknown"
            format!(
                "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
                rand_u32(),
                rand_u16(),
                rand_u16(),
                rand_u16(),
                rand_u48()
            )
        })
}

fn parse_fill_type(node: &SExpr) -> String {
    node.find("fill")
        .and_then(|f| f.find("type"))
        .and_then(|t| t.first_arg())
        .unwrap_or("none")
        .to_string()
}

fn parse_stroke_width(node: &SExpr) -> f64 {
    node.find("stroke")
        .and_then(|s| s.find("width"))
        .and_then(|w| w.arg_f64(0))
        .unwrap_or(0.0)
}

fn is_text_hidden(node: &SExpr) -> bool {
    // Check if (effects ... (hide yes)) or (effects ... (hide)) exists
    let hide = node.find("effects").and_then(|e| e.find("hide"));
    match hide {
        Some(h) => {
            // (hide yes) or (hide) without arg both mean hidden
            h.first_arg().map(|v| v == "yes").unwrap_or(true)
        }
        None => false,
    }
}

// --- Lib symbol parsing ---

fn parse_lib_symbol(symbol_node: &SExpr) -> LibSymbol {
    let id = symbol_node.first_arg().unwrap_or("").to_string();
    let mut graphics = Vec::new();
    let mut pins = Vec::new();

    // Check pin visibility flags
    let show_pin_numbers = symbol_node
        .find("pin_numbers")
        .and_then(|pn| pn.first_arg())
        .map(|v| v != "hide")
        .unwrap_or(true);

    // pin_names can have: (pin_names hide), (pin_names (offset X) hide), (pin_names (offset X))
    let pin_names_node = symbol_node.find("pin_names");
    let show_pin_names = pin_names_node
        .map(|pn| {
            // Check if any child atom is "hide"
            !pn.children()
                .iter()
                .any(|c| matches!(c, super::sexpr::SExpr::Atom(s) if s == "hide"))
        })
        .unwrap_or(true);
    let pin_name_offset = pin_names_node
        .and_then(|pn| pn.find("offset"))
        .and_then(|o| o.arg_f64(0))
        .unwrap_or(0.508);

    // Sort sub-symbols so body-style-0 (_X_0) subs come AFTER body-style->=1 subs
    // (for correct rendering order)
    fn is_body_style_0(sub: &SExpr) -> bool {
        sub.first_arg()
            .and_then(|name| name.rsplit('_').next())
            .and_then(|s| s.parse::<u32>().ok())
            .map(|n| n == 0)
            .unwrap_or(false)
    }

    let mut all_subs = symbol_node.find_all("symbol");
    all_subs.sort_by_key(|s| if is_body_style_0(s) { 1_u8 } else { 0_u8 });

    // Collect graphics and pins from sub-symbols (e.g., "R_0_1" for graphics, "R_1_1" for pins)
    for sub in &all_subs {
        for child in sub.children() {
            match child.keyword() {
                Some("polyline") => {
                    if let Some(pts) = child.find("pts") {
                        let points: Vec<Point> = pts
                            .find_all("xy")
                            .iter()
                            .map(|xy| Point {
                                x: xy.arg_f64(0).unwrap_or(0.0),
                                y: xy.arg_f64(1).unwrap_or(0.0),
                            })
                            .collect();
                        if !points.is_empty() {
                            graphics.push(Graphic::Polyline {
                                points,
                                width: parse_stroke_width(child),
                                fill_type: parse_fill_type(child),
                            });
                        }
                    }
                }
                Some("rectangle") => {
                    let start = child
                        .find("start")
                        .map(|s| Point {
                            x: s.arg_f64(0).unwrap_or(0.0),
                            y: s.arg_f64(1).unwrap_or(0.0),
                        })
                        .unwrap_or(Point { x: 0.0, y: 0.0 });
                    let end = child
                        .find("end")
                        .map(|e| Point {
                            x: e.arg_f64(0).unwrap_or(0.0),
                            y: e.arg_f64(1).unwrap_or(0.0),
                        })
                        .unwrap_or(Point { x: 0.0, y: 0.0 });
                    graphics.push(Graphic::Rectangle {
                        start,
                        end,
                        width: parse_stroke_width(child),
                        fill_type: parse_fill_type(child),
                    });
                }
                Some("circle") => {
                    let center = child
                        .find("center")
                        .map(|c| Point {
                            x: c.arg_f64(0).unwrap_or(0.0),
                            y: c.arg_f64(1).unwrap_or(0.0),
                        })
                        .unwrap_or(Point { x: 0.0, y: 0.0 });
                    let radius = child
                        .find("radius")
                        .and_then(|r| r.arg_f64(0))
                        .unwrap_or(1.0);
                    graphics.push(Graphic::Circle {
                        center,
                        radius,
                        width: parse_stroke_width(child),
                        fill_type: parse_fill_type(child),
                    });
                }
                Some("arc") => {
                    let start = child
                        .find("start")
                        .map(|s| Point {
                            x: s.arg_f64(0).unwrap_or(0.0),
                            y: s.arg_f64(1).unwrap_or(0.0),
                        })
                        .unwrap_or(Point { x: 0.0, y: 0.0 });
                    let mid = child
                        .find("mid")
                        .map(|m| Point {
                            x: m.arg_f64(0).unwrap_or(0.0),
                            y: m.arg_f64(1).unwrap_or(0.0),
                        })
                        .unwrap_or(Point { x: 0.0, y: 0.0 });
                    let end = child
                        .find("end")
                        .map(|e| Point {
                            x: e.arg_f64(0).unwrap_or(0.0),
                            y: e.arg_f64(1).unwrap_or(0.0),
                        })
                        .unwrap_or(Point { x: 0.0, y: 0.0 });
                    graphics.push(Graphic::Arc {
                        start,
                        mid,
                        end,
                        width: parse_stroke_width(child),
                        fill_type: parse_fill_type(child),
                    });
                }
                Some("text") => {
                    let text = child.first_arg().unwrap_or("").to_string();
                    let (position, rotation) = parse_at(child);
                    let effects = child.find("effects");
                    let font = effects.and_then(|e| e.find("font"));
                    let font_size = font.and_then(|f| f.find("size")).and_then(|s| s.arg_f64(0)).unwrap_or(1.27);
                    let bold = font.and_then(|f| f.find("bold")).and_then(|b| b.first_arg()).map(|v| v == "yes").unwrap_or(false);
                    let italic = font.and_then(|f| f.find("italic")).and_then(|b| b.first_arg()).map(|v| v == "yes").unwrap_or(false);
                    let justify = effects.and_then(|e| e.find("justify"));
                    let justify_h = justify.and_then(|j| j.first_arg()).map(|v| match v { "left" => "left", "right" => "right", _ => "center" }).unwrap_or("center").to_string();
                    let justify_v = justify.and_then(|j| j.arg(1)).map(|v| match v { "top" => "top", "bottom" => "bottom", _ => "center" }).unwrap_or("center").to_string();
                    graphics.push(Graphic::Text { text, position, rotation, font_size, bold, italic, justify_h, justify_v });
                }
                Some("text_box") => {
                    let text = child.first_arg().unwrap_or("").to_string();
                    let (position, rotation) = parse_at(child);
                    let size = child.find("size").map(|s| Point { x: s.arg_f64(0).unwrap_or(0.0), y: s.arg_f64(1).unwrap_or(0.0) }).unwrap_or(Point { x: 0.0, y: 0.0 });
                    let effects = child.find("effects");
                    let font = effects.and_then(|e| e.find("font"));
                    let font_size = font.and_then(|f| f.find("size")).and_then(|s| s.arg_f64(0)).unwrap_or(1.27);
                    let bold = font.and_then(|f| f.find("bold")).and_then(|b| b.first_arg()).map(|v| v == "yes").unwrap_or(false);
                    let italic = font.and_then(|f| f.find("italic")).and_then(|b| b.first_arg()).map(|v| v == "yes").unwrap_or(false);
                    graphics.push(Graphic::TextBox { text, position, rotation, size, font_size, bold, italic, width: parse_stroke_width(child), fill_type: parse_fill_type(child) });
                }
                _ => {}
            }
        }

        // Parse pins
        for pin in sub.children().iter().filter(|c| c.keyword() == Some("pin")) {
            let pin_type = pin.first_arg().unwrap_or("unspecified").to_string();
            let shape = pin.arg(1).unwrap_or("line").to_string();
            let (position, rotation) = parse_at(pin);
            let length = pin
                .find("length")
                .and_then(|l| l.arg_f64(0))
                .unwrap_or(2.54);

            let name_node = pin.find("name");
            let name = name_node
                .and_then(|n| n.first_arg())
                .unwrap_or("~")
                .to_string();
            let name_visible = !name_node.map(is_text_hidden).unwrap_or(false);

            let number_node = pin.find("number");
            let number = number_node
                .and_then(|n| n.first_arg())
                .unwrap_or("")
                .to_string();
            let number_visible = !number_node.map(is_text_hidden).unwrap_or(false);

            pins.push(Pin {
                pin_type,
                shape,
                position,
                rotation,
                length,
                name,
                number,
                name_visible,
                number_visible,
            });
        }
    }

    LibSymbol {
        id,
        graphics,
        pins,
        show_pin_numbers,
        show_pin_names,
        pin_name_offset,
    }
}

// --- Schematic element helpers ---

fn parse_title_block(root: &SExpr) -> HashMap<String, String> {
    let mut title_block = HashMap::new();
    let tb = match root.find("title_block") {
        Some(tb) => tb,
        None => return title_block,
    };
    if let Some(v) = tb.find("title").and_then(|t| t.first_arg()) {
        title_block.insert("title".to_string(), v.to_string());
    }
    if let Some(v) = tb.find("date").and_then(|d| d.first_arg()) {
        title_block.insert("date".to_string(), v.to_string());
    }
    if let Some(v) = tb.find("rev").and_then(|r| r.first_arg()) {
        title_block.insert("rev".to_string(), v.to_string());
    }
    if let Some(v) = tb.find("company").and_then(|c| c.first_arg()) {
        title_block.insert("company".to_string(), v.to_string());
    }
    for comment in tb.find_all("comment") {
        if let (Some(num), Some(text)) = (comment.first_arg(), comment.arg(1)) {
            title_block.insert(format!("comment_{}", num), text.to_string());
        }
    }
    title_block
}

fn parse_wire(node: &SExpr) -> Wire {
    let pts = node.find("pts");
    let (start, end) = match pts {
        Some(pts) => {
            let xy_nodes = pts.find_all("xy");
            let start = xy_nodes
                .first()
                .map(|xy| Point {
                    x: xy.arg_f64(0).unwrap_or(0.0),
                    y: xy.arg_f64(1).unwrap_or(0.0),
                })
                .unwrap_or(Point { x: 0.0, y: 0.0 });
            let end = xy_nodes
                .get(1)
                .map(|xy| Point {
                    x: xy.arg_f64(0).unwrap_or(0.0),
                    y: xy.arg_f64(1).unwrap_or(0.0),
                })
                .unwrap_or(start);
            (start, end)
        }
        None => (Point { x: 0.0, y: 0.0 }, Point { x: 0.0, y: 0.0 }),
    };
    Wire {
        uuid: parse_uuid(node),
        start,
        end,
    }
}

fn parse_label(node: &SExpr, label_type: LabelType) -> Label {
    let (position, rotation) = parse_at(node);
    let shape = node
        .find("shape")
        .and_then(|s| s.first_arg())
        .unwrap_or("")
        .to_string();
    let effects = node.find("effects");
    let font_size = effects
        .and_then(|e| e.find("font"))
        .and_then(|f| f.find("size"))
        .and_then(|s| s.arg_f64(0))
        .unwrap_or(1.27);
    let justify = effects
        .and_then(|e| e.find("justify"))
        .and_then(|j| j.first_arg())
        .unwrap_or("left")
        .to_string();
    Label {
        uuid: parse_uuid(node),
        text: node.first_arg().unwrap_or("").to_string(),
        position,
        rotation,
        label_type,
        shape,
        font_size,
        justify,
    }
}

fn parse_symbol_instance(s: &SExpr) -> Symbol {
    let (position, rotation) = parse_at(s);
    let lib_id = s
        .find("lib_id")
        .and_then(|l| l.first_arg())
        .unwrap_or("")
        .to_string();
    let reference = s.property("Reference").unwrap_or("?").to_string();
    let value = s.property("Value").unwrap_or("").to_string();
    let footprint = s.property("Footprint").unwrap_or("").to_string();
    let unit = s
        .find("unit")
        .and_then(|u| u.first_arg())
        .and_then(|u| u.parse::<u32>().ok())
        .unwrap_or(1);
    let is_power = lib_id.starts_with("power:");

    let mirror = s.find("mirror");
    let mirror_x = mirror
        .and_then(|m| m.first_arg())
        .map(|v| v == "x" || v == "xy")
        .unwrap_or(false);
    let mirror_y = mirror
        .and_then(|m| m.first_arg())
        .map(|v| v == "y" || v == "xy")
        .unwrap_or(false);

    let fields_autoplaced = s
        .find("fields_autoplaced")
        .and_then(|f| f.first_arg())
        .map(|v| v == "yes")
        .unwrap_or(false);

    // KiCad 10 fields
    let dnp = s
        .find("dnp")
        .and_then(|f| f.first_arg())
        .map(|v| v == "yes")
        .unwrap_or(false);
    let in_bom = s
        .find("in_bom")
        .and_then(|f| f.first_arg())
        .map(|v| v == "yes")
        .unwrap_or(true);
    let on_board = s
        .find("on_board")
        .and_then(|f| f.first_arg())
        .map(|v| v == "yes")
        .unwrap_or(true);
    let exclude_from_sim = s
        .find("exclude_from_sim")
        .and_then(|f| f.first_arg())
        .map(|v| v == "yes")
        .unwrap_or(false);
    let locked = s.find("locked").is_some();

    let ref_prop = s
        .children()
        .iter()
        .find(|c| c.keyword() == Some("property") && c.first_arg() == Some("Reference"));
    let val_prop = s
        .children()
        .iter()
        .find(|c| c.keyword() == Some("property") && c.first_arg() == Some("Value"));
    let mut ref_text = ref_prop
        .map(|p| parse_text_prop(p, position))
        .unwrap_or(TextProp {
            position,
            rotation: 0.0,
            font_size: 1.27,
            justify_h: "center".into(),
            justify_v: "center".into(),
            hidden: false,
        });
    let mut val_text = val_prop
        .map(|p| parse_text_prop(p, position))
        .unwrap_or(TextProp {
            position,
            rotation: 0.0,
            font_size: 1.27,
            justify_h: "center".into(),
            justify_v: "center".into(),
            hidden: false,
        });

    // KiCad's GetDrawRotation(): stored angle is toggled (H↔V) when symbol
    // rotation is 90° or 270° (transform has y1 != 0).
    // Source: eeschema/sch_field.cpp GetDrawRotation()
    let sym_90_or_270 = (rotation - 90.0).abs() < 0.1 || (rotation - 270.0).abs() < 0.1;
    if sym_90_or_270 {
        // Toggle: horizontal(0) ↔ vertical(90)
        ref_text.rotation = if ref_text.rotation.abs() < 0.1 { 90.0 } else { 0.0 };
        val_text.rotation = if val_text.rotation.abs() < 0.1 { 90.0 } else { 0.0 };
    }

    // Parse custom fields (all properties beyond Reference/Value/Footprint/Datasheet)
    let standard_props = ["Reference", "Value", "Footprint", "Datasheet"];
    let mut fields = HashMap::new();
    for child in s.children() {
        if child.keyword() == Some("property") {
            if let Some(key) = child.first_arg() {
                if !standard_props.contains(&key) {
                    if let Some(val) = child.arg(1) {
                        fields.insert(key.to_string(), val.to_string());
                    }
                }
            }
        }
    }

    Symbol {
        uuid: parse_uuid(s),
        lib_id,
        reference,
        value,
        footprint,
        position,
        rotation,
        mirror_x,
        mirror_y,
        unit,
        is_power,
        ref_text,
        val_text,
        fields_autoplaced,
        dnp,
        in_bom,
        on_board,
        exclude_from_sim,
        locked,
        fields,
    }
}

fn parse_child_sheet(s: &SExpr) -> ChildSheet {
    let (position, _) = parse_at(s);
    let size = s
        .find("size")
        .map(|sz| (sz.arg_f64(0).unwrap_or(20.0), sz.arg_f64(1).unwrap_or(15.0)))
        .unwrap_or((20.0, 15.0));
    // Parse sheet pins (entries): (pin "name" direction (at x y angle) ...)
    let pins: Vec<SheetPin> = s
        .find_all("pin")
        .iter()
        .map(|p| {
            let name = p.first_arg().unwrap_or("").to_string();
            let direction = p.arg(1).unwrap_or("bidirectional").to_string();
            let (position, rotation) = parse_at(p);
            SheetPin {
                uuid: parse_uuid(p),
                name,
                direction,
                position,
                rotation,
            }
        })
        .collect();
    ChildSheet {
        uuid: parse_uuid(s),
        name: s.property("Sheetname").unwrap_or("Unnamed").to_string(),
        filename: s.property("Sheetfile").unwrap_or("").to_string(),
        position,
        size,
        pins,
    }
}

fn parse_drawings(root: &SExpr) -> Vec<SchDrawing> {
    let mut drawings: Vec<SchDrawing> = Vec::new();

    for pl in root.find_all("polyline") {
        let pts: Vec<Point> = pl
            .find("pts")
            .map(|p| {
                p.find_all("xy")
                    .iter()
                    .map(|xy| Point {
                        x: xy.arg_f64(0).unwrap_or(0.0),
                        y: xy.arg_f64(1).unwrap_or(0.0),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let width = parse_stroke_width(pl);
        let fill = pl
            .find("fill")
            .and_then(|f| f.find("type"))
            .and_then(|t| t.first_arg())
            .map(|t| t != "none")
            .unwrap_or(false);
        if pts.len() == 2 {
            drawings.push(SchDrawing::Line {
                uuid: parse_uuid(pl),
                start: pts[0],
                end: pts[1],
                width,
            });
        } else if pts.len() > 2 {
            drawings.push(SchDrawing::Polyline {
                uuid: parse_uuid(pl),
                points: pts,
                width,
                fill,
            });
        }
    }

    for arc in root.find_all("arc") {
        let start = arc
            .find("start")
            .map(|s| Point {
                x: s.arg_f64(0).unwrap_or(0.0),
                y: s.arg_f64(1).unwrap_or(0.0),
            })
            .unwrap_or(Point { x: 0.0, y: 0.0 });
        let mid = arc
            .find("mid")
            .map(|m| Point {
                x: m.arg_f64(0).unwrap_or(0.0),
                y: m.arg_f64(1).unwrap_or(0.0),
            })
            .unwrap_or(Point { x: 0.0, y: 0.0 });
        let end = arc
            .find("end")
            .map(|e| Point {
                x: e.arg_f64(0).unwrap_or(0.0),
                y: e.arg_f64(1).unwrap_or(0.0),
            })
            .unwrap_or(Point { x: 0.0, y: 0.0 });
        drawings.push(SchDrawing::Arc {
            uuid: parse_uuid(arc),
            start,
            mid,
            end,
            width: parse_stroke_width(arc),
        });
    }

    for circ in root.find_all("circle") {
        let center = circ
            .find("center")
            .map(|c| Point {
                x: c.arg_f64(0).unwrap_or(0.0),
                y: c.arg_f64(1).unwrap_or(0.0),
            })
            .unwrap_or(Point { x: 0.0, y: 0.0 });
        let radius = circ
            .find("radius")
            .and_then(|r| r.arg_f64(0))
            .unwrap_or(1.0);
        let fill = circ
            .find("fill")
            .and_then(|f| f.find("type"))
            .and_then(|t| t.first_arg())
            .map(|t| t != "none")
            .unwrap_or(false);
        drawings.push(SchDrawing::Circle {
            uuid: parse_uuid(circ),
            center,
            radius,
            width: parse_stroke_width(circ),
            fill,
        });
    }

    drawings
}

// --- Main schematic parser ---

pub fn parse_schematic(content: &str) -> Result<SchematicSheet, String> {
    let root = sexpr::parse(content)?;

    if root.keyword() != Some("kicad_sch") {
        return Err("Not a KiCad schematic file".to_string());
    }

    let version = root.find("version").and_then(|v| v.first_arg()).unwrap_or("unknown").to_string();
    let generator = root.find("generator").and_then(|v| v.first_arg()).unwrap_or("unknown").to_string();
    let generator_version = root.find("generator_version").and_then(|v| v.first_arg()).unwrap_or("").to_string();
    let paper_size = root.find("paper").and_then(|v| v.first_arg()).unwrap_or("A4").to_string();
    let uuid = parse_uuid(&root);

    // Parse library symbols
    let mut lib_symbols = HashMap::new();
    if let Some(lib_node) = root.find("lib_symbols") {
        for sym in lib_node.find_all("symbol") {
            let parsed = parse_lib_symbol(sym);
            lib_symbols.insert(parsed.id.clone(), parsed);
        }
    }

    let symbols: Vec<Symbol> = root
        .find_all("symbol")
        .iter()
        .filter(|s| s.find("lib_id").is_some())
        .map(|s| parse_symbol_instance(s))
        .collect();

    let wires: Vec<Wire> = root.find_all("wire").iter().map(|w| parse_wire(w)).collect();

    let junctions: Vec<Junction> = root
        .find_all("junction")
        .iter()
        .map(|j| Junction { uuid: parse_uuid(j), position: parse_at(j).0 })
        .collect();

    let mut labels: Vec<Label> = Vec::new();
    for (keyword, ltype) in [
        ("label", LabelType::Net),
        ("global_label", LabelType::Global),
        ("hierarchical_label", LabelType::Hierarchical),
    ] {
        for l in root.find_all(keyword) {
            labels.push(parse_label(l, ltype.clone()));
        }
    }

    let no_connects: Vec<NoConnect> = root
        .find_all("no_connect")
        .iter()
        .map(|nc| NoConnect { uuid: parse_uuid(nc), position: parse_at(nc).0 })
        .collect();

    let buses: Vec<Bus> = root
        .find_all("bus")
        .iter()
        .map(|b| {
            let pts: Vec<Point> = b
                .find("pts")
                .map(|p| {
                    p.find_all("xy")
                        .iter()
                        .map(|xy| Point { x: xy.arg_f64(0).unwrap_or(0.0), y: xy.arg_f64(1).unwrap_or(0.0) })
                        .collect()
                })
                .unwrap_or_default();
            Bus {
                uuid: parse_uuid(b),
                start: pts.first().copied().unwrap_or(Point { x: 0.0, y: 0.0 }),
                end: pts.get(1).copied().unwrap_or(Point { x: 0.0, y: 0.0 }),
            }
        })
        .collect();

    let bus_entries: Vec<BusEntry> = root
        .find_all("bus_entry")
        .iter()
        .map(|be| {
            let (position, _) = parse_at(be);
            let size = be
                .find("size")
                .map(|s| (s.arg_f64(0).unwrap_or(2.54), s.arg_f64(1).unwrap_or(2.54)))
                .unwrap_or((2.54, 2.54));
            BusEntry { uuid: parse_uuid(be), position, size }
        })
        .collect();

    let drawings = parse_drawings(&root);

    let child_sheets: Vec<ChildSheet> = root
        .find_all("sheet")
        .iter()
        .map(|s| parse_child_sheet(s))
        .collect();

    let text_notes: Vec<TextNote> = root
        .find_all("text")
        .iter()
        .map(|t| {
            let (position, rotation) = parse_at(t);
            let font_size = t
                .find("effects")
                .and_then(|e| e.find("font"))
                .and_then(|f| f.find("size"))
                .and_then(|s| s.arg_f64(0))
                .unwrap_or(1.27);
            TextNote { uuid: parse_uuid(t), text: t.first_arg().unwrap_or("").to_string(), position, rotation, font_size }
        })
        .collect();

    let rectangles: Vec<SchRectangle> = root
        .find_all("rectangle")
        .iter()
        .map(|r| {
            let start = r.find("start").map(|s| Point { x: s.arg_f64(0).unwrap_or(0.0), y: s.arg_f64(1).unwrap_or(0.0) }).unwrap_or(Point { x: 0.0, y: 0.0 });
            let end = r.find("end").map(|e| Point { x: e.arg_f64(0).unwrap_or(0.0), y: e.arg_f64(1).unwrap_or(0.0) }).unwrap_or(Point { x: 0.0, y: 0.0 });
            let stroke_type = r.find("stroke").and_then(|s| s.find("type")).and_then(|t| t.first_arg()).unwrap_or("default").to_string();
            SchRectangle { uuid: parse_uuid(r), start, end, stroke_type }
        })
        .collect();

    let no_erc_directives: Vec<NoConnect> = root
        .find_all("no_erc")
        .iter()
        .map(|ne| NoConnect { uuid: parse_uuid(ne), position: parse_at(ne).0 })
        .collect();

    let title_block = parse_title_block(&root);

    Ok(SchematicSheet {
        uuid,
        version,
        generator,
        generator_version,
        paper_size,
        symbols,
        wires,
        junctions,
        labels,
        child_sheets,
        no_connects,
        text_notes,
        rectangles,
        buses,
        bus_entries,
        drawings,
        no_erc_directives,
        title_block,
        lib_symbols,
    })
}

// --- Project parser (lightweight, no full S-expr parse) ---

pub fn parse_project(path: &Path) -> Result<ProjectData, String> {
    let dir = path.parent().unwrap_or(Path::new("."));
    let project_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled")
        .to_string();

    let root_sch_name = format!("{}.kicad_sch", project_name);
    let root_sch_path = dir.join(&root_sch_name);
    let schematic_root = if root_sch_path.exists() {
        Some(root_sch_name.clone())
    } else {
        None
    };

    let pcb_name = format!("{}.kicad_pcb", project_name);
    let pcb_file = if dir.join(&pcb_name).exists() {
        Some(pcb_name)
    } else {
        None
    };

    let mut sheets = Vec::new();
    if let Some(ref root_name) = schematic_root {
        collect_sheets(dir, root_name, &mut sheets)?;
    }

    Ok(ProjectData {
        name: project_name,
        dir: dir.to_string_lossy().to_string(),
        schematic_root,
        pcb_file,
        sheets,
    })
}

const MAX_SHEET_DEPTH: usize = 32;
const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024; // 100 MB

fn collect_sheets(
    dir: &Path,
    root_filename: &str,
    sheets: &mut Vec<SheetEntry>,
) -> Result<(), String> {
    // Iterative BFS with depth tracking (no recursion — safe for any hierarchy)
    let mut queue: Vec<(String, usize)> = vec![(root_filename.to_string(), 0)];

    while let Some((filename, depth)) = queue.pop() {
        if depth > MAX_SHEET_DEPTH {
            continue; // Silently stop at max depth
        }
        if sheets.iter().any(|s| s.filename == filename) {
            continue; // Already visited (cycle detection)
        }

        let file_path = dir.join(&filename);
        // Check file size before reading
        let metadata = std::fs::metadata(&file_path)
            .map_err(|e| format!("Failed to read {}: {}", filename, e))?;
        if metadata.len() > MAX_FILE_SIZE {
            return Err(format!(
                "File too large: {} ({} bytes)",
                filename,
                metadata.len()
            ));
        }

        let content = std::fs::read_to_string(&file_path)
            .map_err(|e| format!("Failed to read {}: {}", filename, e))?;

        let mut symbols_count = 0;
        let mut wires_count = 0;
        let mut labels_count = 0;
        let mut child_filenames: Vec<String> = Vec::new();
        let mut paren_depth: usize = 0;
        let mut in_string = false;

        for line in content.lines() {
            let line_bytes = line.as_bytes();
            // Track paren depth while respecting quoted strings
            for (idx, &b) in line_bytes.iter().enumerate() {
                if in_string {
                    if b == b'"' && (idx == 0 || line_bytes[idx - 1] != b'\\') {
                        in_string = false;
                    }
                } else {
                    match b {
                        b'"' => in_string = true,
                        b'(' => paren_depth += 1,
                        b')' => paren_depth = paren_depth.saturating_sub(1),
                        _ => {}
                    }
                }
            }

            let trimmed = line.trim();
            // Only count top-level elements (depth == 2 because root kicad_sch is depth 1)
            if (1..=2).contains(&paren_depth) {
                if trimmed.starts_with("(symbol") && !trimmed.contains("power:") {
                    symbols_count += 1;
                } else if trimmed.starts_with("(wire") {
                    wires_count += 1;
                } else if trimmed.starts_with("(label")
                    || trimmed.starts_with("(global_label")
                    || trimmed.starts_with("(hierarchical_label")
                {
                    labels_count += 1;
                }
            }

            if trimmed.contains("\"Sheetfile\"") {
                if let Some(start) = trimmed.rfind('"') {
                    let before = &trimmed[..start];
                    if let Some(fname_start) = before.rfind('"') {
                        let fname = &trimmed[fname_start + 1..start];
                        if !fname.is_empty() && fname != "Sheetfile" {
                            child_filenames.push(fname.to_string());
                        }
                    }
                }
            }
        }

        let name = if sheets.is_empty() {
            "Root".to_string()
        } else {
            filename.trim_end_matches(".kicad_sch").to_string()
        };
        sheets.push(SheetEntry {
            name,
            filename: filename.clone(),
            symbols_count,
            wires_count,
            labels_count,
        });

        for child in child_filenames {
            // Prevent path traversal via crafted sheet filenames
            let child_path = std::path::Path::new(&child);
            let has_traversal = child_path.components().any(|c| {
                matches!(
                    c,
                    std::path::Component::ParentDir
                        | std::path::Component::RootDir
                        | std::path::Component::Prefix(_)
                )
            });
            if has_traversal {
                continue;
            }
            let joined = dir.join(&child);
            if joined.exists() {
                if let Ok(canonical) = joined.canonicalize() {
                    if let Ok(canonical_dir) = dir.canonicalize() {
                        if !canonical.starts_with(&canonical_dir) {
                            continue;
                        }
                    }
                }
            }
            queue.push((child, depth + 1));
        }
    }
    Ok(())
}

// --- Symbol library parsing (.kicad_sym files) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolMeta {
    pub symbol_id: String,
    pub reference_prefix: String,
    pub value: String,
    pub description: String,
    pub keywords: Vec<String>,
    pub datasheet: String,
    pub footprint: String,
    pub pin_count: usize,
}

/// Parse a .kicad_sym library file and return all symbols with metadata
pub fn parse_symbol_library(content: &str) -> Result<Vec<(LibSymbol, SymbolMeta)>, String> {
    let root = sexpr::parse(content)?;

    if root.keyword() != Some("kicad_symbol_lib") {
        return Err("Not a KiCad symbol library file".to_string());
    }

    let mut results = Vec::new();

    // Build a set of all top-level symbol IDs for O(1) subsymbol checks
    let all_sym_ids: std::collections::HashSet<String> = root
        .find_all("symbol")
        .iter()
        .filter_map(|s| s.first_arg().map(|a| a.to_string()))
        .collect();

    for sym_node in root.find_all("symbol") {
        let id = sym_node.first_arg().unwrap_or("").to_string();

        // Skip sub-symbols: only skip if the prefix (before _N_M) matches
        // a top-level symbol that already exists in the parent kicad_symbol_lib.
        if id.contains('_') {
            let parts: Vec<&str> = id.rsplitn(3, '_').collect();
            if parts.len() >= 3
                && parts[0].parse::<u32>().is_ok()
                && parts[1].parse::<u32>().is_ok()
            {
                let prefix = parts[2];
                if all_sym_ids.contains(prefix) {
                    continue;
                }
            }
        }

        // Extract properties
        let reference_prefix = sym_node.property("Reference").unwrap_or("?").to_string();
        let value = sym_node.property("Value").unwrap_or(&id).to_string();
        let description = sym_node.property("Description").unwrap_or("").to_string();
        let datasheet = sym_node.property("Datasheet").unwrap_or("").to_string();
        let footprint = sym_node.property("Footprint").unwrap_or("").to_string();
        let keywords_str = sym_node.property("ki_keywords").unwrap_or("").to_string();
        let keywords: Vec<String> = keywords_str
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        let lib = parse_lib_symbol(sym_node);
        let pin_count = lib.pins.len();

        let meta = SymbolMeta {
            symbol_id: id,
            reference_prefix,
            value,
            description,
            keywords,
            datasheet,
            footprint,
            pin_count,
        };

        results.push((lib, meta));
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_kicad_sch() -> String {
        r#"(kicad_sch
  (version 20231120)
  (generator "test")
  (generator_version "0.1")
  (uuid "test-uuid")
  (paper "A4")
  (wire
    (pts (xy 10 20) (xy 30 20))
    (stroke (width 0) (type default))
    (uuid "wire-1")
  )
  (junction
    (at 20 20)
    (uuid "junc-1")
  )
  (label "VCC"
    (at 20 20 0)
    (effects (font (size 1.27 1.27)))
    (uuid "label-1")
  )
  (no_connect
    (at 50 50)
    (uuid "nc-1")
  )
  (text "Hello World"
    (at 100 100 0)
    (effects (font (size 1.27 1.27)))
    (uuid "text-1")
  )
)"#
        .to_string()
    }

    #[test]
    fn parse_minimal_schematic() {
        let content = minimal_kicad_sch();
        let sheet = parse_schematic(&content).unwrap();
        assert_eq!(sheet.uuid, "test-uuid");
        assert_eq!(sheet.version, "20231120");
        assert_eq!(sheet.paper_size, "A4");
        assert_eq!(sheet.wires.len(), 1);
        assert_eq!(sheet.junctions.len(), 1);
        assert_eq!(sheet.labels.len(), 1);
        assert_eq!(sheet.no_connects.len(), 1);
        assert_eq!(sheet.text_notes.len(), 1);
    }

    #[test]
    fn parse_wire_coordinates() {
        let content = minimal_kicad_sch();
        let sheet = parse_schematic(&content).unwrap();
        let wire = &sheet.wires[0];
        assert_eq!(wire.start.x, 10.0);
        assert_eq!(wire.start.y, 20.0);
        assert_eq!(wire.end.x, 30.0);
        assert_eq!(wire.end.y, 20.0);
        assert_eq!(wire.uuid, "wire-1");
    }

    #[test]
    fn parse_label_text_and_type() {
        let content = minimal_kicad_sch();
        let sheet = parse_schematic(&content).unwrap();
        let label = &sheet.labels[0];
        assert_eq!(label.text, "VCC");
        assert!(matches!(label.label_type, LabelType::Net));
        assert_eq!(label.position.x, 20.0);
    }

    #[test]
    fn parse_no_connect_has_uuid() {
        let content = minimal_kicad_sch();
        let sheet = parse_schematic(&content).unwrap();
        let nc = &sheet.no_connects[0];
        assert_eq!(nc.uuid, "nc-1");
        assert_eq!(nc.position.x, 50.0);
    }

    #[test]
    fn parse_text_note() {
        let content = minimal_kicad_sch();
        let sheet = parse_schematic(&content).unwrap();
        let note = &sheet.text_notes[0];
        assert_eq!(note.text, "Hello World");
        assert_eq!(note.uuid, "text-1");
        assert_eq!(note.font_size, 1.27);
    }

    #[test]
    fn write_then_reparse_preserves_data() {
        let content = minimal_kicad_sch();
        let sheet = parse_schematic(&content).unwrap();
        let written = crate::engine::writer::write_schematic(&sheet);
        let reparsed = parse_schematic(&written).unwrap();
        assert_eq!(reparsed.wires.len(), sheet.wires.len());
        assert_eq!(reparsed.junctions.len(), sheet.junctions.len());
        assert_eq!(reparsed.labels.len(), sheet.labels.len());
        assert_eq!(reparsed.no_connects.len(), sheet.no_connects.len());
        assert_eq!(reparsed.text_notes.len(), sheet.text_notes.len());
    }

    #[test]
    fn parse_kicad10_fields_default() {
        // KiCad 10 fields should default correctly when absent
        let content = r#"(kicad_sch
  (version 20260326)
  (generator "eeschema")
  (generator_version "10.0")
  (uuid "kicad10-test")
  (paper "A4")
  (lib_symbols
    (symbol "Device:R"
      (pin_names (offset 0))
      (symbol "R_0_1"
        (rectangle (start -1.016 -2.54) (end 1.016 2.54)
          (stroke (width 0.254) (type default))
          (fill (type none))
        )
      )
      (symbol "R_1_1"
        (pin passive line (at 0 3.81 270) (length 1.27) (name "~" (effects (font (size 1.27 1.27)))) (number "1" (effects (font (size 1.27 1.27)))))
        (pin passive line (at 0 -3.81 90) (length 1.27) (name "~" (effects (font (size 1.27 1.27)))) (number "2" (effects (font (size 1.27 1.27)))))
      )
    )
  )
  (symbol
    (lib_id "Device:R")
    (at 100 50 0)
    (unit 1)
    (exclude_from_sim no)
    (in_bom yes)
    (on_board yes)
    (dnp no)
    (uuid "sym-r1")
    (property "Reference" "R1" (at 100 48 0) (effects (font (size 1.27 1.27))))
    (property "Value" "10k" (at 100 52 0) (effects (font (size 1.27 1.27))))
    (property "Footprint" "" (at 100 50 0) (effects (font (size 1.27 1.27)) (hide yes)))
  )
)"#;
        let sheet = parse_schematic(content).unwrap();
        assert_eq!(sheet.generator_version, "10.0");
        assert_eq!(sheet.symbols.len(), 1);
        let sym = &sheet.symbols[0];
        assert!(!sym.dnp);
        assert!(sym.in_bom);
        assert!(sym.on_board);
        assert!(!sym.exclude_from_sim);
        assert!(!sym.locked);
    }
}
