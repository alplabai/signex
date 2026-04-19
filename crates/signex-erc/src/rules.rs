//! Individual rule implementations. Each takes the read-only snapshot and
//! pushes [`Violation`]s onto the accumulator.

use std::collections::HashMap;

use signex_render::schematic::SchematicRenderSnapshot;
use signex_types::schematic::{Point, SelectedKind};

use crate::{RuleKind, Severity, Violation, sel};

/// Two points within this many mm are considered the same endpoint.
const ENDPOINT_EPS: f64 = 1e-4;

fn same(a: &Point, b: &Point) -> bool {
    (a.x - b.x).abs() < ENDPOINT_EPS && (a.y - b.y).abs() < ENDPOINT_EPS
}

/// Every library-side pin on every placed symbol, transformed to world
/// space. Returned as `(symbol_uuid, world_position)` pairs.
fn pin_world_positions(snapshot: &SchematicRenderSnapshot) -> Vec<(uuid::Uuid, Point)> {
    let mut out = Vec::new();
    for symbol in &snapshot.symbols {
        let Some(lib_sym) = snapshot.lib_symbols.get(&symbol.lib_id) else {
            continue;
        };
        for lib_pin in &lib_sym.pins {
            // Multi-unit symbols have per-unit pins; pins with unit 0 are shared.
            if lib_pin.unit != 0 && lib_pin.unit != symbol.unit {
                continue;
            }
            let (wx, wy) =
                signex_render::schematic::instance_transform(symbol, &lib_pin.pin.position);
            out.push((symbol.uuid, Point::new(wx, wy)));
        }
    }
    out
}

/// Rule: a pin is unused if nothing touches its world-space tip.
pub(crate) fn unused_pin(snapshot: &SchematicRenderSnapshot, out: &mut Vec<Violation>) {
    let pins = pin_world_positions(snapshot);
    for (symbol_uuid, pin_pos) in pins {
        let connected = snapshot
            .wires
            .iter()
            .any(|w| same(&w.start, &pin_pos) || same(&w.end, &pin_pos))
            || snapshot
                .buses
                .iter()
                .any(|b| same(&b.start, &pin_pos) || same(&b.end, &pin_pos))
            || snapshot
                .no_connects
                .iter()
                .any(|nc| same(&nc.position, &pin_pos))
            || snapshot.labels.iter().any(|l| same(&l.position, &pin_pos));
        if connected {
            continue;
        }
        let Some(symbol) = snapshot.symbols.iter().find(|s| s.uuid == symbol_uuid) else {
            continue;
        };
        // Skip power-flag symbols — their whole purpose is to dangle.
        if symbol.is_power {
            continue;
        }
        let reference = if symbol.reference.is_empty() {
            "(unnamed)"
        } else {
            symbol.reference.as_str()
        };
        out.push(Violation {
            rule: RuleKind::UnusedPin,
            severity: RuleKind::UnusedPin.default_severity(),
            message: format!("Pin on {reference} is not connected"),
            location: pin_pos,
            primary: Some(sel(symbol.uuid, SelectedKind::Symbol)),
            peer: None,
        });
    }
}

/// Rule: two placed symbols carry the same non-blank reference.
pub(crate) fn duplicate_ref_designator(
    snapshot: &SchematicRenderSnapshot,
    out: &mut Vec<Violation>,
) {
    let mut by_ref: HashMap<&str, Vec<&signex_types::schematic::Symbol>> = HashMap::new();
    for symbol in &snapshot.symbols {
        let r = symbol.reference.trim();
        if r.is_empty() || r.ends_with('?') {
            // Unannotated symbols (e.g. "U?") aren't a duplicate-ref bug
            // yet — the annotation step hasn't run.
            continue;
        }
        by_ref.entry(r).or_default().push(symbol);
    }
    for (reference, dupes) in by_ref {
        if dupes.len() < 2 {
            continue;
        }
        // Emit one violation per pair so the user can navigate between them.
        for (idx, sym) in dupes.iter().enumerate() {
            let peer = dupes.get((idx + 1) % dupes.len()).copied();
            out.push(Violation {
                rule: RuleKind::DuplicateRefDesignator,
                severity: RuleKind::DuplicateRefDesignator.default_severity(),
                message: format!("Reference '{reference}' is used by {} symbols", dupes.len(),),
                location: sym.position,
                primary: Some(sel(sym.uuid, SelectedKind::Symbol)),
                peer: peer.map(|p| sel(p.uuid, SelectedKind::Symbol)),
            });
        }
    }
}

