//! Built-in rule implementations. Each function takes a read-only
//! [`ErcContext`] and pushes [`Diagnostic`]s onto the accumulator.
//! No render or parser imports — all geometry is already world-space inside
//! the context.

use std::collections::HashMap;

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

fn key(p: &Point) -> (i64, i64) {
    ((p.x * 100.0).round() as i64, (p.y * 100.0).round() as i64)
}

// ---------------------------------------------------------------------------
// Union-find (used by net_label_conflict and bus_bit_width_mismatch)
// ---------------------------------------------------------------------------

fn uf_find(parent: &mut HashMap<(i64, i64), (i64, i64)>, x: (i64, i64)) -> (i64, i64) {
    let p = *parent.entry(x).or_insert(x);
    if p == x {
        x
    } else {
        let r = uf_find(parent, p);
        parent.insert(x, r);
        r
    }
}

fn uf_union(
    parent: &mut HashMap<(i64, i64), (i64, i64)>,
    a: (i64, i64),
    b: (i64, i64),
) {
    let ra = uf_find(parent, a);
    let rb = uf_find(parent, b);
    if ra != rb {
        parent.insert(ra, rb);
    }
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
            let pos = &pin.world_pos;
            let connected = ctx.wires.iter().any(|w| same(&w.start, pos) || same(&w.end, pos))
                || ctx.buses.iter().any(|b| same(&b.start, pos) || same(&b.end, pos))
                || ctx.no_connects.iter().any(|nc| same(&nc.position, pos))
                || ctx.labels.iter().any(|l| same(&l.position, pos));
            if connected {
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
                    *pos,
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
    for label in &ctx.labels {
        if !matches!(label.label_type, LabelType::Hierarchical | LabelType::Global) {
            continue;
        }
        let touched = ctx
            .wires
            .iter()
            .any(|w| same(&w.start, &label.position) || same(&w.end, &label.position))
            || ctx
                .buses
                .iter()
                .any(|b| same(&b.start, &label.position) || same(&b.end, &label.position));
        if touched {
            continue;
        }
        out.push(
            Diagnostic::new(
                RuleKind::HierPortDisconnected,
                format!("{:?} port '{}' is not on a wire", label.label_type, label.text),
                label.position,
            )
            .with_primary(sel(label.uuid, SelectedKind::Label)),
        );
        // Silence unused-severity warning for Severity::Off.
        let _ = Severity::Off;
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
    let mut parent: HashMap<(i64, i64), (i64, i64)> = HashMap::new();
    for w in &ctx.wires {
        uf_union(&mut parent, key(&w.start), key(&w.end));
    }
    for j in &ctx.junctions {
        uf_find(&mut parent, key(&j.position));
    }

    let mut by_root: HashMap<(i64, i64), Vec<&crate::context::ErcLabel>> = HashMap::new();
    for lbl in &ctx.labels {
        if !matches!(lbl.label_type, LabelType::Net) {
            continue;
        }
        let root = uf_find(&mut parent, key(&lbl.position));
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
    for label in &ctx.labels {
        if matches!(label.label_type, LabelType::Hierarchical | LabelType::Global) {
            continue;
        }
        let on_wire = ctx
            .wires
            .iter()
            .any(|w| same(&w.start, &label.position) || same(&w.end, &label.position));
        let on_bus = ctx
            .buses
            .iter()
            .any(|b| same(&b.start, &label.position) || same(&b.end, &label.position));
        let on_pin = ctx
            .symbols
            .iter()
            .flat_map(|s| s.pins.iter())
            .any(|pin| same(&pin.world_pos, &label.position));
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
    let mut parent: HashMap<(i64, i64), (i64, i64)> = HashMap::new();
    for b in &ctx.buses {
        uf_union(&mut parent, key(&b.start), key(&b.end));
    }

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

    type Groups<'a> = HashMap<(i64, i64), Vec<(&'a crate::context::ErcLabel, (i64, i64))>>;
    let mut by_root: Groups<'_> = HashMap::new();
    for lbl in &ctx.labels {
        let Some((_base, lo, hi)) = parse_bus_label(&lbl.text) else {
            continue;
        };
        let root = uf_find(&mut parent, key(&lbl.position));
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
        let (majority_range, _) = range_counts
            .iter()
            .max_by_key(|&(_, c)| *c)
            .map(|(r, c)| (*r, *c))
            .expect("group len >= 2");
        let Some((ref_lbl, ref_range)) =
            group.iter().find(|(_, r)| *r == majority_range).copied()
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
            .filter(|l| {
                matches!(l.label_type, LabelType::Hierarchical | LabelType::Global)
            })
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
    let label_texts: std::collections::HashSet<&str> =
        ctx.labels.iter().map(|l| l.text.as_str()).collect();
    for symbol in &ctx.symbols {
        if !symbol.is_power {
            continue;
        }
        let name = symbol.value.as_str();
        if name.is_empty() || label_texts.contains(name) {
            continue;
        }
        out.push(
            Diagnostic::new(
                RuleKind::MissingPowerFlag,
                format!(
                    "Power port '{name}' is not cross-referenced by a label — add a PWR_FLAG if this is a source net",
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
    let power: Vec<&crate::context::ErcSymbol> =
        ctx.symbols.iter().filter(|s| s.is_power && !s.value.is_empty()).collect();

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
                    format!(
                        "Symbol '{reference}' sits outside the {}×{} mm sheet",
                        w, h,
                    ),
                    symbol.position,
                )
                .with_primary(sel(symbol.uuid, SelectedKind::Symbol)),
            );
        }
    }
}
