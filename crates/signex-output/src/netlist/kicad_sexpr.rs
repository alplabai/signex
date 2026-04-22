//! Standard `.net` S-expression emitter.
//!
//! Uses the AST builder from `standard-parser`'s `sexpr_builder` module +
//! `standard-writer`'s `sexpr_render` — see
//! `reference_standard_sexpr_ast_pipeline` memory note for the convention.

use std::collections::{HashMap, BTreeMap};

use standard_parser::sexpr_builder::{atom, list, raw};
use standard_parser::sexpr::SExpr;
use signex_types::schematic::{Point, SchematicSheet, Symbol};

// Union-find for net connectivity
#[derive(Clone)]
struct NetNode {
    parent: usize,
    name: String,
}

pub struct NetGraph {
    pub nodes: Vec<NetNode>,
    pub net_names: HashMap<usize, String>, // root index -> net name
    pub node_to_pins: HashMap<usize, Vec<(String, String, String)>>, // root -> (ref, pin_number, pin_type)
}

impl NetGraph {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            net_names: HashMap::new(),
            node_to_pins: HashMap::new(),
        }
    }

    /// Add a new node (pin endpoint) to the graph.
    fn add_node(&mut self) -> usize {
        let idx = self.nodes.len();
        self.nodes.push(NetNode {
            parent: idx,
            name: String::new(),
        });
        idx
    }

    /// Union two nodes via union-find.
    fn union(&mut self, a: usize, b: usize) {
        let root_a = self.find_root(a);
        let root_b = self.find_root(b);
        if root_a != root_b {
            self.nodes[root_a].parent = root_b;
        }
    }

    /// Find the root of a node.
    fn find_root(&mut self, mut idx: usize) -> usize {
        while self.nodes[idx].parent != idx {
            let parent = self.nodes[idx].parent;
            idx = parent;
        }
        idx
    }

    /// Set the name for a net (at its root).
    fn set_net_name(&mut self, idx: usize, name: String) {
        let root = self.find_root(idx);
        if name.len() > 0 {
            self.net_names.insert(root, name);
        }
    }

    /// Add a pin to the net.
    fn add_pin(&mut self, node_idx: usize, ref_des: String, pin_number: String, pin_type: String) {
        let root = self.find_root(node_idx);
        self.node_to_pins
            .entry(root)
            .or_insert_with(Vec::new)
            .push((ref_des, pin_number, pin_type));
    }
}

/// Build nets from schematic sheet: wires, junctions, labels.
pub fn build_net_graph(sheet: &SchematicSheet, symbols: &[Symbol]) -> NetGraph {
    let mut graph = NetGraph::new();

    // Map positions to node indices (within a tolerance for junctions/labels)
    let mut pos_to_node: HashMap<String, usize> = HashMap::new();
    let tolerance = 0.01; // mm

    let pos_key = |p: Point| format!("{:.2}_{:.2}", p.x, p.y);

    // Create nodes for each symbol pin
    let mut pin_positions: BTreeMap<String, (String, String, String)> = BTreeMap::new();
    for sym in symbols {
        // Skip power ports
        if sym.reference.starts_with("#PWR") {
            continue;
        }

        // Find the library symbol to get pins
        if let Some(lib_sym) = sheet.lib_symbols.get(&sym.lib_id) {
            for lib_pin in &lib_sym.pins {
                // For now, assume unit 1 (no multi-unit handling yet)
                if lib_pin.unit == 0 || lib_pin.unit == sym.unit {
                    let global_pos = sym.position;
                    let key = pos_key(global_pos);
                    let pin_type = format!("{:?}", lib_pin.pin.pin_type).to_lowercase();
                    pin_positions.insert(
                        key.clone(),
                        (
                            sym.reference.clone(),
                            lib_pin.pin.number.clone(),
                            pin_type,
                        ),
                    );
                    if !pos_to_node.contains_key(&key) {
                        pos_to_node.insert(key, graph.add_node());
                    }
                }
            }
        }
    }

    // Process wires: connect endpoints
    for wire in &sheet.wires {
        let start_key = pos_key(wire.start);
        let end_key = pos_key(wire.end);

        let start_idx = *pos_to_node
            .entry(start_key)
            .or_insert_with(|| graph.add_node());
        let end_idx = *pos_to_node
            .entry(end_key)
            .or_insert_with(|| graph.add_node());

        graph.union(start_idx, end_idx);
    }

    // Process junctions: merge wires at the junction
    for junction in &sheet.junctions {
        let j_key = pos_key(junction.position);
        let j_idx = *pos_to_node
            .entry(j_key)
            .or_insert_with(|| graph.add_node());

        // Find all wires that pass through this junction and merge them
        for wire in &sheet.wires {
            if (wire.start == junction.position || wire.end == junction.position)
                || point_on_segment(junction.position, wire.start, wire.end, tolerance)
            {
                let start_key = pos_key(wire.start);
                let end_key = pos_key(wire.end);
                let start_idx = *pos_to_node
                    .entry(start_key)
                    .or_insert_with(|| graph.add_node());
                let end_idx = *pos_to_node
                    .entry(end_key)
                    .or_insert_with(|| graph.add_node());
                graph.union(j_idx, start_idx);
                graph.union(j_idx, end_idx);
            }
        }
    }

    // Process labels: assign net names
    for label in &sheet.labels {
        let label_key = pos_key(label.position);
        if let Some(&label_idx) = pos_to_node.get(&label_key) {
            graph.set_net_name(label_idx, label.text.clone());
        }
    }

    // Add pins to nets
    for (key, (ref_des, pin_number, pin_type)) in pin_positions {
        if let Some(&node_idx) = pos_to_node.get(&key) {
            graph.add_pin(node_idx, ref_des, pin_number, pin_type);
        }
    }

    graph
}

