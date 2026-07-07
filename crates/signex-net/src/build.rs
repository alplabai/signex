//! Netlist construction — derive the authoritative
//! [`Netlist`](signex_types::net::Netlist) from a parsed [`SchematicSheet`].
//!
//! The geometry mirrors the ERC context's `derive_nets` exactly (union-find
//! over wire endpoints, junction T-merges, world-space pin projection, 1 µm
//! coordinate bucketing) so both agree on net membership. On top of that it
//! records the concrete [`Terminal`]s — reference designator + pin id — that
//! ERC never needed but the ratsnest / PCB net assignment / netlist exporter
//! all do.
//!
//! Scope (ADR-0001 A3.1, increment 1): single sheet only — no hierarchy;
//! net names come from the highest-priority label on the net, matching the
//! current ERC semantics. Same-name label *merging* and cross-sheet
//! stitching are deferred to increment 2.

use std::collections::HashMap;

use signex_types::net::{Net, NetId, Netlist, Terminal};
use signex_types::schematic::{Label, LabelType, Point, SchematicSheet, Symbol};

use crate::uf::{find as uf_find, union as uf_union, Key};

/// Projects a library symbol's local pin coordinates into world space,
/// applying the placed instance's rotation and mirror. Kept in step with
/// the ERC context's `SymbolTransform` so net membership is identical.
#[derive(Debug, Clone, Copy)]
struct SymbolTransform {
    origin: Point,
    rotation_deg: f64,
    mirror_x: bool,
    mirror_y: bool,
}

impl SymbolTransform {
    fn from_symbol(symbol: &Symbol) -> Self {
        Self {
            origin: symbol.position,
            rotation_deg: symbol.rotation,
            mirror_x: symbol.mirror_x,
            mirror_y: symbol.mirror_y,
        }
    }

    fn apply(&self, local: Point) -> Point {
        let x = local.x;
        let y = -local.y;
        let rad = -self.rotation_deg.to_radians();
        let cos = rad.cos();
        let sin = rad.sin();
        let mut rx = x * cos - y * sin;
        let mut ry = x * sin + y * cos;
        if self.mirror_y {
            rx = -rx;
        }
        if self.mirror_x {
            ry = -ry;
        }
        Point::new(rx + self.origin.x, ry + self.origin.y)
    }
}

const EPS: f64 = 1e-4;

fn pt_same(a: &Point, b: &Point) -> bool {
    (a.x - b.x).abs() < EPS && (a.y - b.y).abs() < EPS
}

/// 1 µm integer bucket — the union-find key space. Matches ERC's `pt_key`
/// so both derivations agree on which points are "the same".
fn pt_key(p: &Point) -> Key {
    ((p.x * 1000.0).round() as i64, (p.y * 1000.0).round() as i64)
}

/// True when `p` lies on segment `a`–`b` (endpoints included) in the integer
/// key space. A zero cross-product (computed in `i128` so large micron
/// coordinates can't overflow) plus a bounding-box containment check.
fn point_on_segment(p: Key, a: Key, b: Key) -> bool {
    let cross =
        (b.0 - a.0) as i128 * (p.1 - a.1) as i128 - (b.1 - a.1) as i128 * (p.0 - a.0) as i128;
    if cross != 0 {
        return false;
    }
    let within_x = p.0 >= a.0.min(b.0) && p.0 <= a.0.max(b.0);
    let within_y = p.1 >= a.1.min(b.1) && p.1 <= a.1.max(b.1);
    within_x && within_y
}

/// True when a wire/bus endpoint, label, or no-connect marker sits at `pos`.
/// Mirrors ERC's `point_is_connected`; a pin is only a terminal of a net if
/// something actually lands on its world-space tip.
fn point_is_connected(pos: &Point, sheet: &SchematicSheet) -> bool {
    sheet
        .wires
        .iter()
        .any(|w| pt_same(&w.start, pos) || pt_same(&w.end, pos))
        || sheet
            .buses
            .iter()
            .any(|b| pt_same(&b.start, pos) || pt_same(&b.end, pos))
        || sheet.labels.iter().any(|l| pt_same(&l.position, pos))
        || sheet.no_connects.iter().any(|nc| pt_same(&nc.position, pos))
}

