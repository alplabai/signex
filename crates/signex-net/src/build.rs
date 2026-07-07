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
//! Scope (ADR-0001 A3.1): single sheet only — no hierarchy; net names come
//! from the highest-priority label on the net, matching the current ERC
//! semantics. Same-name label *merging* and cross-sheet stitching are
//! deferred to a later increment.

use std::collections::HashMap;

use signex_types::net::{Net, NetId, Netlist, Terminal};
use signex_types::schematic::{Label, LabelType, Point, SchematicSheet, Symbol};
use uuid::Uuid;

use crate::uf::{Key, find as uf_find, union as uf_union};

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
        || sheet
            .no_connects
            .iter()
            .any(|nc| pt_same(&nc.position, pos))
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

/// The electrical connectivity of a single sheet: a union-find over wire
/// endpoints with junction T-merges. This is the shared core both
/// [`build_netlist`] and the net-colour flood ([`flood_net_elements`]) read,
/// so they can never disagree on which points sit on the same net. The app
/// previously hand-rolled its own coarser copy (0.01 mm buckets, no interior
/// T-merge) — the "D4 leak" that let a highlight bleed across nets.
pub struct SheetConnectivity {
    parent: HashMap<Key, Key>,
}

impl SheetConnectivity {
    /// Build connectivity for `sheet`: union each wire's two endpoints, then
    /// merge every wire whose segment passes through a junction dot —
    /// including a wire that ends on another wire's interior (a T-junction).
    /// Union-find over endpoints alone never merges that case, so the junction
    /// is what asserts the connection. Regression: issue #107.
    pub fn build(sheet: &SchematicSheet) -> Self {
        let mut parent: HashMap<Key, Key> = HashMap::new();
        for w in &sheet.wires {
            uf_union(&mut parent, pt_key(&w.start), pt_key(&w.end));
        }
        for j in &sheet.junctions {
            let jk = pt_key(&j.position);
            for w in &sheet.wires {
                if point_on_segment(jk, pt_key(&w.start), pt_key(&w.end)) {
                    uf_union(&mut parent, jk, pt_key(&w.start));
                }
            }
        }
        Self { parent }
    }

    /// The canonical net root of point `p` — its union-find representative in
    /// the 1 µm key space. Two points sit on the same net iff their roots are
    /// equal. Takes `&mut self` because lookups path-compress.
    pub fn root_of(&mut self, p: &Point) -> Key {
        uf_find(&mut self.parent, pt_key(p))
    }
}

/// The wire and junction uuids the net-colour flood should paint when the
/// user clicks a wire — every wire and junction electrically connected to
/// `target_wire`. Returned by [`flood_net_elements`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FloodElements {
    pub wires: Vec<Uuid>,
    pub junctions: Vec<Uuid>,
}

/// Every wire and junction on the same net as `target_wire`, for the
/// net-colour flood. Returns `None` when `target_wire` is not a wire in
/// `sheet`. Uses the same [`SheetConnectivity`] core as [`build_netlist`], so
/// the highlight follows the real net exactly — it can neither bleed across
/// nets (the old 0.01 mm-bucket over-merge) nor miss a T-junction the way the
/// app's previous inline union-find did.
pub fn flood_net_elements(sheet: &SchematicSheet, target_wire: Uuid) -> Option<FloodElements> {
    let target = sheet.wires.iter().find(|w| w.uuid == target_wire)?;
    let mut conn = SheetConnectivity::build(sheet);
    let root = conn.root_of(&target.start);
    let wires = sheet
        .wires
        .iter()
        .filter(|w| conn.root_of(&w.start) == root)
        .map(|w| w.uuid)
        .collect();
    let junctions = sheet
        .junctions
        .iter()
        .filter(|j| conn.root_of(&j.position) == root)
        .map(|j| j.uuid)
        .collect();
    Some(FloodElements { wires, junctions })
}

