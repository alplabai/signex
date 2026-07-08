//! Cross-sheet netlist stitching — [`build_project_netlist`] (ADR-0002 D8,
//! A3.1 increment 2c).
//!
//! A schematic project is a root sheet plus a map of child sheets keyed by the
//! exact `ChildSheet.filename` string written on the parent. This module walks
//! that hierarchy and derives one [`Netlist`] for the whole design, on top of
//! the same per-sheet analysis [`build_netlist`](crate::build_netlist) uses —
//! so `build_project_netlist(root, &{}, None).netlist` is byte-identical to
//! `build_netlist(root)`.
//!
//! Two-level union-find: **level 1** is the per-sheet derivation (wires,
//! junctions, on-sheet label merge) plus sheet-pin anchoring; **level 2** joins
//! the resulting per-occurrence net roots across the project by three rules —
//! same-name Global/Power labels, power-port symbols as global name carriers,
//! and sheet-pin ↔ child-label binding. Structural problems are reported as
//! [`StitchIssue`]s in-band; the netlist is always produced, deterministically.

use std::collections::{HashMap, HashSet};

use signex_types::net::{Net, NetId, Netlist, Terminal};
use signex_types::schematic::{Label, LabelType, SchematicSheet};
use uuid::Uuid;

use crate::build::{
    collect_membership, collect_net_labels, collect_terminals, dedup_net_names, label_priority,
    merged_sheet_parent, point_on_segment, power_name_carriers, pt_key,
};
use crate::uf::{Key, find as uf_find, union as uf_union};

/// A structural problem found while stitching. The netlist is still produced
/// (best-effort, deterministic); issues tell consumers where it is degraded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StitchIssue {
    /// A `ChildSheet` names a file with no entry in the children map.
    MissingChild {
        parent_path: String,
        sheet_name: String,
        filename: String,
    },
    /// A child reference closes a cycle (its filename is already on the DFS
    /// path, or equals the root filename); the edge is not stitched.
    SheetCycle {
        parent_path: String,
        filename: String,
    },
    /// Two different files carry the same schematic uuid (copy-as-template).
    DuplicateSheetUuid {
        filename_a: String,
        filename_b: String,
    },
    /// One child file instantiated N times: topology is expanded per
    /// occurrence, but refdes collide until per-instance annotation exists.
    SharedReferenceAcrossInstances { filename: String, reference: String },
    /// Two distinct nets resolved to the same final name; a deterministic
    /// suffix was applied.
    NameCollision { name: String },
}

/// The whole-project netlist plus any structural issues found while stitching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectNetlist {
    pub netlist: Netlist,
    pub issues: Vec<StitchIssue>,
}

/// A level-2 node: a per-occurrence level-1 net root.
type L2 = (usize, Key);

/// One visited sheet in the hierarchy (an *occurrence* — the same file
/// instantiated twice yields two occurrences with distinct ids).
struct Occ<'a> {
    sheet: &'a SchematicSheet,
    /// The `ChildSheet.name` chain from the root (empty for the root sheet);
    /// used to qualify Hierarchical / Net names chosen off a non-root sheet.
    name_chain: Vec<String>,
    /// The children-map key (filename) this occurrence was reached by; `None`
    /// for the root unless `root_filename` was given. Occurrences that share a
    /// filename are instances of one file.
    filename: Option<String>,
}

/// Per-occurrence level-1 analysis, sampled after every level-1 union.
struct Analysis<'a> {
    /// Net root → terminals landing on it.
    terminals: HashMap<Key, Vec<Terminal>>,
    /// Net root → labels on it (for naming).
    net_labels: HashMap<Key, Vec<&'a Label>>,
    /// Net root → power-port carrier values on it.
    power_by_root: HashMap<Key, Vec<String>>,
    /// Per `child_sheets` entry: each sheet pin's `(name, net root)`.
    pin_roots: Vec<Vec<(String, Key)>>,
    /// Hierarchical/Global label text → net roots carrying it (the child side
    /// of sheet-pin binding).
    port_labels: HashMap<String, Vec<Key>>,
    /// Net root → (wire uuids, junction uuids) on this sheet.
    membership: HashMap<Key, (Vec<Uuid>, Vec<Uuid>)>,
}

