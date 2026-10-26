//! Bus-entry orientation lives in the sign of `size`, so the format has no
//! new field. These tests pin that: an old file loads with today's default
//! and re-saves byte-identical, and every orientation round-trips.

use super::tests::empty_sheet;
use super::*;
use crate::schematic::{BusEntry, Point as SchPoint};
use uuid::Uuid;

fn entry(size: (f64, f64)) -> BusEntry {
    BusEntry {
        uuid: Uuid::from_u128(7),
        position: SchPoint::new(10.16, 20.32),
        size,
    }
}

#[test]
fn every_orientation_round_trips() {
    for quarter_turns in 0..4 {
        for flip in [false, true] {
            let mut size = BusEntry::rotated_size((2.54, 2.54), quarter_turns);
            if flip {
                size = BusEntry::flipped_horizontal(size);
            }
            let mut sheet = empty_sheet();
            sheet.bus_entries.push(entry(size));
            let text = SnxSchematic::new(sheet).write_string().expect("serialise");
            let back = SnxSchematic::parse(&text).expect("parse");
            assert_eq!(back.sheet.bus_entries[0].size, size);
            assert_eq!(back.write_string().expect("re-serialise"), text);
        }
    }
}

#[test]
fn old_file_loads_with_the_default_orientation_and_resaves_unchanged() {
    // A file as the writer produced it before orientation was a concern:
    // one entry, `size = [2.54, 2.54]`, nothing else about the entry.
    let mut sheet = empty_sheet();
    sheet.bus_entries.push(entry((2.54, 2.54)));
    let old = SnxSchematic::new(sheet).write_string().expect("serialise");
    assert!(old.contains("bus_entries"), "fixture must carry the entry");
    assert!(
        !old.to_lowercase().contains("orientation"),
        "orientation must not be a stored field"
    );

    let loaded = SnxSchematic::parse(&old).expect("old file loads");
    let e = &loaded.sheet.bus_entries[0];
    assert_eq!(e.size, crate::schematic::SCHEMATIC_BUS_ENTRY_DEFAULT_SIZE);
    assert_eq!((e.position.x, e.position.y), (10.16, 20.32));
    assert_eq!(
        loaded.write_string().expect("re-serialise"),
        old,
        "an untouched old file must re-save byte-identical"
    );
}
