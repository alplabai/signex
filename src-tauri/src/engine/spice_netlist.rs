use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::parser::{Label, LabelType, LibSymbol, Pin, Point, SchematicSheet, Symbol, Wire, Junction};

// --- Public types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnalysisType {
    DcOp,
    DcSweep,
    Ac,
    Transient,
    Noise,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisConfig {
    pub analysis_type: AnalysisType,
    pub params: HashMap<String, String>,
}

// --- Union-Find (ported from netResolver.ts) ---

struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
            rank: vec![0; n],
        }
    }
    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            self.parent[x] = self.find(self.parent[x]);
        }
        self.parent[x]
    }
    fn union(&mut self, a: usize, b: usize) {
        let ra = self.find(a);
        let rb = self.find(b);
        if ra == rb {
            return;
        }
        if self.rank[ra] < self.rank[rb] {
            self.parent[ra] = rb;
        } else if self.rank[ra] > self.rank[rb] {
            self.parent[rb] = ra;
        } else {
            self.parent[rb] = ra;
            self.rank[ra] += 1;
        }
    }
}

// --- Point key and matching ---

fn point_key(p: &Point) -> (i64, i64) {
    // Round to 0.05mm buckets (multiply by 20, round)
    ((p.x * 20.0).round() as i64, (p.y * 20.0).round() as i64)
}

fn point_on_segment(p: &Point, a: &Point, b: &Point, tol: f64) -> bool {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len = (dx * dx + dy * dy).sqrt();
    if len < tol {
        return (p.x - a.x).abs() < tol && (p.y - a.y).abs() < tol;
    }
    let dist = (dx * (a.y - p.y) - dy * (a.x - p.x)).abs() / len;
    if dist > tol {
        return false;
    }
    let t = ((p.x - a.x) * dx + (p.y - a.y) * dy) / (len * len);
    (-0.01..=1.01).contains(&t)
}

// --- Pin position calculation ---

fn get_symbol_pin_positions(
    sym: &Symbol,
    lib_symbols: &HashMap<String, LibSymbol>,
) -> Vec<(Point, String, String, String)> {
    // Returns (world_position, pin_number, pin_name, pin_type)
    let lib_sym = match lib_symbols.get(&sym.lib_id) {
        Some(ls) => ls,
        None => return vec![],
    };

    let mut result = Vec::new();
    let rot_rad = sym.rotation.to_radians();
    let cos_r = rot_rad.cos();
    let sin_r = rot_rad.sin();

    for pin in &lib_sym.pins {
        // Transform pin position from library space to world space
        let mut px = pin.position.x;
        let mut py = pin.position.y;

        if sym.mirror_x {
            px = -px;
        }
        if sym.mirror_y {
            py = -py;
        }

        // Rotate
        let rx = px * cos_r - py * sin_r;
        let ry = px * sin_r + py * cos_r;

        // Translate
        let wx = sym.position.x + rx;
        let wy = sym.position.y + ry;

        result.push((
            Point { x: wx, y: wy },
            pin.number.clone(),
            pin.name.clone(),
            pin.pin_type.clone(),
        ));
    }

    result
}

// --- Net resolution (ported from netResolver.ts) ---

struct NetInfo {
    name: Option<String>,
    pins: Vec<(String, String, String)>, // (symbol_ref, pin_number, pin_name)
}

