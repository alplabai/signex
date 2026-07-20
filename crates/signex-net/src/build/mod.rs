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
//! semantics. Single-sheet same-name label *merging* is applied here, by
//! [`SheetConnectivity::merge_named_labels`], and every consumer of that type
//! is expected to apply it too. Only cross-sheet stitching lives elsewhere
//! (the project-level stitcher) and is deferred to a later increment.

use std::collections::{HashMap, HashSet};

use signex_types::designator::compare_references;
use signex_types::net::{Net, NetId, Netlist, Terminal};
use signex_types::schematic::{Label, LabelType, Point, SchematicSheet, SymbolTransform};
use uuid::Uuid;

use crate::uf::{Key, find as uf_find, union as uf_union};

/// 1 µm integer bucket — the union-find key space and the single definition of
/// "same point" for the whole derivation (D5.5). `pub` so `signex-erc`'s rules
/// can compare positions with the exact same metric instead of hand-rolling a
/// second float-epsilon `same()` (issue #388).
pub fn pt_key(p: &Point) -> Key {
    ((p.x * 1000.0).round() as i64, (p.y * 1000.0).round() as i64)
}

/// True when `p` lies on segment `a`–`b` (endpoints included) in the integer
/// key space. A zero cross-product (computed in `i128` so large micron
/// coordinates can't overflow) plus a bounding-box containment check.
///
/// Collinearity is **exact** in the 1 µm bucket space (D5.5): `p` must sit
/// precisely on the integer line through `a`–`b`. For axis-aligned wires — the
/// overwhelming majority — every on-wire bucket is exactly collinear, so this is
/// tight. A point geometrically on a *diagonal* wire can round just off that
/// line and be rejected; we deliberately do **not** widen to a ±1-bucket band
/// here, because a band would also glue near-miss points that are not really on
/// the wire. The real fix is exact integer-nanometre coordinates (the schematic
/// model still stores `f64` mm); that migration is the future coordinate ADR's
/// job, and until then exact collinearity is the safe, deterministic rule.
///
/// `pub` so callers outside this crate (`signex-erc`'s rules) can anchor a
/// point to a wire's interior the same way [`merged_sheet_parent`] anchors
/// labels, instead of re-deriving an endpoint-only approximation that
/// disagrees with the netlist on mid-wire taps (issue #388).
pub fn point_on_segment(p: Key, a: Key, b: Key) -> bool {
    let cross =
        (b.0 - a.0) as i128 * (p.1 - a.1) as i128 - (b.1 - a.1) as i128 * (p.0 - a.0) as i128;
    if cross != 0 {
        return false;
    }
    let within_x = p.0 >= a.0.min(b.0) && p.0 <= a.0.max(b.0);
    let within_y = p.1 >= a.1.min(b.1) && p.1 <= a.1.max(b.1);
    within_x && within_y
}

/// Attach a floating point to the wire it sits on, in the exact 1 µm key space.
///
/// Anchoring **attaches** a point to a segment; it never **asserts** a
/// connection between two segments — only a junction dot does that (issue
/// #107). Two rules keep that promise, and together they make the result a
/// function of the geometry alone rather than of the order `segments` happens
/// to arrive in (issue #402):
///
/// 1. If `pk` is already an endpoint of *any* segment, do nothing. It is
///    already a node of that segment's class (every caller's `parent` has the
///    segment endpoints unioned first), so unioning it into some *other*
///    segment would merge two electrically separate wires — exactly the
///    junction-less T that #107 says must stay disconnected.
/// 2. Otherwise `pk` taps one or more segment interiors. Anchor to exactly
///    **one**, chosen by a total order over the whole slice (smallest
///    normalised endpoint-key pair). Folding in a second candidate would
///    bridge two wires merely crossing at `pk`.
///
/// The old inline form — union into the first matching segment, then `break` —
/// broke both: at a junction-less T it bridged or not depending purely on which
/// wire the slice yielded first.
pub(crate) fn anchor_point(parent: &mut HashMap<Key, Key>, pk: Key, segments: &[(Point, Point)]) {
    let keyed = || segments.iter().map(|(a, b)| (pt_key(a), pt_key(b)));

    if keyed().any(|(ak, bk)| pk == ak || pk == bk) {
        return;
    }

    let chosen = keyed()
        .filter(|&(ak, bk)| point_on_segment(pk, ak, bk))
        .min_by_key(|&(ak, bk)| (ak.min(bk), ak.max(bk)));
    if let Some((ak, bk)) = chosen {
        // Either endpoint identifies the same class; take the smaller one so
        // the union argument itself carries no order dependence.
        uf_union(parent, pk, ak.min(bk));
    }
}

