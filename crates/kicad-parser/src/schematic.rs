use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use uuid::Uuid;

use signex_types::project::{ProjectData, SheetEntry};
use signex_types::schematic::{
    Bus, BusEntry, ChildSheet, FillType, Graphic, HAlign, Junction, Label, LabelType, LibGraphic,
    LibPin, LibSymbol, NoConnect, Pin, PinElectricalType, PinShape, Point, SchDrawing,
    SchematicSheet, SheetInstance, SheetPin, Symbol, SymbolInstance, TextNote, TextProp, VAlign,
    Wire,
};

use crate::error::ParseError;
use crate::sexpr::{self, SExpr};

// ---------------------------------------------------------------------------
// UUID generator for elements missing UUIDs.
// Uses process start time + atomic counter to avoid collisions across sessions.
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

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

    let hidden = is_effects_hidden(effects);

    // Parse justify: (justify left bottom), (justify right), (justify center), etc.
    let justify = effects.and_then(|e| e.find("justify"));
    let mut justify_h = HAlign::Center;
    let mut justify_v = VAlign::Center;
    if let Some(j) = justify {
        for child in j.children() {
            if let SExpr::Atom(s) = child {
                match s.as_str() {
                    "left" => justify_h = HAlign::Left,
                    "right" => justify_h = HAlign::Right,
                    "top" => justify_v = VAlign::Top,
                    "bottom" => justify_v = VAlign::Bottom,
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

fn parse_uuid(node: &SExpr) -> Uuid {
    node.find("uuid")
        .and_then(|u| u.first_arg())
        .and_then(|s| Uuid::parse_str(s).ok())
        .unwrap_or_else(|| {
            // Generate a fresh UUID rather than returning a duplicate "unknown"
            let hex = format!(
                "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
                rand_u32(),
                rand_u16(),
                rand_u16(),
                rand_u16(),
                rand_u48()
            );
            Uuid::parse_str(&hex).unwrap_or_else(|_| Uuid::new_v4())
        })
}

fn parse_fill_type(node: &SExpr) -> FillType {
    match node
        .find("fill")
        .and_then(|f| f.find("type"))
        .and_then(|t| t.first_arg())
        .unwrap_or("none")
    {
        "outline" => FillType::Outline,
        "background" => FillType::Background,
        _ => FillType::None,
    }
}

fn parse_stroke_width(node: &SExpr) -> f64 {
    node.find("stroke")
        .and_then(|s| s.find("width"))
        .and_then(|w| w.arg_f64(0))
        .unwrap_or(0.0)
}

/// Returns true if an `(effects ...)` node contains a hide marker.
///
/// Handles three formats across KiCad versions:
/// - `(hide yes)`  — KiCad 7+ list form with explicit "yes"
/// - `(hide)`      — KiCad 6 list form with no argument
/// - `hide`        — KiCad 5/6 bare-atom form inside `(effects ...)`
fn is_effects_hidden(effects: Option<&SExpr>) -> bool {
    let Some(eff) = effects else { return false };
    // Check for (hide yes) / (hide no) / (hide) as a list child
    if let Some(h) = eff.find("hide") {
        return h.first_arg().map(|v| v == "yes").unwrap_or(true);
    }
    // Check for standalone 'hide' atom (KiCad 5/6 format)
    eff.children()
        .iter()
        .any(|c| matches!(c, SExpr::Atom(s) if s == "hide"))
}

fn is_text_hidden(node: &SExpr) -> bool {
    is_effects_hidden(node.find("effects"))
}

fn parse_halign(s: &str) -> HAlign {
    match s {
        "left" => HAlign::Left,
        "right" => HAlign::Right,
        _ => HAlign::Center,
    }
}

fn parse_valign(s: &str) -> VAlign {
    match s {
        "top" => VAlign::Top,
        "bottom" => VAlign::Bottom,
        _ => VAlign::Center,
    }
}

fn parse_pin_electrical_type(s: &str) -> PinElectricalType {
    match s {
        "input" => PinElectricalType::Input,
        "output" => PinElectricalType::Output,
        "bidirectional" => PinElectricalType::Bidirectional,
        "tri_state" => PinElectricalType::TriState,
        "passive" => PinElectricalType::Passive,
        "free" => PinElectricalType::Free,
        "power_in" => PinElectricalType::PowerIn,
        "power_out" => PinElectricalType::PowerOut,
        "open_collector" => PinElectricalType::OpenCollector,
        "open_emitter" => PinElectricalType::OpenEmitter,
        "no_connect" | "not_connected" => PinElectricalType::NotConnected,
        _ => PinElectricalType::Unspecified,
    }
}

fn parse_pin_shape(s: &str) -> PinShape {
    match s {
        "inverted" => PinShape::Inverted,
        "clock" => PinShape::Clock,
        "inverted_clock" => PinShape::InvertedClock,
        "input_low" => PinShape::InputLow,
        "clock_low" => PinShape::ClockLow,
        "output_low" => PinShape::OutputLow,
        "edge_clock_high" => PinShape::EdgeClockHigh,
        "non_logic" => PinShape::NonLogic,
        _ => PinShape::Line,
    }
}

// ---------------------------------------------------------------------------
// Lib symbol parsing
// ---------------------------------------------------------------------------

pub(crate) fn parse_lib_symbol(symbol_node: &SExpr) -> LibSymbol {
    let id = symbol_node.first_arg().unwrap_or("").to_string();
    let reference = symbol_node.property("Reference").unwrap_or("").to_string();
    let value = symbol_node.property("Value").unwrap_or("").to_string();
    let footprint = symbol_node.property("Footprint").unwrap_or("").to_string();
    let datasheet = symbol_node.property("Datasheet").unwrap_or("").to_string();
    let description = symbol_node
        .property("Description")
        .unwrap_or("")
        .to_string();
    let keywords = symbol_node
        .property("ki_keywords")
        .unwrap_or("")
        .to_string();
    let fp_filters = symbol_node
        .property("ki_fp_filters")
        .unwrap_or("")
        .to_string();
    let in_bom = symbol_node
        .find("in_bom")
        .and_then(|node| node.first_arg())
        .map(|value| value == "yes")
        .unwrap_or(true);
    let on_board = symbol_node
        .find("on_board")
        .and_then(|node| node.first_arg())
        .map(|value| value == "yes")
        .unwrap_or(true);
    let in_pos_files = symbol_node
        .find("in_pos_files")
        .and_then(|node| node.first_arg())
        .map(|value| value == "yes")
        .unwrap_or(true);
    let duplicate_pin_numbers_are_jumpers = symbol_node
        .find("duplicate_pin_numbers_are_jumpers")
        .and_then(|node| node.first_arg())
        .map(|value| value == "yes")
        .unwrap_or(false);
    let mut graphics = Vec::new();
    let mut pins = Vec::new();

    // Check pin visibility flags
    // Handles: (pin_numbers hide), (pin_numbers (hide yes)), (pin_numbers (hide))
    let show_pin_numbers = symbol_node
        .find("pin_numbers")
        .map(|pn| {
            // atom form: (pin_numbers hide)
            if pn.first_arg() == Some("hide") {
                return false;
            }
            // list form: (pin_numbers (hide yes)) or (pin_numbers (hide))
            if let Some(h) = pn.find("hide") {
                return !h.first_arg().map(|v| v == "yes").unwrap_or(true);
            }
            // child atom form: (pin_numbers ... hide ...)
            if pn
                .children()
                .iter()
                .any(|c| matches!(c, SExpr::Atom(s) if s == "hide"))
            {
                return false;
            }
            true
        })
        .unwrap_or(true);

    // pin_names can have: (pin_names hide), (pin_names (offset X) hide), (pin_names (offset X)),
    // (pin_names (hide yes)), (pin_names (hide))
    let pin_names_node = symbol_node.find("pin_names");
    let show_pin_names = pin_names_node
        .map(|pn| {
            // atom form: (pin_names hide)
            if pn.first_arg() == Some("hide") {
                return false;
            }
            // list form: (pin_names (hide yes)) or (pin_names (hide))
            if let Some(h) = pn.find("hide") {
                return !h.first_arg().map(|v| v == "yes").unwrap_or(true);
            }
            // child atom form: (pin_names (offset X) hide)
            !pn.children()
                .iter()
                .any(|c| matches!(c, SExpr::Atom(s) if s == "hide"))
        })
        .unwrap_or(true);
    let pin_name_offset = pin_names_node
        .and_then(|pn| pn.find("offset"))
        .and_then(|o| o.arg_f64(0))
        .unwrap_or(0.508);

    /// Extract `(unit, body_style)` from a sub-symbol name like `"Device_R_1_1"`.
    /// Format: `PREFIX_UNIT_BODYSTYLE` — we split from the right.
    /// - unit 0 = common to all units
    /// - body_style 1 = normal, 2 = De Morgan
    fn sub_unit_style(sub: &SExpr) -> (u32, u32) {
        let name = sub.first_arg().unwrap_or("");
        // rsplitn(3, '_') gives [body_style, unit, prefix] from the right
        let parts: Vec<&str> = name.rsplitn(3, '_').collect();
        let body_style = parts
            .first()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1);
        let unit = parts
            .get(1)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);
        (unit, body_style)
    }

    /// True if a sub-symbol name indicates unit=0 (common to all units).
    fn is_unit_zero(sub: &SExpr) -> bool {
        sub_unit_style(sub).0 == 0
    }

    // Sort sub-symbols so unit=0 (common) subs come AFTER unit-specific ones;
    // this ensures common graphics paint over unit-specific ones.
    let mut all_subs = symbol_node.find_all("symbol");
    all_subs.sort_by_key(|s| if is_unit_zero(s) { 1_u8 } else { 0_u8 });

    // Collect graphics and pins from sub-symbols, tagging each with unit/body_style
    for sub in &all_subs {
        let (unit, body_style) = sub_unit_style(sub);
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
                            graphics.push(LibGraphic {
                                unit,
                                body_style,
                                graphic: Graphic::Polyline {
                                    points,
                                    width: parse_stroke_width(child),
                                    fill: parse_fill_type(child),
                                },
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
                    graphics.push(LibGraphic {
                        unit,
                        body_style,
                        graphic: Graphic::Rectangle {
                            start,
                            end,
                            width: parse_stroke_width(child),
                            fill: parse_fill_type(child),
                        },
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
                    graphics.push(LibGraphic {
                        unit,
                        body_style,
                        graphic: Graphic::Circle {
                            center,
                            radius,
                            width: parse_stroke_width(child),
                            fill: parse_fill_type(child),
                        },
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
                    graphics.push(LibGraphic {
                        unit,
                        body_style,
                        graphic: Graphic::Arc {
                            start,
                            mid,
                            end,
                            width: parse_stroke_width(child),
                            fill: parse_fill_type(child),
                        },
                    });
                }
                Some("bezier") => {
                    // Cubic bezier: 4 control points in (pts (xy x y)...) form
                    if let Some(pts) = child.find("pts") {
                        let points: Vec<Point> = pts
                            .find_all("xy")
                            .iter()
                            .map(|xy| Point {
                                x: xy.arg_f64(0).unwrap_or(0.0),
                                y: xy.arg_f64(1).unwrap_or(0.0),
                            })
                            .collect();
                        // KiCad always has exactly 4 control points for a cubic bezier
                        if points.len() == 4 {
                            graphics.push(LibGraphic {
                                unit,
                                body_style,
                                graphic: Graphic::Bezier {
                                    points,
                                    width: parse_stroke_width(child),
                                    fill: parse_fill_type(child),
                                },
                            });
                        }
                    }
                }
                Some("text") => {
                    let text = child.first_arg().unwrap_or("").to_string();
                    let (position, rotation) = parse_at(child);
                    let effects = child.find("effects");
                    let font = effects.and_then(|e| e.find("font"));
                    let font_size = font
                        .and_then(|f| f.find("size"))
                        .and_then(|s| s.arg_f64(0))
                        .unwrap_or(1.27);
                    let bold = font
                        .map(|f| {
                            f.find("bold")
                                .and_then(|b| b.first_arg())
                                .map(|v| v == "yes")
                                .unwrap_or_else(|| {
                                    f.children()
                                        .iter()
                                        .any(|c| matches!(c, SExpr::Atom(s) if s == "bold"))
                                })
                        })
                        .unwrap_or(false);
                    let italic = font
                        .map(|f| {
                            f.find("italic")
                                .and_then(|b| b.first_arg())
                                .map(|v| v == "yes")
                                .unwrap_or_else(|| {
                                    f.children()
                                        .iter()
                                        .any(|c| matches!(c, SExpr::Atom(s) if s == "italic"))
                                })
                        })
                        .unwrap_or(false);
                    let justify = effects.and_then(|e| e.find("justify"));
                    let justify_h = justify
                        .and_then(|j| j.first_arg())
                        .map(|v| parse_halign(v))
                        .unwrap_or(HAlign::Center);
                    let justify_v = justify
                        .and_then(|j| j.arg(1))
                        .map(|v| parse_valign(v))
                        .unwrap_or(VAlign::Center);
                    graphics.push(LibGraphic {
                        unit,
                        body_style,
                        graphic: Graphic::Text {
                            text,
                            position,
                            rotation,
                            font_size,
                            bold,
                            italic,
                            justify_h,
                            justify_v,
                        },
                    });
                }
                Some("text_box") => {
                    let text = child.first_arg().unwrap_or("").to_string();
                    let (position, rotation) = parse_at(child);
                    let size = child
                        .find("size")
                        .map(|s| Point {
                            x: s.arg_f64(0).unwrap_or(0.0),
                            y: s.arg_f64(1).unwrap_or(0.0),
                        })
                        .unwrap_or(Point { x: 0.0, y: 0.0 });
                    let effects = child.find("effects");
                    let font = effects.and_then(|e| e.find("font"));
                    let font_size = font
                        .and_then(|f| f.find("size"))
                        .and_then(|s| s.arg_f64(0))
                        .unwrap_or(1.27);
                    let bold = font
                        .map(|f| {
                            f.find("bold")
                                .and_then(|b| b.first_arg())
                                .map(|v| v == "yes")
                                .unwrap_or_else(|| {
                                    f.children()
                                        .iter()
                                        .any(|c| matches!(c, SExpr::Atom(s) if s == "bold"))
                                })
                        })
                        .unwrap_or(false);
                    let italic = font
                        .map(|f| {
                            f.find("italic")
                                .and_then(|b| b.first_arg())
                                .map(|v| v == "yes")
                                .unwrap_or_else(|| {
                                    f.children()
                                        .iter()
                                        .any(|c| matches!(c, SExpr::Atom(s) if s == "italic"))
                                })
                        })
                        .unwrap_or(false);
                    graphics.push(LibGraphic {
                        unit,
                        body_style,
                        graphic: Graphic::TextBox {
                            text,
                            position,
                            rotation,
                            size,
                            font_size,
                            bold,
                            italic,
                            width: parse_stroke_width(child),
                            fill: parse_fill_type(child),
                        },
                    });
                }
                _ => {}
            }
        }

        // Parse pins
        for pin in sub.children().iter().filter(|c| c.keyword() == Some("pin")) {
            let pin_type = parse_pin_electrical_type(pin.first_arg().unwrap_or("unspecified"));
            let shape = parse_pin_shape(pin.arg(1).unwrap_or("line"));
            let (position, rotation) = parse_at(pin);
            let length = pin
                .find("length")
                .and_then(|l| l.arg_f64(0))
                .unwrap_or(2.54);
            let visible = !pin
                .find("hide")
                .map(|hide| hide.first_arg().map(|value| value == "yes").unwrap_or(true))
                .unwrap_or(false);

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

            pins.push(LibPin {
                unit,
                body_style,
                pin: Pin {
                    pin_type,
                    shape,
                    position,
                    rotation,
                    length,
                    name,
                    number,
                    visible,
                    name_visible,
                    number_visible,
                },
            });
        }
    }

    LibSymbol {
        id,
        reference,
        value,
        footprint,
        datasheet,
        description,
        keywords,
        fp_filters,
        in_bom,
        on_board,
        in_pos_files,
        duplicate_pin_numbers_are_jumpers,
        graphics,
        pins,
        show_pin_numbers,
        show_pin_names,
        pin_name_offset,
    }
}

// ---------------------------------------------------------------------------
// Schematic element helpers
// ---------------------------------------------------------------------------

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

fn parse_symbol_instances(symbol_node: &SExpr) -> Vec<SymbolInstance> {
    let mut instances = Vec::new();

    if let Some(instances_node) = symbol_node.find("instances") {
        for project_node in instances_node.find_all("project") {
            let project = project_node.first_arg().unwrap_or("").to_string();
            for path_node in project_node.find_all("path") {
                instances.push(SymbolInstance {
                    project: project.clone(),
                    path: path_node.first_arg().unwrap_or("").to_string(),
                    reference: path_node
                        .find("reference")
                        .and_then(|r| r.first_arg())
                        .unwrap_or("")
                        .to_string(),
                    unit: path_node
                        .find("unit")
                        .and_then(|u| u.first_arg())
                        .and_then(|u| u.parse::<u32>().ok())
                        .unwrap_or(1),
                });
            }
        }
    }

    instances
}

fn parse_sheet_instances(sheet_node: &SExpr) -> Vec<SheetInstance> {
    let mut instances = Vec::new();

    if let Some(instances_node) = sheet_node.find("instances") {
        for project_node in instances_node.find_all("project") {
            let project = project_node.first_arg().unwrap_or("").to_string();
            for path_node in project_node.find_all("path") {
                instances.push(SheetInstance {
                    project: project.clone(),
                    path: path_node.first_arg().unwrap_or("").to_string(),
                    page: path_node
                        .find("page")
                        .and_then(|p| p.first_arg())
                        .unwrap_or("1")
                        .to_string(),
                });
            }
        }
    }

    instances
}

fn parse_root_sheet_page(root: &SExpr) -> String {
    if let Some(sheet_instances) = root.find("sheet_instances") {
        for path_node in sheet_instances.find_all("path") {
            if path_node.first_arg() == Some("/") {
                return path_node
                    .find("page")
                    .and_then(|p| p.first_arg())
                    .unwrap_or("1")
                    .to_string();
            }
        }
    }

    root.children()
        .iter()
        .find(|child| child.keyword() == Some("path") && child.first_arg() == Some("/"))
        .and_then(|path_node| path_node.find("page"))
        .and_then(|page| page.first_arg())
        .unwrap_or("1")
        .to_string()
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
        stroke_width: parse_stroke_width(node),
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
        .map(|v| parse_halign(v))
        .unwrap_or(HAlign::Left);
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
    let datasheet = s.property("Datasheet").unwrap_or("").to_string();
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

    // (fields_autoplaced) with no args OR (fields_autoplaced yes) both mean true
    let fields_autoplaced = s
        .find("fields_autoplaced")
        .map(|f| f.first_arg().map(|v| v == "yes").unwrap_or(true))
        .unwrap_or(false);

    let pin_uuids = s
        .children()
        .iter()
        .filter(|child| child.keyword() == Some("pin"))
        .filter_map(|pin| {
            pin.first_arg()
                .map(|number| (number.to_string(), parse_uuid(pin)))
        })
        .collect();

    let instances = parse_symbol_instances(s);

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
    let ref_text = ref_prop
        .map(|p| parse_text_prop(p, position))
        .unwrap_or(TextProp {
            position,
            rotation: 0.0,
            font_size: 1.27,
            justify_h: HAlign::Center,
            justify_v: VAlign::Center,
            hidden: false,
        });
    let val_text = val_prop
        .map(|p| parse_text_prop(p, position))
        .unwrap_or(TextProp {
            position,
            rotation: 0.0,
            font_size: 1.27,
            justify_h: HAlign::Center,
            justify_v: VAlign::Center,
            hidden: false,
        });

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
        datasheet,
        position,
        rotation,
        mirror_x,
        mirror_y,
        unit,
        is_power,
        ref_text: Some(ref_text),
        val_text: Some(val_text),
        fields_autoplaced,
        dnp,
        in_bom,
        on_board,
        exclude_from_sim,
        locked,
        fields,
        pin_uuids,
        instances,
    }
}

fn parse_child_sheet(s: &SExpr) -> ChildSheet {
    let (position, _) = parse_at(s);
    let size = s
        .find("size")
        .map(|sz| (sz.arg_f64(0).unwrap_or(20.0), sz.arg_f64(1).unwrap_or(15.0)))
        .unwrap_or((20.0, 15.0));
    let fields_autoplaced = s
        .find("fields_autoplaced")
        .map(|f| f.first_arg().map(|v| v == "yes").unwrap_or(true))
        .unwrap_or(false);
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
        // Modern KiCad: "Sheet name" / "Sheet file" (with space).
        // Legacy fallback: "Sheetname" / "Sheetfile" (no space).
        name: s
            .property("Sheet name")
            .or_else(|| s.property("Sheetname"))
            .unwrap_or("Unnamed")
            .to_string(),
        filename: s
            .property("Sheet file")
            .or_else(|| s.property("Sheetfile"))
            .unwrap_or("")
            .to_string(),
        position,
        size,
        stroke_width: parse_stroke_width(s),
        fill: parse_fill_type(s),
        fields_autoplaced,
        pins,
        instances: parse_sheet_instances(s),
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
        let fill = parse_fill_type(pl);
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
            fill: parse_fill_type(arc),
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
        let fill_type = parse_fill_type(circ);
        drawings.push(SchDrawing::Circle {
            uuid: parse_uuid(circ),
            center,
            radius,
            width: parse_stroke_width(circ),
            fill: fill_type,
        });
    }

    // Schematic-level rectangle elements (drawing annotations, not lib graphics)
    for rect in root.find_all("rectangle") {
        let start = rect
            .find("start")
            .map(|s| Point {
                x: s.arg_f64(0).unwrap_or(0.0),
                y: s.arg_f64(1).unwrap_or(0.0),
            })
            .unwrap_or(Point { x: 0.0, y: 0.0 });
        let end = rect
            .find("end")
            .map(|e| Point {
                x: e.arg_f64(0).unwrap_or(0.0),
                y: e.arg_f64(1).unwrap_or(0.0),
            })
            .unwrap_or(Point { x: 0.0, y: 0.0 });
        let fill = rect
            .find("fill")
            .and_then(|f| f.find("type"))
            .and_then(|t| t.first_arg())
            .map(|t| match t {
                "outline" => FillType::Outline,
                "background" => FillType::Background,
                _ => FillType::None,
            })
            .unwrap_or(FillType::None);
        drawings.push(SchDrawing::Rect {
            uuid: parse_uuid(rect),
            start,
            end,
            width: parse_stroke_width(rect),
            fill,
        });
    }

    drawings
}

// ---------------------------------------------------------------------------
// Main schematic parser
// ---------------------------------------------------------------------------

/// Parse a `.kicad_sch` file from its string contents.
pub fn parse_schematic(content: &str) -> Result<SchematicSheet, ParseError> {
    let root = sexpr::parse(content)?;

    if root.keyword() != Some("kicad_sch") {
        return Err(ParseError::InvalidSExpr(
            "Not a KiCad schematic file".to_string(),
        ));
    }

    let version = root
        .find("version")
        .and_then(|v| v.first_arg())
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);
    let generator = root
        .find("generator")
        .and_then(|v| v.first_arg())
        .unwrap_or("unknown")
        .to_string();
    let generator_version = root
        .find("generator_version")
        .and_then(|v| v.first_arg())
        .unwrap_or("")
        .to_string();
    let paper_size = root
        .find("paper")
        .and_then(|v| v.first_arg())
        .unwrap_or("A4")
        .to_string();
    let root_sheet_page = parse_root_sheet_page(&root);
    let uuid = parse_uuid(&root);

    // Parse library symbols
    let mut lib_symbols = HashMap::new();
    if let Some(lib_node) = root.find("lib_symbols") {
        for sym in lib_node.find_all("symbol") {
            let parsed = parse_lib_symbol(sym);
            lib_symbols.insert(parsed.id.clone(), parsed);
        }
    }

    // find_all() only searches direct children of `root` (one level deep), so
    // it will not descend into lib_symbols sub-symbols.  The `lib_id` filter
    // is an additional guard: instance symbols always carry a `lib_id` child
    // whereas lib-definition sub-symbols (e.g. "Device:R_0_1") never do.
    let symbols: Vec<Symbol> = root
        .find_all("symbol")
        .iter()
        .filter(|s| s.find("lib_id").is_some())
        .map(|s| parse_symbol_instance(s))
        .collect();

    let wires: Vec<Wire> = root
        .find_all("wire")
        .iter()
        .map(|w| parse_wire(w))
        .collect();

    let junctions: Vec<Junction> = root
        .find_all("junction")
        .iter()
        .map(|j| Junction {
            uuid: parse_uuid(j),
            position: parse_at(j).0,
            diameter: j.find("diameter").and_then(|d| d.arg_f64(0)).unwrap_or(0.0),
        })
        .collect();

    let mut labels: Vec<Label> = Vec::new();
    for (keyword, ltype) in [
        ("label", LabelType::Net),
        ("global_label", LabelType::Global),
        ("hierarchical_label", LabelType::Hierarchical),
    ] {
        for l in root.find_all(keyword) {
            labels.push(parse_label(l, ltype));
        }
    }

    let no_connects: Vec<NoConnect> = root
        .find_all("no_connect")
        .iter()
        .map(|nc| NoConnect {
            uuid: parse_uuid(nc),
            position: parse_at(nc).0,
        })
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
                        .map(|xy| Point {
                            x: xy.arg_f64(0).unwrap_or(0.0),
                            y: xy.arg_f64(1).unwrap_or(0.0),
                        })
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
            BusEntry {
                uuid: parse_uuid(be),
                position,
                size,
            }
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
            let effects = t.find("effects");
            let font_size = effects
                .and_then(|e| e.find("font"))
                .and_then(|f| f.find("size"))
                .and_then(|s| s.arg_f64(0))
                .unwrap_or(1.27);
            let justify = effects.and_then(|e| e.find("justify"));
            let justify_h = justify
                .and_then(|j| j.first_arg())
                .map(parse_halign)
                .unwrap_or(HAlign::Left);
            let justify_v = justify
                .and_then(|j| j.arg(1))
                .map(parse_valign)
                .unwrap_or(VAlign::Center);
            TextNote {
                uuid: parse_uuid(t),
                text: t.first_arg().unwrap_or("").to_string(),
                position,
                rotation,
                font_size,
                justify_h,
                justify_v,
            }
        })
        .collect();

    let no_erc_directives: Vec<NoConnect> = root
        .find_all("no_erc")
        .iter()
        .map(|ne| NoConnect {
            uuid: parse_uuid(ne),
            position: parse_at(ne).0,
        })
        .collect();

    let title_block = parse_title_block(&root);

    Ok(SchematicSheet {
        uuid,
        version,
        generator,
        generator_version,
        paper_size,
        root_sheet_page,
        symbols,
        wires,
        junctions,
        labels,
        child_sheets,
        no_connects,
        text_notes,
        buses,
        bus_entries,
        drawings,
        no_erc_directives,
        title_block,
        lib_symbols,
    })
}