fn resolve_nets(sheet: &SchematicSheet) -> Vec<NetInfo> {
    let mut point_index: HashMap<(i64, i64), usize> = HashMap::new();
    let mut points: Vec<Point> = Vec::new();
    let tol = 0.05;

    let get_node = |p: &Point, pi: &mut HashMap<(i64, i64), usize>, pts: &mut Vec<Point>| -> usize {
        let k = point_key(p);
        if let Some(&idx) = pi.get(&k) {
            return idx;
        }
        let idx = pts.len();
        pts.push(p.clone());
        pi.insert(k, idx);
        idx
    };

    // Wire endpoints
    let mut wire_nodes: Vec<(usize, usize)> = Vec::new();
    for wire in &sheet.wires {
        let si = get_node(&wire.start, &mut point_index, &mut points);
        let ei = get_node(&wire.end, &mut point_index, &mut points);
        wire_nodes.push((si, ei));
    }

    // Junction positions
    let mut junction_nodes: Vec<usize> = Vec::new();
    for j in &sheet.junctions {
        let idx = get_node(&j.position, &mut point_index, &mut points);
        junction_nodes.push(idx);
    }

    // Label positions
    struct LabelNode {
        text: String,
        label_type: LabelType,
        idx: usize,
    }
    let mut label_nodes: Vec<LabelNode> = Vec::new();
    for label in &sheet.labels {
        let idx = get_node(&label.position, &mut point_index, &mut points);
        label_nodes.push(LabelNode {
            text: label.text.clone(),
            label_type: label.label_type.clone(),
            idx,
        });
    }

    // Symbol pin positions
    struct PinNode {
        symbol_ref: String,
        pin_number: String,
        pin_name: String,
        idx: usize,
    }
    let mut pin_nodes: Vec<PinNode> = Vec::new();
    for sym in &sheet.symbols {
        if sym.exclude_from_sim || sym.is_power {
            // Power symbols are handled via labels
            if sym.is_power {
                // Power symbols create an implicit global label at their pin position
                let pins = get_symbol_pin_positions(sym, &sheet.lib_symbols);
                for (pos, _num, _name, _ptype) in pins {
                    let idx = get_node(&pos, &mut point_index, &mut points);
                    label_nodes.push(LabelNode {
                        text: sym.value.clone(),
                        label_type: LabelType::Global,
                        idx,
                    });
                }
            }
            if sym.exclude_from_sim {
                continue;
            }
        }
        let pins = get_symbol_pin_positions(sym, &sheet.lib_symbols);
        for (pos, pin_num, pin_name, _ptype) in pins {
            let idx = get_node(&pos, &mut point_index, &mut points);
            pin_nodes.push(PinNode {
                symbol_ref: sym.reference.clone(),
                pin_number: pin_num,
                pin_name,
                idx,
            });
        }
    }

    // Build union-find
    let n = points.len();
    if n == 0 {
        return vec![];
    }
    let mut uf = UnionFind::new(n);

    // Union wire endpoints
    for &(si, ei) in &wire_nodes {
        uf.union(si, ei);
    }

    // Union junctions with wire midpoints
    for &j_idx in &junction_nodes {
        for &(ws, we) in &wire_nodes {
            if uf.find(ws) == uf.find(j_idx) || uf.find(we) == uf.find(j_idx) {
                continue;
            }
            if point_on_segment(&points[j_idx], &points[ws], &points[we], tol) {
                uf.union(j_idx, ws);
            }
        }
    }

    // Group by root
    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..n {
        let root = uf.find(i);
        groups.entry(root).or_default().push(i);
    }

    // Build NetInfo for each group
    let mut nets = Vec::new();
    for (_, members) in &groups {
        let member_set: std::collections::HashSet<usize> = members.iter().copied().collect();

        let mut net = NetInfo {
            name: None,
            pins: Vec::new(),
        };

        // Labels → net name
        for ln in &label_nodes {
            if member_set.contains(&uf.find(ln.idx)) {
                if net.name.is_none() {
                    net.name = Some(ln.text.clone());
                }
            }
        }

        // Pins
        for pn in &pin_nodes {
            if member_set.contains(&uf.find(pn.idx)) {
                net.pins.push((
                    pn.symbol_ref.clone(),
                    pn.pin_number.clone(),
                    pn.pin_name.clone(),
                ));
            }
        }

        if !net.pins.is_empty() {
            nets.push(net);
        }
    }

    nets
}

// --- SPICE element mapping ---

/// Map a KiCad lib_id prefix to a SPICE element letter.
/// Returns None for components that can't be directly mapped (ICs → subcircuit).
fn spice_element_letter(_lib_id: &str, reference: &str) -> char {
    // Use the reference prefix first (most reliable)
    let prefix = reference
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect::<String>()
        .to_uppercase();

    match prefix.as_str() {
        "R" => 'R',
        "C" => 'C',
        "L" => 'L',
        "D" => 'D',
        "Q" => 'Q', // BJT
        "M" => 'M', // MOSFET
        "J" => 'J', // JFET
        "V" => 'V', // Voltage source
        "I" => 'I', // Current source
        "K" => 'K', // Coupled inductors
        "T" => 'T', // Transmission line
        "S" => 'S', // Voltage-controlled switch
        "W" => 'W', // Current-controlled switch
        "E" => 'E', // VCVS
        "F" => 'F', // CCCS
        "G" => 'G', // VCCS
        "H" => 'H', // CCVS
        _ => 'X',    // Subcircuit for everything else (U, IC, etc.)
    }
}

