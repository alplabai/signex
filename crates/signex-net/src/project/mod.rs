//! Cross-sheet netlist stitching — [`build_project_netlist`] (ADR-0002 D8,
//! A3.1 increment 2c; re-keyed by resolved path in #466).
//!
//! A schematic project is a [`ProjectGraph`]: every sheet keyed by an opaque,
//! caller-resolved [`SheetKey`], plus a per-parent resolution map from each
//! `ChildSheet.filename` string to the child's key. The app resolves every
//! reference against its own parent's directory before handing this crate the
//! graph — this crate never joins paths, normalizes separators, or folds case;
//! it only compares keys. Which host decisions the caller applies (and where
//! it stops) is the caller's business, documented at its own key-assembly
//! step; nothing here depends on them. This module walks the hierarchy from
//! `graph.roots` and derives one [`Netlist`] for the whole design, on top of
//! the same per-sheet analysis [`build_netlist`](crate::build_netlist) uses —
//! so one root with an empty `resolved` map is byte-identical to
//! `build_netlist(root)`.
//!
//! **Multi-root / flat-stitch traversal (#430):** `graph.roots` is walked as
//! ordered by `root_order::order_roots`, each root an independent top-level
//! page — own subtree, own cycle-detection path, own name-chain seed; a peer
//! of the first root, not nested under it. A root already reached as an
//! earlier root's child is skipped rather than walked twice, so a declared
//! page that another page also references contributes exactly one occurrence
//! whichever way the two are named (#540). This is what lets a flat,
//! multi-page project — several sibling sheets, none referencing any other,
//! which is what `Add Existing Sheet` produces — stitch into one netlist
//! instead of leaving every page but the first out of it.
//!
//! Two-level union-find: **level 1** is the per-sheet derivation (wires,
//! junctions, on-sheet label merge) plus sheet-pin anchoring; **level 2** joins
//! the resulting per-occurrence net roots across the project by three rules —
//! same-name Global/Power labels, power-port symbols as global name carriers,
//! and sheet-pin ↔ child-label binding. These rules run over *every*
//! occurrence regardless of which root's subtree it came from, so cross-page
//! merging for a flat project falls out of the existing machinery unchanged —
//! no new net model. Structural problems are reported as [`StitchIssue`]s
//! in-band; the netlist is always produced, deterministically.

use std::collections::{HashMap, HashSet};

use signex_types::designator::compare_references;
use signex_types::net::{Net, NetId, Netlist, Terminal};
use signex_types::schematic::{
    InstancePath, Label, LabelType, Point, SchematicSheet, normalize_port_name,
};
use uuid::Uuid;

use crate::build::{
    POWER_PORT_PRIORITY, anchor_point, collect_membership, collect_net_labels, collect_terminals,
    dedup_net_names, label_priority, merged_sheet_parent, name_outranks, power_name_carriers,
    pt_key,
};
use crate::uf::{Key, find as uf_find, union as uf_union};

mod buses;
mod designators;
mod names;
mod root_order;

pub use names::{MultiNamedNet, NetNameSource};

pub use designators::instance_entry_designators;

/// Opaque, host-neutral identifier for one sheet within a [`ProjectGraph`] —
/// a resolved path made relative to a fixed base and normalized by the
/// caller. This crate never interprets a `SheetKey` as a path: no joining,
/// normalizing, or case-folding happens here, only comparison.
///
/// A newtype, not an alias, on purpose. The whole point of #466 is that a
/// resolved key and a raw `ChildSheet.filename` reference string are
/// different things that happen to share a representation — they sit side by
/// side in [`ProjectGraph::resolved`], and under an alias every one of those
/// call sites would still compile with the two swapped, silently resolving
/// the wrong sheet. That is the defect this module exists to remove, so the
/// distinction is carried in the type system rather than in a comment.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct SheetKey(String);

