//! Built-in rule implementations. Each function takes a read-only
//! [`ErcContext`] and pushes [`Diagnostic`]s onto the accumulator.
//! No render or parser imports — all geometry is already world-space inside
//! the context.
//!
//! This file is already past the size a module should reach, so new rules land
//! in their own sibling file rather than growing it further.

mod ambiguous_label_anchor;
pub(crate) use ambiguous_label_anchor::ambiguous_label_anchor;
mod bus;
pub(crate) use bus::{
    bus_analysis, bus_bit_width_mismatch, bus_entry_unlabeled, bus_label_on_wire,
    bus_member_not_in_bus, bus_name_conflict, bus_unnamed,
};
mod bus_hier;
mod bus_runs;
pub(crate) use bus_hier::{bus_port_width_mismatch, global_bus_width_mismatch};
pub(crate) use bus_runs::bus_runs_width_mismatch;
mod clash_scope;
mod completeness;
pub(crate) use completeness::{
    part_without_footprint, part_without_mpn, part_without_value, unannotated_designator,
};
mod local_label_other_sheets;
pub(crate) use local_label_other_sheets::local_label_name_on_other_sheets;
mod multiple_names;
pub(crate) use multiple_names::net_multiple_names;
mod net_completeness;
mod net_members;
pub(crate) use net_completeness::net_completeness;
mod off_grid;
pub(crate) use off_grid::off_grid;
mod units_not_placed;
pub(crate) use units_not_placed::{
    PlacedUnit, finding as units_finding, missing_units, units_not_placed,
};
mod pin_type_conflict;
pub(crate) use pin_type_conflict::pin_type_conflict;
mod pins_touch_not_connected;
pub(crate) use pins_touch_not_connected::pins_touch_not_connected;

use std::collections::HashMap;

use signex_net::{SegmentIndex, SheetConnectivity, pt_key};
use signex_types::schematic::{
    LabelType, PartKey, PinDirection, Point, SelectedKind, group_into_parts, normalize_port_name,
    normalized_lib_id,
};

use crate::context::ErcContext;
use crate::diagnostic::{Diagnostic, LocationSpace};
use crate::{RuleKind, Severity, sel};

// ---------------------------------------------------------------------------
// Geometry helpers
// ---------------------------------------------------------------------------

const ENDPOINT_EPS: f64 = 1e-4;

fn same(a: &Point, b: &Point) -> bool {
    (a.x - b.x).abs() < ENDPOINT_EPS && (a.y - b.y).abs() < ENDPOINT_EPS
}

/// Every wire on the sheet as a `(start, end)` pair, for anchoring queries
/// against [`SheetConnectivity::root_of_anchored`] / [`on_any_wire`].
pub(super) fn wire_pairs(ctx: &ErcContext) -> Vec<(Point, Point)> {
    ctx.wires.iter().map(|w| (w.start, w.end)).collect()
}

/// Wire + junction net connectivity for the rules that need net roots — the
/// same [`SheetConnectivity`] (wire-endpoint union plus junction T-merge)
/// `build_netlist` derives, so a rule's notion of "same net" matches the
/// netlist's. Replaces the per-rule hand-rolled union-find, which also missed
/// the junction T-merge (its junction loop only `find`-ed, never `union`-ed).
fn wire_connectivity(ctx: &ErcContext) -> SheetConnectivity {
    let junctions: Vec<Point> = ctx.junctions.iter().map(|j| j.position).collect();
    SheetConnectivity::from_segments(&wire_pairs(ctx), &junctions)
}

/// True when `pos` sits on any wire's segment — endpoint **or interior** —
/// via the shared [`point_on_segment`], the same anchoring `build_netlist`
/// applies to labels (issue #388). Replaces the endpoint-only `same()` gate
/// that missed mid-wire label/pin placements.
///
/// Takes a [`SegmentIndex`] built once per rule pass (issue #1366), so a pass
/// over N labels no longer scans every wire per label.
fn on_any_wire(pos: &Point, index: &SegmentIndex) -> bool {
    index.contains(pt_key(pos))
}

