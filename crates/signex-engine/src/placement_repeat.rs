//! One payload holding several shifted copies of another (Smart Paste, #2730).
//! The sheet is destructured with no `..`, so a new sheet field is a compile
//! error here until it is sorted into "each copy" or "once".

use signex_types::object_id::IdSource;
use signex_types::schematic::{Label, SchematicSheet, SheetHarness};

use crate::PlacementPayload;

impl PlacementPayload {
    /// `copies` copies, copy `k` (from 1) shifted `k` times `(dx, dy)`.
    /// `rename` gives a label of copy `k` its new text (`None` keeps it).
    /// Copy 1 keeps its ids; every later copy takes fresh ones from `ids`, so
    /// no two objects of the result share an id. Library bodies and sheet
    /// settings are carried once.
    #[must_use]
    pub fn repeated(
        &self,
        copies: u32,
        dx: f64,
        dy: f64,
        ids: &mut dyn IdSource,
        rename: impl Fn(u32, &Label) -> Option<String>,
    ) -> Self {
        let mut merged = self.fragment.clone();
        clear_objects(&mut merged);
        for k in 1..=copies {
            let step = f64::from(k);
            let mut copy = self.translated(dx * step, dy * step).fragment;
            for label in &mut copy.labels {
                if let Some(text) = rename(k, label) {
                    label.text = text;
                }
            }
            if k > 1 {
                give_fresh_ids(&mut copy, ids);
            }
            append_objects(&mut merged, copy);
        }
        Self::new(merged)
    }
}

/// Empty every collection a copy repeats.
fn clear_objects(sheet: &mut SchematicSheet) {
    let SchematicSheet {
        symbols,
        wires,
        junctions,
        labels,
        no_connects,
        text_notes,
        buses,
        bus_entries,
        drawings,
        no_erc_directives,
        harness,
        // Carried once, from the source.
        uuid: _,
        version: _,
        generator: _,
        generator_version: _,
        paper_size: _,
        font_family: _,
        root_sheet_page: _,
        child_sheets: _,
        directives: _,
        unions: _,
        title_block: _,
        variables: _,
        lib_symbols: _,
        text_widths: _,
        text_extents: _,
        net_colors: _,
        drawing_placements: _,
        frame: _,
        page: _,
        unknown: _,
    } = sheet;
    symbols.clear();
    wires.clear();
    junctions.clear();
    labels.clear();
    no_connects.clear();
    text_notes.clear();
    buses.clear();
    bus_entries.clear();
    drawings.clear();
    no_erc_directives.clear();
    let SheetHarness {
        connectors,
        wires: harness_wires,
        entry_types: _,
    } = harness;
    connectors.clear();
    harness_wires.clear();
}

/// Add the objects of `copy` to `merged`.
fn append_objects(merged: &mut SchematicSheet, copy: SchematicSheet) {
    let SchematicSheet {
        symbols,
        wires,
        junctions,
        labels,
        no_connects,
        text_notes,
        buses,
        bus_entries,
        drawings,
        no_erc_directives,
        harness,
        // Carried once, from the source.
        uuid: _,
        version: _,
        generator: _,
        generator_version: _,
        paper_size: _,
        font_family: _,
        root_sheet_page: _,
        child_sheets: _,
        directives: _,
        unions: _,
        title_block: _,
        variables: _,
        lib_symbols: _,
        text_widths: _,
        text_extents: _,
        net_colors: _,
        drawing_placements: _,
        frame: _,
        page: _,
        unknown: _,
    } = copy;
    merged.symbols.extend(symbols);
    merged.wires.extend(wires);
    merged.junctions.extend(junctions);
    merged.labels.extend(labels);
    merged.no_connects.extend(no_connects);
    merged.text_notes.extend(text_notes);
    merged.buses.extend(buses);
    merged.bus_entries.extend(bus_entries);
    merged.drawings.extend(drawings);
    merged.no_erc_directives.extend(no_erc_directives);
    let SheetHarness {
        connectors,
        wires: harness_wires,
        entry_types: _,
    } = harness;
    merged.harness.connectors.extend(connectors);
    merged.harness.wires.extend(harness_wires);
}

