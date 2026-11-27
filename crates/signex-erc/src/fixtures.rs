//! Hand-built [`ErcContext`] fixtures, one per built-in rule, shared by the
//! severity tests. Each fixture is minimal and triggers its rule.

use std::collections::HashMap;

use signex_types::schematic::{LabelType, PinDirection, Point};
use uuid::Uuid;

use crate::context::{
    ErcBus, ErcBusEntry, ErcChildSheet, ErcContext, ErcLabel, ErcNet, ErcNetPin, ErcPin,
    ErcSheetPin, ErcSymbol, ErcWire, PaperSize,
};
use crate::{RuleKind, Violation, engine};

fn pt(x: f64, y: f64) -> Point {
    Point { x, y }
}

pub(crate) fn empty() -> ErcContext {
    ErcContext {
        paper_size: PaperSize::A4,
        symbols: Vec::new(),
        wires: Vec::new(),
        buses: Vec::new(),
        labels: Vec::new(),
        junctions: Vec::new(),
        no_connects: Vec::new(),
        no_erc_directives: Vec::new(),
        directives: Vec::new(),
        bus_entries: Vec::new(),
        child_sheets: Vec::new(),
        nets: Vec::new(),
        children: HashMap::new(),
        key: None,
        harness: Default::default(),
        images: Vec::new(),
    }
}

pub(crate) fn wire(a: Point, b: Point) -> ErcWire {
    ErcWire {
        uuid: Uuid::now_v7(),
        start: a,
        end: b,
    }
}

pub(crate) fn label(text: &str, at: Point, label_type: LabelType) -> ErcLabel {
    ErcLabel {
        uuid: Uuid::now_v7(),
        text: text.into(),
        position: at,
        label_type,
    }
}

pub(crate) fn symbol(reference: &str, at: Point) -> ErcSymbol {
    ErcSymbol {
        uuid: Uuid::now_v7(),
        reference: reference.into(),
        instance_designators: Vec::new(),
        lib_id: String::new(),
        unit: 1,
        value: String::new(),
        footprint: String::new(),
        on_board: true,
        unit_count: 1,
        position: at,
        is_power: false,
        pins: Vec::new(),
        attrs: HashMap::new(),
    }
}

fn power_port(value: &str, at: Point) -> ErcSymbol {
    ErcSymbol {
        value: value.into(),
        footprint: String::new(),
        on_board: true,
        unit_count: 1,
        is_power: true,
        ..symbol("#PWR01", at)
    }
}