/// Build the whole-project [`Netlist`] by stitching the root sheet to its
/// children.
///
/// `children` is keyed by the exact `ChildSheet.filename` string as written on
/// the parent (not a basename); `root_filename` — the root's own filename, if
/// known — lets a child that re-references the root be caught as a cycle. See
/// the module docs for the stitching rules. `build_project_netlist(root, &{},
/// None).netlist` equals `build_netlist(root)` byte-for-byte.
pub fn build_project_netlist(
    root: &SchematicSheet,
    children: &HashMap<String, SchematicSheet>,
    root_filename: Option<&str>,
) -> ProjectNetlist {
    let mut issues: Vec<StitchIssue> = Vec::new();

    detect_duplicate_uuids(root, children, root_filename, &mut issues);

    // ---- Traverse: build the occurrence tree (pre-order, document order) ----
    let mut occs: Vec<Occ> = Vec::new();
    let mut edges: Vec<(usize, usize, usize)> = Vec::new(); // (parent occ, cs index, child occ)
    let mut path: Vec<String> = Vec::new();
    visit(
        root,
        Vec::new(),
        root_filename,
        &mut path,
        &mut occs,
        &mut edges,
        children,
        &mut issues,
    );

    detect_shared_references(&occs, &mut issues);

    // ---- Level 1: per-occurrence analysis (all unions before any sampling) --
    let analyses: Vec<Analysis> = occs.iter().map(|o| analyze(o.sheet)).collect();

    // ---- Level 2: union net roots across occurrences ------------------------
    let mut l2: HashMap<L2, L2> = HashMap::new();

    // Rules 1 & 2: same-name Global/Power labels and power-port carriers join
    // by bare name project-wide, through one shared name bucket.
    let mut name_bucket: HashMap<String, L2> = HashMap::new();
    for (oid, a) in analyses.iter().enumerate() {
        for (root, labels) in &a.net_labels {
            for l in labels {
                if l.text.is_empty()
                    || !matches!(l.label_type, LabelType::Global | LabelType::Power)
                {
                    continue;
                }
                bucket_join(&mut l2, &mut name_bucket, &l.text, (oid, *root));
            }
        }
        for (root, values) in &a.power_by_root {
            for v in values {
                bucket_join(&mut l2, &mut name_bucket, v, (oid, *root));
            }
        }
    }

    // Rule 3: a named sheet pin binds to child labels of the same text whose
    // kind is Hierarchical or Global (the codebase's port model). Local Net
    // labels never cross (rule 4) — they are simply absent from port_labels.
    for &(parent_occ, cs_index, child_occ) in &edges {
        let parent_a = &analyses[parent_occ];
        let child_a = &analyses[child_occ];
        for (pin_name, pin_root) in &parent_a.pin_roots[cs_index] {
            if pin_name.is_empty() {
                continue;
            }
            if let Some(child_roots) = child_a.port_labels.get(pin_name) {
                for &cr in child_roots {
                    uf_union(&mut l2, (parent_occ, *pin_root), (child_occ, cr));
                }
            }
        }
    }

    // Seed every net-root node so terminal-only nets (no label/binding) still
    // form their own group.
    for (oid, a) in analyses.iter().enumerate() {
        for root in a.terminals.keys() {
            uf_find(&mut l2, (oid, *root));
        }
        for root in a.net_labels.keys() {
            uf_find(&mut l2, (oid, *root));
        }
    }

    // ---- Group nodes into final nets ----------------------------------------
    let mut groups: HashMap<L2, Vec<L2>> = HashMap::new();
    for node in l2.keys().copied().collect::<Vec<_>>() {
        let r = uf_find(&mut l2, node);
        groups.entry(r).or_default().push(node);
    }

    let mut raw: Vec<RawNet> = Vec::new();
    for members in groups.into_values() {
        if let Some(net) = assemble_net(members, &occs, &analyses) {
            raw.push(net);
        }
    }

    // Deterministic ids: sorted-root order extended by (occurrence, root).
    raw.sort_by_key(|r| r.sort_key);

    let mut nets: Vec<Net> = raw
        .into_iter()
        .enumerate()
        .map(|(idx, r)| {
            let id = NetId(idx as u32 + 1);
            let name = r.name.unwrap_or_else(|| format!("N${}", id.0));
            let mut terminals = r.terminals;
            terminals.sort_by(|a, b| a.reference.cmp(&b.reference).then(a.pin.cmp(&b.pin)));
            Net {
                id,
                name,
                class: None,
                wires: r.wires,
                junctions: r.junctions,
                terminals,
            }
        })
        .collect();

    // Two electrically distinct nets may resolve to one name — sibling children
    // that qualify to the same `chain/label`, or any of the single-sheet cases.
    // Suffix the later one and report it. Same pass `build_netlist` runs, so the
    // single-root netlist stays byte-identical.
    for name in dedup_net_names(&mut nets) {
        issues.push(StitchIssue::NameCollision { name });
    }

    ProjectNetlist {
        netlist: Netlist { nets },
        issues,
    }
}

/// A net assembled from a level-2 group, before id assignment and dedup.
struct RawNet {
    sort_key: L2,
    terminals: Vec<Terminal>,
    /// Wire / junction membership, aggregated across the net's occurrences.
    wires: Vec<Uuid>,
    junctions: Vec<Uuid>,
    /// Selected name (bare or already qualified), or `None` for an auto name.
    name: Option<String>,
}

