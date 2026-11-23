//! Multi-channel sheets (#1279): a `Repeat(<net>)` entry on a repeated sheet
//! symbol gives one net per channel, and channel k joins member k of the bus
//! the entry meets on this sheet. Too few members leave channels unjoined.
//! The stitching rule is in `docs/MULTI_CHANNEL.md`.

use signex_net::{BusGeometry, EntryBuses};
use signex_types::schematic::{ChannelRepeat, Point, SelectedKind, repeated_entry_name};

use crate::context::ErcContext;
use crate::diagnostic::Diagnostic;
use crate::{RuleKind, sel};

/// The bus runs of the sheet, named the way the netlist names them for a
/// sheet entry ([`EntryBuses::texts`]).
fn entry_buses(ctx: &ErcContext) -> EntryBuses {
    let buses: Vec<(Point, Point)> = ctx.buses.iter().map(|b| (b.start, b.end)).collect();
    let entries: Vec<(Point, (f64, f64))> = ctx
        .bus_entries
        .iter()
        .map(|e| (e.position, e.size))
        .collect();
    let wires: Vec<(Point, Point)> = ctx.wires.iter().map(|w| (w.start, w.end)).collect();
    let junctions: Vec<Point> = ctx.junctions.iter().map(|j| j.position).collect();
    let labels = ctx.labels.iter().map(|l| (l.position, l.text.as_str()));
    let pins = ctx
        .child_sheets
        .iter()
        .flat_map(|s| s.pins.iter())
        .map(|p| (p.position, p.name.as_str()));
    let texts = EntryBuses::texts(labels, pins);
    EntryBuses::new(&BusGeometry {
        buses: &buses,
        entries: &entries,
        wires: &wires,
        junctions: &junctions,
        labels: &texts,
    })
}

/// A `Repeat(<net>)` entry of a repeated sheet symbol whose bus on this
/// sheet carries fewer members than the symbol has channels (or that meets
/// no bus at all).
pub(crate) fn repeat_entry_needs_bus(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    let repeated: Vec<_> = ctx
        .child_sheets
        .iter()
        .filter_map(|s| ChannelRepeat::parse(&s.name).map(|r| (s, r)))
        .collect();
    if repeated.is_empty() {
        return;
    }
    let buses = entry_buses(ctx);
    for (sheet, repeat) in repeated {
        let needed = repeat.count() as usize;
        for pin in &sheet.pins {
            let Some(net) = repeated_entry_name(&pin.name) else {
                continue;
            };
            let members = buses
                .run_at(pin.position)
                .map_or(0, |r| buses.members(r).len());
            if members >= needed {
                continue;
            }
            out.push(
                Diagnostic::new(
                    RuleKind::RepeatEntryNeedsBus,
                    format!(
                        "Repeat entry {net} needs a bus of {needed} members on the parent sheet"
                    ),
                    pin.position,
                )
                .with_primary(sel(pin.uuid, SelectedKind::SheetPin)),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use signex_types::schematic::{LabelType, Point};
    use uuid::Uuid;

    use crate::RuleKind;
    use crate::context::{ErcBus, ErcChildSheet, ErcContext, ErcSheetPin};
    use crate::diagnostic::Diagnostic;
    use crate::fixtures::{empty, label};

    fn pt(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    fn pin(name: &str, at: Point) -> ErcSheetPin {
        ErcSheetPin {
            uuid: Uuid::now_v7(),
            name: name.into(),
            position: at,
            direction: String::new(),
            connected: true,
        }
    }

    /// A repeated symbol named `designator` with a `Repeat(VOUT)` entry at
    /// (10, 0), and a bus (10, 0)-(40, 0) named `bus_name` when given.
    fn parent(designator: &str, bus_name: Option<&str>) -> ErcContext {
        let mut c = empty();
        c.child_sheets.push(ErcChildSheet {
            uuid: Uuid::now_v7(),
            name: designator.into(),
            filename: "channel.snxsch".into(),
            position: pt(0.0, 0.0),
            pins: vec![pin("Repeat(VOUT)", pt(10.0, 0.0)), pin("VIN", pt(0.0, 5.0))],
        });
        if let Some(name) = bus_name {
            c.buses.push(ErcBus {
                uuid: Uuid::now_v7(),
                start: pt(10.0, 0.0),
                end: pt(40.0, 0.0),
            });
            c.labels.push(label(name, pt(25.0, 0.0), LabelType::Net));
        }
        c
    }

    fn found(c: &ErcContext) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        super::repeat_entry_needs_bus(c, &mut out);
        out
    }

    #[test]
    fn a_wide_enough_bus_is_fine() {
        assert!(found(&parent("Repeat(CH, 1, 4)", Some("VOUT[1..4]"))).is_empty());
        assert!(found(&parent("Repeat(CH, 1, 4)", Some("VOUT[1..8]"))).is_empty());
    }

    #[test]
    fn a_narrow_bus_is_a_finding() {
        let d = found(&parent("Repeat(CH, 1, 4)", Some("VOUT[1..3]")));
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].rule_kind, RuleKind::RepeatEntryNeedsBus);
        assert_eq!(
            d[0].message,
            "Repeat entry VOUT needs a bus of 4 members on the parent sheet"
        );
    }

    #[test]
    fn no_bus_at_all_is_a_finding() {
        let d = found(&parent("Repeat(CH, 1, 2)", None));
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].location, pt(10.0, 0.0));
    }

    #[test]
    fn a_bus_named_by_another_entry_counts() {
        let mut c = parent("Repeat(CH, 1, 4)", None);
        c.buses.push(ErcBus {
            uuid: Uuid::now_v7(),
            start: pt(10.0, 0.0),
            end: pt(40.0, 0.0),
        });
        c.child_sheets.push(ErcChildSheet {
            uuid: Uuid::now_v7(),
            name: "mcu".into(),
            filename: "mcu.snxsch".into(),
            position: pt(50.0, 0.0),
            pins: vec![pin("VOUT[1..4]", pt(40.0, 0.0))],
        });
        assert!(found(&c).is_empty());
    }

    #[test]
    fn a_plain_sheet_symbol_is_not_checked() {
        assert!(found(&parent("channel", None)).is_empty());
    }

    #[test]
    fn the_engine_runs_the_rule() {
        let all = crate::engine::run_all(&parent("Repeat(CH, 1, 4)", None));
        assert!(
            all.iter()
                .any(|d| d.rule_kind == RuleKind::RepeatEntryNeedsBus)
        );
    }
}
