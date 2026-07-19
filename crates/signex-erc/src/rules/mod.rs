//! Built-in rule implementations. Each function takes a read-only
//! [`ErcContext`] and pushes [`Diagnostic`]s onto the accumulator.
//! No render or parser imports — all geometry is already world-space inside
//! the context.
//!
//! This file is already past the size a module should reach, so new rules land
//! in their own sibling file rather than growing it further.

mod ambiguous_label_anchor;
pub(crate) use ambiguous_label_anchor::ambiguous_label_anchor;

use std::collections::HashMap;

use signex_net::{SheetConnectivity, point_on_segment, pt_key};
use signex_types::schematic::{LabelType, Point, SelectedKind};

use crate::context::ErcContext;
use crate::diagnostic::Diagnostic;
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
fn on_any_wire(pos: &Point, wires: &[(Point, Point)]) -> bool {
    let pk = pt_key(pos);
    wires
        .iter()
        .any(|(a, b)| point_on_segment(pk, pt_key(a), pt_key(b)))
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
            // connectivity gate (`context::point_is_connected`, mirroring
            // `signex_net`'s) — trust it instead of re-deriving an
            // endpoint-only, bus-gated approximation here (issue #388, D5.4).
            if pin.connected {
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
                .with_primary(sel(symbol.uuid, SelectedKind::Symbol)),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Rule: DuplicateRefDesignator
// ---------------------------------------------------------------------------

pub(crate) fn duplicate_ref_designator(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    let mut by_ref: HashMap<&str, Vec<&crate::context::ErcSymbol>> = HashMap::new();
    for symbol in &ctx.symbols {
        let r = symbol.reference.trim();
        if r.is_empty() || r.ends_with('?') {
            continue;
        }
        by_ref.entry(r).or_default().push(symbol);
    }
    for (reference, dupes) in by_ref {
        if dupes.len() < 2 {
            continue;
        }
        for (idx, sym) in dupes.iter().enumerate() {
            let peer = dupes.get((idx + 1) % dupes.len()).copied();
            let mut d = Diagnostic::new(
                RuleKind::DuplicateRefDesignator,
                format!("Reference '{reference}' is used by {} symbols", dupes.len()),
                sym.position,
            )
            .with_primary(sel(sym.uuid, SelectedKind::Symbol));
            if let Some(p) = peer {
                d = d.with_peer(sel(p.uuid, SelectedKind::Symbol));
            }
            out.push(d);
        }
    }
}

// ---------------------------------------------------------------------------
// Rule: HierPortDisconnected
// ---------------------------------------------------------------------------

pub(crate) fn hier_port_disconnected(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    let wires = wire_pairs(ctx);
    for label in &ctx.labels {
        if !matches!(
            label.label_type,
            LabelType::Hierarchical | LabelType::Global
        ) {
            continue;
        }
        // Wires anchor by endpoint OR interior (a Global/Hierarchical label
        // may sit mid-span); buses stay endpoint-only (D5.4) — see
        // `on_any_wire` / `on_any_bus_endpoint`.
        let touched =
            on_any_wire(&label.position, &wires) || on_any_bus_endpoint(&label.position, ctx);
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

pub(crate) fn dangling_wire(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    let touched_non_wire = |p: &Point| -> bool {
        ctx.symbols
            .iter()
            .flat_map(|s| s.pins.iter())
            .any(|pin| same(&pin.world_pos, p))
            || ctx.junctions.iter().any(|j| same(&j.position, p))
            || ctx.labels.iter().any(|l| same(&l.position, p))
            || ctx.no_connects.iter().any(|nc| same(&nc.position, p))
            || ctx.bus_entries.iter().any(|be| same(&be.position, p))
    };

    for wire in &ctx.wires {
        for endpoint in [wire.start, wire.end] {
            let other_wire_count = ctx
                .wires
                .iter()
                .filter(|w| w.uuid != wire.uuid)
                .filter(|w| same(&w.start, &endpoint) || same(&w.end, &endpoint))
                .count();
            if other_wire_count > 0 || touched_non_wire(&endpoint) {
                continue;
            }
            out.push(
                Diagnostic::new(
                    RuleKind::DanglingWire,
                    "Wire endpoint is not connected",
                    endpoint,
                )
                .with_primary(sel(wire.uuid, SelectedKind::Wire)),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Rule: NetLabelConflict
// ---------------------------------------------------------------------------

pub(crate) fn net_label_conflict(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    let mut conn = wire_connectivity(ctx);
    let wires = wire_pairs(ctx);

    let net_labels: Vec<&crate::context::ErcLabel> = ctx
        .labels
        .iter()
        .filter(|l| matches!(l.label_type, LabelType::Net))
        .collect();

    // Two passes, matching `merged_sheet_parent`'s invariant: every union
    // completes before any root is sampled. Interleaving anchor-then-sample
    // per label made the result order-dependent — an earlier label's cached
    // root can go stale once a later label's anchoring union re-roots its
    // class (a label at a junction-less T point where two wires meet without
    // a junction dot; issue #388 follow-up).
    for lbl in &net_labels {
        conn.root_of_anchored(&lbl.position, &wires);
    }

    let mut by_root: HashMap<(i64, i64), Vec<&crate::context::ErcLabel>> = HashMap::new();
    for lbl in net_labels {
        let root = conn.root_of_anchored(&lbl.position, &wires);
        by_root.entry(root).or_default().push(lbl);
    }

    for labels in by_root.values() {
        if labels.len() < 2 {
            continue;
        }
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
    let wires = wire_pairs(ctx);
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
        // #388). Buses stay endpoint-only (D5.4).
        let on_wire = on_any_wire(&label.position, &wires);
        let on_bus = on_any_bus_endpoint(&label.position, ctx);
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
// Rule: BusBitWidthMismatch
// ---------------------------------------------------------------------------

pub(crate) fn bus_bit_width_mismatch(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    // Bus bundles connect by segment only (no junction dots), so no junctions
    // are fed to the shared connectivity — same topology as before, now derived
    // through `signex-net` rather than a hand-rolled union-find.
    let buses: Vec<(Point, Point)> = ctx.buses.iter().map(|b| (b.start, b.end)).collect();
    let mut conn = SheetConnectivity::from_segments(&buses, &[]);

    fn parse_bus_label(text: &str) -> Option<(&str, i64, i64)> {
        let open = text.rfind('[')?;
        if !text.ends_with(']') {
            return None;
        }
        let inside = &text[open + 1..text.len() - 1];
        let dots = inside.find("..")?;
        let lo: i64 = inside[..dots].trim().parse().ok()?;
        let hi: i64 = inside[dots + 2..].trim().parse().ok()?;
        let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
        Some((&text[..open], lo, hi))
    }

    // Anchor each range label to the bus segment it sits on — endpoint *or*
    // interior. Mid-span is where users actually put a bus range label, and an
    // unanchored `root_of` gave every one of them its own singleton group, so
    // a group never reached the `len() >= 2` needed to compare widths and the
    // mismatch was never reported (issue #395).
    //
    // Anchoring against buses is correct *here* and nowhere else: `conn` is
    // bus-local, built from bus segments alone, and models bundle grouping —
    // something `build_netlist` does not derive at all, so unlike the net rules
    // (D5.4) there is no netlist for this to be more lenient than.
    //
    // Separate pass for the same reason as the net side: a union re-points a
    // class representative, so a root recorded mid-loop can go stale. Read
    // every root only after the last union.
    for lbl in &ctx.labels {
        if parse_bus_label(&lbl.text).is_none() {
            continue;
        }
        conn.root_of_anchored(&lbl.position, &buses);
    }

    type Groups<'a> = HashMap<(i64, i64), Vec<(&'a crate::context::ErcLabel, (i64, i64))>>;
    let mut by_root: Groups<'_> = HashMap::new();
    for lbl in &ctx.labels {
        let Some((_base, lo, hi)) = parse_bus_label(&lbl.text) else {
            continue;
        };
        let root = conn.root_of(&lbl.position);
        by_root.entry(root).or_default().push((lbl, (lo, hi)));
    }

    for group in by_root.values() {
        if group.len() < 2 {
            continue;
        }
        let mut range_counts: HashMap<(i64, i64), usize> = HashMap::new();
        for (_, r) in group {
            *range_counts.entry(*r).or_insert(0) += 1;
        }
        // HI-18: `group.len() >= 2` does NOT imply `range_counts.is_empty() == false`.
        // If `parse_bus_label` failed on every member of `group`, `range_counts`
        // is empty and the `.expect()` panics. Skip the group in that case.
        let Some((majority_range, _)) = range_counts
            .iter()
            .max_by_key(|&(_, c)| *c)
            .map(|(r, c)| (*r, *c))
        else {
            continue;
        };
        let Some((ref_lbl, ref_range)) = group.iter().find(|(_, r)| *r == majority_range).copied()
        else {
            continue;
        };
        let Some((conflict_lbl, conflict_range)) =
            group.iter().find(|(_, r)| *r != majority_range).copied()
        else {
            continue;
        };
        out.push(
            Diagnostic::new(
                RuleKind::BusBitWidthMismatch,
                format!(
                    "Bus width mismatch: '{}' ({}..{}) vs '{}' ({}..{})",
                    ref_lbl.text,
                    ref_range.0,
                    ref_range.1,
                    conflict_lbl.text,
                    conflict_range.0,
                    conflict_range.1,
                ),
                conflict_lbl.position,
            )
            .with_primary(sel(conflict_lbl.uuid, SelectedKind::Label))
            .with_peer(sel(ref_lbl.uuid, SelectedKind::Label)),
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
                .with_primary(sel(child.uuid, SelectedKind::ChildSheet)),
            );
        }

        // Cross-sheet: parent pin ↔ child hier-label matching.
        let Some(child_ctx) = ctx.children.get(child.filename.as_str()) else {
            continue;
        };
        let hier_text: HashSet<&str> = child_ctx
            .labels
            .iter()
            .filter(|l| matches!(l.label_type, LabelType::Hierarchical | LabelType::Global))
            .map(|l| l.text.as_str())
            .collect();

        for pin in &child.pins {
            if !hier_text.contains(pin.name.as_str()) {
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

        let parent_pins: HashSet<&str> = child.pins.iter().map(|p| p.name.as_str()).collect();
        for lbl in child_ctx
            .labels
            .iter()
            .filter(|l| matches!(l.label_type, LabelType::Hierarchical))
        {
            if !parent_pins.contains(lbl.text.as_str()) {
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
    // interior first (issue #388), THEN read roots — sampling interleaved
    // with anchoring made an earlier label's cached root go stale once a
    // later label's union re-rooted its class (a label at a junction-less T
    // point), which could both miss a real conflict and false-flag a port
    // that IS cross-referenced.
    for lbl in &named_labels {
        conn.root_of_anchored(&lbl.position, &wires);
    }

    // Map each label text → set of net roots its labels sit on.
    let mut label_nets: HashMap<&str, std::collections::HashSet<(i64, i64)>> = HashMap::new();
    for lbl in named_labels {
        let root = conn.root_of_anchored(&lbl.position, &wires);
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
        // Power-port symbols carry a single connection point at their
        // position (no separate pin geometry). Look up the net root for that
        // point — unanchored, like any other pin: a port only taps a wire
        // interior through an explicit junction (`point_is_connected`'s
        // rule), never by proximity alone.
        let port_root = conn.root_of(&symbol.position);
        let same_net_label = label_nets
            .get(name)
            .map(|nets| nets.contains(&port_root))
            .unwrap_or(false);
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
            .with_severity(Severity::Info)
            .with_primary(sel(symbol.uuid, SelectedKind::Symbol)),
        );
    }
}

// ---------------------------------------------------------------------------
// Rule: PowerPortShort
// ---------------------------------------------------------------------------

pub(crate) fn power_port_short(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    let power: Vec<&crate::context::ErcSymbol> = ctx
        .symbols
        .iter()
        .filter(|s| s.is_power && !s.value.is_empty())
        .collect();

    for (i, a) in power.iter().enumerate() {
        for b in &power[i + 1..] {
            if a.value == b.value {
                continue;
            }
            if same(&a.position, &b.position) {
                out.push(
                    Diagnostic::new(
                        RuleKind::PowerPortShort,
                        format!(
                            "Power ports '{}' and '{}' are at the same point",
                            a.value, b.value,
                        ),
                        a.position,
                    )
                    .with_primary(sel(a.uuid, SelectedKind::Symbol))
                    .with_peer(sel(b.uuid, SelectedKind::Symbol)),
                );
            }
        }
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
mod tests {
    use super::*;
    use crate::context::{ErcJunction, ErcLabel, ErcPin, ErcSymbol, ErcWire, PaperSize};
    use signex_types::schematic::{LabelType, PinDirection, Point};
    use std::collections::HashMap;
    use uuid::Uuid;

    fn pt(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    fn wire(a: Point, b: Point) -> ErcWire {
        ErcWire {
            uuid: Uuid::nil(),
            start: a,
            end: b,
        }
    }

    fn power_label(text: &str, pos: Point) -> ErcLabel {
        ErcLabel {
            uuid: Uuid::nil(),
            text: text.into(),
            position: pos,
            label_type: LabelType::Power,
        }
    }

    fn power_port(value: &str, pos: Point) -> ErcSymbol {
        ErcSymbol {
            uuid: Uuid::nil(),
            reference: "#PWR01".into(),
            value: value.into(),
            position: pos,
            is_power: true,
            pins: Vec::new(),
            attrs: HashMap::new(),
        }
    }

    fn net_label(text: &str, pos: Point) -> ErcLabel {
        ErcLabel {
            uuid: Uuid::nil(),
            text: text.into(),
            position: pos,
            label_type: LabelType::Net,
        }
    }

    fn global_label(text: &str, pos: Point) -> ErcLabel {
        ErcLabel {
            uuid: Uuid::nil(),
            text: text.into(),
            position: pos,
            label_type: LabelType::Global,
        }
    }

    /// A non-power symbol with a single pin, its `connected` flag set
    /// exactly as `context::point_is_connected` would compute it — these
    /// tests build `ErcContext` by hand (below the projection step), so the
    /// flag is the fixture's job here.
    fn symbol_with_pin(reference: &str, world_pos: Point, connected: bool) -> ErcSymbol {
        ErcSymbol {
            uuid: Uuid::nil(),
            reference: reference.into(),
            value: String::new(),
            position: world_pos,
            is_power: false,
            pins: vec![ErcPin {
                world_pos,
                electrical_type: PinDirection::Passive,
                required: true,
                connected,
            }],
            attrs: HashMap::new(),
        }
    }

    fn ctx(
        wires: Vec<ErcWire>,
        junctions: Vec<ErcJunction>,
        labels: Vec<ErcLabel>,
        symbols: Vec<ErcSymbol>,
    ) -> ErcContext {
        ErcContext {
            paper_size: PaperSize::A4,
            symbols,
            wires,
            buses: Vec::new(),
            labels,
            junctions,
            no_connects: Vec::new(),
            bus_entries: Vec::new(),
            child_sheets: Vec::new(),
            nets: Vec::new(),
            children: HashMap::new(),
        }
    }

    #[test]
    fn missing_power_flag_honors_a_t_junction() {
        // A +3V3 port at (10,0) on the horizontal wire; a matching +3V3 label at
        // (5,5) on a vertical wire that ends on the horizontal's interior, joined
        // by a junction at (5,0). Port and label share a net only *through* the
        // T-junction — which the shared connectivity merges, so the port is
        // cross-referenced and NOT flagged. (The old inline pass only `find`-ed
        // junctions, split the net, and false-flagged this.)
        let c = ctx(
            vec![
                wire(pt(0.0, 0.0), pt(10.0, 0.0)),
                wire(pt(5.0, 0.0), pt(5.0, 5.0)),
            ],
            vec![ErcJunction {
                position: pt(5.0, 0.0),
            }],
            vec![power_label("+3V3", pt(5.0, 5.0))],
            vec![power_port("+3V3", pt(10.0, 0.0))],
        );
        let mut out = Vec::new();
        missing_power_flag(&c, &mut out);
        assert!(
            out.is_empty(),
            "port cross-referenced through the T-junction: {out:?}"
        );
    }

    #[test]
    fn missing_power_flag_fires_when_no_same_net_label() {
        // Same port, but the +3V3 label sits on a disjoint wire with nothing
        // tying it to the port's net → not cross-referenced → flagged.
        let c = ctx(
            vec![
                wire(pt(0.0, 0.0), pt(10.0, 0.0)),
                wire(pt(50.0, 0.0), pt(60.0, 0.0)),
            ],
            Vec::new(),
            vec![power_label("+3V3", pt(50.0, 0.0))],
            vec![power_port("+3V3", pt(10.0, 0.0))],
        );
        let mut out = Vec::new();
        missing_power_flag(&c, &mut out);
        assert_eq!(
            out.len(),
            1,
            "port on a net with no same-name label is flagged"
        );
    }

    // -----------------------------------------------------------------------
    // Issue #388 regressions — rules must agree with `build_netlist` on
    // mid-wire (interior) label/pin placements and through-junction taps.
    // -----------------------------------------------------------------------

    #[test]
    fn unused_pin_trusts_a_through_junction_tap() {
        // A single wire (0,0)-(10,0) with a junction dot mid-span at (5,0) — a
        // pin tapping exactly there is connected (D5.3), the same as
        // `context::point_is_connected` computes. The old `unused_pin` never
        // consulted junctions at all when re-deriving connectivity, so it
        // would have false-flagged this pin.
        let c = ctx(
            vec![wire(pt(0.0, 0.0), pt(10.0, 0.0))],
            vec![ErcJunction {
                position: pt(5.0, 0.0),
            }],
            Vec::new(),
            vec![symbol_with_pin("R1", pt(5.0, 0.0), true)],
        );
        let mut out = Vec::new();
        unused_pin(&c, &mut out);
        assert!(
            out.is_empty(),
            "a pin tapping a wire interior through a junction is not unused: {out:?}"
        );
    }

    #[test]
    fn unused_pin_still_fires_when_not_connected() {
        // Sanity check: `unused_pin` still reads `connected == false`.
        let c = ctx(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![symbol_with_pin("R1", pt(5.0, 0.0), false)],
        );
        let mut out = Vec::new();
        unused_pin(&c, &mut out);
        assert_eq!(out.len(), 1, "a genuinely unconnected pin is still flagged");
    }

    #[test]
    fn net_label_conflict_catches_wire_interior_labels() {
        // Two conflicting Net labels both sit on the *interior* of the same
        // wire — no endpoints involved. Without anchoring each label to the
        // wire, each lands on its own singleton root and the conflict is
        // missed entirely.
        let c = ctx(
            vec![wire(pt(0.0, 0.0), pt(10.0, 0.0))],
            Vec::new(),
            vec![net_label("A", pt(3.0, 0.0)), net_label("B", pt(7.0, 0.0))],
            Vec::new(),
        );
        let mut out = Vec::new();
        net_label_conflict(&c, &mut out);
        assert_eq!(
            out.len(),
            1,
            "conflicting Net labels on one wire's interior are flagged: {out:?}"
        );
    }

    #[test]
    fn orphan_label_accepts_a_power_label_on_a_wire_interior() {
        // A Power label sits mid-span, not at either endpoint.
        let c = ctx(
            vec![wire(pt(0.0, 0.0), pt(10.0, 0.0))],
            Vec::new(),
            vec![power_label("+3V3", pt(5.0, 0.0))],
            Vec::new(),
        );
        let mut out = Vec::new();
        orphan_label(&c, &mut out);
        assert!(
            out.is_empty(),
            "a Power label on a wire interior is not orphaned: {out:?}"
        );
    }

    #[test]
    fn hier_port_disconnected_accepts_a_global_label_on_a_wire_interior() {
        // A Global label sits mid-span, not at either endpoint.
        let c = ctx(
            vec![wire(pt(0.0, 0.0), pt(10.0, 0.0))],
            Vec::new(),
            vec![global_label("VBUS", pt(5.0, 0.0))],
            Vec::new(),
        );
        let mut out = Vec::new();
        hier_port_disconnected(&c, &mut out);
        assert!(
            out.is_empty(),
            "a Global label on a wire interior is on-wire: {out:?}"
        );
    }

    #[test]
    fn missing_power_flag_accepts_a_mid_wire_cross_ref_label() {
        // The +3V3 label sits mid-span on the same wire the +3V3 port's pin
        // terminates on — connected only via interior anchoring, not a
        // shared endpoint or junction.
        let c = ctx(
            vec![wire(pt(0.0, 0.0), pt(10.0, 0.0))],
            Vec::new(),
            vec![power_label("+3V3", pt(5.0, 0.0))],
            vec![power_port("+3V3", pt(10.0, 0.0))],
        );
        let mut out = Vec::new();
        missing_power_flag(&c, &mut out);
        assert!(
            out.is_empty(),
            "port cross-referenced by a mid-wire label is not flagged: {out:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Anchor-then-sample must not be order-dependent on label document order
    // (follow-up to #388): a wire T where wB ends on wA's interior with NO
    // junction. wA = (0,0)-(10,0), wB = (5,0)-(5,10). The T point (5,0) is
    // simultaneously wA's interior and wB's own endpoint, so anchoring one
    // label there re-roots the whole class — sampling that root before a
    // later label's anchoring union used to cache a stale root.
    // -----------------------------------------------------------------------

    #[test]
    fn net_label_conflict_is_independent_of_label_order() {
        let wires = vec![
            wire(pt(0.0, 0.0), pt(10.0, 0.0)),
            wire(pt(5.0, 0.0), pt(5.0, 10.0)),
        ];
        let n1 = net_label("N1", pt(5.0, 5.0)); // wB interior
        let n2 = net_label("N2", pt(5.0, 0.0)); // the T point

        let forward = ctx(
            wires.clone(),
            Vec::new(),
            vec![n1.clone(), n2.clone()],
            Vec::new(),
        );
        let mut out_fwd = Vec::new();
        net_label_conflict(&forward, &mut out_fwd);

        let reverse = ctx(wires, Vec::new(), vec![n2, n1], Vec::new());
        let mut out_rev = Vec::new();
        net_label_conflict(&reverse, &mut out_rev);

        assert_eq!(
            out_fwd.len(),
            1,
            "N1/N2 share a net through the T and conflict: {out_fwd:?}"
        );
        assert_eq!(
            out_fwd.len(),
            out_rev.len(),
            "verdict must not depend on label document order (fwd={out_fwd:?}, rev={out_rev:?})"
        );
    }

    #[test]
    fn missing_power_flag_is_independent_of_label_order() {
        let wires = vec![
            wire(pt(0.0, 0.0), pt(10.0, 0.0)),
            wire(pt(5.0, 0.0), pt(5.0, 10.0)),
        ];
        let power = power_label("+3V3", pt(5.0, 5.0)); // wB interior
        let other = net_label("X", pt(5.0, 0.0)); // the T point
        let port = power_port("+3V3", pt(5.0, 10.0)); // wB's far endpoint

        let forward = ctx(
            wires.clone(),
            Vec::new(),
            vec![power.clone(), other.clone()],
            vec![port.clone()],
        );
        let mut out_fwd = Vec::new();
        missing_power_flag(&forward, &mut out_fwd);

        let reverse = ctx(wires, Vec::new(), vec![other, power], vec![port]);
        let mut out_rev = Vec::new();
        missing_power_flag(&reverse, &mut out_rev);

        assert!(
            out_fwd.is_empty(),
            "port cross-referenced through the T is not flagged: {out_fwd:?}"
        );
        assert_eq!(
            out_fwd.len(),
            out_rev.len(),
            "verdict must not depend on label document order (fwd={out_fwd:?}, rev={out_rev:?})"
        );
    }

    fn bus_ctx(buses: Vec<crate::context::ErcBus>, labels: Vec<ErcLabel>) -> ErcContext {
        ErcContext {
            buses,
            labels,
            ..ctx(Vec::new(), Vec::new(), Vec::new(), Vec::new())
        }
    }

    fn bus(a: Point, b: Point) -> crate::context::ErcBus {
        crate::context::ErcBus { start: a, end: b }
    }

    #[test]
    fn bus_bit_width_mismatch_catches_mid_bus_range_labels() {
        // Regression for issue #395: both range labels sit on the bus INTERIOR
        // — where users actually place them — not on an endpoint. Unanchored,
        // each landed on its own singleton root, no group ever reached the
        // `len() >= 2` needed to compare widths, and the mismatch went unsaid.
        let buses = vec![bus(pt(0.0, 0.0), pt(10.0, 0.0))];
        let labels = vec![
            net_label("D[0..7]", pt(3.0, 0.0)),
            net_label("D[0..3]", pt(7.0, 0.0)),
        ];
        let mut out = Vec::new();
        bus_bit_width_mismatch(&bus_ctx(buses, labels), &mut out);
        assert_eq!(
            out.len(),
            1,
            "a mid-bus width mismatch must be reported: {out:?}"
        );
    }

    #[test]
    fn bus_bit_width_mismatch_still_catches_endpoint_range_labels() {
        // Guard: the case that already worked before #395 must keep working —
        // anchoring an endpoint label is a no-op union.
        let buses = vec![bus(pt(0.0, 0.0), pt(10.0, 0.0))];
        let labels = vec![
            net_label("D[0..7]", pt(0.0, 0.0)),
            net_label("D[0..3]", pt(10.0, 0.0)),
        ];
        let mut out = Vec::new();
        bus_bit_width_mismatch(&bus_ctx(buses, labels), &mut out);
        assert_eq!(out.len(), 1, "endpoint mismatch still fires: {out:?}");
    }

    #[test]
    fn bus_bit_width_mismatch_accepts_matching_mid_bus_widths() {
        // Anchoring must not turn into a false-positive engine: two mid-span
        // labels that agree on the width are grouped together now (they were
        // two singletons before) and must still report nothing.
        let buses = vec![bus(pt(0.0, 0.0), pt(10.0, 0.0))];
        let labels = vec![
            net_label("D[0..7]", pt(3.0, 0.0)),
            net_label("D[0..7]", pt(7.0, 0.0)),
        ];
        let mut out = Vec::new();
        bus_bit_width_mismatch(&bus_ctx(buses, labels), &mut out);
        assert!(
            out.is_empty(),
            "matching widths are not a mismatch: {out:?}"
        );
    }

    #[test]
    fn bus_bit_width_mismatch_keeps_separate_buses_apart() {
        // Anchoring is per-segment, not global: two disjoint buses each with
        // their own range label are two groups of one, so differing widths
        // across unrelated buses are not a mismatch.
        let buses = vec![
            bus(pt(0.0, 0.0), pt(10.0, 0.0)),
            bus(pt(0.0, 50.0), pt(10.0, 50.0)),
        ];
        let labels = vec![
            net_label("D[0..7]", pt(5.0, 0.0)),
            net_label("A[0..3]", pt(5.0, 50.0)),
        ];
        let mut out = Vec::new();
        bus_bit_width_mismatch(&bus_ctx(buses, labels), &mut out);
        assert!(
            out.is_empty(),
            "unrelated buses must not be compared: {out:?}"
        );
    }

    #[test]
    fn bus_bit_width_mismatch_ignores_a_label_off_every_bus() {
        // A range label floating off the bus stays its own root and never
        // joins the bundle — anchoring must not attract anything nearby.
        let buses = vec![bus(pt(0.0, 0.0), pt(10.0, 0.0))];
        let labels = vec![
            net_label("D[0..7]", pt(5.0, 0.0)),
            net_label("D[0..3]", pt(5.0, 25.0)),
        ];
        let mut out = Vec::new();
        bus_bit_width_mismatch(&bus_ctx(buses, labels), &mut out);
        assert!(
            out.is_empty(),
            "an off-bus label must not join the bundle: {out:?}"
        );
    }
}