/// Determine pin order for a SPICE element based on element type.
/// SPICE has strict pin ordering: R(1,2), C(1,2), D(A,K), Q(C,B,E), M(D,G,S,B), etc.
fn order_pins(
    element: char,
    pins: &[(String, String)], // (pin_number, net_name)
) -> Vec<String> {
    // For 2-terminal devices, pin order is pin 1, pin 2
    if matches!(element, 'R' | 'C' | 'L') {
        let mut ordered = vec![String::new(); 2];
        for (num, net) in pins {
            match num.as_str() {
                "1" => ordered[0] = net.clone(),
                "2" => ordered[1] = net.clone(),
                _ => {
                    // Fallback: just use the order given
                    return pins.iter().map(|(_, n)| n.clone()).collect();
                }
            }
        }
        return ordered;
    }

    // For diodes: Anode (A/1), Cathode (K/2)
    if element == 'D' {
        let mut ordered = vec![String::new(); 2];
        for (num, net) in pins {
            match num.as_str() {
                "1" | "A" => ordered[0] = net.clone(),
                "2" | "K" => ordered[1] = net.clone(),
                _ => {}
            }
        }
        return ordered;
    }

    // For BJTs: Collector, Base, Emitter
    if element == 'Q' {
        let mut ordered = vec![String::new(); 3];
        for (num, net) in pins {
            match num.as_str() {
                "1" | "C" => ordered[0] = net.clone(),
                "2" | "B" => ordered[1] = net.clone(),
                "3" | "E" => ordered[2] = net.clone(),
                _ => {}
            }
        }
        return ordered;
    }

    // For MOSFETs: Drain, Gate, Source, Bulk
    if element == 'M' {
        let mut ordered = vec![String::new(); 4];
        for (num, net) in pins {
            match num.as_str() {
                "1" | "D" => ordered[0] = net.clone(),
                "2" | "G" => ordered[1] = net.clone(),
                "3" | "S" => ordered[2] = net.clone(),
                "4" | "B" => ordered[3] = net.clone(),
                _ => {}
            }
        }
        // If bulk not connected, default to source
        if ordered[3].is_empty() && !ordered[2].is_empty() {
            ordered[3] = ordered[2].clone();
        }
        return ordered;
    }

    // Default: return in pin number order
    let mut sorted = pins.to_vec();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    sorted.into_iter().map(|(_, n)| n).collect()
}

// --- Main entry point ---

pub fn generate_spice_netlist(
    sheet: &SchematicSheet,
    config: &AnalysisConfig,
) -> Result<String, String> {
    let nets = resolve_nets(sheet);

    // Assign net numbers. Ground net (GND, 0, VSS) gets number 0.
    let mut net_numbers: HashMap<String, u32> = HashMap::new();
    let mut next_net = 1u32;
    net_numbers.insert("GND".to_string(), 0);
    net_numbers.insert("0".to_string(), 0);
    net_numbers.insert("gnd".to_string(), 0);
    net_numbers.insert("VSS".to_string(), 0);

    // Build pin-to-net mapping: (symbol_ref, pin_number) → net_name
    let mut pin_net: HashMap<(String, String), String> = HashMap::new();

    for (idx, net) in nets.iter().enumerate() {
        let net_name = net.name.clone().unwrap_or_else(|| format!("net_{}", idx));

        // Ensure net has a number
        if !net_numbers.contains_key(&net_name) {
            net_numbers.insert(net_name.clone(), next_net);
            next_net += 1;
        }

        for (sym_ref, pin_num, _pin_name) in &net.pins {
            pin_net.insert((sym_ref.clone(), pin_num.clone()), net_name.clone());
        }
    }

    // Generate SPICE netlist
    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("* Signex SPICE netlist"));
    lines.push(format!(
        "* Generated from: {}",
        sheet.title_block.get("Title").unwrap_or(&"Untitled".to_string())
    ));
    lines.push(String::new());

    // Components
    for sym in &sheet.symbols {
        if sym.exclude_from_sim || sym.is_power || sym.reference.starts_with('#') {
            continue;
        }

        let element = spice_element_letter(&sym.lib_id, &sym.reference);

        // Collect this symbol's pin→net connections
        let mut sym_pins: Vec<(String, String)> = Vec::new();
        let lib_sym = sheet.lib_symbols.get(&sym.lib_id);
        let _pin_count = lib_sym.map(|ls| ls.pins.len()).unwrap_or(0);

        if let Some(ls) = lib_sym {
            for pin in &ls.pins {
                let net_name = pin_net
                    .get(&(sym.reference.clone(), pin.number.clone()))
                    .cloned()
                    .unwrap_or_else(|| "NC".to_string());
                sym_pins.push((pin.number.clone(), net_name));
            }
        }

        // Order pins for SPICE
        let ordered_nets = order_pins(element, &sym_pins);
        let net_nodes: Vec<String> = ordered_nets
            .iter()
            .map(|n| {
                let num = net_numbers.get(n).copied().unwrap_or_else(|| {
                    let nn = next_net;
                    net_numbers.insert(n.clone(), nn);
                    next_net += 1;
                    nn
                });
                num.to_string()
            })
            .collect();

        // Build SPICE line
        let nodes_str = net_nodes.join(" ");
        let value = &sym.value;

        match element {
            'R' | 'C' | 'L' => {
                // Simple 2-terminal: R1 n1 n2 value
                lines.push(format!("{}{} {} {}", element, sym.reference.trim_start_matches(|c: char| c.is_alphabetic()), nodes_str, value));
            }
            'D' => {
                // Diode: D1 n+ n- modelname
                let model = if value.is_empty() { "D" } else { value };
                lines.push(format!("D{} {} {}", sym.reference.trim_start_matches(|c: char| c.is_alphabetic()), nodes_str, model));
            }
            'Q' => {
                let model = if value.is_empty() { "NPN" } else { value };
                lines.push(format!("Q{} {} {}", sym.reference.trim_start_matches(|c: char| c.is_alphabetic()), nodes_str, model));
            }
            'M' => {
                let model = if value.is_empty() { "NMOS" } else { value };
                lines.push(format!("M{} {} {}", sym.reference.trim_start_matches(|c: char| c.is_alphabetic()), nodes_str, model));
            }
            'V' | 'I' => {
                // Source: V1 n+ n- value
                lines.push(format!("{}{} {} {}", element, sym.reference.trim_start_matches(|c: char| c.is_alphabetic()), nodes_str, value));
            }
            'X' => {
                // Subcircuit: X1 n1 n2 ... subckt_name
                let subckt_name = sym.lib_id.split(':').last().unwrap_or(&sym.lib_id);
                lines.push(format!("X{} {} {}", sym.reference.trim_start_matches(|c: char| c.is_alphabetic()), nodes_str, subckt_name));
            }
            _ => {
                // Generic: use reference as-is
                lines.push(format!("{} {} {}", sym.reference, nodes_str, value));
            }
        }
    }

    lines.push(String::new());

    // Analysis card
    lines.push(generate_analysis_card(config));

    lines.push(String::new());
    lines.push(".end".to_string());

    Ok(lines.join("\n"))
}