/// Highest-priority label name for a net: `Global > Power > Hierarchical >
/// Net`, ignoring empty text. `None` when the net carries no named label.
fn best_label_name(labels: &[&Label]) -> Option<String> {
    labels
        .iter()
        .filter(|l| !l.text.is_empty())
        .max_by_key(|l| match l.label_type {
            LabelType::Global => 3u8,
            LabelType::Power => 2,
            LabelType::Hierarchical => 1,
            LabelType::Net => 0,
        })
        .map(|l| l.text.clone())
}

/// Net class = the first word of the name before `_`, lowercased (`"i2c"`
/// from `"I2C_SDA"`); the whole lowercased name when there is no `_`. Empty
/// for auto-named nets, which carry no class intent.
fn class_from_name(name: Option<&str>) -> String {
    match name {
        Some(n) => n
            .find('_')
            .map(|i| n[..i].to_ascii_lowercase())
            .unwrap_or_else(|| n.to_ascii_lowercase()),
        None => String::new(),
    }
}

/// Build the authoritative [`Netlist`] for a single schematic sheet.
///
/// Connectivity is union-find over wire endpoints, with junctions merging
/// wires that meet (including a wire terminating on another's interior — a
/// T-junction, issue #107). Component pins are projected to world space and
/// attached as [`Terminal`]s to the net their tip lands on. Output is
/// deterministic: nets are numbered `1..=N` in sorted-root order and each
/// net's terminals are sorted by `(reference, pin)`.
pub fn build_netlist(sheet: &SchematicSheet) -> Netlist {
    let mut parent: HashMap<Key, Key> = HashMap::new();

    // Wires union their two endpoints.
    for w in &sheet.wires {
        uf_union(&mut parent, pt_key(&w.start), pt_key(&w.end));
    }

    // Junctions merge every wire whose segment passes through the dot —
    // including a wire that ends on another wire's interior (T-junction).
    // Union-find over endpoints alone never merges that case, so the
    // junction is what asserts the connection. Regression: issue #107.
    for j in &sheet.junctions {
        let jk = pt_key(&j.position);
        for w in &sheet.wires {
            if point_on_segment(jk, pt_key(&w.start), pt_key(&w.end)) {
                uf_union(&mut parent, jk, pt_key(&w.start));
            }
        }
    }

    // Group labels by net root — the highest-priority one names the net.
    let mut net_labels: HashMap<Key, Vec<&Label>> = HashMap::new();
    for lbl in &sheet.labels {
        let root = uf_find(&mut parent, pt_key(&lbl.position));
        net_labels.entry(root).or_default().push(lbl);
    }

    // Group connected pins into terminals by net root. A pin only counts if
    // something actually lands on its tip (wire/bus/label/no-connect).
    let mut net_terms: HashMap<Key, Vec<Terminal>> = HashMap::new();
    for sym in &sheet.symbols {
        let Some(lib_sym) = sheet.lib_symbols.get(&sym.lib_id) else {
            continue;
        };
        let xform = SymbolTransform::from_symbol(sym);
        for lp in &lib_sym.pins {
            // Unit 0 = "common to all units"; otherwise the placed unit only.
            if lp.unit != 0 && lp.unit != sym.unit {
                continue;
            }
            let world_pos = xform.apply(lp.pin.position);
            if !point_is_connected(&world_pos, sheet) {
                continue;
            }
            // Pin id: the number when present (KiCad convention), else name.
            let pin = if !lp.pin.number.is_empty() {
                lp.pin.number.clone()
            } else {
                lp.pin.name.clone()
            };
            let root = uf_find(&mut parent, pt_key(&world_pos));
            net_terms.entry(root).or_default().push(Terminal {
                reference: sym.reference.clone(),
                pin,
            });
        }
    }

    // A net exists wherever at least one terminal lands. A label with no pins
    // is a dangling label — it carries no connectivity, so it forms no net.
    let mut roots: Vec<Key> = net_terms.keys().copied().collect();
    roots.sort_unstable();

    let nets = roots
        .into_iter()
        .enumerate()
        .map(|(idx, root)| {
            let id = NetId(idx as u32 + 1);
            let label_name = net_labels
                .get(&root)
                .and_then(|lbls| best_label_name(lbls));
            let class = class_from_name(label_name.as_deref());
            let name = label_name.unwrap_or_else(|| format!("N${}", id.0));

            let mut terminals = net_terms.remove(&root).unwrap_or_default();
            terminals.sort_by(|a, b| a.reference.cmp(&b.reference).then(a.pin.cmp(&b.pin)));

            Net {
                id,
                name,
                class,
                terminals,
            }
        })
        .collect();

    Netlist { nets }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::{
        Junction, Label, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Wire,
    };
    use std::collections::HashMap;
    use uuid::Uuid;

    fn pt(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn empty_sheet() -> SchematicSheet {
        SchematicSheet {
            uuid: Uuid::nil(),
            version: 0,
            generator: String::new(),
            generator_version: String::new(),
            paper_size: "A4".to_string(),
            root_sheet_page: "1".to_string(),
            symbols: Vec::new(),
            wires: Vec::new(),
            junctions: Vec::new(),
            labels: Vec::new(),
            child_sheets: Vec::new(),
            no_connects: Vec::new(),
            text_notes: Vec::new(),
            buses: Vec::new(),
            bus_entries: Vec::new(),
            drawings: Vec::new(),
            no_erc_directives: Vec::new(),
            title_block: HashMap::new(),
            lib_symbols: HashMap::new(),
        }
    }

    fn wire(a: Point, b: Point) -> Wire {
        Wire {
            uuid: Uuid::nil(),
            start: a,
            end: b,
            stroke_width: 0.0,
        }
    }

    fn junction(pos: Point) -> Junction {
        Junction {
            uuid: Uuid::nil(),
            position: pos,
            diameter: 0.0,
        }
    }

    fn label(text: &str, pos: Point, ty: LabelType) -> Label {
        Label {
            uuid: Uuid::nil(),
            text: text.to_string(),
            position: pos,
            rotation: 0.0,
            label_type: ty,
            shape: String::new(),
            font_size: 1.27,
            justify: signex_types::schematic::HAlign::Left,
            justify_v: signex_types::schematic::VAlign::Bottom,
        }
    }

    fn lib_pin(number: &str, local: Point, dir: PinDirection) -> LibPin {
        LibPin {
            unit: 0,
            body_style: 1,
            pin: Pin {
                direction: dir,
                shape_style: PinShapeStyle::Plain,
                position: local,
                rotation: 0.0,
                length: 0.0,
                name: String::new(),
                number: number.to_string(),
                visible: true,
                name_visible: true,
                number_visible: true,
            },
        }
    }

    /// Register a library symbol keyed by `lib_id`, whose pins are given in
    /// **local** coordinates.
    fn add_lib(sheet: &mut SchematicSheet, lib_id: &str, pins: Vec<LibPin>) {
        sheet.lib_symbols.insert(
            lib_id.to_string(),
            LibSymbol {
                id: lib_id.to_string(),
                reference: String::new(),
                value: String::new(),
                footprint: String::new(),
                datasheet: String::new(),
                description: String::new(),
                keywords: String::new(),
                fp_filters: String::new(),
                in_bom: true,
                on_board: true,
                in_pos_files: true,
                duplicate_pin_numbers_are_jumpers: false,
                graphics: Vec::new(),
                pins,
                show_pin_numbers: true,
                show_pin_names: true,
                pin_name_offset: 0.0,
            },
        );
    }

    /// Place a symbol instance of `lib_id` at world `origin` (no rotation /
    /// mirror). With that transform a local pin `(lx, ly)` lands at world
    /// `(origin.x + lx, origin.y - ly)`.
    fn place(sheet: &mut SchematicSheet, reference: &str, lib_id: &str, origin: Point) {
        sheet.symbols.push(Symbol {
            uuid: Uuid::nil(),
            lib_id: lib_id.to_string(),
            reference: reference.to_string(),
            value: String::new(),
            footprint: String::new(),
            datasheet: String::new(),
            position: origin,
            rotation: 0.0,
            mirror_x: false,
            mirror_y: false,
            unit: 1,
            is_power: false,
            ref_text: None,
            val_text: None,
            fields_autoplaced: false,
            fields_user_placed: false,
            dnp: false,
            in_bom: true,
            on_board: true,
            exclude_from_sim: false,
            locked: false,
            fields: HashMap::new(),
            custom_properties: Vec::new(),
            pin_uuids: HashMap::new(),
            instances: Vec::new(),
            library_id: None,
            row_id: None,
            library_version: String::new(),
        });
    }

    #[test]
    fn point_on_segment_detects_interior_and_rejects_off_segment() {
        let a = (0, 0);
        let b = (10_000, 0);
        assert!(point_on_segment((5_000, 0), a, b), "interior point");
        assert!(point_on_segment((0, 0), a, b), "endpoint");
        assert!(!point_on_segment((5_000, 1_000), a, b), "off the line");
        assert!(
            !point_on_segment((11_000, 0), a, b),
            "collinear but past the end"
        );
    }

    #[test]
    fn two_pins_on_one_wire_share_a_net_with_both_terminals() {
        // A single wire from (0,0) to (10,0). R1 pin 1 sits at (0,0),
        // R2 pin 1 at (10,0). Both land on the wire's endpoints, so they
        // belong to one net carrying two terminals.
        let mut sheet = empty_sheet();
        sheet.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
        add_lib(
            &mut sheet,
            "R",
            vec![lib_pin("1", pt(0.0, 0.0), PinDirection::Passive)],
        );
        place(&mut sheet, "R1", "R", pt(0.0, 0.0));
        place(&mut sheet, "R2", "R", pt(10.0, 0.0));

        let netlist = build_netlist(&sheet);
        assert_eq!(netlist.nets.len(), 1);
        let net = &netlist.nets[0];
        assert_eq!(net.terminals.len(), 2);
        assert_eq!(net.terminals[0].reference, "R1");
        assert_eq!(net.terminals[1].reference, "R2");
        assert_eq!(net.id, NetId(1));
        assert_eq!(net.name, "N$1", "unlabelled net gets an auto name");
    }

    #[test]
    fn t_junction_merges_wire_ending_on_another_wires_interior() {
        // Horizontal wire (0,0)-(10,0); a vertical wire (5,0)-(5,5) ends on
        // its interior; a junction dot at (5,0). A pin at each far end must
        // land on ONE net once the junction connects the two wires.
        // Regression for issue #107.
        let mut sheet = empty_sheet();
        sheet.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
        sheet.wires.push(wire(pt(5.0, 0.0), pt(5.0, 5.0)));
        sheet.junctions.push(junction(pt(5.0, 0.0)));
        add_lib(
            &mut sheet,
            "U",
            vec![lib_pin("A", pt(0.0, 0.0), PinDirection::Output)],
        );
        // Local pin (0,0) at origin (0,0) → world (0,0).
        place(&mut sheet, "U1", "U", pt(0.0, 0.0));
        // Local pin (0,0) at origin (5,5) → world (5,5).
        place(&mut sheet, "U2", "U", pt(5.0, 5.0));

        let netlist = build_netlist(&sheet);
        assert_eq!(
            netlist.nets.len(),
            1,
            "a T-junction must merge both wires into one net"
        );
        assert_eq!(
            netlist.nets[0].terminals.len(),
            2,
            "both pins belong to the merged net"
        );
    }

    #[test]
    fn t_intersection_without_junction_stays_two_nets() {
        // Same geometry, no junction dot: the connection is not asserted, so
        // the wires remain two separate nets.
        let mut sheet = empty_sheet();
        sheet.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
        sheet.wires.push(wire(pt(5.0, 0.0), pt(5.0, 5.0)));
        add_lib(
            &mut sheet,
            "U",
            vec![lib_pin("A", pt(0.0, 0.0), PinDirection::Output)],
        );
        place(&mut sheet, "U1", "U", pt(0.0, 0.0));
        place(&mut sheet, "U2", "U", pt(5.0, 5.0));

        let netlist = build_netlist(&sheet);
        assert_eq!(
            netlist.nets.len(),
            2,
            "without a junction the T is two separate nets"
        );
    }

    #[test]
    fn label_names_the_net_by_priority() {
        // Two labels on the same wire: a plain Net label and a Global label.
        // The Global one wins.
        let mut sheet = empty_sheet();
        sheet.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
        sheet.labels.push(label("local", pt(0.0, 0.0), LabelType::Net));
        sheet
            .labels
            .push(label("VBUS", pt(10.0, 0.0), LabelType::Global));
        add_lib(
            &mut sheet,
            "R",
            vec![lib_pin("1", pt(0.0, 0.0), PinDirection::Passive)],
        );
        place(&mut sheet, "R1", "R", pt(0.0, 0.0));

        let netlist = build_netlist(&sheet);
        assert_eq!(netlist.nets.len(), 1);
        assert_eq!(netlist.nets[0].name, "VBUS");
    }

    #[test]
    fn class_is_first_word_before_underscore() {
        let mut sheet = empty_sheet();
        sheet.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
        sheet
            .labels
            .push(label("I2C_SDA", pt(0.0, 0.0), LabelType::Net));
        add_lib(
            &mut sheet,
            "R",
            vec![lib_pin("1", pt(0.0, 0.0), PinDirection::Passive)],
        );
        place(&mut sheet, "R1", "R", pt(0.0, 0.0));

        let netlist = build_netlist(&sheet);
        assert_eq!(netlist.nets[0].class, "i2c");
    }

    #[test]
    fn unconnected_pins_form_no_net() {
        // A symbol with a pin, but no wire/label anywhere near it: nothing
        // lands on the tip, so it contributes no terminal and no net.
        let mut sheet = empty_sheet();
        add_lib(
            &mut sheet,
            "R",
            vec![lib_pin("1", pt(0.0, 0.0), PinDirection::Passive)],
        );
        place(&mut sheet, "R1", "R", pt(0.0, 0.0));

        let netlist = build_netlist(&sheet);
        assert!(netlist.nets.is_empty());
    }

    #[test]
    fn net_numbering_is_deterministic() {
        // Two independent one-wire nets far apart. Building twice yields the
        // same ids, and ids are assigned 1..=N in sorted-root order.
        let mut sheet = empty_sheet();
        sheet.wires.push(wire(pt(0.0, 0.0), pt(1.0, 0.0)));
        sheet.wires.push(wire(pt(50.0, 0.0), pt(51.0, 0.0)));
        add_lib(
            &mut sheet,
            "R",
            vec![lib_pin("1", pt(0.0, 0.0), PinDirection::Passive)],
        );
        place(&mut sheet, "R1", "R", pt(0.0, 0.0));
        place(&mut sheet, "R2", "R", pt(50.0, 0.0));

        let a = build_netlist(&sheet);
        let b = build_netlist(&sheet);
        assert_eq!(a, b, "deterministic across builds");
        assert_eq!(a.nets.len(), 2);
        assert_eq!(a.nets[0].id, NetId(1));
        assert_eq!(a.nets[1].id, NetId(2));
    }
}