/// Assemble one final net from its level-2 member nodes. Returns `None` when
/// the group carries no terminals (a dangling label/pin forms no net).
fn assemble_net(mut members: Vec<L2>, occs: &[Occ], analyses: &[Analysis]) -> Option<RawNet> {
    members.sort_unstable();

    // Terminals across all members.
    let mut terminals: Vec<Terminal> = Vec::new();
    for &(oid, root) in &members {
        if let Some(ts) = analyses[oid].terminals.get(&root) {
            terminals.extend(ts.iter().cloned());
        }
    }
    if terminals.is_empty() {
        return None;
    }

    // Best name across members: Global > Power > Hierarchical > Net, power
    // carriers rank as Power; ties keep the last candidate in member then
    // document order (matching build_netlist for a single-member net).
    let mut best: Option<(u8, bool, String, usize)> = None; // (prio, qualifiable, text, occ)
    for &(oid, root) in &members {
        if let Some(labels) = analyses[oid].net_labels.get(&root) {
            for l in labels {
                if l.text.is_empty() {
                    continue;
                }
                let p = label_priority(l.label_type);
                let q = matches!(l.label_type, LabelType::Hierarchical | LabelType::Net);
                if best.as_ref().is_none_or(|(bp, ..)| p >= *bp) {
                    best = Some((p, q, l.text.clone(), oid));
                }
            }
        }
        if let Some(values) = analyses[oid].power_by_root.get(&root) {
            for v in values {
                if v.is_empty() {
                    continue;
                }
                if best.as_ref().is_none_or(|(bp, ..)| 2 >= *bp) {
                    best = Some((2, false, v.clone(), oid));
                }
            }
        }
    }

    let name = match best {
        None => None,
        Some((_, qualifiable, text, occ)) => {
            let chain = &occs[occ].name_chain;
            if qualifiable && !chain.is_empty() {
                Some(format!("{}/{}", chain.join("/"), text))
            } else {
                Some(text)
            }
        }
    };

    // Wire / junction membership across every member occurrence.
    let mut wires: Vec<Uuid> = Vec::new();
    let mut junctions: Vec<Uuid> = Vec::new();
    for &(oid, root) in &members {
        if let Some((w, j)) = analyses[oid].membership.get(&root) {
            wires.extend(w.iter().copied());
            junctions.extend(j.iter().copied());
        }
    }

    Some(RawNet {
        sort_key: members[0],
        terminals,
        wires,
        junctions,
        name,
    })
}

/// Level-1 analysis for one sheet: the per-sheet derivation plus sheet-pin
/// anchoring, sampled into the tables the stitcher reads.
fn analyze(sheet: &SchematicSheet) -> Analysis<'_> {
    let mut parent = merged_sheet_parent(sheet);

    // Anchor every child-sheet pin to the wire it sits on (endpoint or
    // interior), like a label — before sampling any root.
    for cs in &sheet.child_sheets {
        for sp in &cs.pins {
            let pk = pt_key(&sp.position);
            for w in &sheet.wires {
                if point_on_segment(pk, pt_key(&w.start), pt_key(&w.end)) {
                    uf_union(&mut parent, pk, pt_key(&w.start));
                    break;
                }
            }
        }
    }

    let terminals = collect_terminals(sheet, &mut parent);
    let net_labels = collect_net_labels(sheet, &mut parent);
    let membership = collect_membership(sheet, &mut parent);

    let mut power_by_root: HashMap<Key, Vec<String>> = HashMap::new();
    for (root, value) in power_name_carriers(sheet, &mut parent) {
        power_by_root.entry(root).or_default().push(value);
    }

    let pin_roots: Vec<Vec<(String, Key)>> = sheet
        .child_sheets
        .iter()
        .map(|cs| {
            cs.pins
                .iter()
                .map(|sp| (sp.name.clone(), uf_find(&mut parent, pt_key(&sp.position))))
                .collect()
        })
        .collect();

    let mut port_labels: HashMap<String, Vec<Key>> = HashMap::new();
    for (root, labels) in &net_labels {
        for l in labels {
            if l.text.is_empty()
                || !matches!(l.label_type, LabelType::Hierarchical | LabelType::Global)
            {
                continue;
            }
            port_labels.entry(l.text.clone()).or_default().push(*root);
        }
    }

    Analysis {
        terminals,
        net_labels,
        power_by_root,
        pin_roots,
        port_labels,
        membership,
    }
}

/// DFS the hierarchy, recording occurrences, parent→child edges, and structural
/// issues (missing children, cycles).
#[allow(clippy::too_many_arguments)]
fn visit<'a>(
    sheet: &'a SchematicSheet,
    name_chain: Vec<String>,
    this_key: Option<&str>,
    path: &mut Vec<String>,
    occs: &mut Vec<Occ<'a>>,
    edges: &mut Vec<(usize, usize, usize)>,
    children: &'a HashMap<String, SchematicSheet>,
    issues: &mut Vec<StitchIssue>,
) -> usize {
    let my_id = occs.len();
    let parent_path = if name_chain.is_empty() {
        this_key.unwrap_or("<root>").to_string()
    } else {
        name_chain.join("/")
    };
    occs.push(Occ {
        sheet,
        name_chain,
        filename: this_key.map(|k| k.to_string()),
    });
    if let Some(k) = this_key {
        path.push(k.to_string());
    }

    for (cs_index, cs) in sheet.child_sheets.iter().enumerate() {
        let key = cs.filename.as_str();
        if path.iter().any(|p| p == key) {
            issues.push(StitchIssue::SheetCycle {
                parent_path: parent_path.clone(),
                filename: key.to_string(),
            });
            continue;
        }
        match children.get(key) {
            None => issues.push(StitchIssue::MissingChild {
                parent_path: parent_path.clone(),
                sheet_name: cs.name.clone(),
                filename: key.to_string(),
            }),
            Some(child_sheet) => {
                let mut child_chain = occs[my_id].name_chain.clone();
                child_chain.push(cs.name.clone());
                let cid = visit(
                    child_sheet,
                    child_chain,
                    Some(key),
                    path,
                    occs,
                    edges,
                    children,
                    issues,
                );
                edges.push((my_id, cs_index, cid));
            }
        }
    }

    if this_key.is_some() {
        path.pop();
    }
    my_id
}