impl SheetKey {
    /// Wrap a string the caller has already resolved and normalized.
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    /// Borrow the underlying string — for display and for hashing into
    /// caller-side maps, never for path interpretation.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SheetKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// One entry point the stitcher walks the hierarchy from.
///
/// Every root starts with an **empty** name chain, so its qualifiable
/// (Hierarchical/Net) label names stay bare. For the primary root that is what
/// keeps the single-root byte-identity contract with `build_netlist`; for a
/// declared page (#430) it is the deliberate reading of what a page *is* — a
/// peer of the root rather than something nested under it, so a `VCC` on page
/// two is the same net as a `VCC` on page one.
///
/// A per-root name seed would be the knob for the other reading — qualifying a
/// page's sheet-scoped labels by its own stem, which changes exported net
/// names. Nothing asks for it, so it is not carried: add it back with the first
/// caller that wants it rather than shipping a field only a test ever sets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRoot {
    pub key: SheetKey,
}

/// Pre-resolved input to [`build_project_netlist`]. The caller (the app) owns
/// every path/host decision — resolving a `ChildSheet.filename` against its
/// parent's directory, joining, normalizing, case-folding — and hands this
/// crate opaque keys; this crate only compares them.
#[derive(Debug, Clone, Copy)]
pub struct ProjectGraph<'a> {
    /// Every sheet in the project, keyed by its own [`SheetKey`] (root
    /// included).
    pub sheets: &'a HashMap<SheetKey, SchematicSheet>,
    /// Per-parent resolution: `resolved[parent_key][cs.filename]` is the
    /// child's [`SheetKey`]. The outer key is a resolved [`SheetKey`], the
    /// inner one the raw `ChildSheet.filename` string exactly as written on
    /// that parent — the newtype is what keeps the two from being transposed.
    /// A parent absent from this map, or a `cs.filename` missing from its
    /// submap, stitches as [`StitchIssue::MissingChild`].
    pub resolved: &'a HashMap<SheetKey, HashMap<String, SheetKey>>,
    /// Entry points the stitcher walks from: the project root first, then
    /// every declared page the root's hierarchy does not reach (#430). Order
    /// decides occurrence numbering and therefore [`NetId`] assignment, so the
    /// caller must produce it deterministically.
    ///
    /// This crate reorders one case before walking: a root that reaches
    /// another root is moved ahead of it, so the page contributing the nested
    /// occurrence is never stitched twice (#540). Everything else keeps the
    /// caller's order, which is what breaks ties.
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
    /// One child file instantiated N times where placements resolve a symbol
    /// to the same designator (no instance entry tells them apart): topology
    /// is expanded per occurrence, but the refdes collide.
    SharedReferenceAcrossInstances { filename: String, reference: String },
    /// Two distinct nets resolved to the same final name; a deterministic
    /// suffix was applied.
    NameCollision { name: String },
    /// Two distinct sheets the caller loaded collapsed onto one [`SheetKey`],
    /// so only one of them is in `sheets` at all — e.g. a case-fold collision
    /// on a case-insensitive host (`A.snxsch` vs `a.snxsch`). The caller's
    /// key-assembly step decides which one survives; `dropped` contributes
    /// nothing to the netlist and would be missing without this issue.
    ///
    /// Renamed from `AmbiguousChildFilename` in #466: the old name (and its
    /// message) described a *reference* string resolving two ways under the
    /// filename-keyed model, which per-parent resolution removed. What is
    /// left is a collision between two loaded sheets over one key, which is a
    /// different thing with a different remedy — renaming rather than reusing
    /// the name keeps a consumer from silently inheriting the old story.
    SheetKeyCollision {
        /// The key both sheets assembled to, as the caller derived it — not
        /// a string the user typed anywhere.
        key: String,
        /// The sheet that kept the key and is in the netlist.
        kept: String,
        /// The sheet that lost it and is absent from the netlist entirely.
        dropped: String,
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
    /// The `ChildSheet.uuid` chain from the root to this occurrence (empty for
    /// a root page); what a symbol's per-placement designator is keyed by.
    path: InstancePath,
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
    /// Net root → power-port carrier `(value, pin tip, symbol uuid)` on it.
    power_by_root: HashMap<Key, Vec<(String, Point, Uuid)>>,
    /// Per `child_sheets` entry: each sheet pin's `(name, net root)`.
    pin_roots: Vec<Vec<(String, Key)>>,
    /// Hierarchical/Global label text → net roots carrying it (the child side
    /// of sheet-pin binding).
    port_labels: HashMap<String, Vec<Key>>,
    /// Net root → (wire uuids, junction uuids) on this sheet.
    membership: HashMap<Key, (Vec<Uuid>, Vec<Uuid>)>,
    /// Bus-typed sheet pins and Ports, with the member nets each carries
    /// (#1049).
    buses: buses::SheetBuses,
}

