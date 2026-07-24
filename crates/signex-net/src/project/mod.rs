//! Cross-sheet netlist stitching — [`build_project_netlist`] (ADR-0002 D8,
//! A3.1 increment 2c; re-keyed by resolved path in #466).
//!
//! A schematic project is a [`ProjectGraph`]: every sheet keyed by an opaque,
//! caller-resolved [`SheetKey`], plus a per-parent resolution map from each
//! `ChildSheet.filename` string to the child's key. The app resolves every
//! reference against its own parent's directory before handing this crate the
//! graph — this crate never joins paths, normalizes separators, or folds case;
//! it only compares keys. This module walks the hierarchy from `graph.roots`
//! and derives one [`Netlist`] for the whole design, on top of the same
//! per-sheet analysis [`build_netlist`](crate::build_netlist) uses — so one
//! root with an empty `resolved` map is byte-identical to `build_netlist(root)`.
//!
//! Two-level union-find: **level 1** is the per-sheet derivation (wires,
//! junctions, on-sheet label merge) plus sheet-pin anchoring; **level 2** joins
//! the resulting per-occurrence net roots across the project by three rules —
//! same-name Global/Power labels, power-port symbols as global name carriers,
//! and sheet-pin ↔ child-label binding. Structural problems are reported as
//! [`StitchIssue`]s in-band; the netlist is always produced, deterministically.

use std::collections::{HashMap, HashSet};

use signex_types::designator::compare_references;
use signex_types::net::{Net, NetId, Netlist, Terminal};
use signex_types::schematic::{Label, LabelType, Point, SchematicSheet};
use uuid::Uuid;

use crate::build::{
    anchor_point, collect_membership, collect_net_labels, collect_terminals, dedup_net_names,
    label_priority, merged_sheet_parent, power_name_carriers, pt_key,
};
use crate::uf::{Key, find as uf_find, union as uf_union};

/// Opaque, host-neutral identifier for one sheet within a [`ProjectGraph`] —
/// a resolved path made relative to a fixed base and normalized by the
/// caller (separators, case-folding on a case-insensitive host). This crate
/// never interprets a `SheetKey` as a path: no joining, normalizing, or
/// case-folding happens here, only string comparison.
pub type SheetKey = String;

/// One entry point the stitcher walks the hierarchy from.
pub struct ProjectRoot {
    pub key: SheetKey,
    /// Name-chain seed for this root's qualifiable (Hierarchical/Net) label
    /// names: `None` for the primary root — names stay bare, which is what
    /// keeps the single-root byte-identity contract with `build_netlist`.
    /// `Some(page stem as declared)` seeds a flat page's own chain (#430).
    pub name: Option<String>,
}

/// Pre-resolved input to [`build_project_netlist`]. The caller (the app) owns
/// every path/host decision — resolving a `ChildSheet.filename` against its
/// parent's directory, joining, normalizing, case-folding — and hands this
/// crate opaque keys; this crate only compares them.
pub struct ProjectGraph<'a> {
    /// Every sheet in the project, keyed by its own [`SheetKey`] (root
    /// included).
    pub sheets: &'a HashMap<SheetKey, SchematicSheet>,
    /// Per-parent resolution: `resolved[parent_key][cs.filename]` is the
    /// child's [`SheetKey`]. A parent absent from this map, or a
    /// `cs.filename` missing from its submap, stitches as
    /// [`StitchIssue::MissingChild`].
    pub resolved: &'a HashMap<SheetKey, HashMap<String, SheetKey>>,
    /// Entry points the stitcher walks from, in order. PR1 (#466) always
    /// passes exactly one (the project root); #430 will add the project's
    /// other declared pages here.
    pub roots: &'a [ProjectRoot],
}

/// A structural problem found while stitching. The netlist is still produced
/// (best-effort, deterministic); issues tell consumers where it is degraded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StitchIssue {
    /// A `ChildSheet` names a reference with no entry in the parent's
    /// resolution map, or one that resolves to a key absent from `sheets`.
    MissingChild {
        parent_path: String,
        sheet_name: String,
        filename: String,
    },
    /// A child reference closes a cycle (its resolved key is already on the
    /// DFS path); the edge is not stitched.
    SheetCycle {
        parent_path: String,
        filename: String,
    },
    /// Two different sheets carry the same schematic uuid (copy-as-template).
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
    /// Two distinct sheets the caller loaded collapsed onto the same
    /// [`SheetKey`] — e.g. a case-fold collision on a case-insensitive host
    /// (`A.snxsch` vs `a.snxsch`). The caller's key-assembly step keeps one;
    /// the other's subtree never made it into `sheets` and is silently
    /// missing without this issue.
    AmbiguousChildFilename {
        filename: String,
        path_a: String,
        path_b: String,
    },
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
    /// The [`SheetKey`] this occurrence was reached by (a root's own key, or
    /// the child's resolved key). Occurrences that share a key are instances
    /// of one sheet.
    key: SheetKey,
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