/// True when a wire endpoint, junction, label, or no-connect marker sits at
/// `pos`, compared in the 1 µm key space — the *same* "same point" definition
/// the union-find uses, so the connectivity gate and the net partition can
/// never disagree (D5.5). A pin is a terminal only if something lands on its
/// world-space tip.
///
/// Junctions count: a pin may tap a wire mid-span where a junction sits (D5.3).
/// Buses do **not**: a bus is a member bundle, not a single net, and the
/// union-find never merges buses, so gating on a bus endpoint used to mint a
/// one-terminal phantom net (D5.4). Direct bus-pin connectivity (through bus
/// entries) is deliberately out of scope here.
fn point_is_connected(pos: &Point, sheet: &SchematicSheet) -> bool {
    let k = pt_key(pos);
    sheet
        .wires
        .iter()
        .any(|w| pt_key(&w.start) == k || pt_key(&w.end) == k)
        || sheet.junctions.iter().any(|j| pt_key(&j.position) == k)
        || sheet.labels.iter().any(|l| pt_key(&l.position) == k)
        || sheet.no_connects.iter().any(|nc| pt_key(&nc.position) == k)
}

/// Selection priority of a label kind for naming a net: `Global > Power >
/// Hierarchical > Net`.
pub(crate) fn label_priority(kind: LabelType) -> u8 {
    match kind {
        LabelType::Global => 3,
        LabelType::Power => 2,
        LabelType::Hierarchical => 1,
        LabelType::Net => 0,
    }
}

/// The highest-priority name for a net from its labels plus any power-port
/// carrier values (which rank as `Power`, priority 2). Returns
/// `(priority, text)`, or `None` when nothing names the net. Ties resolve to
/// the last candidate in (label-document order, then power-carrier order),
/// matching the previous `max_by_key` tie-break — so label-only nets keep their
/// exact names and the single-sheet equivalence gate holds.
pub(crate) fn best_net_name(labels: &[&Label], power_values: &[&str]) -> Option<(u8, String)> {
    let mut best: Option<(u8, String)> = None;
    for l in labels {
        if l.text.is_empty() {
            continue;
        }
        let p = label_priority(l.label_type);
        if best.as_ref().is_none_or(|(bp, _)| p >= *bp) {
            best = Some((p, l.text.clone()));
        }
    }
    for v in power_values {
        if v.is_empty() {
            continue;
        }
        if best.as_ref().is_none_or(|(bp, _)| 2 >= *bp) {
            best = Some((2, (*v).to_string()));
        }
    }
    best
}

