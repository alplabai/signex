//! Parser-independent ERC context. Built by projecting a
//! [`SchematicSheet`] once; all rule functions read from here so
//! they stay independent from renderer internals.

use std::collections::HashMap;

use signex_net::SheetConnectivity;
use signex_types::schematic::SchematicSheet;
use signex_types::schematic::{LabelType, PinDirection, Point, SymbolTransform};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Paper size
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaperSize {
    A4,
    A3,
    A2,
    A1,
    A0,
}

impl PaperSize {
    /// Returns `(width_mm, height_mm)` for **landscape** orientation
    /// (long side first). LO-5: schematic ERC currently treats every
    /// sheet as landscape; if the schema ever grows an explicit
    /// orientation flag (e.g. `A4_L` vs `A4`), the consumer should
    /// swap the tuple instead of expecting this method to do it.
    pub fn dimensions_mm(self) -> (f64, f64) {
        match self {
            PaperSize::A4 => (297.0, 210.0),
            PaperSize::A3 => (420.0, 297.0),
            PaperSize::A2 => (594.0, 420.0),
            PaperSize::A1 => (841.0, 594.0),
            PaperSize::A0 => (1189.0, 841.0),
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "A3" => PaperSize::A3,
            "A2" => PaperSize::A2,
            "A1" => PaperSize::A1,
            "A0" => PaperSize::A0,
            _ => PaperSize::A4,
        }
    }
}

// ---------------------------------------------------------------------------
// ErcPin
// ---------------------------------------------------------------------------

/// A single pin instance in world-space, ready for rule evaluation.
#[derive(Debug, Clone, Copy)]
pub struct ErcPin {
    pub world_pos: Point,
    pub electrical_type: PinDirection,
    /// `false` for `Unclassified` and `DoNotConnect` pin types — those may be
    /// left unconnected by design. DSL: `pin.required == true`.
    pub required: bool,
    /// `true` if a wire endpoint, bus endpoint, label, or no-connect marker
    /// sits at this pin's world-space tip. DSL: `pin.connected == false`.
    pub connected: bool,
}

// ---------------------------------------------------------------------------
// ErcSymbol
// ---------------------------------------------------------------------------

/// A placed component instance. Its `pins` are already transformed to
/// world-space coordinates (rotation + mirror applied during projection).
#[derive(Debug, Clone)]
pub struct ErcSymbol {
    pub uuid: Uuid,
    pub reference: String,
    pub value: String,
    pub position: Point,
    pub is_power: bool,
    /// World-space pin instances with connectivity pre-computed.
    pub pins: Vec<ErcPin>,
    /// Component attributes / fields (from symbol properties).
    /// DSL: `component.attr("class")`.
    pub attrs: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// Wire / Bus / Label / Junction / NoConnect / BusEntry / ChildSheet
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct ErcWire {
    pub uuid: Uuid,
    pub start: Point,
    pub end: Point,
}

#[derive(Debug, Clone, Copy)]
pub struct ErcBus {
    pub start: Point,
    pub end: Point,
}

#[derive(Debug, Clone)]
pub struct ErcLabel {
    pub uuid: Uuid,
    pub text: String,
    pub position: Point,
    pub label_type: LabelType,
}

#[derive(Debug, Clone, Copy)]
pub struct ErcJunction {
    pub position: Point,
}

#[derive(Debug, Clone, Copy)]
pub struct ErcNoConnect {
    pub position: Point,
}

#[derive(Debug, Clone, Copy)]
pub struct ErcBusEntry {
    pub position: Point,
}

#[derive(Debug, Clone)]
pub struct ErcChildSheet {
    pub uuid: Uuid,
    pub name: String,
    pub filename: String,
    pub position: Point,
    pub pins: Vec<ErcSheetPin>,
}

#[derive(Debug, Clone)]
pub struct ErcSheetPin {
    pub uuid: Uuid,
    pub name: String,
    pub position: Point,
}

// ---------------------------------------------------------------------------
// ErcNet  (derived — not directly from the snapshot)
// ---------------------------------------------------------------------------

/// A logical net: the set of pins and labels connected by wires/junctions.
/// Derived during projection via union-find over wire endpoints.
#[derive(Debug, Clone)]
pub struct ErcNet {
    /// Net name from the highest-priority label on this net (empty = unnamed).
    pub name: String,
    /// First word of `name` before `_`, lowercased. E.g. `"i2c"` from
    /// `"I2C_SDA"`. Equals the full lowercased name when no `_` is present.
    /// DSL: `net.class == "i2c"`.
    pub class: String,
    /// Electrical types of every pin on this net. DSL: `has_pin_kind(...)`.
    pub pin_types: Vec<PinDirection>,
    /// `true` if any pin is Output / PowerOutput / ThreeStatable /
    /// OpenDrainLow / OpenDrainHigh. DSL: `has_driver()`.
    pub has_driver: bool,
    /// `true` if any pin is Passive (rough pull-up heuristic).
    /// DSL: `has_pullup()`.
    pub has_pullup: bool,
}

// ---------------------------------------------------------------------------
// ErcContext
// ---------------------------------------------------------------------------

/// Normalised, render-independent view of a schematic sheet for ERC purposes.
/// Built once per run via [`ErcContext::from_snapshot`]; all rules read from it.
#[derive(Debug, Clone)]
pub struct ErcContext {
    pub paper_size: PaperSize,
    pub symbols: Vec<ErcSymbol>,
    pub wires: Vec<ErcWire>,
    pub buses: Vec<ErcBus>,
    pub labels: Vec<ErcLabel>,
    pub junctions: Vec<ErcJunction>,
    pub no_connects: Vec<ErcNoConnect>,
    pub bus_entries: Vec<ErcBusEntry>,
    pub child_sheets: Vec<ErcChildSheet>,
    /// Logical net summaries for the DSL, projected off the shared
    /// [`SheetConnectivity`] derivation (see `summarize_nets`).
    pub nets: Vec<ErcNet>,
    /// Child sheet contexts keyed by filename. Only populated when built via
    /// [`from_snapshot_with_children`].
    pub children: HashMap<String, ErcContext>,
}

impl ErcContext {
    pub fn from_snapshot(snapshot: &SchematicSheet) -> Self {
        Self::project(snapshot, HashMap::new())
    }

