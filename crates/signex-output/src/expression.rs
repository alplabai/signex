use std::collections::HashMap;

use signex_types::markup::standard_auto_net_name_from_pins;
use signex_types::schematic::{LabelType, Point, Symbol, SymbolInstance};

use crate::SheetSnapshot;

#[derive(Debug, Clone, Default)]
pub struct ExpressionTables {
    pub global_refdes: HashMap<String, String>,
    pub net_name_by_symbol_pin: HashMap<String, HashMap<String, String>>,
}

pub fn build_expression_tables(sheets: &[SheetSnapshot]) -> ExpressionTables {
    ExpressionTables {
        global_refdes: build_global_refdes_lookup(sheets),
        net_name_by_symbol_pin: build_pin_net_lookup(sheets),
    }
}

pub fn sheet_cell_value(sheet: &SheetSnapshot) -> String {
    let page = sheet.schematic.root_sheet_page.trim();
    if page.is_empty() {
        sheet.sheet_number.to_string()
    } else {
        page.to_string()
    }
}

fn build_global_refdes_lookup(sheets: &[SheetSnapshot]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for sheet in sheets {
        for sym in &sheet.schematic.symbols {
            if sym.reference.is_empty() {
                continue;
            }

            out.entry(sym.uuid.to_string())
                .or_insert_with(|| sym.reference.clone());
            out.entry(sym.reference.clone())
                .or_insert_with(|| sym.reference.clone());

            for instance in &sym.instances {
                insert_instance_keys(&mut out, instance, &sym.reference);
            }
        }
    }
    out
}

fn insert_instance_keys(
    out: &mut HashMap<String, String>,
    instance: &SymbolInstance,
    reference: &str,
) {
    if instance.path.is_empty() {
        return;
    }
    out.entry(instance.path.clone())
        .or_insert_with(|| reference.to_string());
    let trimmed = instance.path.trim_matches('/');
    if !trimmed.is_empty() {
        out.entry(trimmed.to_string())
            .or_insert_with(|| reference.to_string());
    }
}