/// The per-sheet union-find after physical connectivity ([`SheetConnectivity`])
/// plus same-name label anchoring and on-sheet merging — the "level 1" analysis
/// shared by [`build_netlist`] and the cross-sheet project stitcher. Every union
/// (wire, junction, and label anchoring) completes here, before any root is
/// sampled: sampling a root and then mutating the map again is a correctness
/// hazard the two-level stitcher relies on this to avoid.
pub(crate) fn merged_sheet_parent(sheet: &SchematicSheet) -> HashMap<Key, Key> {
    // Start from the physical connectivity core (wires + junction T-merges),
    // owned so the label-merge below never disturbs what the net-flood reads
    // through `SheetConnectivity`.
    let mut parent = SheetConnectivity::build(sheet).parent;

    // Anchor each label to the wire it sits on (endpoint or interior), then
    // merge net roots that share a same-name label whose kind joins by name,
    // within this sheet: Global, Power (power nets), and local Net. (Global and
    // Power also join across sheets by name — the cross-sheet stitcher's job;
    // here we only see one sheet.) Hierarchical labels join to a parent sheet's
    // pins, not to same-name peers, so they are left to cross-sheet stitching.
    let mut name_root: HashMap<&str, Key> = HashMap::new();
    for lbl in &sheet.labels {
        let lk = pt_key(&lbl.position);
        for w in &sheet.wires {
            if point_on_segment(lk, pt_key(&w.start), pt_key(&w.end)) {
                uf_union(&mut parent, lk, pt_key(&w.start));
                break;
            }
        }
        if lbl.text.is_empty()
            || !matches!(
                lbl.label_type,
                LabelType::Global | LabelType::Power | LabelType::Net
            )
        {
            continue;
        }
        let root = uf_find(&mut parent, lk);
        match name_root.get(lbl.text.as_str()) {
            Some(&existing) => {
                uf_union(&mut parent, root, existing);
            }
            None => {
                name_root.insert(lbl.text.as_str(), root);
            }
        }
    }
    parent
}

/// Group each sheet label under its merged net root, so the highest-priority
/// label can name the net. `parent` must already be fully merged
/// ([`merged_sheet_parent`]).
pub(crate) fn collect_net_labels<'a>(
    sheet: &'a SchematicSheet,
    parent: &mut HashMap<Key, Key>,
) -> HashMap<Key, Vec<&'a Label>> {
    let mut net_labels: HashMap<Key, Vec<&Label>> = HashMap::new();
    for lbl in &sheet.labels {
        let root = uf_find(parent, pt_key(&lbl.position));
        net_labels.entry(root).or_default().push(lbl);
    }
    net_labels
}

/// Project every connected component pin to world space and group it as a
/// [`Terminal`] under its net root. A pin counts only if something lands on its
/// tip (wire/bus/label/no-connect) — see [`point_is_connected`]. `parent` must
/// already be fully merged ([`merged_sheet_parent`]).
pub(crate) fn collect_terminals(
    sheet: &SchematicSheet,
    parent: &mut HashMap<Key, Key>,
) -> HashMap<Key, Vec<Terminal>> {
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
            // Pin id: prefer the pin number when present, else fall back to its name.
            let pin = if !lp.pin.number.is_empty() {
                lp.pin.number.clone()
            } else {
                lp.pin.name.clone()
            };
            let root = uf_find(parent, pt_key(&world_pos));
            net_terms.entry(root).or_default().push(Terminal {
                reference: sym.reference.clone(),
                pin,
            });
        }
    }
    net_terms
}