    pub fn from_snapshot_with_children(
        snapshot: &SchematicSheet,
        children: &HashMap<String, SchematicSheet>,
    ) -> Self {
        let child_ctxs = children
            .iter()
            .map(|(k, v)| (k.clone(), Self::from_snapshot(v)))
            .collect();
        Self::project(snapshot, child_ctxs)
    }

    fn project(snapshot: &SchematicSheet, children: HashMap<String, ErcContext>) -> Self {
        // --- Step 1: geometry primitives (no symbols yet) -----------------
        let wires: Vec<ErcWire> = snapshot
            .wires
            .iter()
            .map(|w| ErcWire {
                uuid: w.uuid,
                start: w.start,
                end: w.end,
            })
            .collect();

        let buses: Vec<ErcBus> = snapshot
            .buses
            .iter()
            .map(|b| ErcBus {
                start: b.start,
                end: b.end,
            })
            .collect();

        let labels: Vec<ErcLabel> = snapshot
            .labels
            .iter()
            .map(|l| ErcLabel {
                uuid: l.uuid,
                text: l.text.clone(),
                position: l.position,
                label_type: l.label_type,
            })
            .collect();

        let junctions: Vec<ErcJunction> = snapshot
            .junctions
            .iter()
            .map(|j| ErcJunction {
                position: j.position,
            })
            .collect();

        let no_connects: Vec<ErcNoConnect> = snapshot
            .no_connects
            .iter()
            .map(|nc| ErcNoConnect {
                position: nc.position,
            })
            .collect();

        let bus_entries: Vec<ErcBusEntry> = snapshot
            .bus_entries
            .iter()
            .map(|be| ErcBusEntry {
                position: be.position,
            })
            .collect();

        let child_sheets: Vec<ErcChildSheet> = snapshot
            .child_sheets
            .iter()
            .map(|cs| ErcChildSheet {
                uuid: cs.uuid,
                name: cs.name.clone(),
                filename: cs.filename.clone(),
                position: cs.position,
                pins: cs
                    .pins
                    .iter()
                    .map(|p| ErcSheetPin {
                        uuid: p.uuid,
                        name: p.name.clone(),
                        position: p.position,
                    })
                    .collect(),
            })
            .collect();

        // --- Step 2: symbols with pre-computed pin connectivity -----------
        let symbols: Vec<ErcSymbol> = snapshot
            .symbols
            .iter()
            .filter_map(|sym| {
                let lib_sym = snapshot.lib_symbols.get(&sym.lib_id)?;
                let pins = lib_sym
                    .pins
                    .iter()
                    .filter(|lp| lp.unit == 0 || lp.unit == sym.unit)
                    .map(|lp| {
                        let _world = SymbolTransform::from_symbol(sym).apply(lp.pin.position);
                        let (wx, wy) = (_world.x, _world.y);
                        let world_pos = Point::new(wx, wy);
                        let connected = point_is_connected(
                            &world_pos,
                            &wires,
                            &junctions,
                            &labels,
                            &no_connects,
                        );
                        let required = !matches!(
                            lp.pin.direction,
                            PinDirection::Unclassified | PinDirection::DoNotConnect
                        );
                        ErcPin {
                            world_pos,
                            electrical_type: lp.pin.direction,
                            required,
                            connected,
                        }
                    })
                    .collect();

                Some(ErcSymbol {
                    uuid: sym.uuid,
                    reference: sym.reference.clone(),
                    value: sym.value.clone(),
                    position: sym.position,
                    is_power: sym.is_power,
                    pins,
                    attrs: sym.fields.clone(),
                })
            })
            .collect();

        // --- Step 3: derive logical nets ----------------------------------
        let nets = summarize_nets(&wires, &labels, &junctions, &symbols);

        ErcContext {
            paper_size: PaperSize::parse(&snapshot.paper_size),
            symbols,
            wires,
            buses,
            labels,
            junctions,
            no_connects,
            bus_entries,
            child_sheets,
            nets,
            children,
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// MD-6: see `rules::key` — the 1 µm bucket, the single "same point" metric so
/// context + rule projections agree on net membership (D5.5).
fn pt_key(p: &Point) -> (i64, i64) {
    ((p.x * 1000.0).round() as i64, (p.y * 1000.0).round() as i64)
}

/// True when a wire endpoint, junction, label, or no-connect sits at `pos`,
/// compared in the 1 µm key space — the same "same point" definition the
/// union-find uses, so the gate and the net partition never disagree (D5.5).
/// Junctions count: a pin may tap a wire mid-span where a junction sits (D5.3).
/// Buses do not: a bundle is never unioned, so gating on a bus endpoint minted
/// a one-terminal phantom net and a spurious unconnected-pin warning (D5.4).
fn point_is_connected(
    pos: &Point,
    wires: &[ErcWire],
    junctions: &[ErcJunction],
    labels: &[ErcLabel],
    no_connects: &[ErcNoConnect],
) -> bool {
    let k = pt_key(pos);
    wires
        .iter()
        .any(|w| pt_key(&w.start) == k || pt_key(&w.end) == k)
        || junctions.iter().any(|j| pt_key(&j.position) == k)
        || labels.iter().any(|l| pt_key(&l.position) == k)
        || no_connects.iter().any(|nc| pt_key(&nc.position) == k)
}

// ---------------------------------------------------------------------------
// Net summaries for the DSL
// ---------------------------------------------------------------------------

/// Summarise each geometric net into the flat [`ErcNet`] the ERC DSL reads:
/// its highest-priority label name, a coarse class, and the electrical types of
/// the pins on it.
///
/// Connectivity is **not** derived here — it comes from the shared
/// [`SheetConnectivity`] (the same wire-endpoint union plus junction T-merge
/// `build_netlist` uses), so ERC and the netlist agree on membership by
/// construction instead of ERC hand-rolling a second union-find.
fn summarize_nets(
    wires: &[ErcWire],
    labels: &[ErcLabel],
    junctions: &[ErcJunction],
    symbols: &[ErcSymbol],
) -> Vec<ErcNet> {
    let segments: Vec<(Point, Point)> = wires.iter().map(|w| (w.start, w.end)).collect();
    let junction_pts: Vec<Point> = junctions.iter().map(|j| j.position).collect();
    let mut conn = SheetConnectivity::from_segments(&segments, &junction_pts);

    // Group labels by net root.
    let mut net_labels: HashMap<(i64, i64), Vec<&ErcLabel>> = HashMap::new();
    for lbl in labels {
        let root = conn.root_of(&lbl.position);
        net_labels.entry(root).or_default().push(lbl);
    }

    // Group connected (non-no-connect) pins by net root.
    // No-connect pins are skipped — they're isolated by design.
    let mut net_pins: HashMap<(i64, i64), Vec<PinDirection>> = HashMap::new();
    for sym in symbols {
        for pin in &sym.pins {
            // Only wire- or label-connected pins belong to a logical net.
            if !pin.connected {
                continue;
            }
            // Skip pins that are exclusively covered by a no-connect marker.
            // We can't distinguish nc-only from wire-connected here without
            // re-probing, so we include everything that's "connected" — the
            // no-connect case creates a small singleton net that harms nothing.
            let root = conn.root_of(&pin.world_pos);
            net_pins.entry(root).or_default().push(pin.electrical_type);
        }
    }

    // All unique roots that have at least one label OR pin.
    let mut all_roots: std::collections::HashSet<(i64, i64)> = std::collections::HashSet::new();
    all_roots.extend(net_labels.keys().copied());
    all_roots.extend(net_pins.keys().copied());

    const DRIVING: &[PinDirection] = &[
        PinDirection::Output,
        PinDirection::PowerOutput,
        PinDirection::ThreeStatable,
        PinDirection::OpenDrainLow,
        PinDirection::OpenDrainHigh,
    ];

    all_roots
        .into_iter()
        .map(|root| {
            let lbls = net_labels.get(&root).map(Vec::as_slice).unwrap_or(&[]);
            let pins = net_pins.get(&root).map(Vec::as_slice).unwrap_or(&[]);

            // Highest-priority label name: Global > Power > Hierarchical > Net.
            let name = lbls
                .iter()
                .filter(|l| !l.text.is_empty())
                .max_by_key(|l| match l.label_type {
                    LabelType::Global => 3u8,
                    LabelType::Power => 2,
                    LabelType::Hierarchical => 1,
                    LabelType::Net => 0,
                })
                .map(|l| l.text.clone())
                .unwrap_or_default();

            let class = name
                .find('_')
                .map(|i| name[..i].to_ascii_lowercase())
                .unwrap_or_else(|| name.to_ascii_lowercase());

            let has_driver = pins.iter().any(|t| DRIVING.contains(t));
            let has_pullup = pins.contains(&PinDirection::Passive);

            ErcNet {
                name,
                class,
                pin_types: pins.to_vec(),
                has_driver,
                has_pullup,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn pin(pos: Point, dir: PinDirection) -> ErcPin {
        ErcPin {
            world_pos: pos,
            electrical_type: dir,
            required: true,
            connected: true,
        }
    }

    fn symbol(pins: Vec<ErcPin>) -> ErcSymbol {
        ErcSymbol {
            uuid: Uuid::nil(),
            reference: "U1".into(),
            value: String::new(),
            position: pt(0.0, 0.0),
            is_power: false,
            pins,
            attrs: HashMap::new(),
        }
    }

    #[test]
    fn junction_gates_connectivity_but_off_points_do_not() {
        // D5.3: a pin tapping a wire mid-span where a junction sits is
        // connected. D5.4: buses no longer gate, so a point that is only on a
        // bus (off every wire/junction/label) is not connected — no phantom net
        // or spurious unconnected-pin warning.
        let wires = vec![wire(pt(0.0, 0.0), pt(10.0, 0.0))];
        let junctions = vec![ErcJunction {
            position: pt(5.0, 0.0),
        }];
        let labels: Vec<ErcLabel> = Vec::new();
        let no_connects: Vec<ErcNoConnect> = Vec::new();
        assert!(
            point_is_connected(&pt(5.0, 0.0), &wires, &junctions, &labels, &no_connects),
            "pin on a mid-wire junction is connected"
        );
        assert!(
            point_is_connected(&pt(0.0, 0.0), &wires, &junctions, &labels, &no_connects),
            "pin on a wire endpoint is connected"
        );
        assert!(
            !point_is_connected(&pt(7.0, 3.0), &wires, &junctions, &labels, &no_connects),
            "a point off every wire/junction/label is not connected"
        );
    }

    #[test]
    fn t_junction_merges_wire_ending_on_another_wires_interior() {
        // Horizontal wire (0,0)-(10,0); a vertical wire (5,0)-(5,5)
        // ends on its interior; a junction dot sits at (5,0). A pin at
        // each far end must land on ONE net once the junction connects
        // the two wires. Regression for issue #107.
        let wires = vec![
            wire(pt(0.0, 0.0), pt(10.0, 0.0)),
            wire(pt(5.0, 0.0), pt(5.0, 5.0)),
        ];
        let junctions = vec![ErcJunction {
            position: pt(5.0, 0.0),
        }];
        let syms = vec![symbol(vec![
            pin(pt(0.0, 0.0), PinDirection::Output),
            pin(pt(5.0, 5.0), PinDirection::Input),
        ])];

        let nets = summarize_nets(&wires, &[], &junctions, &syms);
        assert_eq!(
            nets.len(),
            1,
            "a T-junction must merge both wires into one net"
        );
        assert_eq!(
            nets[0].pin_types.len(),
            2,
            "both pins belong to the merged net"
        );
    }

    #[test]
    fn t_intersection_without_junction_stays_two_nets() {
        // Same geometry, no junction dot: the connection is not
        // asserted, so the wires remain two separate nets. Documents
        // that the junction is what drives a T-connection.
        let wires = vec![
            wire(pt(0.0, 0.0), pt(10.0, 0.0)),
            wire(pt(5.0, 0.0), pt(5.0, 5.0)),
        ];
        let syms = vec![symbol(vec![
            pin(pt(0.0, 0.0), PinDirection::Output),
            pin(pt(5.0, 5.0), PinDirection::Input),
        ])];

        let nets = summarize_nets(&wires, &[], &[], &syms);
        assert_eq!(
            nets.len(),
            2,
            "without a junction the T is two separate nets"
        );
    }
}