/// One visited sheet in the hierarchy, as the stitcher walks it: the same file
/// placed twice yields two occurrences.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetOccurrence {
    /// The sheet this occurrence was reached by.
    pub key: SheetKey,
    /// The `ChildSheet.uuid` chain from the walk root (empty for a root page).
    pub instance_path: InstancePath,
    /// The `ChildSheet.name` chain from the walk root, for display.
    pub name_chain: Vec<String>,
}

/// The occurrences [`build_project_netlist`] stitches, in its walk order
/// (pre-order, document order, roots in stitcher order). Later consumers
/// (BOM, annotation, the export context) reuse this walk instead of repeating
/// it.
pub fn project_occurrences(graph: &ProjectGraph) -> Vec<SheetOccurrence> {
    let mut issues = Vec::new();
    let (occs, _edges) = walk_occurrences(graph, &mut issues);
    occs.into_iter()
        .map(|o| SheetOccurrence {
            key: o.key,
            instance_path: o.path,
            name_chain: o.name_chain,
        })
        .collect()
}

/// Build the whole-project [`Netlist`] by stitching `graph.roots` down
/// through `graph.resolved`. See the module docs for the stitching rules —
/// and for the byte-identity contract with `build_netlist` for a single-root,
/// unresolved graph.
pub fn build_project_netlist(graph: &ProjectGraph) -> ProjectNetlist {
    stitch_project(graph, false).0
}

/// Every net of the stitched project that carries two or more different names,
/// with each name's sheet and position and the name the export uses (#1061).
/// Derived by the same stitch as [`build_project_netlist`], so the two agree.
pub fn multi_named_nets(graph: &ProjectGraph) -> Vec<MultiNamedNet> {
    stitch_project(graph, true).1
}

