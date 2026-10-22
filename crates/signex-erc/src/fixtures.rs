//! Hand-built [`ErcContext`] fixtures, one per built-in rule, shared by the
//! severity tests. Each fixture is minimal and triggers its rule.

use std::collections::HashMap;

use signex_types::schematic::{LabelType, PinDirection, Point};
use uuid::Uuid;

use crate::context::{
    ErcBus, ErcChildSheet, ErcContext, ErcLabel, ErcPin, ErcSheetPin, ErcSymbol, ErcWire, PaperSize,
};
use crate::{RuleKind, Violation, engine};

fn pt(x: f64, y: f64) -> Point {
    Point { x, y }
}

fn empty() -> ErcContext {
    ErcContext {
        paper_size: PaperSize::A4,
        symbols: Vec::new(),
        wires: Vec::new(),
        buses: Vec::new(),
        labels: Vec::new(),
        junctions: Vec::new(),
        no_connects: Vec::new(),
        no_erc_directives: Vec::new(),
        bus_entries: Vec::new(),
        child_sheets: Vec::new(),
        nets: Vec::new(),
        children: HashMap::new(),
    }
}

fn wire(a: Point, b: Point) -> ErcWire {
    ErcWire {
        uuid: Uuid::new_v4(),
        start: a,
        end: b,
    }
}

fn label(text: &str, at: Point, label_type: LabelType) -> ErcLabel {
    ErcLabel {
        uuid: Uuid::new_v4(),
        text: text.into(),
        position: at,
        label_type,
    }
}

fn symbol(reference: &str, at: Point) -> ErcSymbol {
    ErcSymbol {
        uuid: Uuid::new_v4(),
        reference: reference.into(),
        value: String::new(),
        position: at,
        is_power: false,
        pins: Vec::new(),
        attrs: HashMap::new(),
    }
}

fn power_port(value: &str, at: Point) -> ErcSymbol {
    ErcSymbol {
        value: value.into(),
        is_power: true,
        ..symbol("#PWR01", at)
    }
}

fn sheet_pin(name: &str) -> ErcSheetPin {
    ErcSheetPin {
        uuid: Uuid::new_v4(),
        name: name.into(),
        position: pt(1.0, 1.0),
    }
}

/// A context that makes `kind` fire at least once.
pub(crate) fn context_for(kind: RuleKind) -> ErcContext {
    let mut c = empty();
    match kind {
        RuleKind::UnusedPin => {
            let mut s = symbol("U1", pt(10.0, 10.0));
            s.pins.push(ErcPin {
                world_pos: pt(10.0, 10.0),
                electrical_type: PinDirection::Passive,
                required: true,
                connected: false,
            });
            c.symbols.push(s);
        }
        RuleKind::DuplicateRefDesignator => {
            c.symbols.push(symbol("R1", pt(10.0, 10.0)));
            c.symbols.push(symbol("R1", pt(20.0, 10.0)));
        }
        RuleKind::HierPortDisconnected => {
            c.labels
                .push(label("SIG", pt(10.0, 10.0), LabelType::Global));
        }
        RuleKind::DanglingWire => c.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0))),
        RuleKind::NetLabelConflict => {
            c.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
            c.labels.push(label("A", pt(0.0, 0.0), LabelType::Net));
            c.labels.push(label("B", pt(10.0, 0.0), LabelType::Net));
        }
        RuleKind::OrphanLabel => c.labels.push(label("N", pt(5.0, 5.0), LabelType::Net)),
        RuleKind::BusBitWidthMismatch => {
            c.buses.push(ErcBus {
                start: pt(0.0, 0.0),
                end: pt(10.0, 0.0),
            });
            c.labels
                .push(label("D[0..7]", pt(0.0, 0.0), LabelType::Net));
            c.labels
                .push(label("D[0..3]", pt(10.0, 0.0), LabelType::Net));
        }
        RuleKind::BadHierSheetPin => {
            // Duplicate pin names (Error) plus a pin-less child (Warning).
            c.child_sheets.push(ErcChildSheet {
                uuid: Uuid::new_v4(),
                name: "Dup".into(),
                filename: "dup.snxsch".into(),
                position: pt(0.0, 0.0),
                pins: vec![sheet_pin("P"), sheet_pin("P")],
            });
            c.child_sheets.push(ErcChildSheet {
                uuid: Uuid::new_v4(),
                name: "Bare".into(),
                filename: "bare.snxsch".into(),
                position: pt(50.0, 0.0),
                pins: Vec::new(),
            });
        }
        RuleKind::MissingPowerFlag => c.symbols.push(power_port("+3V3", pt(10.0, 10.0))),
        RuleKind::PowerPortShort => {
            c.symbols.push(power_port("+3V3", pt(10.0, 10.0)));
            c.symbols.push(power_port("+5V", pt(10.0, 10.0)));
        }
        RuleKind::SymbolOutsideSheet => c.symbols.push(symbol("U1", pt(-5.0, -5.0))),
        RuleKind::AmbiguousLabelAnchor => {
            c.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
            c.wires.push(wire(pt(5.0, -5.0), pt(5.0, 5.0)));
            c.labels.push(label("X", pt(5.0, 0.0), LabelType::Net));
        }
    }
    c
}

/// Every violation of `kind` that the fixture for `kind` produces.
pub(crate) fn violations_for(kind: RuleKind) -> Vec<Violation> {
    engine::run_all(&context_for(kind))
        .into_iter()
        .map(Violation::from)
        .filter(|v| v.rule == kind)
        .collect()
}