/// True when `pos` sits on a bus **endpoint** — buses are member bundles, not
/// single nets, so (D5.4) they deliberately never get interior anchoring;
/// only the "same point" metric changes here, from float-epsilon `same()` to
/// the canonical 1 µm `pt_key` (D5.5).
fn on_any_bus_endpoint(pos: &Point, ctx: &ErcContext) -> bool {
    let pk = pt_key(pos);
    ctx.buses
        .iter()
        .any(|b| pt_key(&b.start) == pk || pt_key(&b.end) == pk)
}

// ---------------------------------------------------------------------------
// Rule: UnusedPin
// ---------------------------------------------------------------------------

pub(crate) fn unused_pin(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    for symbol in &ctx.symbols {
        if symbol.is_power {
            continue;
        }
        for pin in &symbol.pins {
            // `ErcPin.connected` is already the shared, junction-aware
            // connectivity gate (`signex_net::ConnectedPoints`, the
            // same gate `build_netlist` uses) — trust it instead of re-deriving an
            // endpoint-only, bus-gated approximation here (issue #388, D5.4).
            if pin.connected {
                continue;
            }
            // Do-not-connect pins are left open by design. Unclassified pins
            // (the default for freshly authored library pins) stay flagged.
            if pin.electrical_type == PinDirection::DoNotConnect {
                continue;
            }
            let reference = if symbol.reference.is_empty() {
                "(unnamed)"
            } else {
                symbol.reference.as_str()
            };
            out.push(
                Diagnostic::new(
                    RuleKind::UnusedPin,
                    format!("Pin on {reference} is not connected"),
                    pin.world_pos,
                )
                .with_primary(sel(symbol.uuid, SelectedKind::Symbol))
                .at_pin_number(pin.anchor()),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Rule: DuplicateRefDesignator
// ---------------------------------------------------------------------------

pub(crate) fn duplicate_ref_designator(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    // Symbols of this sheet (origin 0, unnamed) plus every descendant sheet's
    // (origin n, named by its filename), so a designator reused across sheets
    // is a duplicate too. Descendants are walked depth-first in filename order
    // for stable output; a file reached more than once (or the top sheet
    // reached through a cycle) is taken once, so it is never compared with
    // itself. Power/flag symbols (`is_power`, `#`-prefixed refs) are not
    // designators, matching annotate.
    let mut sources: Vec<(Option<&str>, &ErcContext)> = vec![(None, ctx)];
    let mut seen: Vec<&signex_net::SheetKey> = ctx.key.iter().collect();
    collect_descendants(ctx, &mut sources, &mut seen);
    /// `(source index, sheet name, symbol)`.
    type Entry<'a> = (usize, Option<&'a str>, &'a crate::context::ErcSymbol);
    let mut by_ref: HashMap<&str, Vec<Entry>> = HashMap::new();
    for (origin, (name, sheet)) in sources.iter().enumerate() {
        for symbol in &sheet.symbols {
            let r = symbol.reference.trim();
            if r.is_empty() || r.ends_with('?') || symbol.is_power || r.starts_with('#') {
                continue;
            }
            by_ref.entry(r).or_default().push((origin, *name, symbol));
        }
    }
    // Sorted so the output order does not depend on hash iteration, and each
    // reference's entries by (sheet, uuid) so which part is reported against
    // which (primary / peer) does not depend on document order.
    let mut by_ref: Vec<(&str, Vec<Entry>)> = by_ref.into_iter().collect();
    by_ref.sort_by(|a, b| a.0.cmp(b.0));
    for (_, entries) in &mut by_ref {
        entries.sort_by_key(|e| (e.0, e.2.uuid));
    }
    for (reference, entries) in by_ref {
        let symbols: Vec<&crate::context::ErcSymbol> = entries.iter().map(|e| e.2).collect();
        // Units of one multi-unit part share a designator legitimately; only
        // distinct parts holding the same designator are a duplicate (#691).
        let parts = group_into_parts(symbols.iter().map(|s| PartKey {
            reference: s.reference.trim(),
            lib_id: normalized_lib_id(&s.lib_id),
            unit: s.unit,
        }));
        if parts.len() < 2 {
            continue;
        }
        // A clash wholly inside one child sheet belongs to that sheet's own run.
        let first_origin = entries[0].0;
        if first_origin != 0 && entries.iter().all(|e| e.0 == first_origin) {
            continue;
        }
        // A clash among descendants only is reported by the lowest sheet
        // that sees both sides, not by every ancestor above it (#998).
        if entries.iter().all(|e| e.0 != 0) {
            let involved: Vec<&ErcContext> = entries.iter().map(|e| sources[e.0].1).collect();
            if clash_scope::confined_to_one_child(ctx, &involved) {
                continue;
            }
        }
        // Units that carry one designator but come from different library
        // symbols are far more likely a wrong symbol pick than two parts that
        // collided, so say so.
        let first_lib = normalized_lib_id(&symbols[0].lib_id);
        let hint = if symbols
            .iter()
            .any(|s| normalized_lib_id(&s.lib_id) != first_lib)
        {
            format!("; units of {reference} come from different library symbols")
        } else {
            String::new()
        };
        for (part_idx, part) in parts.iter().enumerate() {
            let (peer_origin, _, peer) = entries[parts[(part_idx + 1) % parts.len()][0]];
            for &member in part {
                let (origin, name, sym) = entries[member];
                let mut message = format!(
                    "Reference '{reference}' is used by {} parts{hint}",
                    parts.len()
                );
                if let Some(sheet) = name {
                    message.push_str(&format!("; this one is on sheet '{sheet}'"));
                }
                let mut d =
                    Diagnostic::new(RuleKind::DuplicateRefDesignator, message, sym.position);
                // Selections only resolve on this sheet; a child symbol's
                // position lives in the child's own coordinates.
                if origin == 0 {
                    d = d.with_primary(sel(sym.uuid, SelectedKind::Symbol));
                    if peer_origin == 0 {
                        d = d.with_peer(sel(peer.uuid, SelectedKind::Symbol));
                    }
                } else {
                    d = d
                        .in_space(LocationSpace::ChildSheet)
                        .owned_by(sources[origin].1.key.clone());
                }
                out.push(d);
            }
        }
    }
}

/// Depth-first, filename-ordered walk of `ctx`'s descendants into `out`,
/// skipping any sheet whose key is already in `seen` (cycles, repeated
/// instances). Sheets without a key are always taken.
fn collect_descendants<'a>(
    ctx: &'a ErcContext,
    out: &mut Vec<(Option<&'a str>, &'a ErcContext)>,
    seen: &mut Vec<&'a signex_net::SheetKey>,
) {
    let mut names: Vec<&String> = ctx.children.keys().collect();
    names.sort();
    for name in names {
        let child = &ctx.children[name];
        if let Some(k) = &child.key {
            if seen.contains(&k) {
                continue;
            }
            seen.push(k);
        }
        out.push((Some(name.as_str()), child));
        collect_descendants(child, out, seen);
    }
}

// ---------------------------------------------------------------------------
// Rule: HierPortDisconnected
// ---------------------------------------------------------------------------

pub(crate) fn hier_port_disconnected(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    let wires = SegmentIndex::new(&wire_pairs(ctx));
    for label in &ctx.labels {
        if !matches!(
            label.label_type,
            LabelType::Hierarchical | LabelType::Global
        ) {
            continue;
        }
        // Wires anchor by endpoint OR interior (a Global/Hierarchical label
        // may sit mid-span). Buses follow `bus::label_touches_bus`: a bus-name
        // label anywhere along a segment, any other label at an endpoint
        // only (D5.4).
        let touched = on_any_wire(&label.position, &wires) || bus::label_touches_bus(ctx, label);
        if touched {
            continue;
        }
        out.push(
            Diagnostic::new(
                RuleKind::HierPortDisconnected,
                format!(
                    "{:?} port '{}' is not on a wire",
                    label.label_type, label.text
                ),
                label.position,
            )
            .with_primary(sel(label.uuid, SelectedKind::Label)),
        );
    }
}

// ---------------------------------------------------------------------------
// Rule: DanglingWire
// ---------------------------------------------------------------------------

fn pt_of(x: f64, y: f64) -> Point {
    Point { x, y }
}

/// Points bucketed on an `ENDPOINT_EPS` grid so a "is anything within
/// `same()` of here" query probes 9 cells instead of scanning every element.
/// The grid only narrows candidates; the exact [`same`] test decides.
struct EpsPointIndex<T> {
    cells: HashMap<(i64, i64), Vec<(Point, T)>>,
}

impl<T: Copy> EpsPointIndex<T> {
    fn new() -> Self {
        Self {
            cells: HashMap::new(),
        }
    }

    fn cell(p: &Point) -> (i64, i64) {
        (
            (p.x / ENDPOINT_EPS).floor() as i64,
            (p.y / ENDPOINT_EPS).floor() as i64,
        )
    }

    fn insert(&mut self, p: Point, tag: T) {
        self.cells.entry(Self::cell(&p)).or_default().push((p, tag));
    }

    /// Tags of every indexed point `same()` as `p`.
    fn near<'a>(&'a self, p: &'a Point) -> impl Iterator<Item = T> + 'a {
        let (cx, cy) = Self::cell(p);
        (-1i64..=1).flat_map(move |dx| {
            (-1i64..=1).flat_map(move |dy| {
                self.cells
                    .get(&(cx.saturating_add(dx), cy.saturating_add(dy)))
                    .into_iter()
                    .flatten()
                    .filter(move |(q, _)| same(q, p))
                    .map(|&(_, t)| t)
            })
        })
    }
}

pub(crate) fn dangling_wire(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    // Everything a wire end can legitimately land on besides another wire.
    let mut non_wire: EpsPointIndex<()> = EpsPointIndex::new();
    for pin in ctx.symbols.iter().flat_map(|s| s.pins.iter()) {
        non_wire.insert(pin.world_pos, ());
    }
    for j in &ctx.junctions {
        non_wire.insert(j.position, ());
    }
    for l in &ctx.labels {
        non_wire.insert(l.position, ());
    }
    for nc in &ctx.no_connects {
        non_wire.insert(nc.position, ());
    }
    for be in &ctx.bus_entries {
        // An entry meets a wire at either end: the bus end (`position`) or
        // the far end (`position + size`).
        non_wire.insert(be.position, ());
        non_wire.insert(
            pt_of(be.position.x + be.size.0, be.position.y + be.size.1),
            (),
        );
    }
    let mut wire_ends = EpsPointIndex::new();
    for w in &ctx.wires {
        wire_ends.insert(w.start, w.uuid);
        wire_ends.insert(w.end, w.uuid);
    }

    for wire in &ctx.wires {
        for (end, endpoint) in [("start", wire.start), ("end", wire.end)] {
            let touches_other_wire = wire_ends.near(&endpoint).any(|id| id != wire.uuid);
            if touches_other_wire || non_wire.near(&endpoint).next().is_some() {
                continue;
            }
            out.push(
                Diagnostic::new(
                    RuleKind::DanglingWire,
                    "Wire endpoint is not connected",
                    endpoint,
                )
                .with_primary(sel(wire.uuid, SelectedKind::Wire))
                // Each end is its own finding: a waived stub at one end must
                // not hide a later dangle at the other.
                .tagged(end),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Rule: NetLabelConflict
// ---------------------------------------------------------------------------

/// The label groups [`net_label_conflict`] reports: each is the Net / Global /
/// Power labels of one net that carry at least two different texts, Net labels
/// first. `net_multiple_names` uses it to leave these nets to this rule.
pub(crate) fn label_conflict_groups(ctx: &ErcContext) -> Vec<Vec<&crate::context::ErcLabel>> {
    let mut conn = wire_connectivity(ctx);
    let wires = wire_pairs(ctx);

    // Merge on ALL labels, not just the Net ones we then group by. A Global or
    // Power label joins two nets whose Net labels share no text at all — wire A
    // holding `SIG1` + `VCC`, wire B holding `SIG2` + `VCC` is ONE net to
    // `build_netlist`, which silently drops both signal names in favour of the
    // higher-priority `VCC`. Grouping only the Net-filtered subset through the
    // merge never sees that join and reports nothing (issue #404).
    //
    // This also satisfies `merged_sheet_parent`'s invariant: every union —
    // anchoring and same-name merging alike — completes before any root is
    // sampled. Interleaving anchor-then-sample per label made the result
    // order-dependent, because an earlier label's cached root goes stale once a
    // later union re-roots its class (issue #388 follow-up).
    conn.merge_named_labels(
        ctx.labels
            .iter()
            .map(|l| (l.position, l.label_type, l.text.as_str())),
        &wires,
    );

    let mut by_root: HashMap<(i64, i64), Vec<&crate::context::ErcLabel>> = HashMap::new();
    for lbl in ctx.labels.iter().filter(|l| {
        matches!(
            l.label_type,
            LabelType::Net | LabelType::Global | LabelType::Power
        )
    }) {
        // `merge_named_labels` already anchored every label, so a plain
        // `root_of` reads the final class.
        let root = conn.root_of(&lbl.position);
        by_root.entry(root).or_default().push(lbl);
    }

    // Sorted by root so the output order does not depend on hash iteration.
    let mut by_root: Vec<_> = by_root.into_iter().collect();
    by_root.sort_by_key(|(root, _)| *root);
    let mut groups: Vec<Vec<&crate::context::ErcLabel>> = by_root
        .into_iter()
        .map(|(_, labels)| labels)
        .filter(|labels| labels.iter().any(|l| l.text != labels[0].text))
        .collect();
    for labels in &mut groups {
        // Report a Net-vs-Net clash in preference to one involving a
        // Global/Power name: the Net names are what the netlist drops.
        labels.sort_by_key(|l| !matches!(l.label_type, LabelType::Net));
    }
    groups
}

pub(crate) fn net_label_conflict(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    for labels in label_conflict_groups(ctx) {
        let first = labels[0];
        let Some(conflicting) = labels.iter().find(|l| l.text != first.text) else {
            continue;
        };
        out.push(
            Diagnostic::new(
                RuleKind::NetLabelConflict,
                format!(
                    "Net label conflict: '{}' and '{}' on the same net",
                    first.text, conflicting.text,
                ),
                first.position,
            )
            .with_primary(sel(first.uuid, SelectedKind::Label))
            .with_peer(sel(conflicting.uuid, SelectedKind::Label)),
        );
    }
}

// ---------------------------------------------------------------------------
// Rule: OrphanLabel
// ---------------------------------------------------------------------------

pub(crate) fn orphan_label(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    let wires = SegmentIndex::new(&wire_pairs(ctx));
    for label in &ctx.labels {
        // MD-13: Hierarchical and Global labels are handled by
        // `hier_port_disconnected`. Power labels DO need orphan
        // detection — a floating `+3V3` label is a real silent net
        // problem the user wants flagged, the same as a floating
        // local Net label.
        if matches!(
            label.label_type,
            LabelType::Hierarchical | LabelType::Global
        ) {
            continue;
        }
        // A Power/Net label at a wire interior (not just an endpoint) counts
        // as on-wire, matching `build_netlist`'s label anchoring (issue
        // #388). Buses follow `bus::label_touches_bus` (bus-name labels
        // anchor mid-span, others at an endpoint only; D5.4).
        let on_wire = on_any_wire(&label.position, &wires);
        let on_bus = bus::label_touches_bus(ctx, label);
        let on_pin = ctx
            .symbols
            .iter()
            .flat_map(|s| s.pins.iter())
            .any(|pin| pt_key(&pin.world_pos) == pt_key(&label.position));
        if on_wire || on_bus || on_pin {
            continue;
        }
        out.push(
            Diagnostic::new(
                RuleKind::OrphanLabel,
                format!("Label '{}' is not on a wire", label.text),
                label.position,
            )
            .with_primary(sel(label.uuid, SelectedKind::Label)),
        );
    }
}

// ---------------------------------------------------------------------------
// Rule: BadHierSheetPin
// ---------------------------------------------------------------------------

pub(crate) fn bad_hier_sheet_pin(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    use std::collections::HashSet;

    for child in &ctx.child_sheets {
        // Duplicate pin names on the parent sheet symbol.
        let mut seen: HashMap<&str, uuid::Uuid> = HashMap::new();
        for pin in &child.pins {
            if let Some(prev_uuid) = seen.get(pin.name.as_str()) {
                out.push(
                    Diagnostic::new(
                        RuleKind::BadHierSheetPin,
                        format!(
                            "Duplicate sheet pin '{}' on sheet '{}'",
                            pin.name, child.name,
                        ),
                        pin.position,
                    )
                    .with_primary(sel(pin.uuid, SelectedKind::SheetPin))
                    .with_peer(sel(*prev_uuid, SelectedKind::SheetPin)),
                );
            } else {
                seen.insert(pin.name.as_str(), pin.uuid);
            }
        }

        // Sheet references a child file but exposes no ports.
        if !child.filename.is_empty() && child.pins.is_empty() {
            out.push(
                Diagnostic::new(
                    RuleKind::BadHierSheetPin,
                    format!(
                        "Hierarchical sheet '{}' has no pins — the child schematic can't be wired in",
                        child.name,
                    ),
                    child.position,
                )
                .with_severity(Severity::Warning)
                .with_primary(sel(child.uuid, SelectedKind::ChildSheet))
                .tagged("no-pins"),
            );
        }

        // Cross-sheet: parent pin ↔ child hier-label matching.
        let Some(child_ctx) = ctx.children.get(child.filename.as_str()) else {
            // A referenced file that never loaded: everything below needs the
            // child, so say why the checks stop. The run log carries the same
            // fact as a project-level warning; this puts it on the sheet
            // symbol where the user can click it.
            if !child.filename.is_empty() {
                out.push(
                    Diagnostic::new(
                        RuleKind::BadHierSheetPin,
                        format!(
                            "Child schematic '{}' of sheet '{}' could not be loaded — its sheet pins were not checked",
                            child.filename, child.name,
                        ),
                        child.position,
                    )
                    .with_severity(Severity::Warning)
                    .with_primary(sel(child.uuid, SelectedKind::ChildSheet))
                    .tagged("not-loaded"),
                );
            }
            continue;
        };
        let hier_text: HashSet<&str> = child_ctx
            .labels
            .iter()
            .filter_map(|l| l.port_name())
            .collect();

        for pin in &child.pins {
            let matched = normalize_port_name(&pin.name).is_some_and(|n| {
                hier_text.contains(n) || bus_hier::has_bus_counterpart(n, hier_text.iter().copied())
            });
            if !matched {
                out.push(
                    Diagnostic::new(
                        RuleKind::BadHierSheetPin,
                        format!(
                            "Sheet pin '{}' on '{}' has no matching hierarchical label in child schematic '{}'",
                            pin.name, child.name, child.filename,
                        ),
                        pin.position,
                    )
                    .with_primary(sel(pin.uuid, SelectedKind::SheetPin)),
                );
            }
        }

        let parent_pins: HashSet<&str> = child
            .pins
            .iter()
            .filter_map(|p| normalize_port_name(&p.name))
            .collect();
        for lbl in child_ctx
            .labels
            .iter()
            .filter(|l| matches!(l.label_type, LabelType::Hierarchical))
        {
            if lbl.port_name().is_some_and(|n| {
                !parent_pins.contains(n)
                    && !bus_hier::has_bus_counterpart(n, parent_pins.iter().copied())
            }) {
                out.push(
                    Diagnostic::new(
                        RuleKind::BadHierSheetPin,
                        format!(
                            "Hierarchical label '{}' in '{}' is not exposed as a sheet pin on the parent",
                            lbl.text, child.filename,
                        ),
                        lbl.position,
                    )
                    .with_severity(Severity::Warning)
                    .in_child_sheet(child.filename.as_str())
                    .with_primary(sel(lbl.uuid, SelectedKind::Label)),
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Rule: MissingPowerFlag
// ---------------------------------------------------------------------------

pub(crate) fn missing_power_flag(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    // MD-12: verify the power port shares a NET with a same-text label, not just
    // that the text exists somewhere on the sheet (a port `+3V3` floating on its
    // own net must not be suppressed by ANY other `+3V3` label). Connectivity is
    // the shared [`SheetConnectivity`] — the same topology `build_netlist`
    // derives — which also honors T-junctions the old inline pass silently
    // dropped (its junction loop only `find`-ed, never `union`-ed).
    use std::collections::HashMap;
    let mut conn = wire_connectivity(ctx);
    let wires = wire_pairs(ctx);
    let named_labels: Vec<&crate::context::ErcLabel> =
        ctx.labels.iter().filter(|l| !l.text.is_empty()).collect();

    // Two passes, matching `merged_sheet_parent`'s invariant: every union
    // completes before any root is sampled. Anchor every label to its wire
    // interior first (issue #388) and merge the same-name ones, THEN read
    // roots — sampling interleaved with anchoring made an earlier label's
    // cached root go stale once a later label's union re-rooted its class (a
    // label at a junction-less T point), which could both miss a real conflict
    // and false-flag a port that IS cross-referenced. The same-name merge is
    // what makes the "shares a net" claim below mean what `build_netlist`
    // means by it: without it, a port whose fragment is joined to the labelled
    // fragment by a third same-name label false-positived (issue #404).
    conn.merge_named_labels(
        named_labels
            .iter()
            .map(|l| (l.position, l.label_type, l.text.as_str())),
        &wires,
    );

    // Map each label text → set of net roots its labels sit on.
    let mut label_nets: HashMap<&str, std::collections::HashSet<(i64, i64)>> = HashMap::new();
    let wire_index = SegmentIndex::new(&wires);
    for lbl in named_labels {
        let root = conn.root_of_anchored_in(&lbl.position, &wire_index);
        label_nets
            .entry(lbl.text.as_str())
            .or_default()
            .insert(root);
    }

    for symbol in &ctx.symbols {
        if !symbol.is_power {
            continue;
        }
        let name = symbol.value.as_str();
        if name.is_empty() {
            continue;
        }
        // A port connects at its pin tips, not at its origin — the same points
        // `build_netlist` names the net from (issue #675). Look each tip up
        // unanchored, like any other pin: a port only taps a wire interior
        // through an explicit junction, never by proximity alone. A port
        // projected with no pins falls back to its origin.
        let tips: Vec<Point> = if symbol.pins.is_empty() {
            vec![symbol.position]
        } else {
            symbol.pins.iter().map(|p| p.world_pos).collect()
        };
        let same_net_label = label_nets
            .get(name)
            .is_some_and(|nets| tips.iter().any(|tip| nets.contains(&conn.root_of(tip))));
        if same_net_label {
            continue;
        }
        out.push(
            Diagnostic::new(
                RuleKind::MissingPowerFlag,
                format!(
                    "Power port '{name}' is not cross-referenced by a same-net label — add a PWR_FLAG if this is a source net",
                ),
                symbol.position,
            )
            .with_primary(sel(symbol.uuid, SelectedKind::Symbol)),
        );
    }
}

// ---------------------------------------------------------------------------
// Rule: PowerPortShort
// ---------------------------------------------------------------------------

/// A port's pin tips — where it connects — falling back to its origin when
/// the projection carries no pins.
fn port_tips(symbol: &crate::context::ErcSymbol) -> Vec<Point> {
    if symbol.pins.is_empty() {
        vec![symbol.position]
    } else {
        symbol.pins.iter().map(|p| p.world_pos).collect()
    }
}

pub(crate) fn power_port_short(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    // Same net model as `label_conflict_groups`: wires, junctions and every
    // same-name label merge first, then roots are read, so a wire (or a
    // Global/Power label) joining two rails is one net here as in the netlist.
    let mut conn = wire_connectivity(ctx);
    let wires = wire_pairs(ctx);
    conn.merge_named_labels(
        ctx.labels
            .iter()
            .map(|l| (l.position, l.label_type, l.text.as_str())),
        &wires,
    );

    // Net root -> the first port seen for each distinct value on it, with
    // the tip that put it there. Insertion order keeps the report stable.
    type RootPorts<'a> = ((i64, i64), Vec<(&'a crate::context::ErcSymbol, Point)>);
    let mut roots: Vec<RootPorts> = Vec::new();
    // By uuid, so which port speaks for a value does not depend on document
    // order.
    let mut ports_by_id: Vec<&crate::context::ErcSymbol> = ctx
        .symbols
        .iter()
        .filter(|s| s.is_power && !s.value.is_empty() && !signex_net::is_power_flag(&s.lib_id))
        .collect();
    ports_by_id.sort_by_key(|s| s.uuid);
    for symbol in ports_by_id {
        for tip in port_tips(symbol) {
            let root = conn.root_of(&tip);
            let slot = match roots.iter().position(|(r, _)| *r == root) {
                Some(i) => i,
                None => {
                    roots.push((root, Vec::new()));
                    roots.len() - 1
                }
            };
            let ports = &mut roots[slot].1;
            if !ports.iter().any(|(p, _)| p.value == symbol.value) {
                ports.push((symbol, tip));
            }
        }
    }

    for (_, mut ports) in roots {
        // Rails in name order: the primary / peer pair is canonical.
        ports.sort_by(|x, y| x.0.value.cmp(&y.0.value));
        let [(a, tip), (b, b_tip), rest @ ..] = ports.as_slice() else {
            continue;
        };
        // Which rails collide, and at which port pins: a multi-pin port pair
        // shorting two separate nets is two findings, never one waiver.
        let mut rails: Vec<&str> = ports.iter().map(|(p, _)| p.value.as_str()).collect();
        rails.sort_unstable();
        rails.dedup();
        let pin_at = |port: &crate::context::ErcSymbol, at: &Point| {
            port.pins
                .iter()
                .find(|p| pt_key(&p.world_pos) == pt_key(at))
                .map(|p| p.anchor())
                .unwrap_or_default()
        };
        let mut names = format!("'{}' and '{}'", a.value, b.value);
        if !rest.is_empty() {
            names = format!("'{}'", a.value);
            for (p, _) in &ports[1..ports.len() - 1] {
                names.push_str(&format!(", '{}'", p.value));
            }
            names.push_str(&format!(" and '{}'", ports[ports.len() - 1].0.value));
        }
        out.push(
            Diagnostic::new(
                RuleKind::PowerPortShort,
                format!("Power ports {names} are on the same net — the rails are shorted"),
                *tip,
            )
            .with_primary(sel(a.uuid, SelectedKind::Symbol))
            .with_peer(sel(b.uuid, SelectedKind::Symbol))
            .at_pin_number(format!("{}/{}", pin_at(a, tip), pin_at(b, b_tip)))
            .tagged(format!("nets:{}", rails.join(","))),
        );
    }
}

// ---------------------------------------------------------------------------
// Rule: SymbolOutsideSheet
// ---------------------------------------------------------------------------

pub(crate) fn symbol_outside_sheet(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    let (w, h) = ctx.paper_size.dimensions_mm();
    for symbol in &ctx.symbols {
        if symbol.position.x < 0.0
            || symbol.position.y < 0.0
            || symbol.position.x > w
            || symbol.position.y > h
        {
            let reference = if symbol.reference.is_empty() {
                "(unnamed)"
            } else {
                symbol.reference.as_str()
            };
            out.push(
                Diagnostic::new(
                    RuleKind::SymbolOutsideSheet,
                    format!("Symbol '{reference}' sits outside the {}×{} mm sheet", w, h,),
                    symbol.position,
                )
                .with_primary(sel(symbol.uuid, SelectedKind::Symbol)),
            );
        }
    }
}

#[cfg(test)]
mod tests;