/// Rule: a hierarchical label/port must sit on a wire endpoint.
pub(crate) fn hier_port_disconnected(snapshot: &SchematicRenderSnapshot, out: &mut Vec<Violation>) {
    use signex_types::schematic::LabelType;
    for label in &snapshot.labels {
        if !matches!(
            label.label_type,
            LabelType::Hierarchical | LabelType::Global
        ) {
            continue;
        }
        let touched = snapshot
            .wires
            .iter()
            .any(|w| same(&w.start, &label.position) || same(&w.end, &label.position))
            || snapshot
                .buses
                .iter()
                .any(|b| same(&b.start, &label.position) || same(&b.end, &label.position));
        if touched {
            continue;
        }
        out.push(Violation {
            rule: RuleKind::HierPortDisconnected,
            severity: RuleKind::HierPortDisconnected.default_severity(),
            message: format!(
                "{:?} port '{}' is not on a wire",
                label.label_type, label.text,
            ),
            location: label.position,
            primary: Some(sel(label.uuid, SelectedKind::Label)),
            peer: None,
        });
    }
    // Stub to silence unused-severity warning when all rules disabled.
    let _ = Severity::Off;
}

/// Rule: a wire endpoint touches nothing — no pin, no other wire, no junction,
/// no label, no NC, no bus entry.
pub(crate) fn dangling_wire(snapshot: &SchematicRenderSnapshot, out: &mut Vec<Violation>) {
    let pins = pin_world_positions(snapshot);
    let touched_non_wire = |p: &Point| -> bool {
        pins.iter().any(|(_, pp)| same(pp, p))
            || snapshot.junctions.iter().any(|j| same(&j.position, p))
            || snapshot.labels.iter().any(|l| same(&l.position, p))
            || snapshot.no_connects.iter().any(|nc| same(&nc.position, p))
            || snapshot.bus_entries.iter().any(|be| same(&be.position, p))
    };
    for wire in &snapshot.wires {
        for endpoint in [wire.start, wire.end] {
            // Other wires sharing this endpoint.
            let other_wire_count = snapshot
                .wires
                .iter()
                .filter(|w| w.uuid != wire.uuid)
                .filter(|w| same(&w.start, &endpoint) || same(&w.end, &endpoint))
                .count();
            if other_wire_count > 0 || touched_non_wire(&endpoint) {
                continue;
            }
            out.push(Violation {
                rule: RuleKind::DanglingWire,
                severity: RuleKind::DanglingWire.default_severity(),
                message: "Wire endpoint is not connected".to_string(),
                location: endpoint,
                primary: Some(sel(wire.uuid, SelectedKind::Wire)),
                peer: None,
            });
        }
    }
}