/// `want_names` also collects the name sources behind [`multi_named_nets`];
/// the netlist refresh passes `false` and skips that work.
fn stitch_project(graph: &ProjectGraph, want_names: bool) -> (ProjectNetlist, Vec<MultiNamedNet>) {
    let mut issues: Vec<StitchIssue> = Vec::new();

    detect_duplicate_uuids(graph.sheets, &mut issues);

    let (occs, edges) = walk_occurrences(graph, &mut issues);

    detect_shared_references(&occs, &mut issues);

    // ---- Level 1: per-occurrence analysis (all unions before any sampling) --
    let analyses: Vec<Analysis> = occs.iter().map(|o| analyze(o.sheet, &o.path)).collect();

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
            for (v, ..) in values {
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
            if let Some(child_roots) = child_a.port_labels.get(pin_name) {
                for &cr in child_roots {
                    uf_union(&mut l2, (parent_occ, *pin_root), (child_occ, cr));
                }
            }
        }
    }

    // Rule 3b (#1049): a bus-typed pin joins the same-keyed Hierarchical or
    // Global bus on its child member by member, and a Global bus joins every
    // same-keyed Global bus project-wide. A bus mints no net; each shared
    // member's net joins its counterpart.
    let mut bus_bucket: HashMap<String, L2> = HashMap::new();
    for &(parent_occ, cs_index, child_occ) in &edges {
        let child_ports = &analyses[child_occ].buses.ports;
        for pin in &analyses[parent_occ].buses.pins[cs_index] {
            for (_, port) in child_ports
                .iter()
                .filter(|(ty, p)| *ty != LabelType::Net && p.key == pin.key)
            {
                for m in buses::shared_members(pin, port) {
                    for &pr in pin.roots.get(m).into_iter().flatten() {
                        for &cr in port.roots.get(m).into_iter().flatten() {
                            uf_union(&mut l2, (parent_occ, pr), (child_occ, cr));
                        }
                    }
                }
            }
        }
    }
    for (oid, a) in analyses.iter().enumerate() {
        for (_, port) in a
            .buses
            .ports
            .iter()
            .filter(|(ty, _)| *ty == LabelType::Global)
        {
            for (m, roots) in &port.roots {
                for &root in roots {
                    let name = format!("{}\u{1}{m}", port.key);
                    bucket_join(&mut l2, &mut bus_bucket, &name, (oid, root));
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
    // Multi-named groups with no terminal: no net is exported for them, but a
    // bare wire with two labels is still a naming problem to report.
    let mut pinless = Vec::new();
    for members in groups.into_values() {
        let net = assemble_net(members, &occs, &analyses, want_names);
        if !net.terminals.is_empty() {
            raw.push(net);
        } else if let Some(report) = net.names {
            pinless.push((net.name.unwrap_or_default(), report));
        }
    }

    // Deterministic ids: sorted-root order extended by (occurrence, root).
    raw.sort_by_key(|r| r.sort_key);

    let mut multi = Vec::new();
    let mut nets: Vec<Net> = raw
        .into_iter()
        .enumerate()
        .map(|(idx, r)| {
            if let Some(report) = r.names {
                multi.push((idx, report));
            }
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

    let reports = names::collect_reports(multi, pinless, &nets);
    (
        ProjectNetlist {
            netlist: Netlist { nets },
            issues,
        },
        reports,
    )
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
    /// Every name source, when the net has two or more different names.
    names: Option<names::NameReport>,
}

/// Assemble one final net from its level-2 member nodes. A group with no
/// terminals (a dangling label/pin) forms no net: the caller drops it, keeping
/// only its name sources.
fn assemble_net(
    mut members: Vec<L2>,
    occs: &[Occ],
    analyses: &[Analysis],
    want_names: bool,
) -> RawNet {
    members.sort_unstable();

    // Terminals across all members.
    let mut terminals: Vec<Terminal> = Vec::new();
    for &(oid, root) in &members {
        if let Some(ts) = analyses[oid].terminals.get(&root) {
            terminals.extend(ts.iter().cloned());
        }
    }

    // Best name across members: the same rule as `best_net_name_of`
    // (Power > Global > Hierarchical > Net), with ties broken on the fully
    // qualified text so the winner never depends on member or file order.
    let qualify = |oid: usize, qualifiable: bool, text: &str| {
        let chain = &occs[oid].name_chain;
        if qualifiable && !chain.is_empty() {
            format!("{}/{}", chain.join("/"), text)
        } else {
            text.to_string()
        }
    };
    let mut best: Option<(u8, String)> = None;
    for &(oid, root) in &members {
        if let Some(labels) = analyses[oid].net_labels.get(&root) {
            for l in labels {
                if l.text.is_empty() {
                    continue;
                }
                let p = label_priority(l.label_type);
                let q = matches!(l.label_type, LabelType::Hierarchical | LabelType::Net);
                let full = qualify(oid, q, &l.text);
                if name_outranks(p, &full, best.as_ref().map(|b| (b.0, b.1.as_str()))) {
                    best = Some((p, full));
                }
            }
        }
        if let Some(values) = analyses[oid].power_by_root.get(&root) {
            for (v, ..) in values {
                if v.is_empty() {
                    continue;
                }
                if name_outranks(
                    POWER_PORT_PRIORITY,
                    v,
                    best.as_ref().map(|b| (b.0, b.1.as_str())),
                ) {
                    best = Some((POWER_PORT_PRIORITY, v.clone()));
                }
            }
        }
    }
    let name = best.map(|(_, n)| n);

    // Wire / junction membership across every member occurrence.
    let mut wires: Vec<Uuid> = Vec::new();
    let mut junctions: Vec<Uuid> = Vec::new();
    for &(oid, root) in &members {
        if let Some((w, j)) = analyses[oid].membership.get(&root) {
            wires.extend(w.iter().copied());
            junctions.extend(j.iter().copied());
        }
    }

    let names = want_names
        .then(|| names::multi_name_sources(&members, occs, analyses, name.as_deref()))
        .flatten();
    RawNet {
        sort_key: members[0],
        terminals,
        wires,
        junctions,
        name,
        names,
    }
}

/// Level-1 analysis for one sheet: the per-sheet derivation plus sheet-pin
/// anchoring, sampled into the tables the stitcher reads.
fn analyze<'a>(sheet: &'a SchematicSheet, path: &InstancePath) -> Analysis<'a> {
    let mut parent = merged_sheet_parent(sheet);

    // Anchor every child-sheet pin to the wire it sits on (endpoint or
    // interior), like a label — before sampling any root.
    let wire_pairs: Vec<(Point, Point)> = sheet.wires.iter().map(|w| (w.start, w.end)).collect();
    for cs in &sheet.child_sheets {
        for sp in &cs.pins {
            anchor_point(&mut parent, pt_key(&sp.position), &wire_pairs);
        }
    }

    let terminals = collect_terminals(sheet, path, &mut parent);
    let net_labels = collect_net_labels(sheet, &mut parent);
    let membership = collect_membership(sheet, &mut parent);

    let mut power_by_root: HashMap<Key, Vec<(String, Point, Uuid)>> = HashMap::new();
    for (root, value, tip, id) in power_name_carriers(sheet, &mut parent) {
        power_by_root
            .entry(root)
            .or_default()
            .push((value, tip, id));
    }

    let pin_roots: Vec<Vec<(String, Key)>> = sheet
        .child_sheets
        .iter()
        .map(|cs| {
            cs.pins
                .iter()
                .filter_map(|sp| {
                    let name = normalize_port_name(&sp.name)?;
                    Some((name.to_string(), uf_find(&mut parent, pt_key(&sp.position))))
                })
                .collect()
        })
        .collect();

    let mut port_labels: HashMap<String, Vec<Key>> = HashMap::new();
    for (root, labels) in &net_labels {
        for l in labels {
            let Some(name) = l.port_name() else {
                continue;
            };
            port_labels.entry(name.to_string()).or_default().push(*root);
        }
    }

    let buses = buses::sheet_buses(sheet, &mut parent);

    Analysis {
        terminals,
        net_labels,
        power_by_root,
        pin_roots,
        port_labels,
        membership,
        buses,
    }
}

/// Walk every root top-level, recording the occurrence tree (pre-order,
/// document order) and its parent→child edges `(parent occ, cs index, child
/// occ)`. Structural issues (missing children, cycles) go to `issues`.
fn walk_occurrences<'a>(
    graph: &ProjectGraph<'a>,
    issues: &mut Vec<StitchIssue>,
) -> (Vec<Occ<'a>>, Vec<(usize, usize, usize)>) {
    // Every root is walked as its own top-level page (#430). `visited`
    // accumulates every key any walk reaches — it only ever grows, and it is
    // consulted per iteration rather than precomputed, because walking one
    // root's subtree can reach a later root: a declared page that another page
    // happens to reference should contribute one occurrence, not two. Two
    // occurrences of one sheet duplicate its terminals, raise a spurious
    // `SharedReferenceAcrossInstances`, and shift every subsequent `NetId`.
    //
    // For that skip to fire, the *referencing* page has to be walked first,
    // which the caller's own order decides for reasons unrelated to who
    // references whom. `order_roots` is what puts the two in that order (#540);
    // the caller's order is kept as the tiebreak, so a flat project is walked
    // exactly as handed.
    let mut occs: Vec<Occ> = Vec::new();
    let mut edges: Vec<(usize, usize, usize)> = Vec::new(); // (parent occ, cs index, child occ)
    let mut path: Vec<SheetKey> = Vec::new();
    let mut visited: HashSet<SheetKey> = HashSet::new();
    for key in root_order::order_roots(graph) {
        if visited.contains(&key) {
            continue;
        }
        let Some(sheet) = graph.sheets.get(&key) else {
            continue;
        };
        path.clear();
        visit(
            graph,
            &key,
            sheet,
            Vec::new(),
            InstancePath::root(),
            &mut path,
            &mut occs,
            &mut edges,
            &mut visited,
            issues,
        );
    }

    (occs, edges)
}

/// DFS the hierarchy, recording occurrences, parent→child edges, and structural
/// issues (missing children, cycles).
///
/// Every key this walk reaches is inserted into `visited`, so the caller can
/// tell which of its remaining roots are already covered (#430).
#[expect(
    clippy::too_many_arguments,
    reason = "10 arguments: the hierarchy DFS threads its accumulators explicitly so the caller can inspect them (#430)"
)]
fn visit<'a>(
    graph: &ProjectGraph<'a>,
    key: &SheetKey,
    sheet: &'a SchematicSheet,
    name_chain: Vec<String>,
    inst_path: InstancePath,
    path: &mut Vec<SheetKey>,
    occs: &mut Vec<Occ<'a>>,
    edges: &mut Vec<(usize, usize, usize)>,
    visited: &mut HashSet<SheetKey>,
    issues: &mut Vec<StitchIssue>,
) -> usize {
    let my_id = occs.len();
    let parent_path = if name_chain.is_empty() {
        key.to_string()
    } else {
        name_chain.join("/")
    };
    occs.push(Occ {
        sheet,
        name_chain,
        path: inst_path,
        key: key.clone(),
    });
    path.push(key.clone());
    visited.insert(key.clone());

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
                let child_path = occs[my_id].path.child(cs.uuid);
                let cid = visit(
                    graph,
                    child_key,
                    child_sheet,
                    child_chain,
                    child_path,
                    path,
                    occs,
                    edges,
                    visited,
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

/// Report each designator that two placements of one sheet key resolve to for
/// the same symbol. Per-occurrence expansion keeps the instances electrically
/// distinct; the designators collide only when their placements carry no
/// distinguishing instance entry (or carry the same one). One issue per
/// `(key, designator)`, in sorted-key then document order.
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
        // colliding designator once, in the sheet's document order.
        let sheet = occs[occ_ids[0]].sheet;
        let mut seen: HashSet<&str> = HashSet::new();
        for sym in &sheet.symbols {
            let mut resolved: HashSet<&str> = HashSet::new();
            for &oid in occ_ids {
                let designator = sym.designator_in(&occs[oid].path);
                if designator.is_empty() {
                    continue;
                }
                if !resolved.insert(designator) && seen.insert(designator) {
                    issues.push(StitchIssue::SharedReferenceAcrossInstances {
                        filename: key.to_string(),
                        reference: designator.to_string(),
                    });
                }
            }
        }
    }
}

/// Report every pair of sheets that share a schematic uuid — copy-as-template
/// corruption. Sheet identity is the [`SheetKey`], never the uuid; every sheet
/// in the graph is checked uniformly, root included, since the root is just
/// another entry in `sheets`.
///
/// The scope is deliberately the **whole graph**, not just the sheets reachable
/// from the root (#537): since #430 a declared project page outside the
/// hierarchy is stitched as its own top-level root, so a uuid it shares with
/// any other sheet is the same corruption and must be reported.
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
                filename_b: key.to_string(),
            }),
            None => {
                by_uuid.insert(uuid, key);
            }
        }
    }
}

#[cfg(test)]
mod tests;