/// Build the authoritative [`Netlist`] for a single schematic sheet.
///
/// Physical connectivity is [`SheetConnectivity`] — union-find over wire
/// endpoints, with junctions merging wires that meet (including a wire
/// terminating on another's interior, a T-junction, issue #107). On top of
/// that, same-name labels join nets **within this sheet**: same-name `Global`,
/// `Power` (power nets like `GND` / `VCC`), or local `Net` labels each merge
/// every group *on this sheet* carrying that name into one net. `Global` and
/// `Power` labels also connect by name *across* sheets, but that whole-design
/// stitching is the cross-sheet increment's job — `build_netlist` sees a single
/// sheet, so it realises only the on-sheet part. `Hierarchical` labels connect
/// to a parent sheet's pins rather than to same-name peers, so they too are
/// left to cross-sheet stitching.
///
/// Component pins are projected to world space and attached as [`Terminal`]s to
/// the net their tip lands on. Output is deterministic: nets are numbered
/// `1..=N` in sorted-root order and each net's terminals are sorted by
/// `(reference, pin)`.
pub fn build_netlist(sheet: &SchematicSheet) -> Netlist {
    let mut parent = merged_sheet_parent(sheet);
    let net_labels = collect_net_labels(sheet, &mut parent);
    let mut net_terms = collect_terminals(sheet, &mut parent);

    // A net exists wherever at least one terminal lands. A label with no pins
    // is a dangling label — it carries no connectivity, so it forms no net.
    let mut roots: Vec<Key> = net_terms.keys().copied().collect();
    roots.sort_unstable();

    let nets = roots
        .into_iter()
        .enumerate()
        .map(|(idx, root)| {
            let id = NetId(idx as u32 + 1);
            let label_name = net_labels.get(&root).and_then(|lbls| best_label_name(lbls));
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
        sheet
            .labels
            .push(label("local", pt(0.0, 0.0), LabelType::Net));
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

    /// Two physically separate one-wire groups, each with a pin and a same-name
    /// label of `ty`. Returns the built netlist so a test can assert whether
    /// they were merged by name.
    fn two_labelled_groups(text: &str, ty: LabelType) -> Netlist {
        let mut sheet = empty_sheet();
        sheet.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
        sheet.wires.push(wire(pt(50.0, 0.0), pt(60.0, 0.0)));
        sheet.labels.push(label(text, pt(0.0, 0.0), ty));
        sheet.labels.push(label(text, pt(50.0, 0.0), ty));
        add_lib(
            &mut sheet,
            "R",
            vec![lib_pin("1", pt(0.0, 0.0), PinDirection::Passive)],
        );
        place(&mut sheet, "R1", "R", pt(0.0, 0.0));
        place(&mut sheet, "R2", "R", pt(50.0, 0.0));
        build_netlist(&sheet)
    }

    #[test]
    fn global_labels_of_the_same_name_merge_separate_groups() {
        // Same-name Global labels are one electrical net across the whole
        // design, even with no wire between the two groups.
        let netlist = two_labelled_groups("VBUS", LabelType::Global);
        assert_eq!(netlist.nets.len(), 1, "same-name globals merge");
        assert_eq!(netlist.nets[0].name, "VBUS");
        assert_eq!(netlist.nets[0].terminals.len(), 2);
    }

    #[test]
    fn power_labels_of_the_same_name_merge() {
        // Every `GND` power port is the same net.
        let netlist = two_labelled_groups("GND", LabelType::Power);
        assert_eq!(netlist.nets.len(), 1, "same-name power nets merge");
        assert_eq!(netlist.nets[0].name, "GND");
        assert_eq!(netlist.nets[0].terminals.len(), 2);
    }

    #[test]
    fn local_net_labels_of_the_same_name_merge_on_one_sheet() {
        // Same-name local labels connect within a single sheet.
        let netlist = two_labelled_groups("SDA", LabelType::Net);
        assert_eq!(netlist.nets.len(), 1, "same-name local labels connect");
        assert_eq!(netlist.nets[0].name, "SDA");
        assert_eq!(netlist.nets[0].terminals.len(), 2);
    }

    #[test]
    fn hierarchical_labels_of_the_same_name_do_not_merge_on_one_sheet() {
        // Hierarchical labels connect to a parent sheet's pins, not to
        // same-name peers — that stitching is the cross-sheet increment's job,
        // so two same-name hierarchical groups stay separate here.
        let netlist = two_labelled_groups("BUS", LabelType::Hierarchical);
        assert_eq!(
            netlist.nets.len(),
            2,
            "hierarchical same-name is not merged on one sheet"
        );
    }

    #[test]
    fn differently_named_labels_stay_separate() {
        let mut sheet = empty_sheet();
        sheet.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
        sheet.wires.push(wire(pt(50.0, 0.0), pt(60.0, 0.0)));
        sheet
            .labels
            .push(label("NET_A", pt(0.0, 0.0), LabelType::Global));
        sheet
            .labels
            .push(label("NET_B", pt(50.0, 0.0), LabelType::Global));
        add_lib(
            &mut sheet,
            "R",
            vec![lib_pin("1", pt(0.0, 0.0), PinDirection::Passive)],
        );
        place(&mut sheet, "R1", "R", pt(0.0, 0.0));
        place(&mut sheet, "R2", "R", pt(50.0, 0.0));

        let netlist = build_netlist(&sheet);
        assert_eq!(netlist.nets.len(), 2, "different names are different nets");
    }

    #[test]
    fn label_anchors_to_a_wire_interior_not_just_endpoints() {
        // A label placed mid-segment — (5,0) on the (0,0)-(10,0) wire, not an
        // endpoint — must anchor to that wire's net so its name-merge joins the
        // wire's group. Without the interior on-segment anchoring the label
        // would be a stray singleton at (5,0), and the two groups would NOT
        // merge (R1 and R2 would land on separate nets). Regression guard for
        // the point_on_segment anchoring path.
        let mut sheet = empty_sheet();
        sheet.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
        sheet.wires.push(wire(pt(50.0, 0.0), pt(60.0, 0.0)));
        // First VBUS label sits on the *interior* of wire A, not an endpoint.
        sheet
            .labels
            .push(label("VBUS", pt(5.0, 0.0), LabelType::Global));
        sheet
            .labels
            .push(label("VBUS", pt(50.0, 0.0), LabelType::Global));
        add_lib(
            &mut sheet,
            "R",
            vec![lib_pin("1", pt(0.0, 0.0), PinDirection::Passive)],
        );
        place(&mut sheet, "R1", "R", pt(0.0, 0.0));
        place(&mut sheet, "R2", "R", pt(50.0, 0.0));

        let netlist = build_netlist(&sheet);
        assert_eq!(
            netlist.nets.len(),
            1,
            "an interior-anchored label merges its wire's net"
        );
        assert_eq!(netlist.nets[0].name, "VBUS");
        assert_eq!(netlist.nets[0].terminals.len(), 2);
    }

    fn wire_id(a: Point, b: Point, id: Uuid) -> Wire {
        Wire {
            uuid: id,
            start: a,
            end: b,
            stroke_width: 0.0,
        }
    }

    #[test]
    fn flood_returns_only_the_clicked_net() {
        // Two independent one-wire nets far apart. Clicking one must paint
        // only that wire — no spurious merge across the gap.
        let mut sheet = empty_sheet();
        let a = Uuid::from_u128(1);
        let b = Uuid::from_u128(2);
        sheet.wires.push(wire_id(pt(0.0, 0.0), pt(10.0, 0.0), a));
        sheet.wires.push(wire_id(pt(50.0, 0.0), pt(60.0, 0.0), b));

        let flood = flood_net_elements(&sheet, a).expect("clicked wire exists");
        assert_eq!(flood.wires, vec![a]);
        assert!(flood.junctions.is_empty());
    }

    #[test]
    fn flood_follows_a_t_junction_and_paints_the_dot() {
        // Horizontal wire (0,0)-(10,0); a vertical wire ends on its interior
        // at (5,0); a junction dot ties them. Clicking the horizontal wire
        // must paint both wires and the junction — the app's old inline flood
        // missed the interior T-merge and left the second wire uncoloured.
        let mut sheet = empty_sheet();
        let h = Uuid::from_u128(10);
        let v = Uuid::from_u128(11);
        let j = Uuid::from_u128(12);
        sheet.wires.push(wire_id(pt(0.0, 0.0), pt(10.0, 0.0), h));
        sheet.wires.push(wire_id(pt(5.0, 0.0), pt(5.0, 5.0), v));
        sheet.junctions.push(Junction {
            uuid: j,
            position: pt(5.0, 0.0),
            diameter: 0.0,
        });

        let mut flood = flood_net_elements(&sheet, h).expect("clicked wire exists");
        flood.wires.sort();
        assert_eq!(flood.wires, vec![h, v]);
        assert_eq!(flood.junctions, vec![j]);
    }

    #[test]
    fn flood_respects_micron_precision_and_does_not_leak() {
        // Two wires whose nearest endpoints are 4 µm apart: (10,0) and
        // (10, 0.004). At 1 µm resolution these are distinct points, so the
        // wires are two separate nets. The app's old 0.01 mm bucket rounded
        // both to the same cell and merged them — the leak. Clicking one must
        // now paint only itself.
        let mut sheet = empty_sheet();
        let a = Uuid::from_u128(20);
        let b = Uuid::from_u128(21);
        sheet.wires.push(wire_id(pt(0.0, 0.0), pt(10.0, 0.0), a));
        sheet
            .wires
            .push(wire_id(pt(10.0, 0.004), pt(20.0, 0.004), b));

        let flood = flood_net_elements(&sheet, a).expect("clicked wire exists");
        assert_eq!(flood.wires, vec![a], "must not leak into the 4 µm-away net");
    }

    #[test]
    fn flood_returns_none_for_an_unknown_wire() {
        let sheet = empty_sheet();
        assert!(flood_net_elements(&sheet, Uuid::from_u128(99)).is_none());
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