fn sheet_pin(name: &str) -> ErcSheetPin {
    ErcSheetPin {
        uuid: Uuid::now_v7(),
        name: name.into(),
        position: pt(1.0, 1.0),
        direction: String::new(),
        connected: false,
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
                local_pos: pt(10.0, 10.0),
                number: "1".into(),
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
                uuid: Uuid::now_v7(),
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
                uuid: Uuid::now_v7(),
                name: "Dup".into(),
                filename: "dup.snxsch".into(),
                position: pt(0.0, 0.0),
                pins: vec![sheet_pin("P"), sheet_pin("P")],
            });
            c.child_sheets.push(ErcChildSheet {
                uuid: Uuid::now_v7(),
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
        RuleKind::PinTypeConflict => {
            // Two outputs on one wire.
            c.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
            for (reference, at) in [("U1", pt(0.0, 0.0)), ("U2", pt(10.0, 0.0))] {
                let mut s = symbol(reference, at);
                s.pins.push(ErcPin {
                    world_pos: at,
                    local_pos: at,
                    number: "1".into(),
                    electrical_type: PinDirection::Output,
                    required: true,
                    connected: true,
                });
                c.symbols.push(s);
            }
            c.nets.push(net_of(&c.symbols));
        }
        RuleKind::BusUnnamed => c.buses.push(fixture_bus()),
        RuleKind::BusLabelOnWire => {
            c.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
            c.labels
                .push(label("D[0..7]", pt(5.0, 0.0), LabelType::Net));
        }
        RuleKind::BusNameConflict => {
            c.buses.push(fixture_bus());
            c.labels
                .push(label("D[0..7]", pt(0.0, 10.0), LabelType::Net));
            c.labels
                .push(label("D[8..15]", pt(0.0, 15.0), LabelType::Net));
        }
        RuleKind::BusEntryUnlabeled => {
            c.buses.push(fixture_bus());
            c.labels
                .push(label("D[0..7]", pt(0.0, 10.0), LabelType::Net));
            c.bus_entries.push(fixture_entry());
            c.wires.push(wire(pt(2.54, 7.54), pt(12.54, 7.54)));
        }
        RuleKind::BusMemberNotInBus => {
            c.buses.push(fixture_bus());
            c.labels
                .push(label("D[0..7]", pt(0.0, 10.0), LabelType::Net));
            c.bus_entries.push(fixture_entry());
            c.wires.push(wire(pt(2.54, 7.54), pt(12.54, 7.54)));
            c.labels.push(label("D33", pt(6.0, 7.54), LabelType::Net));
        }
        RuleKind::PinsTouchNotConnected => {
            for r in ["R1", "R2"] {
                let mut s = symbol(r, pt(10.0, 10.0));
                s.pins.push(ErcPin {
                    world_pos: pt(10.0, 10.0),
                    local_pos: pt(10.0, 10.0),
                    number: "1".into(),
                    electrical_type: PinDirection::Passive,
                    required: true,
                    connected: false,
                });
                c.symbols.push(s);
            }
        }
        RuleKind::NetMultipleNames => {
            c.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
            c.labels.push(label("A", pt(0.0, 0.0), LabelType::Net));
            c.labels
                .push(label("B", pt(10.0, 0.0), LabelType::Hierarchical));
        }
        RuleKind::LocalLabelNameOnOtherSheets => {
            let mut child = empty();
            child.key = Some(signex_net::SheetKey::new("child"));
            child
                .labels
                .push(label("SDA", pt(0.0, 0.0), LabelType::Net));
            c.labels.push(label("SDA", pt(0.0, 0.0), LabelType::Net));
            c.children
                .insert("child.snxsch".into(), std::sync::Arc::new(child));
        }
        RuleKind::PartWithoutFootprint | RuleKind::PartWithoutValue | RuleKind::PartWithoutMpn => {
            c.symbols.push(symbol("R1", pt(10.0, 10.0)));
        }
        RuleKind::PartWithoutLibraryBody => {
            let mut s = symbol("R1", pt(10.0, 10.0));
            s.unit_count = 0;
            c.symbols.push(s);
        }
        RuleKind::UnannotatedDesignator => c.symbols.push(symbol("R?", pt(10.0, 10.0))),
        RuleKind::SinglePinNet => {
            c.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
            c.labels.push(label("N", pt(10.0, 0.0), LabelType::Net));
            c.symbols
                .push(with_pin("U1", pt(0.0, 0.0), PinDirection::Passive));
        }
        RuleKind::NetWithNoDriver | RuleKind::PowerInputNotDriven => {
            let dir = if kind == RuleKind::NetWithNoDriver {
                PinDirection::Input
            } else {
                PinDirection::PowerInput
            };
            c.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
            c.symbols.push(with_pin("U1", pt(0.0, 0.0), dir));
            c.symbols.push(with_pin("U2", pt(10.0, 0.0), dir));
        }
        RuleKind::UnitsNotPlaced => {
            let mut s = symbol("U1", pt(10.0, 10.0));
            s.unit_count = 2;
            c.symbols.push(s);
        }
        RuleKind::OffGrid => c.wires.push(wire(pt(0.0, 0.0), pt(1.0, 0.0))),
        RuleKind::SheetPinUnconnected => c.child_sheets.push(ErcChildSheet {
            uuid: Uuid::now_v7(),
            name: "Sub".into(),
            filename: "sub.snxsch".into(),
            position: pt(0.0, 0.0),
            pins: vec![sheet_pin("IN")],
        }),
        RuleKind::SheetPinDirectionConflict => {
            c.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
            let output_pin = |at: Point| ErcSheetPin {
                position: at,
                direction: "output".into(),
                ..sheet_pin("OUT")
            };
            c.child_sheets.push(ErcChildSheet {
                uuid: Uuid::now_v7(),
                name: "Sub".into(),
                filename: "sub.snxsch".into(),
                position: pt(0.0, 0.0),
                pins: vec![output_pin(pt(0.0, 0.0)), output_pin(pt(10.0, 0.0))],
            });
        }
        RuleKind::LabelsOnlyNet => {
            c.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
            c.labels.push(label("N", pt(0.0, 0.0), LabelType::Net));
        }
        RuleKind::RepeatEntryNeedsBus => c.child_sheets.push(ErcChildSheet {
            uuid: Uuid::now_v7(),
            name: "Repeat(CH, 1, 4)".into(),
            filename: "channel.snxsch".into(),
            position: pt(0.0, 0.0),
            pins: vec![sheet_pin("Repeat(VOUT)")],
        }),
        RuleKind::HarnessEntryMismatch | RuleKind::HarnessTypeConflict => {
            return harness_context(kind);
        }
        RuleKind::ImageFileMissing => c.images.push(crate::images::ErcImage {
            uuid: Uuid::now_v7(),
            file: "assets/images/gone.png".into(),
            centre: pt(10.0, 10.0),
            missing: true,
        }),
    }
    c
}

