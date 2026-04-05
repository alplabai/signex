use serde::{Deserialize, Serialize};
use std::path::Path;

use super::sexpr::{self, SExpr};

// --- Data structures ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchematicSheet {
    pub uuid: String,
    pub version: String,
    pub generator: String,
    pub paper_size: String,
    pub symbols: Vec<Symbol>,
    pub wires: Vec<Wire>,
    pub junctions: Vec<Junction>,
    pub labels: Vec<Label>,
    pub child_sheets: Vec<ChildSheet>,
    pub no_connects: Vec<Point>,
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
    pub unit: u32,
    pub is_power: bool,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LabelType {
    Net,
    Global,
    Hierarchical,
    Power,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildSheet {
    pub uuid: String,
    pub name: String,
    pub filename: String,
    pub position: Point,
    pub size: (f64, f64),
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

// --- Parsing functions ---

fn parse_at(node: &SExpr) -> (Point, f64) {
    let at = node.find("at");
    match at {
        Some(at) => {
            let x = at.arg_f64(0).unwrap_or(0.0);
            let y = at.arg_f64(1).unwrap_or(0.0);
            let rot = at.arg_f64(2).unwrap_or(0.0);
            (Point { x, y }, rot)
        }
        None => (Point { x: 0.0, y: 0.0 }, 0.0),
    }
}

fn parse_uuid(node: &SExpr) -> String {
    node.find("uuid")
        .and_then(|u| u.first_arg())
        .unwrap_or("unknown")
        .to_string()
}

pub fn parse_schematic(content: &str) -> Result<SchematicSheet, String> {
    let root = sexpr::parse(content)?;

    if root.keyword() != Some("kicad_sch") {
        return Err("Not a KiCad schematic file".to_string());
    }

    let version = root
        .find("version")
        .and_then(|v| v.first_arg())
        .unwrap_or("unknown")
        .to_string();
    let generator = root
        .find("generator")
        .and_then(|v| v.first_arg())
        .unwrap_or("unknown")
        .to_string();
    let paper_size = root
        .find("paper")
        .and_then(|v| v.first_arg())
        .unwrap_or("A4")
        .to_string();
    let uuid = parse_uuid(&root);

    // Parse symbols (skip lib_symbols section — those are definitions, not instances)
    let lib_symbol_ids: Vec<&str> = root
        .find("lib_symbols")
        .map(|ls| {
            ls.find_all("symbol")
                .iter()
                .filter_map(|s| s.first_arg())
                .collect()
        })
        .unwrap_or_default();

    let symbols: Vec<Symbol> = root
        .find_all("symbol")
        .iter()
        .filter(|s| {
            // Only instance symbols, not library definitions inside lib_symbols
            let is_lib_def = s
                .first_arg()
                .map(|id| lib_symbol_ids.contains(&id))
                .unwrap_or(false);
            !is_lib_def
        })
        .map(|s| {
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

            Symbol {
                uuid: parse_uuid(s),
                lib_id,
                reference,
                value,
                footprint,
                position,
                rotation,
                unit,
                is_power,
            }
        })
        .collect();

    // Parse wires
    let wires: Vec<Wire> = root
        .find_all("wire")
        .iter()
        .map(|w| {
            let pts = w.find("pts");
            let (start, end) = match pts {
                Some(pts) => {
                    let xy_nodes = pts.find_all("xy");
                    let start = xy_nodes.first().map(|xy| Point {
                        x: xy.arg_f64(0).unwrap_or(0.0),
                        y: xy.arg_f64(1).unwrap_or(0.0),
                    }).unwrap_or(Point { x: 0.0, y: 0.0 });
                    let end = xy_nodes.get(1).map(|xy| Point {
                        x: xy.arg_f64(0).unwrap_or(0.0),
                        y: xy.arg_f64(1).unwrap_or(0.0),
                    }).unwrap_or(start);
                    (start, end)
                }
                None => (Point { x: 0.0, y: 0.0 }, Point { x: 0.0, y: 0.0 }),
            };
            Wire {
                uuid: parse_uuid(w),
                start,
                end,
            }
        })
        .collect();

    // Parse junctions
    let junctions: Vec<Junction> = root
        .find_all("junction")
        .iter()
        .map(|j| {
            let (position, _) = parse_at(j);
            Junction {
                uuid: parse_uuid(j),
                position,
            }
        })
        .collect();

    // Parse labels
    let mut labels: Vec<Label> = Vec::new();
    for l in root.find_all("label") {
        let (position, rotation) = parse_at(l);
        labels.push(Label {
            uuid: parse_uuid(l),
            text: l.first_arg().unwrap_or("").to_string(),
            position,
            rotation,
            label_type: LabelType::Net,
        });
    }
    for l in root.find_all("global_label") {
        let (position, rotation) = parse_at(l);
        labels.push(Label {
            uuid: parse_uuid(l),
            text: l.first_arg().unwrap_or("").to_string(),
            position,
            rotation,
            label_type: LabelType::Global,
        });
    }
    for l in root.find_all("hierarchical_label") {
        let (position, rotation) = parse_at(l);
        labels.push(Label {
            uuid: parse_uuid(l),
            text: l.first_arg().unwrap_or("").to_string(),
            position,
            rotation,
            label_type: LabelType::Hierarchical,
        });
    }

    // Parse no-connect markers
    let no_connects: Vec<Point> = root
        .find_all("no_connect")
        .iter()
        .map(|nc| parse_at(nc).0)
        .collect();

    // Parse child sheet references
    let child_sheets: Vec<ChildSheet> = root
        .find_all("sheet")
        .iter()
        .map(|s| {
            let (position, _) = parse_at(s);
            let size_node = s.find("size");
            let size = match size_node {
                Some(sz) => (
                    sz.arg_f64(0).unwrap_or(20.0),
                    sz.arg_f64(1).unwrap_or(15.0),
                ),
                None => (20.0, 15.0),
            };
            let name = s.property("Sheetname").unwrap_or("Unnamed").to_string();
            let filename = s.property("Sheetfile").unwrap_or("").to_string();
            ChildSheet {
                uuid: parse_uuid(s),
                name,
                filename,
                position,
                size,
            }
        })
        .collect();

    Ok(SchematicSheet {
        uuid,
        version,
        generator,
        paper_size,
        symbols,
        wires,
        junctions,
        labels,
        child_sheets,
        no_connects,
    })
}

/// Parse a KiCad .kicad_pro project file (JSON format)
pub fn parse_project(path: &Path) -> Result<ProjectData, String> {
    let dir = path.parent().unwrap_or(Path::new("."));
    let project_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled")
        .to_string();

    // Find the root schematic (same name as project, .kicad_sch extension)
    let root_sch_name = format!("{}.kicad_sch", project_name);
    let root_sch_path = dir.join(&root_sch_name);

    let schematic_root = if root_sch_path.exists() {
        Some(root_sch_name.clone())
    } else {
        None
    };

    // Find PCB file
    let pcb_name = format!("{}.kicad_pcb", project_name);
    let pcb_path = dir.join(&pcb_name);
    let pcb_file = if pcb_path.exists() {
        Some(pcb_name)
    } else {
        None
    };

    // Parse all schematic sheets recursively
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

/// Lightweight scanner — counts elements and finds child sheets without full S-expr parsing.
/// Much faster than parse_schematic for project tree population.
fn collect_sheets(
    dir: &Path,
    filename: &str,
    sheets: &mut Vec<SheetEntry>,
) -> Result<(), String> {
    if sheets.iter().any(|s| s.filename == filename) {
        return Ok(());
    }

    let sch_path = dir.join(filename);
    let content = std::fs::read_to_string(&sch_path)
        .map_err(|e| format!("Failed to read {}: {}", filename, e))?;

    // Count top-level elements by scanning for patterns (no full parse needed)
    let mut symbols_count = 0;
    let mut wires_count = 0;
    let mut labels_count = 0;
    let mut child_filenames: Vec<String> = Vec::new();

    let mut depth = 0;
    for line in content.lines() {
        let trimmed = line.trim();

        // Track nesting depth to only count top-level elements
        let opens = trimmed.matches('(').count();
        let closes = trimmed.matches(')').count();

        if depth == 1 {
            if trimmed.starts_with("(symbol") && !trimmed.contains("lib_symbols") {
                // Skip power symbols (lib_id starts with "power:")
                if !trimmed.contains("power:") {
                    symbols_count += 1;
                }
            } else if trimmed.starts_with("(wire") {
                wires_count += 1;
            } else if trimmed.starts_with("(label")
                || trimmed.starts_with("(global_label")
                || trimmed.starts_with("(hierarchical_label")
            {
                labels_count += 1;
            }
        }

        // Find Sheetfile properties inside (sheet ...) blocks
        if trimmed.contains("\"Sheetfile\"") {
            // Extract filename from: (property "Sheetfile" "filename.kicad_sch"
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

        depth += opens;
        depth = depth.saturating_sub(closes);
    }

    let name = if sheets.is_empty() {
        "Root".to_string()
    } else {
        filename.trim_end_matches(".kicad_sch").to_string()
    };

    sheets.push(SheetEntry {
        name,
        filename: filename.to_string(),
        symbols_count,
        wires_count,
        labels_count,
    });

    for child in child_filenames {
        collect_sheets(dir, &child, sheets)?;
    }

    Ok(())
}