/// Union `node` into the level-2 class of `name`, seeding the bucket the first
/// time a name is seen.
fn bucket_join(l2: &mut HashMap<L2, L2>, bucket: &mut HashMap<String, L2>, name: &str, node: L2) {
    match bucket.get(name).copied() {
        Some(rep) => uf_union(l2, node, rep),
        None => {
            bucket.insert(name.to_string(), node);
            uf_find(l2, node);
        }
    }
}

/// Report each reference designator carried by a file instantiated more than
/// once: per-occurrence expansion keeps the instances electrically distinct,
/// but the refdes collide until per-instance annotation exists. One issue per
/// `(filename, reference)`, in sorted-filename then document order.
fn detect_shared_references(occs: &[Occ], issues: &mut Vec<StitchIssue>) {
    let mut by_file: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, o) in occs.iter().enumerate() {
        if let Some(f) = &o.filename {
            by_file.entry(f.as_str()).or_default().push(i);
        }
    }
    let mut files: Vec<&str> = by_file.keys().copied().collect();
    files.sort_unstable();
    for filename in files {
        let occ_ids = &by_file[filename];
        if occ_ids.len() < 2 {
            continue;
        }
        // Instances of one file share the same symbols; report each reference
        // once, in the sheet's document order.
        let sheet = occs[occ_ids[0]].sheet;
        let mut seen: HashSet<&str> = HashSet::new();
        for sym in &sheet.symbols {
            if sym.reference.is_empty() {
                continue;
            }
            if seen.insert(sym.reference.as_str()) {
                issues.push(StitchIssue::SharedReferenceAcrossInstances {
                    filename: filename.to_string(),
                    reference: sym.reference.clone(),
                });
            }
        }
    }
}