/// Check if point p lies on the segment from a to b (within tolerance).
fn point_on_segment(p: Point, a: Point, b: Point, tol: f64) -> bool {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len_sq = dx * dx + dy * dy;

    if len_sq < tol * tol {
        return (p.x - a.x).abs() < tol && (p.y - a.y).abs() < tol;
    }

    let t = ((p.x - a.x) * dx + (p.y - a.y) * dy) / len_sq;
    if t < 0.0 || t > 1.0 {
        return false;
    }

    let proj_x = a.x + t * dx;
    let proj_y = a.y + t * dy;
    (p.x - proj_x).abs() < tol && (p.y - proj_y).abs() < tol
}

/// Emit a comp (component) node for a symbol.
pub fn emit_comp(
    sym: &Symbol,
    lib_sym: &signex_types::schematic::LibSymbol,
    sheet_path: &str,
    sheet_tstamp: &str,
    include_tstamp: bool,
) -> SExpr {
    let mut items = vec![raw("comp")];
    items.push(list(vec![raw("ref"), atom(&sym.reference)]));
    items.push(list(vec![raw("value"), atom(&sym.value)]));

    if !sym.footprint.is_empty() {
        items.push(list(vec![raw("footprint"), atom(&sym.footprint)]));
    }

    // Fields
    let mut fields = Vec::new();
    for (name, value) in &sym.fields {
        fields.push(list(vec![raw("field"), atom(name), atom(value)]));
    }
    if !fields.is_empty() {
        let mut field_items = vec![raw("fields")];
        field_items.extend(fields);
        items.push(list(field_items));
    }

    // Libsource
    let libsource_parts = sym.lib_id.split('/').collect::<Vec<_>>();
    let lib_name = libsource_parts.first().copied().unwrap_or("Device");
    let part_name = libsource_parts.last().copied().unwrap_or(&sym.lib_id);
    items.push(list(vec![
        raw("libsource"),
        list(vec![raw("lib"), atom(lib_name)]),
        list(vec![raw("part"), atom(part_name)]),
        list(vec![
            raw("description"),
            atom(if lib_sym.description.is_empty() {
                &sym.value
            } else {
                &lib_sym.description
            }),
        ]),
    ]));

    // Sheetpath
    items.push(list(vec![
        raw("sheetpath"),
        list(vec![raw("names"), atom(sheet_path)]),
        list(vec![raw("tstamps"), atom(sheet_tstamp)]),
    ]));

    // Timestamp
    if include_tstamp {
        items.push(list(vec![raw("tstamp"), atom(sym.uuid.to_string())]));
    }

    list(items)
}

/// Emit a net node with code, name, and pins.
pub fn emit_net(
    code: u32,
    name: &str,
    pins: &[(String, String, String)],
) -> SExpr {
    let mut items = vec![raw("net")];
    items.push(list(vec![raw("code"), atom(code)]));
    items.push(list(vec![raw("name"), atom(name)]));

    for (ref_des, pin_num, pin_type) in pins {
        items.push(list(vec![
            raw("node"),
            list(vec![raw("ref"), atom(ref_des)]),
            list(vec![raw("pin"), atom(pin_num)]),
            list(vec![raw("pintype"), atom(pin_type)]),
        ]));
    }

    list(items)
}

/// Emit the root (export ...) node.
pub fn emit_header(
    source: &str,
    timestamp: &str,
    tool_version: &str,
) -> SExpr {
    list(vec![
        raw("export"),
        list(vec![raw("version"), raw("D")]),
        list(vec![
            raw("design"),
            list(vec![raw("source"), atom(source)]),
            list(vec![raw("date"), atom(timestamp)]),
            list(vec![raw("tool"), atom(tool_version)]),
        ]),
    ])
}