fn build_pin_net_lookup(sheets: &[SheetSnapshot]) -> HashMap<String, HashMap<String, String>> {
    type Node = (usize, i64, i64);

    fn q(p: Point) -> (i64, i64) {
        ((p.x * 100.0).round() as i64, (p.y * 100.0).round() as i64)
    }

    fn find(parent: &mut HashMap<Node, Node>, x: Node) -> Node {
        let p = *parent.entry(x).or_insert(x);
        if p == x {
            x
        } else {
            let r = find(parent, p);
            parent.insert(x, r);
            r
        }
    }

    fn union(parent: &mut HashMap<Node, Node>, a: Node, b: Node) {
        let ra = find(parent, a);
        let rb = find(parent, b);
        if ra != rb {
            parent.insert(ra, rb);
        }
    }

    fn point_on_segment(p: Point, a: Point, b: Point, tol: f64) -> bool {
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let len_sq = dx * dx + dy * dy;

        if len_sq < tol * tol {
            return (p.x - a.x).abs() < tol && (p.y - a.y).abs() < tol;
        }

        let t = ((p.x - a.x) * dx + (p.y - a.y) * dy) / len_sq;
        if !(0.0..=1.0).contains(&t) {
            return false;
        }

        let proj_x = a.x + t * dx;
        let proj_y = a.y + t * dy;
        (p.x - proj_x).abs() < tol && (p.y - proj_y).abs() < tol
    }

    fn transform_pin_position(sym: &Symbol, local_pos: &Point) -> Point {
        let x = local_pos.x;
        let y = -local_pos.y;

        let rad = -sym.rotation.to_radians();
        let cos = rad.cos();
        let sin = rad.sin();
        let rx = x * cos - y * sin;
        let ry = x * sin + y * cos;

        let rx = if sym.mirror_y { -rx } else { rx };
        let ry = if sym.mirror_x { -ry } else { ry };

        Point::new(rx + sym.position.x, ry + sym.position.y)
    }

    fn label_priority(kind: LabelType) -> u8 {
        match kind {
            LabelType::Global => 4,
            LabelType::Power => 3,
            LabelType::Hierarchical => 2,
            LabelType::Net => 1,
        }
    }

    #[derive(Clone)]
    struct LabelBinding {
        root: Node,
        text: String,
        priority: u8,
        unify_text: Option<String>,
    }

    let mut parent: HashMap<Node, Node> = HashMap::new();
    let mut labels: Vec<LabelBinding> = Vec::new();
    let mut global_label_root: HashMap<String, Node> = HashMap::new();
    let tolerance = 0.01;

    for (sheet_idx, snap) in sheets.iter().enumerate() {
        for wire in &snap.schematic.wires {
            let a = (sheet_idx, q(wire.start).0, q(wire.start).1);
            let b = (sheet_idx, q(wire.end).0, q(wire.end).1);
            union(&mut parent, a, b);
        }

        for junction in &snap.schematic.junctions {
            let j = (sheet_idx, q(junction.position).0, q(junction.position).1);
            parent.entry(j).or_insert(j);

            for wire in &snap.schematic.wires {
                if wire.start == junction.position
                    || wire.end == junction.position
                    || point_on_segment(junction.position, wire.start, wire.end, tolerance)
                {
                    let a = (sheet_idx, q(wire.start).0, q(wire.start).1);
                    let b = (sheet_idx, q(wire.end).0, q(wire.end).1);
                    union(&mut parent, j, a);
                    union(&mut parent, j, b);
                }
            }
        }

        for label in &snap.schematic.labels {
            let mut node = (sheet_idx, q(label.position).0, q(label.position).1);
            parent.entry(node).or_insert(node);

            let mut anchored = false;
            for wire in &snap.schematic.wires {
                if point_on_segment(label.position, wire.start, wire.end, tolerance) {
                    let a = (sheet_idx, q(wire.start).0, q(wire.start).1);
                    let b = (sheet_idx, q(wire.end).0, q(wire.end).1);
                    union(&mut parent, a, b);
                    union(&mut parent, node, a);
                    node = a;
                    anchored = true;
                    break;
                }
            }

            if !anchored {
                parent.entry(node).or_insert(node);
            }

            let root = find(&mut parent, node);
            let unify_text = if matches!(label.label_type, LabelType::Global | LabelType::Hierarchical)
                && !label.text.is_empty()
            {
                Some(label.text.clone())
            } else {
                None
            };

            labels.push(LabelBinding {
                root,
                text: label.text.clone(),
                priority: label_priority(label.label_type),
                unify_text,
            });
        }
    }

    for binding in &labels {
        if let Some(name) = &binding.unify_text {
            let root = find(&mut parent, binding.root);
            if let Some(existing) = global_label_root.get(name).copied() {
                union(&mut parent, root, existing);
            } else {
                global_label_root.insert(name.clone(), root);
            }
        }
    }

    let mut root_name: HashMap<Node, (u8, String)> = HashMap::new();
    for binding in labels {
        if binding.text.is_empty() {
            continue;
        }
        let root = find(&mut parent, binding.root);
        match root_name.get(&root) {
            Some((p, _)) if *p >= binding.priority => {}
            _ => {
                root_name.insert(root, (binding.priority, binding.text));
            }
        }
    }

    let mut root_pins: HashMap<Node, Vec<(String, String)>> = HashMap::new();
    let mut pin_entries: Vec<(String, String, Node)> = Vec::new();

    for (sheet_idx, snap) in sheets.iter().enumerate() {
        for sym in &snap.schematic.symbols {
            let Some(lib_sym) = snap.schematic.lib_symbols.get(&sym.lib_id) else {
                continue;
            };

            for lib_pin in &lib_sym.pins {
                if !(lib_pin.unit == 0 || lib_pin.unit == sym.unit) {
                    continue;
                }
                let world = transform_pin_position(sym, &lib_pin.pin.position);
                let node = (sheet_idx, q(world).0, q(world).1);
                let root = find(&mut parent, node);

                root_pins
                    .entry(root)
                    .or_default()
                    .push((sym.reference.clone(), lib_pin.pin.number.clone()));
                pin_entries.push((sym.uuid.to_string(), lib_pin.pin.number.clone(), root));
            }
        }
    }

    let mut resolved_root_name: HashMap<Node, String> = HashMap::new();
    for root in root_pins.keys().copied() {
        let named = root_name.get(&root).map(|(_, n)| n.clone()).unwrap_or_default();
        if !named.is_empty() {
            resolved_root_name.insert(root, named);
            continue;
        }
        let auto = root_pins
            .get(&root)
            .and_then(|pins| standard_auto_net_name_from_pins(pins))
            .unwrap_or_default();
        resolved_root_name.insert(root, auto);
    }

    let mut result: HashMap<String, HashMap<String, String>> = HashMap::new();
    for (sym_uuid, pin_number, root) in pin_entries {
        let net_name = resolved_root_name.get(&root).cloned().unwrap_or_default();
        if !net_name.is_empty() {
            result
                .entry(sym_uuid)
                .or_default()
                .insert(pin_number, net_name);
        }
    }

    result
}