/// Report every pair of children-map entries (and the root, when its filename
/// is known) that share a schematic uuid — copy-as-template corruption. Sheet
/// identity is the filename key, never the uuid.
fn detect_duplicate_uuids(
    root: &SchematicSheet,
    children: &HashMap<String, SchematicSheet>,
    root_filename: Option<&str>,
    issues: &mut Vec<StitchIssue>,
) {
    let mut by_uuid: HashMap<Uuid, String> = HashMap::new();
    let mut entries: Vec<(String, Uuid)> = Vec::new();
    if let Some(rf) = root_filename {
        entries.push((rf.to_string(), root.uuid));
    }
    let mut keys: Vec<&String> = children.keys().collect();
    keys.sort();
    for k in keys {
        entries.push((k.clone(), children[k].uuid));
    }
    for (filename, uuid) in entries {
        if uuid == Uuid::nil() {
            continue;
        }
        match by_uuid.get(&uuid) {
            Some(prev) => issues.push(StitchIssue::DuplicateSheetUuid {
                filename_a: prev.clone(),
                filename_b: filename,
            }),
            None => {
                by_uuid.insert(uuid, filename);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_netlist;
    use signex_types::schematic::{
        ChildSheet, FillType, HAlign, Junction, LibPin, LibSymbol, Pin, PinDirection,
        PinShapeStyle, Point, SheetPin, Symbol, VAlign, Wire,
    };

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
            justify: HAlign::Left,
            justify_v: VAlign::Bottom,
        }
    }

    fn lib_pin(number: &str, local: Point) -> LibPin {
        LibPin {
            unit: 0,
            body_style: 1,
            pin: Pin {
                direction: PinDirection::Passive,
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

    /// A one-pin library symbol whose pin sits at local `(0, 0)`.
    fn add_lib(sheet: &mut SchematicSheet, lib_id: &str) {
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
                pins: vec![lib_pin("1", pt(0.0, 0.0))],
                show_pin_numbers: true,
                show_pin_names: true,
                pin_name_offset: 0.0,
            },
        );
    }

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

    fn place_power(
        sheet: &mut SchematicSheet,
        reference: &str,
        lib_id: &str,
        value: &str,
        at: Point,
    ) {
        place(sheet, reference, lib_id, at);
        let s = sheet.symbols.last_mut().unwrap();
        s.is_power = true;
        s.value = value.to_string();
    }

    fn place_xform(
        sheet: &mut SchematicSheet,
        reference: &str,
        lib_id: &str,
        origin: Point,
        rotation: f64,
        mirror_x: bool,
        mirror_y: bool,
    ) {
        place(sheet, reference, lib_id, origin);
        let s = sheet.symbols.last_mut().unwrap();
        s.rotation = rotation;
        s.mirror_x = mirror_x;
        s.mirror_y = mirror_y;
    }

    fn sheet_pin(name: &str, pos: Point) -> SheetPin {
        SheetPin {
            uuid: Uuid::nil(),
            name: name.to_string(),
            direction: String::new(),
            position: pos,
            rotation: 0.0,
            auto_generated: false,
            user_moved: false,
        }
    }

    fn child_sheet(name: &str, filename: &str, pins: Vec<SheetPin>) -> ChildSheet {
        ChildSheet {
            uuid: Uuid::nil(),
            name: name.to_string(),
            filename: filename.to_string(),
            position: pt(0.0, 0.0),
            size: (10.0, 10.0),
            stroke_width: 0.0,
            fill: FillType::None,
            stroke_color: None,
            fill_color: None,
            fields_autoplaced: false,
            pins,
            instances: Vec::new(),
        }
    }

    fn names(nl: &Netlist) -> Vec<&str> {
        nl.nets.iter().map(|n| n.name.as_str()).collect()
    }

    // 1 ── Equivalence gate: build_project_netlist(root, &{}, None).netlist is
    //      byte-for-byte build_netlist(root), incl. rotated/mirrored symbols.
    fn assert_equiv(sheet: &SchematicSheet) {
        let p = build_project_netlist(sheet, &HashMap::new(), None);
        assert!(
            p.issues.is_empty(),
            "single sheet has no issues: {:?}",
            p.issues
        );
        assert_eq!(
            p.netlist,
            build_netlist(sheet),
            "project(root, &{{}}) must equal build_netlist(root)"
        );
    }

    #[test]
    fn equivalence_gate_root_only() {
        // (a) two pins on one wire.
        let mut a = empty_sheet();
        a.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
        add_lib(&mut a, "R");
        place(&mut a, "R1", "R", pt(0.0, 0.0));
        place(&mut a, "R2", "R", pt(10.0, 0.0));
        assert_equiv(&a);

        // (b) T-junction (issue #107) with a label.
        let mut b = empty_sheet();
        b.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
        b.wires.push(wire(pt(5.0, 0.0), pt(5.0, 5.0)));
        b.junctions.push(junction(pt(5.0, 0.0)));
        b.labels.push(label("SDA", pt(0.0, 0.0), LabelType::Net));
        add_lib(&mut b, "R");
        place(&mut b, "R1", "R", pt(0.0, 0.0));
        place(&mut b, "R2", "R", pt(5.0, 5.0));
        assert_equiv(&b);

        // (c) rotated (90°) + mirrored symbol: its pin projects to (0, 2.54);
        //     a wire there gives it a terminal. Both paths use one transform.
        let mut c = empty_sheet();
        c.wires.push(wire(pt(0.0, 2.54), pt(10.0, 2.54)));
        sheet_add_pin_lib(&mut c, "U", pt(2.54, 0.0));
        place_xform(&mut c, "U1", "U", pt(0.0, 0.0), 90.0, true, false);
        place(&mut c, "R1", "R", pt(10.0, 2.54));
        add_lib(&mut c, "R");
        assert_equiv(&c);

        // (d) power-port naming.
        let mut d = empty_sheet();
        d.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
        add_lib(&mut d, "R");
        add_lib(&mut d, "PWR");
        place(&mut d, "R1", "R", pt(10.0, 0.0));
        place_power(&mut d, "#PWR01", "PWR", "GND", pt(0.0, 0.0));
        assert_equiv(&d);
    }

    /// Library whose single pin sits at `local` (for the rotated-symbol case).
    fn sheet_add_pin_lib(sheet: &mut SchematicSheet, lib_id: &str, local: Point) {
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
                pins: vec![lib_pin("1", local)],
                show_pin_numbers: true,
                show_pin_names: true,
                pin_name_offset: 0.0,
            },
        );
    }

    // A parent with one child; the child has a labelled pin net. Returns
    // (root, children map) for the binding tests.
    fn parent_child(
        pin_name: &str,
        pin_pos: Point,
        child_label: &str,
        child_label_type: LabelType,
    ) -> (SchematicSheet, HashMap<String, SchematicSheet>) {
        let mut root = empty_sheet();
        // A resistor on the root net that the sheet pin sits on.
        root.wires
            .push(wire(pin_pos, pt(pin_pos.x + 5.0, pin_pos.y)));
        add_lib(&mut root, "R");
        place(&mut root, "RP", "R", pt(pin_pos.x + 5.0, pin_pos.y));
        root.child_sheets.push(child_sheet(
            "U1",
            "child.sch",
            vec![sheet_pin(pin_name, pin_pos)],
        ));

        let mut child = empty_sheet();
        child.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
        child
            .labels
            .push(label(child_label, pt(0.0, 0.0), child_label_type));
        add_lib(&mut child, "R");
        place(&mut child, "RC", "R", pt(10.0, 0.0));

        let mut map = HashMap::new();
        map.insert("child.sch".to_string(), child);
        (root, map)
    }

    // 2 ── SheetPin ↔ child label binding (Hierarchical and Global); an
    //      unmatched pin leaves the parent net local.
    #[test]
    fn sheet_pin_binds_hierarchical_child_label() {
        let (root, map) = parent_child("BUS", pt(0.0, 0.0), "BUS", LabelType::Hierarchical);
        let p = build_project_netlist(&root, &map, None);
        assert!(p.issues.is_empty(), "{:?}", p.issues);
        // RP (root) and RC (child) are one net through the pin↔label binding.
        assert_eq!(p.netlist.nets.len(), 1);
        assert_eq!(p.netlist.nets[0].terminals.len(), 2);
    }

    #[test]
    fn sheet_pin_binds_global_child_label() {
        let (root, map) = parent_child("VCC", pt(0.0, 0.0), "VCC", LabelType::Global);
        let p = build_project_netlist(&root, &map, None);
        assert_eq!(p.netlist.nets.len(), 1);
        assert_eq!(p.netlist.nets[0].terminals.len(), 2);
    }

    #[test]
    fn unmatched_sheet_pin_stays_local() {
        // Pin "BUS" but the child's label is "OTHER" → no binding: two nets.
        let (root, map) = parent_child("BUS", pt(0.0, 0.0), "OTHER", LabelType::Hierarchical);
        let p = build_project_netlist(&root, &map, None);
        assert_eq!(p.netlist.nets.len(), 2, "unbound pin keeps nets separate");
    }

    // 3 ── Global name spans sheets; power-port symbol + Power label merge.
    #[test]
    fn global_label_spans_two_sheets() {
        let mut root = empty_sheet();
        root.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
        root.labels
            .push(label("NET5V", pt(0.0, 0.0), LabelType::Global));
        add_lib(&mut root, "R");
        place(&mut root, "R1", "R", pt(10.0, 0.0));
        root.child_sheets
            .push(child_sheet("A", "a.sch", Vec::new()));

        let mut child = empty_sheet();
        child.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
        child
            .labels
            .push(label("NET5V", pt(0.0, 0.0), LabelType::Global));
        add_lib(&mut child, "R");
        place(&mut child, "R2", "R", pt(10.0, 0.0));

        let mut map = HashMap::new();
        map.insert("a.sch".to_string(), child);
        let p = build_project_netlist(&root, &map, None);
        assert_eq!(p.netlist.nets.len(), 1, "same-name Global spans sheets");
        assert_eq!(p.netlist.nets[0].name, "NET5V");
        assert_eq!(p.netlist.nets[0].terminals.len(), 2);
    }

    #[test]
    fn membership_aggregates_across_sheet_occurrences() {
        // D3.1 cross-sheet: a net stitched from a Global label on two sheets
        // carries the wire uuids from *both* occurrences, so the net-flood
        // highlights the whole net project-wide — not just the clicked sheet.
        let rw = Uuid::from_u128(0x11);
        let cw = Uuid::from_u128(0x22);

        let mut root = empty_sheet();
        root.wires.push(Wire {
            uuid: rw,
            start: pt(0.0, 0.0),
            end: pt(10.0, 0.0),
            stroke_width: 0.0,
        });
        root.labels
            .push(label("NET5V", pt(0.0, 0.0), LabelType::Global));
        add_lib(&mut root, "R");
        place(&mut root, "R1", "R", pt(10.0, 0.0));
        root.child_sheets
            .push(child_sheet("A", "a.sch", Vec::new()));

        let mut child = empty_sheet();
        child.wires.push(Wire {
            uuid: cw,
            start: pt(0.0, 0.0),
            end: pt(10.0, 0.0),
            stroke_width: 0.0,
        });
        child
            .labels
            .push(label("NET5V", pt(0.0, 0.0), LabelType::Global));
        add_lib(&mut child, "R");
        place(&mut child, "R2", "R", pt(10.0, 0.0));

        let mut map = HashMap::new();
        map.insert("a.sch".to_string(), child);
        let p = build_project_netlist(&root, &map, None);
        assert_eq!(p.netlist.nets.len(), 1, "same-name Global spans sheets");
        let net = &p.netlist.nets[0];
        assert!(
            net.wires.contains(&rw) && net.wires.contains(&cw),
            "both occurrences' wires belong to the net: {:?}",
            net.wires
        );
    }

    #[test]
    fn power_symbol_and_power_label_merge_across_sheets() {
        // Root: GND power-port symbol. Child: GND Power label. One net.
        let mut root = empty_sheet();
        root.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
        add_lib(&mut root, "R");
        add_lib(&mut root, "PWR");
        place(&mut root, "R1", "R", pt(10.0, 0.0));
        place_power(&mut root, "#PWR01", "PWR", "GND", pt(0.0, 0.0));
        root.child_sheets
            .push(child_sheet("A", "a.sch", Vec::new()));

        let mut child = empty_sheet();
        child.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
        child
            .labels
            .push(label("GND", pt(0.0, 0.0), LabelType::Power));
        add_lib(&mut child, "R");
        place(&mut child, "R2", "R", pt(10.0, 0.0));

        let mut map = HashMap::new();
        map.insert("a.sch".to_string(), child);
        let p = build_project_netlist(&root, &map, None);
        assert_eq!(p.netlist.nets.len(), 1, "GND port + GND label are one net");
        assert_eq!(p.netlist.nets[0].name, "GND");
        // R1 + the #PWR01 power-port pin (root) + R2 (child).
        assert_eq!(p.netlist.nets[0].terminals.len(), 3);
    }

    // 4 ── Local Net labels never cross; the two are distinct, qualified nets.
    #[test]
    fn local_net_labels_do_not_cross_sheets() {
        let mut root = empty_sheet();
        root.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
        root.labels.push(label("SDA", pt(0.0, 0.0), LabelType::Net));
        add_lib(&mut root, "R");
        place(&mut root, "R1", "R", pt(10.0, 0.0));
        root.child_sheets
            .push(child_sheet("charger", "a.sch", Vec::new()));

        let mut child = empty_sheet();
        child.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
        child
            .labels
            .push(label("SDA", pt(0.0, 0.0), LabelType::Net));
        add_lib(&mut child, "R");
        place(&mut child, "R2", "R", pt(10.0, 0.0));

        let mut map = HashMap::new();
        map.insert("a.sch".to_string(), child);
        let p = build_project_netlist(&root, &map, None);
        assert_eq!(p.netlist.nets.len(), 2, "local Net labels stay per-sheet");
        // Root SDA stays bare; the child SDA is qualified by its sheet name.
        let ns = names(&p.netlist);
        assert!(ns.contains(&"SDA"), "root net bare: {ns:?}");
        assert!(ns.contains(&"charger/SDA"), "child net qualified: {ns:?}");
    }

    // 5 ── One child file instantiated twice: instances not shorted; refdes
    //      collision reported; terminals per occurrence.
    #[test]
    fn same_child_instantiated_twice_is_not_shorted() {
        let mut root = empty_sheet();
        root.child_sheets.push(child_sheet(
            "A",
            "child.sch",
            vec![sheet_pin("P", pt(0.0, 0.0))],
        ));
        root.child_sheets.push(child_sheet(
            "B",
            "child.sch",
            vec![sheet_pin("P", pt(20.0, 0.0))],
        ));

        let mut child = empty_sheet();
        child.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
        child
            .labels
            .push(label("P", pt(0.0, 0.0), LabelType::Hierarchical));
        add_lib(&mut child, "R");
        place(&mut child, "R1", "R", pt(10.0, 0.0));

        let mut map = HashMap::new();
        map.insert("child.sch".to_string(), child);
        let p = build_project_netlist(&root, &map, None);
        assert_eq!(
            p.netlist.nets.len(),
            2,
            "instances are not shorted together"
        );
        for net in &p.netlist.nets {
            assert_eq!(net.terminals.len(), 1, "one R1 per instance");
            assert_eq!(net.terminals[0].reference, "R1");
        }
        assert!(
            p.issues
                .contains(&StitchIssue::SharedReferenceAcrossInstances {
                    filename: "child.sch".to_string(),
                    reference: "R1".to_string(),
                }),
            "refdes collision reported: {:?}",
            p.issues
        );
    }

    // 6 ── Missing child file → issue, parent net stays local, deterministic.
    #[test]
    fn missing_child_reported_and_local() {
        let mut root = empty_sheet();
        root.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
        add_lib(&mut root, "R");
        place(&mut root, "R1", "R", pt(10.0, 0.0));
        root.child_sheets.push(child_sheet(
            "A",
            "gone.sch",
            vec![sheet_pin("P", pt(0.0, 0.0))],
        ));

        let p = build_project_netlist(&root, &HashMap::new(), None);
        assert_eq!(p.netlist.nets.len(), 1, "root net survives");
        assert!(
            p.issues.iter().any(|i| matches!(
                i,
                StitchIssue::MissingChild { filename, .. } if filename == "gone.sch"
            )),
            "missing child reported: {:?}",
            p.issues
        );
        // Deterministic.
        assert_eq!(p, build_project_netlist(&root, &HashMap::new(), None));
    }

    // 7 ── Cycles: A→B→A and child-instantiates-root → SheetCycle, no hang.
    #[test]
    fn cycles_are_reported_without_hanging() {
        let mut a = empty_sheet();
        a.child_sheets.push(child_sheet("toB", "b.sch", Vec::new()));
        let mut b = empty_sheet();
        b.child_sheets.push(child_sheet("toA", "a.sch", Vec::new()));

        let mut map = HashMap::new();
        map.insert("a.sch".to_string(), a.clone());
        map.insert("b.sch".to_string(), b);
        // Root is a.sch; root_filename lets the B→A edge be seen as a cycle.
        let p = build_project_netlist(&a, &map, Some("a.sch"));
        assert!(
            p.issues
                .iter()
                .any(|i| matches!(i, StitchIssue::SheetCycle { .. })),
            "cycle reported: {:?}",
            p.issues
        );
    }

    // 8 ── Sheet pin anchored on a wire *interior* on the parent.
    #[test]
    fn sheet_pin_anchors_to_wire_interior() {
        let mut root = empty_sheet();
        // Wire 0..10; the sheet pin sits mid-span at (5,0), and RP at (10,0).
        root.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
        add_lib(&mut root, "R");
        place(&mut root, "RP", "R", pt(0.0, 0.0));
        root.child_sheets.push(child_sheet(
            "A",
            "a.sch",
            vec![sheet_pin("BUS", pt(5.0, 0.0))],
        ));

        let mut child = empty_sheet();
        child.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
        child
            .labels
            .push(label("BUS", pt(0.0, 0.0), LabelType::Hierarchical));
        add_lib(&mut child, "R");
        place(&mut child, "RC", "R", pt(10.0, 0.0));

        let mut map = HashMap::new();
        map.insert("a.sch".to_string(), child);
        let p = build_project_netlist(&root, &map, None);
        assert_eq!(
            p.netlist.nets.len(),
            1,
            "interior-anchored pin binds the net"
        );
        assert_eq!(p.netlist.nets[0].terminals.len(), 2);
    }

    // 9 ── Two same-name sheet pins on one ChildSheet merge through the child.
    #[test]
    fn two_same_name_sheet_pins_merge_through_child() {
        let mut root = empty_sheet();
        // Two separate root nets, each with a "BUS" sheet pin to the same child.
        root.wires.push(wire(pt(0.0, 0.0), pt(5.0, 0.0)));
        root.wires.push(wire(pt(20.0, 0.0), pt(25.0, 0.0)));
        add_lib(&mut root, "R");
        place(&mut root, "RA", "R", pt(5.0, 0.0));
        place(&mut root, "RB", "R", pt(25.0, 0.0));
        root.child_sheets.push(child_sheet(
            "A",
            "a.sch",
            vec![
                sheet_pin("BUS", pt(0.0, 0.0)),
                sheet_pin("BUS", pt(20.0, 0.0)),
            ],
        ));

        let mut child = empty_sheet();
        child.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
        child
            .labels
            .push(label("BUS", pt(0.0, 0.0), LabelType::Hierarchical));
        add_lib(&mut child, "R");
        place(&mut child, "RC", "R", pt(10.0, 0.0));

        let mut map = HashMap::new();
        map.insert("a.sch".to_string(), child);
        let p = build_project_netlist(&root, &map, None);
        // Both root nets merge through the shared child BUS label → one net.
        assert_eq!(
            p.netlist.nets.len(),
            1,
            "same-name pins merge through child"
        );
        assert_eq!(p.netlist.nets[0].terminals.len(), 3);
    }

    // 10 ── Name collision via duplicate sibling ChildSheet.name → suffix + issue.
    #[test]
    fn duplicate_sibling_name_collision_is_suffixed() {
        // Two sibling children with the SAME name "charger", each a distinct
        // file, each with a local Net "SDA" → both qualify to "charger/SDA".
        let mut root = empty_sheet();
        root.child_sheets
            .push(child_sheet("charger", "a.sch", Vec::new()));
        root.child_sheets
            .push(child_sheet("charger", "b.sch", Vec::new()));

        let mut a = empty_sheet();
        a.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
        a.labels.push(label("SDA", pt(0.0, 0.0), LabelType::Net));
        add_lib(&mut a, "R");
        place(&mut a, "RA", "R", pt(10.0, 0.0));
        let mut b = empty_sheet();
        b.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
        b.labels.push(label("SDA", pt(0.0, 0.0), LabelType::Net));
        add_lib(&mut b, "R");
        place(&mut b, "RB", "R", pt(10.0, 0.0));

        let mut map = HashMap::new();
        map.insert("a.sch".to_string(), a);
        map.insert("b.sch".to_string(), b);
        let p = build_project_netlist(&root, &map, None);
        assert_eq!(p.netlist.nets.len(), 2, "two distinct qualified nets");
        let ns = names(&p.netlist);
        assert!(ns.contains(&"charger/SDA"), "first keeps the name: {ns:?}");
        assert!(
            ns.iter().any(|n| n.starts_with("charger/SDA_")),
            "second is suffixed: {ns:?}"
        );
        assert!(
            p.issues.contains(&StitchIssue::NameCollision {
                name: "charger/SDA".to_string()
            }),
            "collision reported: {:?}",
            p.issues
        );
    }

    // 10b ── A bare single-sheet collision dedups through the same shared pass,
    // so the root netlist stays byte-identical to build_netlist while the
    // stitcher still surfaces the clash.
    #[test]
    fn single_sheet_name_collision_matches_build_netlist_and_is_reported() {
        let mut root = empty_sheet();
        root.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
        root.wires.push(wire(pt(50.0, 0.0), pt(60.0, 0.0)));
        root.labels
            .push(label("BUS", pt(0.0, 0.0), LabelType::Hierarchical));
        root.labels
            .push(label("BUS", pt(50.0, 0.0), LabelType::Hierarchical));
        add_lib(&mut root, "R");
        place(&mut root, "R1", "R", pt(0.0, 0.0));
        place(&mut root, "R2", "R", pt(50.0, 0.0));

        let p = build_project_netlist(&root, &HashMap::new(), None);
        assert_eq!(
            p.netlist,
            build_netlist(&root),
            "dedup keeps both paths byte-identical"
        );
        let ns = names(&p.netlist);
        assert!(ns.contains(&"BUS"), "one keeps the bare name: {ns:?}");
        assert!(
            ns.iter().any(|n| n.starts_with("BUS_")),
            "the other is suffixed: {ns:?}"
        );
        assert!(
            p.issues.contains(&StitchIssue::NameCollision {
                name: "BUS".to_string()
            }),
            "collision reported: {:?}",
            p.issues
        );
    }

    // 11 ── Determinism: children-map insertion order never changes the output.
    #[test]
    fn output_is_deterministic_across_map_order() {
        let build = |forward: bool| {
            let mut root = empty_sheet();
            root.labels
                .push(label("NET5V", pt(0.0, 0.0), LabelType::Global));
            root.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
            add_lib(&mut root, "R");
            place(&mut root, "R1", "R", pt(10.0, 0.0));
            root.child_sheets
                .push(child_sheet("A", "a.sch", Vec::new()));
            root.child_sheets
                .push(child_sheet("B", "b.sch", Vec::new()));

            let mk = |name: &str| {
                let mut c = empty_sheet();
                c.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
                c.labels.push(label(name, pt(0.0, 0.0), LabelType::Global));
                add_lib(&mut c, "R");
                place(&mut c, "R2", "R", pt(10.0, 0.0));
                c
            };
            let mut map = HashMap::new();
            if forward {
                map.insert("a.sch".to_string(), mk("NET5V"));
                map.insert("b.sch".to_string(), mk("OTHER"));
            } else {
                map.insert("b.sch".to_string(), mk("OTHER"));
                map.insert("a.sch".to_string(), mk("NET5V"));
            }
            build_project_netlist(&root, &map, None)
        };
        assert_eq!(build(true), build(false));
    }
}