/// A fresh id for every object of `sheet`, a connector's entries included.
fn give_fresh_ids(sheet: &mut SchematicSheet, ids: &mut dyn IdSource) {
    sheet
        .symbols
        .iter_mut()
        .for_each(|o| o.uuid = ids.next_id());
    sheet.wires.iter_mut().for_each(|o| o.uuid = ids.next_id());
    sheet.buses.iter_mut().for_each(|o| o.uuid = ids.next_id());
    sheet
        .bus_entries
        .iter_mut()
        .for_each(|o| o.uuid = ids.next_id());
    sheet
        .junctions
        .iter_mut()
        .for_each(|o| o.uuid = ids.next_id());
    sheet
        .no_connects
        .iter_mut()
        .for_each(|o| o.uuid = ids.next_id());
    sheet
        .no_erc_directives
        .iter_mut()
        .for_each(|o| o.uuid = ids.next_id());
    sheet.labels.iter_mut().for_each(|o| o.uuid = ids.next_id());
    sheet
        .text_notes
        .iter_mut()
        .for_each(|o| o.uuid = ids.next_id());
    sheet
        .drawings
        .iter_mut()
        .for_each(|o| o.set_uuid(ids.next_id()));
    for connector in &mut sheet.harness.connectors {
        connector.uuid = ids.next_id();
        for entry in &mut connector.entries {
            entry.uuid = ids.next_id();
        }
    }
    sheet
        .harness
        .wires
        .iter_mut()
        .for_each(|o| o.uuid = ids.next_id());
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use signex_types::object_id::SequentialIds;
    use signex_types::schematic::{
        HarnessConnector, HarnessEntry, HarnessWire, LabelType, Point, SchematicSheet,
    };
    use uuid::Uuid;

    use super::*;

    /// One connector with two entries and one harness wire off its tip.
    fn harness_fragment() -> SchematicSheet {
        let connector = HarnessConnector {
            uuid: Uuid::from_u128(0xA1),
            harness_type: "SPI_BUS".into(),
            position: Point::new(0.0, 0.0),
            width: 10.16,
            entry_pitch: 2.54,
            rotation: 0.0,
            entries: ["SCK", "CS"]
                .iter()
                .enumerate()
                .map(|(i, name)| HarnessEntry {
                    uuid: Uuid::from_u128(0xB0 + i as u128),
                    name: (*name).into(),
                })
                .collect(),
            locked: false,
        };
        let tip = connector.tip_point();
        let mut sheet = SchematicSheet::default();
        sheet.harness.wires.push(HarnessWire {
            uuid: Uuid::from_u128(0xC1),
            start: tip,
            end: Point::new(tip.x + 5.0, tip.y),
            locked: false,
        });
        sheet.harness.connectors.push(connector);
        sheet
    }

    #[test]
    fn each_copy_carries_its_own_shifted_harness_with_distinct_ids() {
        let payload = PlacementPayload::new(harness_fragment());
        let repeated = payload.repeated(3, 10.0, 0.0, &mut SequentialIds::default(), |_, _| None);
        let harness = &repeated.fragment().harness;
        let xs: Vec<f64> = harness.connectors.iter().map(|c| c.position.x).collect();
        assert_eq!(xs, [10.0, 20.0, 30.0]);
        let source_start = payload.fragment().harness.wires[0].start.x;
        let wire_xs: Vec<f64> = harness.wires.iter().map(|w| w.start.x).collect();
        assert_eq!(
            wire_xs,
            [
                source_start + 10.0,
                source_start + 20.0,
                source_start + 30.0
            ]
        );
        let mut seen = HashSet::new();
        for c in &harness.connectors {
            assert!(seen.insert(c.uuid), "connector id repeated");
            for e in &c.entries {
                assert!(seen.insert(e.uuid), "entry id repeated");
            }
        }
        for w in &harness.wires {
            assert!(seen.insert(w.uuid), "wire id repeated");
        }
    }

    #[test]
    fn one_copy_keeps_its_harness_one_offset_away() {
        let payload = PlacementPayload::new(harness_fragment());
        let repeated = payload.repeated(1, 20.0, 5.0, &mut SequentialIds::default(), |_, _| None);
        let connector = &repeated.fragment().harness.connectors[0];
        assert_eq!((connector.position.x, connector.position.y), (20.0, 5.0));
        assert_eq!(connector.uuid, Uuid::from_u128(0xA1), "copy 1 keeps its id");
    }

    #[test]
    fn rename_reaches_each_copys_labels_by_number() {
        let mut sheet = SchematicSheet::default();
        sheet
            .labels
            .push(Label::new("FB", Point::new(0.0, 0.0), LabelType::Net));
        let repeated = PlacementPayload::new(sheet).repeated(
            2,
            1.0,
            0.0,
            &mut SequentialIds::default(),
            |k, label| Some(format!("{}_{k}", label.text)),
        );
        let texts: Vec<&str> = repeated
            .fragment()
            .labels
            .iter()
            .map(|l| l.text.as_str())
            .collect();
        assert_eq!(texts, ["FB_1", "FB_2"]);
    }
}
