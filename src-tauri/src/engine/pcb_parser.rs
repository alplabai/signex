use super::sexpr::SExpr;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════
// KiCad PCB Parser (.kicad_pcb)
// Parses the S-expression format into structured Rust types
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PcbBoard {
    pub uuid: String,
    pub version: String,
    pub generator: String,
    pub thickness: f64,
    pub outline: Vec<Point>,
    pub layers: Vec<LayerDef>,
    pub setup: PcbSetup,
    pub nets: Vec<NetDef>,
    pub footprints: Vec<Footprint>,
    pub segments: Vec<Segment>,
    pub vias: Vec<Via>,
    pub zones: Vec<Zone>,
    pub graphics: Vec<BoardGraphic>,
    pub texts: Vec<BoardText>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerDef {
    pub id: String,
    pub name: String,
    pub layer_type: String, // "signal", "power", "mixed", "user"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PcbSetup {
    pub grid_size: f64,
    pub trace_width: f64,
    pub via_diameter: f64,
    pub via_drill: f64,
    pub clearance: f64,
    pub track_min_width: f64,
    pub via_min_diameter: f64,
    pub via_min_drill: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetDef {
    pub number: u32,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Footprint {
    pub uuid: String,
    pub reference: String,
    pub value: String,
    pub footprint_id: String,
    pub position: Point,
    pub rotation: f64,
    pub layer: String,
    pub locked: bool,
    pub pads: Vec<Pad>,
    pub graphics: Vec<FpGraphic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pad {
    pub uuid: String,
    pub number: String,
    pub pad_type: String,
    pub shape: String,
    pub position: Point,
    pub size: [f64; 2],
    pub drill: Option<DrillDef>,
    pub layers: Vec<String>,
    pub net: Option<PadNet>,
    pub roundrect_ratio: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrillDef {
    pub diameter: f64,
    pub shape: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PadNet {
    pub number: u32,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FpGraphic {
    pub graphic_type: String, // "line", "rect", "circle", "arc", "text", "poly"
    pub layer: String,
    pub width: f64,
    pub start: Option<Point>,
    pub end: Option<Point>,
    pub center: Option<Point>,
    pub mid: Option<Point>,
    pub radius: Option<f64>,
    pub points: Vec<Point>,
    pub text: Option<String>,
    pub font_size: Option<f64>,
    pub position: Option<Point>,
    pub rotation: Option<f64>,
    pub fill: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub uuid: String,
    pub start: Point,
    pub end: Point,
    pub width: f64,
    pub layer: String,
    pub net: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Via {
    pub uuid: String,
    pub position: Point,
    pub diameter: f64,
    pub drill: f64,
    pub layers: [String; 2],
    pub net: u32,
    pub via_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Zone {
    pub uuid: String,
    pub net: u32,
    pub net_name: String,
    pub layer: String,
    pub outline: Vec<Point>,
    pub priority: u32,
    pub fill_type: String,
    pub thermal_relief: bool,
    pub thermal_gap: f64,
    pub thermal_width: f64,
    pub clearance: f64,
    pub min_thickness: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardGraphic {
    pub graphic_type: String,
    pub layer: String,
    pub width: f64,
    pub start: Option<Point>,
    pub end: Option<Point>,
    pub center: Option<Point>,
    pub radius: Option<f64>,
    pub points: Vec<Point>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardText {
    pub uuid: String,
    pub text: String,
    pub position: Point,
    pub layer: String,
    pub font_size: f64,
    pub rotation: f64,
}

// --- Parser ---

fn parse_point(node: &SExpr) -> Point {
    Point {
        x: node.arg(0).and_then(|s| s.parse().ok()).unwrap_or(0.0),
        y: node.arg(1).and_then(|s| s.parse().ok()).unwrap_or(0.0),
    }
}

fn parse_at(node: &SExpr) -> (Point, f64) {
    if let Some(at) = node.find("at") {
        let x = at.arg(0).and_then(|s| s.parse().ok()).unwrap_or(0.0);
        let y = at.arg(1).and_then(|s| s.parse().ok()).unwrap_or(0.0);
        let rot = at.arg(2).and_then(|s| s.parse().ok()).unwrap_or(0.0);
        (Point { x, y }, rot)
    } else {
        (Point { x: 0.0, y: 0.0 }, 0.0)
    }
}

fn parse_uuid(node: &SExpr) -> String {
    node.find("uuid")
        .and_then(|u| u.first_arg())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("gen-{}", uuid::Uuid::new_v4()))
}

pub fn parse_pcb(content: &str) -> Result<PcbBoard, String> {
    let tokens = super::sexpr::tokenize(content)?;
    let root = super::sexpr::parse_tokens(&tokens)?;

    if root.keyword() != Some("kicad_pcb") {
        return Err("Not a KiCad PCB file".to_string());
    }

    let version = root.find("version")
        .and_then(|v| v.first_arg())
        .unwrap_or("0")
        .to_string();

    let generator = root.find("generator")
        .and_then(|g| g.first_arg())
        .unwrap_or("unknown")
        .to_string();

    let uuid = parse_uuid(&root);

    // Layers
    let layers: Vec<LayerDef> = if let Some(layers_node) = root.find("layers") {
        layers_node.children().iter().filter_map(|l| {
            let id_num = l.first_arg()?;
            let name = l.arg(1)?;
            let ltype = l.arg(2).unwrap_or("signal");
            Some(LayerDef {
                id: name.to_string(),
                name: name.to_string(),
                layer_type: ltype.to_string(),
            })
        }).collect()
    } else {
        vec![]
    };

    // Setup
    let setup = if let Some(s) = root.find("setup") {
        PcbSetup {
            grid_size: s.find("grid_origin").and_then(|g| g.arg(0)?.parse().ok()).unwrap_or(1.27),
            trace_width: s.find("trace_min").and_then(|t| t.first_arg()?.parse().ok()).unwrap_or(0.2),
            via_diameter: s.find("via_size").and_then(|v| v.first_arg()?.parse().ok()).unwrap_or(0.6),
            via_drill: s.find("via_drill").and_then(|v| v.first_arg()?.parse().ok()).unwrap_or(0.3),
            clearance: s.find("clearance").and_then(|c| c.first_arg()?.parse().ok()).unwrap_or(0.2),
            track_min_width: s.find("trace_min").and_then(|t| t.first_arg()?.parse().ok()).unwrap_or(0.1),
            via_min_diameter: s.find("via_size").and_then(|v| v.first_arg()?.parse().ok()).unwrap_or(0.4),
            via_min_drill: s.find("via_drill").and_then(|v| v.first_arg()?.parse().ok()).unwrap_or(0.2),
        }
    } else {
        PcbSetup {
            grid_size: 1.27, trace_width: 0.25, via_diameter: 0.6, via_drill: 0.3,
            clearance: 0.2, track_min_width: 0.1, via_min_diameter: 0.4, via_min_drill: 0.2,
        }
    };

    // Nets
    let nets: Vec<NetDef> = root.find_all("net").iter().filter_map(|n| {
        let num: u32 = n.first_arg()?.parse().ok()?;
        let name = n.arg(1).unwrap_or("").to_string();
        Some(NetDef { number: num, name })
    }).collect();

    // Board outline (from Edge.Cuts lines)
    let mut outline_points = Vec::new();
    for gr in root.find_all("gr_line") {
        let layer = gr.find("layer").and_then(|l| l.first_arg()).unwrap_or("");
        if layer == "Edge.Cuts" {
            if let (Some(start), Some(end)) = (gr.find("start"), gr.find("end")) {
                let s = parse_point(start);
                let e = parse_point(end);
                if outline_points.is_empty() || outline_points.last().map(|p: &Point| (p.x - s.x).abs() > 0.01 || (p.y - s.y).abs() > 0.01).unwrap_or(true) {
                    outline_points.push(s);
                }
                outline_points.push(e);
            }
        }
    }

    // Footprints
    let footprints: Vec<Footprint> = root.find_all("footprint").iter().map(|fp| {
        let footprint_id = fp.first_arg().unwrap_or("").to_string();
        let (pos, rot) = parse_at(fp);
        let layer = fp.find("layer").and_then(|l| l.first_arg()).unwrap_or("F.Cu").to_string();
        let locked = fp.find("locked").is_some();
        let uuid = parse_uuid(fp);

        let reference = fp.find_all("property").iter()
            .find(|p| p.first_arg() == Some("Reference"))
            .and_then(|p| p.arg(1))
            .unwrap_or("?")
            .to_string();

        let value = fp.find_all("property").iter()
            .find(|p| p.first_arg() == Some("Value"))
            .and_then(|p| p.arg(1))
            .unwrap_or("")
            .to_string();

        // Pads
        let pads: Vec<Pad> = fp.find_all("pad").iter().map(|p| {
            let number = p.first_arg().unwrap_or("").to_string();
            let pad_type = p.arg(1).unwrap_or("smd").to_string();
            let shape = p.arg(2).unwrap_or("rect").to_string();
            let (pad_pos, _) = parse_at(p);
            let size = if let Some(sz) = p.find("size") {
                [
                    sz.arg(0).and_then(|s| s.parse().ok()).unwrap_or(1.0),
                    sz.arg(1).and_then(|s| s.parse().ok()).unwrap_or(1.0),
                ]
            } else {
                [1.0, 1.0]
            };
            let drill = p.find("drill").map(|d| DrillDef {
                diameter: d.first_arg().and_then(|s| s.parse().ok()).unwrap_or(0.3),
                shape: None,
            });
            let pad_layers: Vec<String> = if let Some(layers) = p.find("layers") {
                layers.args().iter().map(|s| s.to_string()).collect()
            } else {
                vec![layer.clone()]
            };
            let net = p.find("net").map(|n| PadNet {
                number: n.first_arg().and_then(|s| s.parse().ok()).unwrap_or(0),
                name: n.arg(1).unwrap_or("").to_string(),
            });
            let roundrect_ratio = p.find("roundrect_rratio")
                .and_then(|r| r.first_arg()?.parse().ok());

            Pad {
                uuid: parse_uuid(p),
                number, pad_type, shape, position: pad_pos, size, drill,
                layers: pad_layers, net, roundrect_ratio,
            }
        }).collect();

        // Footprint graphics
        let mut graphics = Vec::new();
        for g in fp.find_all("fp_line") {
            let gl = g.find("layer").and_then(|l| l.first_arg()).unwrap_or("").to_string();
            let w = g.find("stroke").and_then(|s| s.find("width")).and_then(|w| w.first_arg()?.parse().ok()).unwrap_or(0.1);
            let start = g.find("start").map(|s| parse_point(s));
            let end = g.find("end").map(|e| parse_point(e));
            graphics.push(FpGraphic {
                graphic_type: "line".to_string(), layer: gl, width: w,
                start, end, center: None, mid: None, radius: None,
                points: vec![], text: None, font_size: None, position: None, rotation: None, fill: None,
            });
        }
        for g in fp.find_all("fp_circle") {
            let gl = g.find("layer").and_then(|l| l.first_arg()).unwrap_or("").to_string();
            let w = g.find("stroke").and_then(|s| s.find("width")).and_then(|w| w.first_arg()?.parse().ok()).unwrap_or(0.1);
            let center = g.find("center").map(|c| parse_point(c));
            let end = g.find("end").map(|e| parse_point(e));
            let radius = if let (Some(c), Some(e)) = (&center, &end) {
                Some(((e.x - c.x).powi(2) + (e.y - c.y).powi(2)).sqrt())
            } else { None };
            graphics.push(FpGraphic {
                graphic_type: "circle".to_string(), layer: gl, width: w,
                start: None, end: None, center, mid: None, radius,
                points: vec![], text: None, font_size: None, position: None, rotation: None, fill: None,
            });
        }
        for g in fp.find_all("fp_text") {
            let text_type = g.first_arg().unwrap_or("user");
            let text_val = g.arg(1).unwrap_or("").to_string();
            let (text_pos, text_rot) = parse_at(g);
            let gl = g.find("layer").and_then(|l| l.first_arg()).unwrap_or("").to_string();
            let fs = g.find("effects").and_then(|e| e.find("font")).and_then(|f| f.find("size"))
                .and_then(|s| s.first_arg()?.parse().ok()).unwrap_or(1.0);
            let display_text = match text_type {
                "reference" => "%R".to_string(),
                "value" => "%V".to_string(),
                _ => text_val,
            };
            graphics.push(FpGraphic {
                graphic_type: "text".to_string(), layer: gl, width: 0.1,
                start: None, end: None, center: None, mid: None, radius: None,
                points: vec![], text: Some(display_text), font_size: Some(fs),
                position: Some(text_pos), rotation: Some(text_rot), fill: None,
            });
        }
        for g in fp.find_all("fp_rect") {
            let gl = g.find("layer").and_then(|l| l.first_arg()).unwrap_or("").to_string();
            let w = g.find("stroke").and_then(|s| s.find("width")).and_then(|w| w.first_arg()?.parse().ok()).unwrap_or(0.1);
            let start = g.find("start").map(|s| parse_point(s));
            let end = g.find("end").map(|e| parse_point(e));
            let fill = g.find("fill").and_then(|f| f.first_arg()).map(|f| f != "none");
            graphics.push(FpGraphic {
                graphic_type: "rect".to_string(), layer: gl, width: w,
                start, end, center: None, mid: None, radius: None,
                points: vec![], text: None, font_size: None, position: None, rotation: None, fill,
            });
        }

        Footprint {
            uuid, reference, value, footprint_id: footprint_id, position: pos,
            rotation: rot, layer, locked, pads, graphics,
        }
    }).collect();

    // Trace segments
    let segments: Vec<Segment> = root.find_all("segment").iter().map(|s| {
        let start = s.find("start").map(|p| parse_point(p)).unwrap_or(Point { x: 0.0, y: 0.0 });
        let end = s.find("end").map(|p| parse_point(p)).unwrap_or(Point { x: 0.0, y: 0.0 });
        let width = s.find("width").and_then(|w| w.first_arg()?.parse().ok()).unwrap_or(0.25);
        let layer = s.find("layer").and_then(|l| l.first_arg()).unwrap_or("F.Cu").to_string();
        let net: u32 = s.find("net").and_then(|n| n.first_arg()?.parse().ok()).unwrap_or(0);
        Segment { uuid: parse_uuid(s), start, end, width, layer, net }
    }).collect();

    // Vias
    let vias: Vec<Via> = root.find_all("via").iter().map(|v| {
        let (pos, _) = parse_at(v);
        let diameter = v.find("size").and_then(|s| s.first_arg()?.parse().ok()).unwrap_or(0.6);
        let drill = v.find("drill").and_then(|d| d.first_arg()?.parse().ok()).unwrap_or(0.3);
        let layers = if let Some(l) = v.find("layers") {
            [
                l.arg(0).unwrap_or("F.Cu").to_string(),
                l.arg(1).unwrap_or("B.Cu").to_string(),
            ]
        } else {
            ["F.Cu".to_string(), "B.Cu".to_string()]
        };
        let net: u32 = v.find("net").and_then(|n| n.first_arg()?.parse().ok()).unwrap_or(0);
        let via_type = if v.find("blind").is_some() { "blind" }
            else if v.find("micro").is_some() { "micro" }
            else { "through" };
        Via { uuid: parse_uuid(v), position: pos, diameter, drill, layers, net, via_type: via_type.to_string() }
    }).collect();

    // Zones
    let zones: Vec<Zone> = root.find_all("zone").iter().map(|z| {
        let net: u32 = z.find("net").and_then(|n| n.first_arg()?.parse().ok()).unwrap_or(0);
        let net_name = z.find("net_name").and_then(|n| n.first_arg()).unwrap_or("").to_string();
        let layer = z.find("layer").and_then(|l| l.first_arg()).unwrap_or("F.Cu").to_string();
        let priority: u32 = z.find("priority").and_then(|p| p.first_arg()?.parse().ok()).unwrap_or(0);
        let clearance = z.find("clearance").and_then(|c| c.first_arg()?.parse().ok()).unwrap_or(0.2);
        let min_thickness = z.find("min_thickness").and_then(|m| m.first_arg()?.parse().ok()).unwrap_or(0.254);

        // Outline polygon
        let outline: Vec<Point> = if let Some(poly) = z.find("polygon") {
            if let Some(pts) = poly.find("pts") {
                pts.find_all("xy").iter().map(|xy| parse_point(xy)).collect()
            } else { vec![] }
        } else { vec![] };

        // Thermal
        let thermal_relief = z.find("thermal_bridge_width").is_some();
        let thermal_gap = z.find("thermal_gap").and_then(|t| t.first_arg()?.parse().ok()).unwrap_or(0.508);
        let thermal_width = z.find("thermal_bridge_width").and_then(|t| t.first_arg()?.parse().ok()).unwrap_or(0.254);

        Zone {
            uuid: parse_uuid(z), net, net_name, layer, outline, priority,
            fill_type: "solid".to_string(), thermal_relief, thermal_gap, thermal_width,
            clearance, min_thickness,
        }
    }).collect();

    // Board-level graphics
    let mut board_graphics = Vec::new();
    for g in root.find_all("gr_line") {
        let layer = g.find("layer").and_then(|l| l.first_arg()).unwrap_or("").to_string();
        if layer == "Edge.Cuts" { continue; } // Already handled as outline
        let w = g.find("stroke").and_then(|s| s.find("width")).and_then(|w| w.first_arg()?.parse().ok())
            .or_else(|| g.find("width").and_then(|w| w.first_arg()?.parse().ok()))
            .unwrap_or(0.1);
        let start = g.find("start").map(|s| parse_point(s));
        let end = g.find("end").map(|e| parse_point(e));
        board_graphics.push(BoardGraphic {
            graphic_type: "line".to_string(), layer, width: w,
            start, end, center: None, radius: None, points: vec![],
        });
    }

    // Board-level texts
    let texts: Vec<BoardText> = root.find_all("gr_text").iter().map(|t| {
        let text = t.first_arg().unwrap_or("").to_string();
        let (pos, rot) = parse_at(t);
        let layer = t.find("layer").and_then(|l| l.first_arg()).unwrap_or("F.SilkS").to_string();
        let fs = t.find("effects").and_then(|e| e.find("font")).and_then(|f| f.find("size"))
            .and_then(|s| s.first_arg()?.parse().ok()).unwrap_or(1.0);
        BoardText { uuid: parse_uuid(t), text, position: pos, layer, font_size: fs, rotation: rot }
    }).collect();

    let thickness = root.find("general")
        .and_then(|g| g.find("thickness"))
        .and_then(|t| t.first_arg()?.parse().ok())
        .unwrap_or(1.6);

    Ok(PcbBoard {
        uuid, version, generator, thickness, outline: outline_points,
        layers, setup, nets, footprints, segments, vias, zones,
        graphics: board_graphics, texts,
    })
}
