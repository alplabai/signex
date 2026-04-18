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
fn pin_world_positions(
    snapshot: &SchematicRenderSnapshot,
) -> Vec<(uuid::Uuid, Point)> {
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
            let (wx, wy) = signex_render::schematic::instance_transform(
                symbol,
                &lib_pin.pin.position,
            );
            out.push((symbol.uuid, Point::new(wx, wy)));
        }
    }
    out
}

/// Rule: a pin is unused if nothing touches its world-space tip.
pub(crate) fn unused_pin(
    snapshot: &SchematicRenderSnapshot,
    out: &mut Vec<Violation>,
) {
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
            || snapshot
                .labels
                .iter()
                .any(|l| same(&l.position, &pin_pos));
        if connected {
            continue;
        }
        let Some(symbol) = snapshot.symbols.iter().find(|s| s.uuid == symbol_uuid)
        else {
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
    let mut by_ref: HashMap<&str, Vec<&signex_types::schematic::Symbol>> =
        HashMap::new();
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
                message: format!(
                    "Reference '{reference}' is used by {} symbols",
                    dupes.len(),
                ),
                location: sym.position,
                primary: Some(sel(sym.uuid, SelectedKind::Symbol)),
                peer: peer.map(|p| sel(p.uuid, SelectedKind::Symbol)),
            });
        }
    }
}

/// Rule: a hierarchical label/port must sit on a wire endpoint.
pub(crate) fn hier_port_disconnected(
    snapshot: &SchematicRenderSnapshot,
    out: &mut Vec<Violation>,
) {
    use signex_types::schematic::LabelType;
    for label in &snapshot.labels {
        if !matches!(
            label.label_type,
            LabelType::Hierarchical | LabelType::Global
        ) {
            continue;
        }
        let touched = snapshot.wires.iter().any(|w| {
            same(&w.start, &label.position) || same(&w.end, &label.position)
        }) || snapshot.buses.iter().any(|b| {
            same(&b.start, &label.position) || same(&b.end, &label.position)
        });
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
pub(crate) fn dangling_wire(
    snapshot: &SchematicRenderSnapshot,
    out: &mut Vec<Violation>,
) {
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
/// on a local-only subnet. Until connectivity analysis lands, this rule
/// emits nothing — the scaffold stays so severity config works end-to-end.
pub(crate) fn net_label_conflict(
    _snapshot: &SchematicRenderSnapshot,
    _out: &mut [Violation],
) {
    // Connectivity analysis required — placeholder for v0.7.1.
}

/// Rule: a label sits in free space (not on a wire, bus, or pin).
pub(crate) fn orphan_label(
    snapshot: &SchematicRenderSnapshot,
    out: &mut Vec<Violation>,
) {
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

/// Rule: two buses with explicit bit-range definitions connect and disagree
/// on their width. Signex doesn't parse bus bit-ranges yet (v0.7.2);
/// placeholder.
pub(crate) fn bus_bit_width_mismatch(
    _snapshot: &SchematicRenderSnapshot,
    _out: &mut [Violation],
) {
    // Bus bit-range parsing lands with v1.1 (Advanced Schematic).
}

/// Rule: a child-sheet declares a port that the sheet symbol doesn't expose,
/// or vice versa. Needs cross-sheet resolution which Signex defers to v1.1
/// (Hierarchical navigator); placeholder.
pub(crate) fn bad_hier_sheet_pin(
    _snapshot: &SchematicRenderSnapshot,
    _out: &mut [Violation],
) {
    // Cross-sheet validation belongs with the hierarchical navigator (v1.1).
}

/// Rule: a net that contains a symbol pin of type Power In or Power Out must
/// also have a PWR_FLAG so the ERC engine knows the net is fed from somewhere.
/// Needs full connectivity analysis; v0.7 ships a simpler heuristic — flag
/// any power port whose net name isn't also used by a label elsewhere.
pub(crate) fn missing_power_flag(
    snapshot: &SchematicRenderSnapshot,
    out: &mut Vec<Violation>,
) {
    // Collect every explicit label text so power ports tied to a labelled
    // net are silent.
    let label_texts: std::collections::HashSet<&str> = snapshot
        .labels
        .iter()
        .map(|l| l.text.as_str())
        .collect();
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
pub(crate) fn power_port_short(
    snapshot: &SchematicRenderSnapshot,
    out: &mut Vec<Violation>,
) {
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
pub(crate) fn symbol_outside_sheet(
    snapshot: &SchematicRenderSnapshot,
    out: &mut Vec<Violation>,
) {
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