/// For each power-port symbol (`is_power && !value.is_empty()`) on the sheet,
/// the net root each connected pin lands on, tagged with the port's `value`
/// (its global net name). Power ports are implicit global name carriers — ERC
/// already reads them so, while `build_netlist` used to ignore them; cross-sheet
/// supply rails and same-sheet power-only nets both depend on this. `parent`
/// must already be fully merged ([`merged_sheet_parent`]).
pub(crate) fn power_name_carriers(
    sheet: &SchematicSheet,
    parent: &mut HashMap<Key, Key>,
) -> Vec<(Key, String)> {
    let mut carriers = Vec::new();
    for sym in &sheet.symbols {
        if !sym.is_power || sym.value.is_empty() {
            continue;
        }
        let Some(lib_sym) = sheet.lib_symbols.get(&sym.lib_id) else {
            continue;
        };
        let xform = SymbolTransform::from_symbol(sym);
        for lp in &lib_sym.pins {
            if lp.unit != 0 && lp.unit != sym.unit {
                continue;
            }
            let world_pos = xform.apply(lp.pin.position);
            let root = uf_find(parent, pt_key(&world_pos));
            carriers.push((root, sym.value.clone()));
        }
    }
    carriers
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
        let wires: Vec<(Point, Point)> = sheet.wires.iter().map(|w| (w.start, w.end)).collect();
        let junctions: Vec<Point> = sheet.junctions.iter().map(|j| j.position).collect();
        Self::from_segments(&wires, &junctions)
    }

    /// The geometry-level core of [`build`](Self::build): the same wire-endpoint
    /// union plus junction T-merge, but over raw `(start, end)` segments and
    /// junction points rather than a [`SchematicSheet`]. This is the single
    /// connectivity primitive shared across the crate boundary — the ERC context
    /// feeds its own snapshot geometry through here so it derives net membership
    /// identically instead of hand-rolling a second union-find.
    pub fn from_segments(wires: &[(Point, Point)], junctions: &[Point]) -> Self {
        let mut parent: HashMap<Key, Key> = HashMap::new();
        for (start, end) in wires {
            uf_union(&mut parent, pt_key(start), pt_key(end));
        }
        for jp in junctions {
            let jk = pt_key(jp);
            for (start, end) in wires {
                if point_on_segment(jk, pt_key(start), pt_key(end)) {
                    uf_union(&mut parent, jk, pt_key(start));
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

    /// The net root of `p` after anchoring it to any wire in `wires` whose
    /// segment it lies on — endpoint **or interior** — via
    /// [`point_on_segment`], the same anchoring [`merged_sheet_parent`]
    /// applies to labels before computing net roots. Use this (not
    /// [`root_of`](Self::root_of)) for points that are not themselves wire
    /// endpoints, such as a Net/Global/Power label sitting mid-span, so a
    /// "same net" comparison agrees with [`build_netlist`] by construction
    /// (issue #388) instead of missing the interior tap.
    ///
    /// Anchor only against the segments this connectivity was **built from**.
    /// For net connectivity — anything derived from `sheet.wires` — that means
    /// wire segments and never buses: a bus is a member bundle, not a single
    /// net, `build_netlist` never anchors labels against buses either, and
    /// anchoring to a bus interior would make the caller more lenient than the
    /// netlist (D5.4). A **bus-local** connectivity built only from bus
    /// segments (`from_segments(&buses, &[])`, as `bus_bit_width_mismatch`
    /// uses to group range labels per bundle) is the one exception: it models
    /// bundle grouping, which the netlist does not derive at all, so there is
    /// no netlist to diverge from and its own bus segments are the correct
    /// anchor (issue #395). Mixing the two — anchoring bus segments into wire
    /// connectivity — is what D5.4 forbids.
    pub fn root_of_anchored(&mut self, p: &Point, segments: &[(Point, Point)]) -> Key {
        let pk = pt_key(p);
        anchor_point(&mut self.parent, pk, segments);
        uf_find(&mut self.parent, pk)
    }

    /// The *logical* layer on top of the physical connectivity: anchor each
    /// label to the wire it sits on (endpoint or interior, via
    /// [`root_of_anchored`](Self::root_of_anchored)), then merge net roots that
    /// share a same-name label whose kind joins by name — `Global`, `Power`,
    /// local `Net`. `Hierarchical` is excluded: it binds to a parent sheet's
    /// pins, not to same-name peers, and is left to cross-sheet stitching.
    ///
    /// Labels arrive as plain `(position, kind, text)` tuples so consumers
    /// outside `signex-net` — which hold their own snapshot types, not a
    /// [`SchematicSheet`] — apply the *same* merge instead of re-deriving a
    /// geometry-only copy that reports more nets than [`build_netlist`] does
    /// (issues #388, #396, #404).
    ///
    /// Anchoring and merging are both unions and [`uf_union`] re-`find`s its
    /// operands, so a representative re-pointed by a later merge never leaks a
    /// stale root. Callers must still read every root *after* this returns.
    pub fn merge_named_labels<'a>(
        &mut self,
        labels: impl IntoIterator<Item = (Point, LabelType, &'a str)>,
        segments: &[(Point, Point)],
    ) {
        let mut name_root: HashMap<&'a str, Key> = HashMap::new();
        for (pos, kind, text) in labels {
            let root = self.root_of_anchored(&pos, segments);
            if text.is_empty()
                || !matches!(kind, LabelType::Global | LabelType::Power | LabelType::Net)
            {
                continue;
            }
            match name_root.get(text) {
                Some(&existing) => uf_union(&mut self.parent, root, existing),
                None => {
                    name_root.insert(text, root);
                }
            }
        }
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
/// `sheet`. Uses the same [`merged_connectivity`] as [`build_netlist`], so the
/// highlight follows the real net exactly — it can neither bleed across nets
/// (the old 0.01 mm-bucket over-merge) nor miss a T-junction the way the app's
/// previous inline union-find did, and it paints *every* wire on the net,
/// including a physically disjoint one joined only by a same-name label
/// (issue #404 — a physical-only flood contradicted the netlist it claims to
/// colour).
pub fn flood_net_elements(sheet: &SchematicSheet, target_wire: Uuid) -> Option<FloodElements> {
    let target = sheet.wires.iter().find(|w| w.uuid == target_wire)?;
    let mut conn = merged_connectivity(sheet);
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
    merged_connectivity(sheet).parent
}

/// The whole per-sheet topology a consumer must read to agree with
/// [`build_netlist`]: the physical [`SheetConnectivity`] core (wire endpoints +
/// junction T-merges) **plus** the logical same-name label merge on top. Every
/// on-sheet consumer — the netlist, the net-colour flood, and ERC across the
/// crate boundary — goes through this pair; deriving only the physical half is
/// the #404 defect (a consumer reports more nets than the netlist has).
///
/// (`Global` and `Power` also join *across* sheets by name — the cross-sheet
/// stitcher's job; here we only ever see one sheet.)
///
/// Wire-order independence (#402/#420): every union routes through the canonical
/// min-root `uf_union`, and the junction-less T-junction anchor is picked by
/// `anchor_point`'s integer-key `min_by_key` — so a T merges the same way
/// regardless of `sheet.wires` order, and the fix reaches every consumer through
/// this single derivation (ADR-0002 D2/D5.3).
fn merged_connectivity(sheet: &SchematicSheet) -> SheetConnectivity {
    let mut conn = SheetConnectivity::build(sheet);
    let segments: Vec<(Point, Point)> = sheet.wires.iter().map(|w| (w.start, w.end)).collect();
    conn.merge_named_labels(
        sheet
            .labels
            .iter()
            .map(|l| (l.position, l.label_type, l.text.as_str())),
        &segments,
    );
    conn
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
                symbol: sym.uuid,
                reference: sym.reference.clone(),
                pin,
            });
        }
    }
    net_terms
}

/// Group each wire and junction uuid under its net root — the geometric
/// membership of the net (what the net-flood highlights and the ratsnest read).
/// Wires and junctions are kept in document order for determinism. `parent`
/// must already be fully merged ([`merged_sheet_parent`]).
pub(crate) fn collect_membership(
    sheet: &SchematicSheet,
    parent: &mut HashMap<Key, Key>,
) -> HashMap<Key, (Vec<Uuid>, Vec<Uuid>)> {
    let mut m: HashMap<Key, (Vec<Uuid>, Vec<Uuid>)> = HashMap::new();
    for w in &sheet.wires {
        let root = uf_find(parent, pt_key(&w.start));
        m.entry(root).or_default().0.push(w.uuid);
    }
    for j in &sheet.junctions {
        let root = uf_find(parent, pt_key(&j.position));
        m.entry(root).or_default().1.push(j.uuid);
    }
    m
}

/// Enforce net-name uniqueness in net order: the first net to claim a name
/// keeps it; any later net with the same name is renamed with a deterministic
/// `_<n>` suffix. This closes two ambiguities a bare projection leaves — two
/// electrically distinct nets sharing a label (e.g. two non-merging
/// `Hierarchical` labels of one name on a sheet), and an auto `N$k` colliding
/// with a user label spelt `N$k`.
///
/// The suffix starts at the net's own `id` (stable across builds) and only
/// bumps in the pathological case a user label already occupies it, so the
/// result is deterministic and itself collision-free. Both entry points —
/// [`build_netlist`] and the cross-sheet stitcher — run this same pass, so the
/// names they emit agree. Returns each base name that collided, in net order,
/// for callers that surface it (the stitcher's `NameCollision`); single-sheet
/// callers discard it.
pub(crate) fn dedup_net_names(nets: &mut [Net]) -> Vec<String> {
    let mut used: HashSet<String> = HashSet::with_capacity(nets.len());
    let mut collided: Vec<String> = Vec::new();
    let mut reported: HashSet<String> = HashSet::new();
    for net in nets.iter_mut() {
        if used.insert(net.name.clone()) {
            continue;
        }
        if reported.insert(net.name.clone()) {
            collided.push(net.name.clone());
        }
        let base = net.name.clone();
        let mut n = net.id.0;
        let mut candidate = format!("{base}_{n}");
        while !used.insert(candidate.clone()) {
            n += 1;
            candidate = format!("{base}_{n}");
        }
        net.name = candidate;
    }
    collided
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
    // Power-port symbols carry their net's global name like a `Power` label, so
    // a `GND` port names its net even without a `GND` label. Group by root.
    let mut power_by_root: HashMap<Key, Vec<String>> = HashMap::new();
    for (root, value) in power_name_carriers(sheet, &mut parent) {
        power_by_root.entry(root).or_default().push(value);
    }
    let mut membership = collect_membership(sheet, &mut parent);
    let mut net_terms = collect_terminals(sheet, &mut parent);

    // A net exists wherever at least one terminal lands. A label with no pins
    // is a dangling label — it carries no connectivity, so it forms no net.
    //
    // `NetId` is the position in this sorted root order, and an unlabelled net
    // is named `N$<id>` from it. The root is the minimum key of its class
    // (`uf::union` keeps the smaller), so the order is a pure function of the
    // partition — but it is still *geometry*-derived: move a wire and the ids
    // renumber. `net_name` is a persisted PCB field (on pads, tracks, zones),
    // so a future schematic → PCB net sync MUST match by terminal set, never
    // by these auto-generated names (issue #402).
    let mut roots: Vec<Key> = net_terms.keys().copied().collect();
    roots.sort_unstable();

    let mut nets: Vec<Net> = roots
        .into_iter()
        .enumerate()
        .map(|(idx, root)| {
            let id = NetId(idx as u32 + 1);
            let labels = net_labels.get(&root).map(Vec::as_slice).unwrap_or(&[]);
            let power_vals: Vec<&str> = power_by_root
                .get(&root)
                .map(|v| v.iter().map(String::as_str).collect())
                .unwrap_or_default();
            let selected = best_net_name(labels, &power_vals);
            let name = selected
                .map(|(_, t)| t)
                .unwrap_or_else(|| format!("N${}", id.0));

            let (wires, junctions) = membership.remove(&root).unwrap_or_default();
            let mut terminals = net_terms.remove(&root).unwrap_or_default();
            terminals.sort_by(|a, b| {
                compare_references(&a.reference, &b.reference)
                    .then_with(|| compare_references(&a.pin, &b.pin))
            });

            Net {
                id,
                name,
                class: None,
                wires,
                junctions,
                terminals,
            }
        })
        .collect();

    // Two distinct nets may still carry the same name (e.g. same-name
    // `Hierarchical` labels that don't merge on one sheet, or an auto `N$k`
    // meeting a user label of that spelling). Rename the later one.
    dedup_net_names(&mut nets);

    Netlist { nets }
}

#[cfg(test)]
mod tests;