/// A harness connector of type `ty` at `at` with entries `names` (#1280).
pub(crate) fn harness_connector(
    ty: &str,
    at: Point,
    names: &[&str],
) -> signex_types::schematic::HarnessConnector {
    signex_types::schematic::HarnessConnector {
        uuid: Uuid::now_v7(),
        harness_type: ty.into(),
        position: at,
        width: 10.16,
        entry_pitch: 2.54,
        rotation: 0.0,
        entries: names
            .iter()
            .map(|n| signex_types::schematic::HarnessEntry {
                uuid: Uuid::now_v7(),
                name: (*n).into(),
            })
            .collect(),
        locked: false,
    }
}

/// A definition `name` listing `entries`.
pub(crate) fn harness_definition(
    name: &str,
    entries: &[&str],
) -> signex_types::schematic::HarnessDefinition {
    signex_types::schematic::HarnessDefinition {
        name: name.into(),
        description: String::new(),
        entries: entries
            .iter()
            .map(|e| signex_types::schematic::HarnessDefinitionEntry {
                name: (*e).into(),
                harness: None,
            })
            .collect(),
    }
}

fn harness_wire(a: Point, b: Point) -> signex_types::schematic::HarnessWire {
    signex_types::schematic::HarnessWire {
        uuid: Uuid::now_v7(),
        start: a,
        end: b,
        locked: false,
    }
}

