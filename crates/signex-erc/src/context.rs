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
// Sub-types
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
    /// World-space pin tip positions with their electrical types.
    pub pins: Vec<ErcPin>,
}

#[derive(Debug, Clone, Copy)]
pub struct ErcPin {
    pub world_pos: Point,
    pub electrical_type: PinElectricalType,
}

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
    /// Child sheet contexts keyed by the filename as it appears on the parent's
    /// sheet symbol. Only populated when built via [`from_snapshot_with_children`].
    pub children: HashMap<String, ErcContext>,
}

impl ErcContext {
    /// Project a single snapshot into an [`ErcContext`] with no children.
    pub fn from_snapshot(snapshot: &SchematicRenderSnapshot) -> Self {
        Self::project(snapshot, HashMap::new())
    }

    /// Project a snapshot and its child snapshots into an [`ErcContext`].
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
        let symbols = snapshot
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
                        ErcPin {
                            world_pos: Point::new(wx, wy),
                            electrical_type: lp.pin.pin_type,
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
                })
            })
            .collect();

        let wires = snapshot
            .wires
            .iter()
            .map(|w| ErcWire { uuid: w.uuid, start: w.start, end: w.end })
            .collect();

        let buses = snapshot
            .buses
            .iter()
            .map(|b| ErcBus { start: b.start, end: b.end })
            .collect();

        let labels = snapshot
            .labels
            .iter()
            .map(|l| ErcLabel {
                uuid: l.uuid,
                text: l.text.clone(),
                position: l.position,
                label_type: l.label_type,
            })
            .collect();

        let junctions = snapshot
            .junctions
            .iter()
            .map(|j| ErcJunction { position: j.position })
            .collect();

        let no_connects = snapshot
            .no_connects
            .iter()
            .map(|nc| ErcNoConnect { position: nc.position })
            .collect();

        let bus_entries = snapshot
            .bus_entries
            .iter()
            .map(|be| ErcBusEntry { position: be.position })
            .collect();

        let child_sheets = snapshot
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
            children,
        }
    }
}