/// Parse a `.kicad_sch` file from a file path.
pub fn parse_schematic_file(path: &Path) -> Result<SchematicSheet, ParseError> {
    let content = std::fs::read_to_string(path)?;
    parse_schematic(&content)
}

// ---------------------------------------------------------------------------
// Project parser
// ---------------------------------------------------------------------------

/// Parse a `.kicad_pro` or `.snxprj` project file and discover all sheets.
pub fn parse_project(path: &Path) -> Result<ProjectData, ParseError> {
    let dir = path.parent().unwrap_or(Path::new("."));
    let project_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled")
        .to_string();

    // Look for schematic root: prefer .snxsch, fall back to .kicad_sch
    let snx_sch_name = format!("{}.snxsch", project_name);
    let kicad_sch_name = format!("{}.kicad_sch", project_name);
    let schematic_root = if dir.join(&snx_sch_name).exists() {
        Some(snx_sch_name)
    } else if dir.join(&kicad_sch_name).exists() {
        Some(kicad_sch_name)
    } else {
        None
    };

    // Look for PCB: prefer .snxpcb, fall back to .kicad_pcb
    let snx_pcb_name = format!("{}.snxpcb", project_name);
    let kicad_pcb_name = format!("{}.kicad_pcb", project_name);
    let pcb_file = if dir.join(&snx_pcb_name).exists() {
        Some(snx_pcb_name)
    } else if dir.join(&kicad_pcb_name).exists() {
        Some(kicad_pcb_name)
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
) -> Result<(), ParseError> {
    // Iterative BFS with depth tracking (no recursion -- safe for any hierarchy)
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
        let metadata = std::fs::metadata(&file_path).map_err(|e| {
            ParseError::IoError(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Failed to read {}: {}", filename, e),
            ))
        })?;
        if metadata.len() > MAX_FILE_SIZE {
            return Err(ParseError::InvalidValue(format!(
                "File too large: {} ({} bytes)",
                filename,
                metadata.len()
            )));
        }

        let content = std::fs::read_to_string(&file_path).map_err(|e| {
            ParseError::IoError(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Failed to read {}: {}", filename, e),
            ))
        })?;

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
            // Prevent path traversal via crafted sheet filenames.
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
            // Always try to canonicalize rather than checking exists() first:
            // the exists()+canonicalize() pattern is a TOCTOU race.  If the
            // file doesn't exist, canonicalize() will return an error and we
            // simply skip the path; if it does exist we verify it stays within
            // the project directory before queueing it.
            if let Ok(canonical) = joined.canonicalize() {
                if let Ok(canonical_dir) = dir.canonicalize() {
                    if !canonical.starts_with(&canonical_dir) {
                        continue;
                    }
                }
            } else {
                // File does not exist (or is otherwise inaccessible) — skip.
                continue;
            }
            queue.push((child, depth + 1));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_kicad_sch() -> String {
        r#"(kicad_sch
  (version 20231120)
  (generator "test")
  (generator_version "0.1")
  (uuid "00000000-0000-0000-0000-000000000001")
  (paper "A4")
  (wire
    (pts (xy 10 20) (xy 30 20))
    (stroke (width 0) (type default))
    (uuid "00000000-0000-0000-0000-000000000002")
  )
  (junction
    (at 20 20)
    (uuid "00000000-0000-0000-0000-000000000003")
  )
  (label "VCC"
    (at 20 20 0)
    (effects (font (size 1.27 1.27)))
    (uuid "00000000-0000-0000-0000-000000000004")
  )
  (no_connect
    (at 50 50)
    (uuid "00000000-0000-0000-0000-000000000005")
  )
  (text "Hello World"
    (at 100 100 0)
    (effects (font (size 1.27 1.27)))
    (uuid "00000000-0000-0000-0000-000000000006")
  )
)"#
        .to_string()
    }

    #[test]
    fn parse_minimal_schematic() {
        let content = minimal_kicad_sch();
        let sheet = parse_schematic(&content).unwrap();
        assert_eq!(
            sheet.uuid,
            Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap()
        );
        assert_eq!(sheet.version, 20231120);
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
        assert_eq!(
            wire.uuid,
            Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap()
        );
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
        assert_eq!(
            nc.uuid,
            Uuid::parse_str("00000000-0000-0000-0000-000000000005").unwrap()
        );
        assert_eq!(nc.position.x, 50.0);
    }

    #[test]
    fn parse_text_note() {
        let content = minimal_kicad_sch();
        let sheet = parse_schematic(&content).unwrap();
        let note = &sheet.text_notes[0];
        assert_eq!(note.text, "Hello World");
        assert_eq!(
            note.uuid,
            Uuid::parse_str("00000000-0000-0000-0000-000000000006").unwrap()
        );
        assert_eq!(note.font_size, 1.27);
    }

    #[test]
    fn parse_legacy_not_connected_pin_type() {
        assert_eq!(
            parse_pin_electrical_type("not_connected"),
            PinElectricalType::NotConnected
        );
    }

    #[test]
    fn parse_kicad10_fields_default() {
        // KiCad 10 fields should default correctly when absent
        let content = r#"(kicad_sch
  (version 20260326)
  (generator "eeschema")
  (generator_version "10.0")
  (uuid "00000000-0000-0000-0000-000000000010")
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
    (uuid "00000000-0000-0000-0000-000000000011")
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

    #[test]
    fn parse_lib_symbol_preserves_parent_metadata() {
        let symbol = sexpr::parse(
            r#"(symbol "Interface_Ethernet:W5500"
  (in_bom yes)
  (on_board yes)
  (in_pos_files yes)
  (duplicate_pin_numbers_are_jumpers no)
  (property "Reference" "U" (id 0) (at 0 0 0) (effects (font (size 1.27 1.27))))
  (property "Value" "W5500" (id 1) (at 0 0 0) (effects (font (size 1.27 1.27))))
  (property "Footprint" "Package_QFP:LQFP-48_7x7mm_P0.5mm" (id 2) (at 0 0 0) (effects (font (size 1.27 1.27))))
  (property "Datasheet" "http://example.invalid/ds.pdf" (id 3) (at 0 0 0) (effects (font (size 1.27 1.27))))
  (property "Description" "Ethernet controller" (id 4) (at 0 0 0) (effects (font (size 1.27 1.27))))
  (property "ki_keywords" "WIZnet Ethernet" (id 5) (at 0 0 0) (effects (font (size 1.27 1.27))))
  (property "ki_fp_filters" "LQFP*" (id 6) (at 0 0 0) (effects (font (size 1.27 1.27))))
  (symbol "W5500_0_1")
  (symbol "W5500_1_1")
)"#,
        )
        .unwrap();

        let parsed = parse_lib_symbol(&symbol);
        assert_eq!(parsed.description, "Ethernet controller");
        assert_eq!(parsed.keywords, "WIZnet Ethernet");
        assert_eq!(parsed.fp_filters, "LQFP*");
        assert!(parsed.in_pos_files);
        assert!(!parsed.duplicate_pin_numbers_are_jumpers);
    }

    #[test]
    fn parse_lib_symbol_preserves_pin_hide_flag() {
        let symbol = sexpr::parse(
            r#"(symbol "Interface_Ethernet:W5500"
    (symbol "W5500_1_1"
        (pin no_connect line
            (at 20.32 0 0)
            (length 0)
            (hide yes)
            (name "NC" (effects (font (size 1.27 1.27))))
            (number "7" (effects (font (size 1.27 1.27))))
        )
    )
)"#,
        )
        .unwrap();

        let parsed = parse_lib_symbol(&symbol);
        assert_eq!(parsed.pins.len(), 1);
        assert!(!parsed.pins[0].pin.visible);
    }

    #[test]
    fn parse_symbol_and_sheet_instances_and_root_page() {
        let content = r#"(kicad_sch
    (version 20231120)
    (generator "test")
    (uuid "00000000-0000-0000-0000-000000000001")
    (paper "A4")
    (symbol
        (lib_id "Device:R")
        (at 10 10 0)
        (unit 1)
        (in_bom yes)
        (on_board yes)
        (uuid "00000000-0000-0000-0000-000000000010")
        (property "Reference" "R1" (at 10 8 0) (effects (font (size 1.27 1.27))))
        (property "Value" "10k" (at 10 12 0) (effects (font (size 1.27 1.27))))
        (property "Footprint" "Resistor_SMD:R_0402" (at 0 0 0) (effects (font (size 1.27 1.27)) (hide yes)))
        (property "Datasheet" "https://example.invalid/r1" (at 0 0 0) (effects (font (size 1.27 1.27)) (hide yes)))
        (pin "1" (uuid "00000000-0000-0000-0000-000000000011"))
        (instances
            (project "GateMagic"
                (path "/00000000-0000-0000-0000-000000000001"
                    (reference "R1")
                    (unit 1)
                )
            )
        )
    )
    (sheet
        (at 20 20)
        (size 30 20)
        (fields_autoplaced)
        (stroke (width 0.2) (type default))
        (fill (type background))
        (uuid "00000000-0000-0000-0000-000000000020")
        (property "Sheet name" "Child" (at 20 19 0) (effects (font (size 1.27 1.27))))
        (property "Sheet file" "child.kicad_sch" (at 20 41 0) (effects (font (size 1.27 1.27))))
        (instances
            (project "GateMagic"
                (path "/00000000-0000-0000-0000-000000000001/00000000-0000-0000-0000-000000000020"
                    (page "2")
                )
            )
        )
    )
    (sheet_instances
        (path "/"
            (page "7")
        )
    )
)"#;

        let sheet = parse_schematic(content).unwrap();
        assert_eq!(sheet.root_sheet_page, "7");
        assert_eq!(sheet.symbols[0].datasheet, "https://example.invalid/r1");
        assert_eq!(sheet.symbols[0].pin_uuids.len(), 1);
        assert_eq!(sheet.symbols[0].instances.len(), 1);
        assert_eq!(sheet.symbols[0].instances[0].project, "GateMagic");
        assert_eq!(sheet.child_sheets[0].stroke_width, 0.2);
        assert!(matches!(sheet.child_sheets[0].fill, FillType::Background));
        assert!(sheet.child_sheets[0].fields_autoplaced);
        assert_eq!(sheet.child_sheets[0].instances[0].page, "2");
    }
}