/// Rule: two local labels with the same text assigned to electrically
/// disjoint nets (labels on wires that never touch). Detecting "disjoint"
/// rigorously needs full connectivity; v0.7 ships a simpler check that
/// flags any label whose text matches a global/hier label's text but sits
/// Rule: two different net-label texts land on the same electrical net.
/// Uses a union-find over wire endpoints (and labels as "ghost endpoints")
/// so connectivity follows any wire path between the two labels.
pub(crate) fn net_label_conflict(snapshot: &SchematicRenderSnapshot, out: &mut Vec<Violation>) {
    // Quantise points to an integer grid so small float noise doesn't
    // split a single net. Grid spacing 0.01 mm is tighter than any
    // real schematic grid (smallest is 0.1 mm on KiCad).
    fn key(p: &Point) -> (i64, i64) {
        ((p.x * 100.0).round() as i64, (p.y * 100.0).round() as i64)
    }

    // Union-find over quantised points.
    let mut parent: std::collections::HashMap<(i64, i64), (i64, i64)> =
        std::collections::HashMap::new();
    fn find(
        parent: &mut std::collections::HashMap<(i64, i64), (i64, i64)>,
        x: (i64, i64),
    ) -> (i64, i64) {
        let p = *parent.entry(x).or_insert(x);
        if p == x {
            x
        } else {
            let r = find(parent, p);
            parent.insert(x, r);
            r
        }
    }
    fn union(
        parent: &mut std::collections::HashMap<(i64, i64), (i64, i64)>,
        a: (i64, i64),
        b: (i64, i64),
    ) {
        let ra = find(parent, a);
        let rb = find(parent, b);
        if ra != rb {
            parent.insert(ra, rb);
        }
    }

    // Every wire segment joins its endpoints into one net.
    for w in &snapshot.wires {
        union(&mut parent, key(&w.start), key(&w.end));
    }
    // Junctions explicitly merge touching endpoints. (Wires already cover
    // this implicitly, but explicit junctions on crossings need us to
    // union their point with anything else we see at that position.)
    for j in &snapshot.junctions {
        let k = key(&j.position);
        // Insert so the key exists even if nothing else references it.
        find(&mut parent, k);
    }

    // Group net-type labels (not hier/global — those are cross-sheet and
    // already handled by BadHierSheetPin / orphan label) by their net
    // root. A label's "root" is the union-find root of the quantised
    // point the label sits on.
    let mut by_root: std::collections::HashMap<(i64, i64), Vec<&signex_types::schematic::Label>> =
        std::collections::HashMap::new();
    for lbl in &snapshot.labels {
        if !matches!(lbl.label_type, signex_types::schematic::LabelType::Net) {
            continue;
        }
        let root = find(&mut parent, key(&lbl.position));
        by_root.entry(root).or_default().push(lbl);
    }

    // Any root with 2+ labels carrying different texts is a conflict.
    for labels in by_root.values() {
        if labels.len() < 2 {
            continue;
        }
        let first = labels[0];
        // Find the first label whose text differs from `first.text`.
        let Some(conflicting) = labels.iter().find(|l| l.text != first.text) else {
            continue; // All same text — fine.
        };
        out.push(Violation {
            rule: RuleKind::NetLabelConflict,
            severity: RuleKind::NetLabelConflict.default_severity(),
            message: format!(
                "Net label conflict: '{}' and '{}' on the same net",
                first.text, conflicting.text,
            ),
            location: first.position,
            primary: Some(sel(first.uuid, SelectedKind::Label)),
            peer: Some(sel(conflicting.uuid, SelectedKind::Label)),
        });
    }
}

/// Rule: a label sits in free space (not on a wire, bus, or pin).
pub(crate) fn orphan_label(snapshot: &SchematicRenderSnapshot, out: &mut Vec<Violation>) {
    let pins = pin_world_positions(snapshot);
    for label in &snapshot.labels {
        let on_wire = snapshot
            .wires
            .iter()
            .any(|w| same(&w.start, &label.position) || same(&w.end, &label.position));
        let on_bus = snapshot
            .buses
            .iter()
            .any(|b| same(&b.start, &label.position) || same(&b.end, &label.position));
        let on_pin = pins.iter().any(|(_, p)| same(p, &label.position));
        if on_wire || on_bus || on_pin {
            continue;
        }
        // Hier/global ports already have their own rule.
        if matches!(
            label.label_type,
            signex_types::schematic::LabelType::Hierarchical
                | signex_types::schematic::LabelType::Global
        ) {
            continue;
        }
        out.push(Violation {
            rule: RuleKind::OrphanLabel,
            severity: RuleKind::OrphanLabel.default_severity(),
            message: format!("Label '{}' is not on a wire", label.text),
            location: label.position,
            primary: Some(sel(label.uuid, SelectedKind::Label)),
            peer: None,
        });
    }
}