/// The harness rules' fixtures: a harness whose child side expects an entry
/// this side lacks, and two types on one harness.
fn harness_context(kind: RuleKind) -> ErcContext {
    use signex_types::schematic::{Label, SchematicSheet};
    let mut sheet = SchematicSheet::default();
    let a = harness_connector("SPI_BUS", pt(10.0, 10.0), &["SCK", "CS"]);
    if kind == RuleKind::HarnessTypeConflict {
        let b = harness_connector("UART", pt(40.0, 40.0), &["TX"]);
        sheet
            .harness
            .wires
            .push(harness_wire(a.tip_point(), b.tip_point()));
        sheet.harness.connectors.extend([a, b]);
        return ErcContext::from_snapshot(&sheet);
    }
    // `a` runs to the typed entry SPI_BUS of the sheet symbol `Sub`.
    let to = pt(60.0, 20.0);
    sheet.harness.wires.push(harness_wire(a.tip_point(), to));
    let pin = signex_types::schematic::SheetPin {
        uuid: Uuid::now_v7(),
        name: "SPI_BUS".into(),
        direction: String::new(),
        position: to,
        rotation: 0.0,
        auto_generated: false,
        user_moved: false,
    };
    sheet.harness.entry_types.insert(pin.uuid, "SPI_BUS".into());
    sheet
        .child_sheets
        .push(signex_types::schematic::ChildSheet {
            uuid: Uuid::now_v7(),
            name: "Sub".into(),
            filename: "sub.snxsch".into(),
            position: pt(60.0, 10.0),
            size: (20.0, 20.0),
            stroke_width: 0.0,
            fill: signex_types::schematic::FillType::None,
            stroke_color: None,
            fill_color: None,
            fields_autoplaced: false,
            pins: vec![pin],
            instances: Vec::new(),
        });
    sheet.harness.connectors.push(a);
    // The child's port SPI_BUS runs to a connector expecting CS_ADC.
    let mut child = SchematicSheet::default();
    let far = harness_connector("SPI_BUS", pt(30.0, 10.0), &["SCK", "CS_ADC"]);
    let port = pt(5.0, 5.0);
    child
        .harness
        .wires
        .push(harness_wire(port, far.tip_point()));
    child
        .labels
        .push(Label::new("SPI_BUS", port, LabelType::Hierarchical));
    child.harness.connectors.push(far);
    let mut ctx = ErcContext::from_snapshot(&sheet);
    ctx.children.insert(
        "sub.snxsch".into(),
        std::sync::Arc::new(ErcContext::from_snapshot(&child)),
    );
    ctx
}

/// A part `reference` with one connected pin of `direction` at `at`.
fn with_pin(reference: &str, at: Point, direction: PinDirection) -> ErcSymbol {
    let mut s = symbol(reference, at);
    s.pins.push(ErcPin {
        world_pos: at,
        local_pos: at,
        number: "1".into(),
        electrical_type: direction,
        required: true,
        connected: true,
    });
    s
}

/// One net holding every pin of `symbols`, as `summarize_nets` would build it
/// for symbols whose pins all sit on one wire.
pub(crate) fn net_of(symbols: &[ErcSymbol]) -> ErcNet {
    let pins: Vec<ErcNetPin> = symbols
        .iter()
        .flat_map(|s| {
            s.pins.iter().map(|p| ErcNetPin {
                symbol: s.uuid,
                occurrence: signex_types::schematic::InstancePath::root(),
                reference: s.reference.clone(),
                number: p.number.clone(),
                position: p.world_pos,
                local_pos: p.local_pos,
                direction: p.electrical_type,
                is_power_flag: s.is_power && signex_net::is_power_flag(&s.lib_id),
            })
        })
        .collect();
    ErcNet {
        name: String::new(),
        names: Vec::new(),
        class: String::new(),
        pin_types: pins.iter().map(|p| p.direction).collect(),
        pins,
        has_driver: false,
        has_pullup: false,
    }
}

/// Every violation of `kind` that the fixture for `kind` produces.
pub(crate) fn violations_for(kind: RuleKind) -> Vec<Violation> {
    engine::run_all(&context_for(kind))
        .into_iter()
        .map(Violation::from)
        .filter(|v| v.rule == kind)
        .collect()
}

/// A vertical bus (0,0)-(0,20).
fn fixture_bus() -> ErcBus {
    ErcBus {
        uuid: Uuid::now_v7(),
        start: pt(0.0, 0.0),
        end: pt(0.0, 20.0),
    }
}

/// An entry stub from the bus at (0,5) out to (2.54, 7.54).
fn fixture_entry() -> ErcBusEntry {
    ErcBusEntry {
        uuid: Uuid::now_v7(),
        position: pt(0.0, 5.0),
        size: (2.54, 2.54),
    }
}
