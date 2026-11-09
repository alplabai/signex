//! The ends of a headless bus run (#2199) that connect to nothing. Rules:
//! `docs/CLI.md`, "Bus geometry". Bus connectivity itself stays in
//! [`analyse_buses`](crate::analyse_buses); this only asks whether a point
//! has something to attach to.

use signex_types::schematic::{Point, SchematicSheet};

use crate::{RunEnd, SegmentIndex, pt_key};

/// The ends of the run `start`..`end` with no existing bus (end or interior),
/// bus entry or label at the point.
pub fn dangling_bus_ends(sheet: &SchematicSheet, start: Point, end: Point) -> Vec<RunEnd> {
    let buses: Vec<(Point, Point)> = sheet.buses.iter().map(|b| (b.start, b.end)).collect();
    let index = SegmentIndex::new(&buses);
    let attached = |p: &Point| {
        let key = pt_key(p);
        index.contains(key)
            || sheet.bus_entries.iter().any(|e| pt_key(&e.position) == key)
            || sheet.labels.iter().any(|l| pt_key(&l.position) == key)
    };
    [(RunEnd::Start, start), (RunEnd::End, end)]
        .into_iter()
        .filter(|(_, p)| !attached(p))
        .map(|(end, _)| end)
        .collect()
}

#[cfg(test)]
mod tests {
    use signex_types::schematic::{Bus, BusEntry, LabelType};
    use uuid::Uuid;

    use super::*;
    use crate::fixtures::{empty_sheet, label, pt};

    fn bus(a: Point, b: Point) -> Bus {
        Bus {
            uuid: Uuid::nil(),
            start: a,
            end: b,
        }
    }

    #[test]
    fn an_end_on_a_bus_end_or_interior_an_entry_or_a_label_is_attached() {
        let mut sheet = empty_sheet();
        sheet.buses.push(bus(pt(0.0, 0.0), pt(10.0, 0.0)));
        sheet.bus_entries.push(BusEntry {
            uuid: Uuid::nil(),
            position: pt(0.0, 20.0),
            size: (2.54, 2.54),
        });
        sheet
            .labels
            .push(label("D[0..7]", pt(0.0, 30.0), LabelType::Net));
        // Interior of the bus, an entry, a label: each is attached.
        for from in [pt(5.0, 0.0), pt(0.0, 20.0), pt(0.0, 30.0), pt(10.0, 0.0)] {
            assert!(dangling_bus_ends(&sheet, from, from).is_empty(), "{from:?}");
        }
    }

    #[test]
    fn an_end_with_nothing_under_it_dangles() {
        let mut sheet = empty_sheet();
        sheet.buses.push(bus(pt(0.0, 0.0), pt(10.0, 0.0)));
        let got = dangling_bus_ends(&sheet, pt(5.0, 0.0), pt(5.0, 10.0));
        assert_eq!(got, [RunEnd::End]);
        let got = dangling_bus_ends(&sheet, pt(50.0, 0.0), pt(60.0, 0.0));
        assert_eq!(got, [RunEnd::Start, RunEnd::End]);
    }
}
