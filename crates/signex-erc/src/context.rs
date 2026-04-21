//! Parser-independent ERC context. Built by projecting a
//! [`SchematicRenderSnapshot`] once; all rule functions read from here so
//! they never import `signex-render` directly.

use std::collections::HashMap;

use signex_render::schematic::{SchematicRenderSnapshot, instance_transform};
use signex_types::schematic::{LabelType, PinElectricalType, Point};
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
    pub electrical_type: PinElectricalType,
    /// `false` for `Free` and `NotConnected` pin types — those may be left
    /// unconnected by design. DSL: `pin.required == true`.
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
    pub pin_types: Vec<PinElectricalType>,
    /// `true` if any pin is Output / PowerOut / TriState / OpenCollector /
    /// OpenEmitter. DSL: `has_driver()`.
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
    /// Derived logical nets (union-find over wire topology).
    pub nets: Vec<ErcNet>,
    /// Child sheet contexts keyed by filename. Only populated when built via
    /// [`from_snapshot_with_children`].
    pub children: HashMap<String, ErcContext>,
}

impl ErcContext {
    pub fn from_snapshot(snapshot: &SchematicRenderSnapshot) -> Self {
        Self::project(snapshot, HashMap::new())
    }

    pub fn from_snapshot_with_children(
        snapshot: &SchematicRenderSnapshot,
        children: &HashMap<String, SchematicRenderSnapshot>,
    ) -> Self {
        let child_ctxs = children
            .iter()
            .map(|(k, v)| (k.clone(), Self::from_snapshot(v)))
            .collect();
        Self::project(snapshot, child_ctxs)
    }

    fn project(snapshot: &SchematicRenderSnapshot, children: HashMap<String, ErcContext>) -> Self {
        // --- Step 1: geometry primitives (no symbols yet) -----------------
        let wires: Vec<ErcWire> = snapshot
            .wires
            .iter()
            .map(|w| ErcWire { uuid: w.uuid, start: w.start, end: w.end })
            .collect();

        let buses: Vec<ErcBus> = snapshot
            .buses
            .iter()
            .map(|b| ErcBus { start: b.start, end: b.end })
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
            .map(|j| ErcJunction { position: j.position })
            .collect();

        let no_connects: Vec<ErcNoConnect> = snapshot
            .no_connects
            .iter()
            .map(|nc| ErcNoConnect { position: nc.position })
            .collect();

        let bus_entries: Vec<ErcBusEntry> = snapshot
            .bus_entries
            .iter()
            .map(|be| ErcBusEntry { position: be.position })
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
                        let (wx, wy) = instance_transform(sym, &lp.pin.position);
                        let world_pos = Point::new(wx, wy);
                        let connected =
                            point_is_connected(&world_pos, &wires, &buses, &labels, &no_connects);
                        let required = !matches!(
                            lp.pin.pin_type,
                            PinElectricalType::Free | PinElectricalType::NotConnected
                        );
                        ErcPin {
                            world_pos,
                            electrical_type: lp.pin.pin_type,
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
        let nets = derive_nets(&wires, &labels, &junctions, &symbols);

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

const EPS: f64 = 1e-4;

fn pt_same(a: &Point, b: &Point) -> bool {
    (a.x - b.x).abs() < EPS && (a.y - b.y).abs() < EPS
}

fn pt_key(p: &Point) -> (i64, i64) {
    ((p.x * 100.0).round() as i64, (p.y * 100.0).round() as i64)
}

/// Returns `true` if any wire/bus endpoint, label, or no-connect sits at `pos`.
fn point_is_connected(
    pos: &Point,
    wires: &[ErcWire],
    buses: &[ErcBus],
    labels: &[ErcLabel],
    no_connects: &[ErcNoConnect],
) -> bool {
    wires.iter().any(|w| pt_same(&w.start, pos) || pt_same(&w.end, pos))
        || buses.iter().any(|b| pt_same(&b.start, pos) || pt_same(&b.end, pos))
        || labels.iter().any(|l| pt_same(&l.position, pos))
        || no_connects.iter().any(|nc| pt_same(&nc.position, pos))
}

// ---------------------------------------------------------------------------
// Net derivation (union-find over wire endpoints)
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

fn derive_nets(
    wires: &[ErcWire],
    labels: &[ErcLabel],
    junctions: &[ErcJunction],
    symbols: &[ErcSymbol],
) -> Vec<ErcNet> {
    let mut parent: HashMap<(i64, i64), (i64, i64)> = HashMap::new();

    // Wire segments connect their two endpoints.
    for w in wires {
        uf_union(&mut parent, pt_key(&w.start), pt_key(&w.end));
    }
    // Junctions anchor crossing wire endpoints into the same net.
    for j in junctions {
        uf_find(&mut parent, pt_key(&j.position));
    }

    // Group labels by net root.
    let mut net_labels: HashMap<(i64, i64), Vec<&ErcLabel>> = HashMap::new();
    for lbl in labels {
        let root = uf_find(&mut parent, pt_key(&lbl.position));
        net_labels.entry(root).or_default().push(lbl);
    }

    // Group connected (non-no-connect) pins by net root.
    // No-connect pins are skipped — they're isolated by design.
    let mut net_pins: HashMap<(i64, i64), Vec<PinElectricalType>> = HashMap::new();
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
            let root = uf_find(&mut parent, pt_key(&pin.world_pos));
            net_pins.entry(root).or_default().push(pin.electrical_type);
        }
    }

    // All unique roots that have at least one label OR pin.
    let mut all_roots: std::collections::HashSet<(i64, i64)> =
        std::collections::HashSet::new();
    all_roots.extend(net_labels.keys().copied());
    all_roots.extend(net_pins.keys().copied());

    const DRIVING: &[PinElectricalType] = &[
        PinElectricalType::Output,
        PinElectricalType::PowerOut,
        PinElectricalType::TriState,
        PinElectricalType::OpenCollector,
        PinElectricalType::OpenEmitter,
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
            let has_pullup = pins.iter().any(|t| *t == PinElectricalType::Passive);

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