/// Build the whole-project [`Netlist`] by stitching `graph.roots` down
/// through `graph.resolved`. See the module docs for the stitching rules —
/// and for the byte-identity contract with `build_netlist` for a single-root,
/// unresolved graph.
pub fn build_project_netlist(graph: &ProjectGraph) -> ProjectNetlist {
    let mut issues: Vec<StitchIssue> = Vec::new();

    detect_duplicate_uuids(graph.sheets, &mut issues);

    // ---- Traverse: build the occurrence tree (pre-order, document order) ----
    // PR1 (#466) always hands exactly one root; the double-visit skip #430
    // needs for a declared page that's also reached as a child is not wired
    // here yet — deferred to that PR alongside the multi-root call sites.
    let mut occs: Vec<Occ> = Vec::new();
    let mut edges: Vec<(usize, usize, usize)> = Vec::new(); // (parent occ, cs index, child occ)
    let mut path: Vec<SheetKey> = Vec::new();
    for root in graph.roots {
        let Some(sheet) = graph.sheets.get(&root.key) else {
            continue;
        };
        let name_chain: Vec<String> = root.name.iter().cloned().collect();
        visit(
            graph,
            &root.key,
            sheet,
            name_chain,
            &mut path,
            &mut occs,
            &mut edges,
            &mut issues,
        );
    }

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
            terminals.sort_by(|a, b| {
                compare_references(&a.reference, &b.reference)
                    .then_with(|| compare_references(&a.pin, &b.pin))
            });
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
    let wire_pairs: Vec<(Point, Point)> = sheet.wires.iter().map(|w| (w.start, w.end)).collect();
    for cs in &sheet.child_sheets {
        for sp in &cs.pins {
            anchor_point(&mut parent, pt_key(&sp.position), &wire_pairs);
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
    graph: &ProjectGraph<'a>,
    key: &SheetKey,
    sheet: &'a SchematicSheet,
    name_chain: Vec<String>,
    path: &mut Vec<SheetKey>,
    occs: &mut Vec<Occ<'a>>,
    edges: &mut Vec<(usize, usize, usize)>,
    issues: &mut Vec<StitchIssue>,
) -> usize {
    let my_id = occs.len();
    let parent_path = if name_chain.is_empty() {
        key.clone()
    } else {
        name_chain.join("/")
    };
    occs.push(Occ {
        sheet,
        name_chain,
        key: key.clone(),
    });
    path.push(key.clone());

    let submap = graph.resolved.get(key);
    for (cs_index, cs) in sheet.child_sheets.iter().enumerate() {
        let Some(child_key) = submap.and_then(|m| m.get(cs.filename.as_str())) else {
            issues.push(StitchIssue::MissingChild {
                parent_path: parent_path.clone(),
                sheet_name: cs.name.clone(),
                filename: cs.filename.clone(),
            });
            continue;
        };
        if path.contains(child_key) {
            issues.push(StitchIssue::SheetCycle {
                parent_path: parent_path.clone(),
                filename: cs.filename.clone(),
            });
            continue;
        }
        match graph.sheets.get(child_key) {
            None => issues.push(StitchIssue::MissingChild {
                parent_path: parent_path.clone(),
                sheet_name: cs.name.clone(),
                filename: cs.filename.clone(),
            }),
            Some(child_sheet) => {
                let mut child_chain = occs[my_id].name_chain.clone();
                child_chain.push(cs.name.clone());
                let cid = visit(
                    graph,
                    child_key,
                    child_sheet,
                    child_chain,
                    path,
                    occs,
                    edges,
                    issues,
                );
                edges.push((my_id, cs_index, cid));
            }
        }
    }

    path.pop();
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

/// Report each reference designator carried by a sheet key instantiated more
/// than once: per-occurrence expansion keeps the instances electrically
/// distinct, but the refdes collide until per-instance annotation exists. One
/// issue per `(key, reference)`, in sorted-key then document order.
fn detect_shared_references(occs: &[Occ], issues: &mut Vec<StitchIssue>) {
    let mut by_key: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, o) in occs.iter().enumerate() {
        by_key.entry(o.key.as_str()).or_default().push(i);
    }
    let mut keys: Vec<&str> = by_key.keys().copied().collect();
    keys.sort_unstable();
    for key in keys {
        let occ_ids = &by_key[key];
        if occ_ids.len() < 2 {
            continue;
        }
        // Instances of one sheet share the same symbols; report each
        // reference once, in the sheet's document order.
        let sheet = occs[occ_ids[0]].sheet;
        let mut seen: HashSet<&str> = HashSet::new();
        for sym in &sheet.symbols {
            if sym.reference.is_empty() {
                continue;
            }
            if seen.insert(sym.reference.as_str()) {
                issues.push(StitchIssue::SharedReferenceAcrossInstances {
                    filename: key.to_string(),
                    reference: sym.reference.clone(),
                });
            }
        }
    }
}

/// Report every pair of sheets that share a schematic uuid — copy-as-template
/// corruption. Sheet identity is the [`SheetKey`], never the uuid; every sheet
/// in the graph is checked uniformly, root included, since the root is just
/// another entry in `sheets`.
fn detect_duplicate_uuids(
    sheets: &HashMap<SheetKey, SchematicSheet>,
    issues: &mut Vec<StitchIssue>,
) {
    let mut by_uuid: HashMap<Uuid, &SheetKey> = HashMap::new();
    let mut keys: Vec<&SheetKey> = sheets.keys().collect();
    keys.sort();
    for key in keys {
        let uuid = sheets[key].uuid;
        if uuid == Uuid::nil() {
            continue;
        }
        match by_uuid.get(&uuid) {
            Some(prev) => issues.push(StitchIssue::DuplicateSheetUuid {
                filename_a: prev.to_string(),
                filename_b: key.clone(),
            }),
            None => {
                by_uuid.insert(uuid, key);
            }
        }
    }
}

#[cfg(test)]
mod tests;