fn generate_analysis_card(config: &AnalysisConfig) -> String {
    let p = &config.params;
    match config.analysis_type {
        AnalysisType::DcOp => ".op".to_string(),
        AnalysisType::DcSweep => {
            let src = p.get("source").map(|s| s.as_str()).unwrap_or("V1");
            let start = p.get("start").map(|s| s.as_str()).unwrap_or("0");
            let stop = p.get("stop").map(|s| s.as_str()).unwrap_or("5");
            let step = p.get("step").map(|s| s.as_str()).unwrap_or("0.1");
            format!(".dc {} {} {} {}", src, start, stop, step)
        }
        AnalysisType::Ac => {
            let variation = p.get("variation").map(|s| s.as_str()).unwrap_or("dec");
            let points = p.get("points").map(|s| s.as_str()).unwrap_or("100");
            let fstart = p.get("fstart").map(|s| s.as_str()).unwrap_or("1");
            let fstop = p.get("fstop").map(|s| s.as_str()).unwrap_or("1G");
            format!(".ac {} {} {} {}", variation, points, fstart, fstop)
        }
        AnalysisType::Transient => {
            let tstep = p.get("tstep").map(|s| s.as_str()).unwrap_or("1u");
            let tstop = p.get("tstop").map(|s| s.as_str()).unwrap_or("10m");
            let tstart = p.get("tstart").map(|s| s.as_str()).unwrap_or("");
            if tstart.is_empty() {
                format!(".tran {} {}", tstep, tstop)
            } else {
                format!(".tran {} {} {}", tstep, tstop, tstart)
            }
        }
        AnalysisType::Noise => {
            let output = p.get("output").map(|s| s.as_str()).unwrap_or("V(out)");
            let src = p.get("source").map(|s| s.as_str()).unwrap_or("V1");
            let variation = p.get("variation").map(|s| s.as_str()).unwrap_or("dec");
            let points = p.get("points").map(|s| s.as_str()).unwrap_or("100");
            let fstart = p.get("fstart").map(|s| s.as_str()).unwrap_or("1");
            let fstop = p.get("fstop").map(|s| s.as_str()).unwrap_or("1G");
            format!(
                ".noise {} {} {} {} {} {}",
                output, src, variation, points, fstart, fstop
            )
        }
    }
}