/// Rule: bus labels on the same physical bus disagree on bit width.
/// Each bus label is parsed for a `[low..high]` range suffix; labels on
/// the same connected bus with mismatched ranges trigger an error.
pub(crate) fn bus_bit_width_mismatch(snapshot: &SchematicRenderSnapshot, out: &mut Vec<Violation>) {
    fn key(p: &Point) -> (i64, i64) {
        ((p.x * 100.0).round() as i64, (p.y * 100.0).round() as i64)
    }
    // Union-find, same structure as NetLabelConflict but over bus
    // endpoints only. Local fns mirror the net_label_conflict helpers.
    let mut parent: std::collections::HashMap<(i64, i64), (i64, i64)> =
        std::collections::HashMap::new();
    fn find(
        parent: &mut std::collections::HashMap<(i64, i64), (i64, i64)>,
        x: (i64, i64),
    ) -> (i64, i64) {
        let p = *parent.entry(x).or_insert(x);
        if p == x {
            x
        } else {
            let r = find(parent, p);
            parent.insert(x, r);
            r
        }
    }
    fn union(
        parent: &mut std::collections::HashMap<(i64, i64), (i64, i64)>,
        a: (i64, i64),
        b: (i64, i64),
    ) {
        let ra = find(parent, a);
        let rb = find(parent, b);
        if ra != rb {
            parent.insert(ra, rb);
        }
    }
    for b in &snapshot.buses {
        union(&mut parent, key(&b.start), key(&b.end));
    }

    /// Parses a bus label like `DATA[0..7]` or `A[15..0]` into a (base,
    /// low, high) tuple. Returns None for labels that don't match the
    /// pattern — those are treated as "unknown width" and skipped.
    fn parse_bus_label(text: &str) -> Option<(&str, i64, i64)> {
        let open = text.rfind('[')?;
        if !text.ends_with(']') {
            return None;
        }
        let inside = &text[open + 1..text.len() - 1];
        let dots = inside.find("..")?;
        let lo: i64 = inside[..dots].trim().parse().ok()?;
        let hi: i64 = inside[dots + 2..].trim().parse().ok()?;
        // Normalise so low <= high regardless of MSB/LSB-first notation.
        let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
        Some((&text[..open], lo, hi))
    }

    type BusLabelGroups<'a> = std::collections::HashMap<
        (i64, i64),
        Vec<(&'a signex_types::schematic::Label, (i64, i64))>,
    >;
    let mut by_root: BusLabelGroups<'_> = BusLabelGroups::new();
    for lbl in &snapshot.labels {
        let Some((_base, lo, hi)) = parse_bus_label(&lbl.text) else {
            continue;
        };
        let root = find(&mut parent, key(&lbl.position));
        by_root.entry(root).or_default().push((lbl, (lo, hi)));
    }

    for group in by_root.values() {
        if group.len() < 2 {
            continue;
        }
        // Pick the most common bit-range as the reference so the
        // report flags the outlier, not the majority. With a tie,
        // the first occurrence wins.
        let mut range_counts: HashMap<(i64, i64), usize> = HashMap::new();
        for (_, r) in group {
            *range_counts.entry(*r).or_insert(0) += 1;
        }
        let (majority_range, _) = range_counts
            .iter()
            .max_by_key(|&(_, count)| *count)
            .map(|(r, c)| (*r, *c))
            .expect("group.len() >= 2 guarantees non-empty counts");
        let Some((ref_lbl, ref_range)) = group.iter().find(|(_, r)| *r == majority_range).copied()
        else {
            continue;
        };
        let Some((conflict_lbl, conflict_range)) =
            group.iter().find(|(_, r)| *r != majority_range).copied()
        else {
            continue;
        };
        out.push(Violation {
            rule: RuleKind::BusBitWidthMismatch,
            severity: RuleKind::BusBitWidthMismatch.default_severity(),
            message: format!(
                "Bus width mismatch: '{}' ({}..{}) vs '{}' ({}..{})",
                ref_lbl.text,
                ref_range.0,
                ref_range.1,
                conflict_lbl.text,
                conflict_range.0,
                conflict_range.1,
            ),
            location: conflict_lbl.position,
            primary: Some(sel(conflict_lbl.uuid, SelectedKind::Label)),
            peer: Some(sel(ref_lbl.uuid, SelectedKind::Label)),
        });
    }
}

/// Rule: a hierarchical sheet symbol declares duplicate port pin names,
/// or declares zero pins while referencing a child schematic file. Full
/// cross-sheet validation (port name ↔ child-sheet hier label mapping)
/// needs cross-sheet snapshots and lands with v1.1's hierarchical
/// navigator; this thin check catches the common local mistake.
pub(crate) fn bad_hier_sheet_pin(snapshot: &SchematicRenderSnapshot, out: &mut Vec<Violation>) {
    for child in &snapshot.child_sheets {
        let mut seen: HashMap<&str, uuid::Uuid> = HashMap::new();
        for pin in &child.pins {
            if let Some(prev_uuid) = seen.get(pin.name.as_str()) {
                out.push(Violation {
                    rule: RuleKind::BadHierSheetPin,
                    severity: RuleKind::BadHierSheetPin.default_severity(),
                    message: format!(
                        "Duplicate sheet pin '{}' on sheet '{}'",
                        pin.name, child.name,
                    ),
                    location: pin.position,
                    primary: Some(sel(pin.uuid, SelectedKind::Label)),
                    peer: Some(sel(*prev_uuid, SelectedKind::Label)),
                });
            } else {
                seen.insert(pin.name.as_str(), pin.uuid);
            }
        }
        // Flag a sheet that references a child file but exposes no
        // ports at all — usually a leftover of an unfinished wiring.
        if !child.filename.is_empty() && child.pins.is_empty() {
            out.push(Violation {
                rule: RuleKind::BadHierSheetPin,
                severity: Severity::Warning,
                message: format!(
                    "Hierarchical sheet '{}' has no pins — the child schematic can't be wired in",
                    child.name,
                ),
                location: child.position,
                primary: Some(sel(child.uuid, SelectedKind::ChildSheet)),
                peer: None,
            });
        }
    }
}

/// Rule: a net that contains a symbol pin of type Power In or Power Out must
/// also have a PWR_FLAG so the ERC engine knows the net is fed from somewhere.
/// Needs full connectivity analysis; v0.7 ships a simpler heuristic — flag
/// any power port whose net name isn't also used by a label elsewhere.
pub(crate) fn missing_power_flag(snapshot: &SchematicRenderSnapshot, out: &mut Vec<Violation>) {
    // Collect every explicit label text so power ports tied to a labelled
    // net are silent.
    let label_texts: std::collections::HashSet<&str> =
        snapshot.labels.iter().map(|l| l.text.as_str()).collect();
    for symbol in &snapshot.symbols {
        if !symbol.is_power {
            continue;
        }
        let name = symbol.value.as_str();
        if name.is_empty() {
            continue;
        }
        if label_texts.contains(name) {
            continue;
        }
        // A single power port on its own — we can't prove the net is fed
        // without connectivity analysis. For now emit an Info so the user
        // sees it appear in the Messages panel and knows the rule exists.
        out.push(Violation {
            rule: RuleKind::MissingPowerFlag,
            severity: Severity::Info,
            message: format!(
                "Power port '{name}' is not cross-referenced by a label — add a PWR_FLAG if this is a source net",
            ),
            location: symbol.position,
            primary: Some(sel(symbol.uuid, SelectedKind::Symbol)),
            peer: None,
        });
    }
}

/// Rule: two power ports with different nets sit on the same wire — that
/// would short the power rails. Needs connectivity to detect rigorously;
/// v0.7 ships the simpler endpoint-sharing check.
pub(crate) fn power_port_short(snapshot: &SchematicRenderSnapshot, out: &mut Vec<Violation>) {
    let power_ports: Vec<&signex_types::schematic::Symbol> = snapshot
        .symbols
        .iter()
        .filter(|s| s.is_power && !s.value.is_empty())
        .collect();

    for (i, a) in power_ports.iter().enumerate() {
        for b in &power_ports[i + 1..] {
            if a.value == b.value {
                continue;
            }
            // Endpoint share — same world position.
            if same(&a.position, &b.position) {
                out.push(Violation {
                    rule: RuleKind::PowerPortShort,
                    severity: RuleKind::PowerPortShort.default_severity(),
                    message: format!(
                        "Power ports '{}' and '{}' are at the same point",
                        a.value, b.value,
                    ),
                    location: a.position,
                    primary: Some(sel(a.uuid, SelectedKind::Symbol)),
                    peer: Some(sel(b.uuid, SelectedKind::Symbol)),
                });
            }
        }
    }
}

/// Rule: a symbol sits outside the active page bounds. We use a generous
/// A4 bound (297×210 mm) — users can override in the future.
pub(crate) fn symbol_outside_sheet(snapshot: &SchematicRenderSnapshot, out: &mut Vec<Violation>) {
    // Defensive: parse the snapshot's paper size; fall back to A4.
    let (w, h) = match snapshot.paper_size.as_str() {
        "A3" => (420.0_f64, 297.0),
        "A2" => (594.0, 420.0),
        "A1" => (841.0, 594.0),
        "A0" => (1189.0, 841.0),
        _ => (297.0, 210.0),
    };
    for symbol in &snapshot.symbols {
        if symbol.position.x < 0.0
            || symbol.position.y < 0.0
            || symbol.position.x > w
            || symbol.position.y > h
        {
            out.push(Violation {
                rule: RuleKind::SymbolOutsideSheet,
                severity: RuleKind::SymbolOutsideSheet.default_severity(),
                message: format!(
                    "Symbol '{}' sits outside the {}×{} mm sheet",
                    if symbol.reference.is_empty() {
                        "(unnamed)"
                    } else {
                        symbol.reference.as_str()
                    },
                    w,
                    h,
                ),
                location: symbol.position,
                primary: Some(sel(symbol.uuid, SelectedKind::Symbol)),
                peer: None,
            });
        }
    }
}
