//! World positions of the connection-point markers shown while dragging.

use std::cell::OnceCell;
use std::collections::HashMap;

use signex_types::schematic::{
    Point, SchematicSheet, SelectedItem, SelectedKind, SymbolTransform, instance_pins,
};
use uuid::Uuid;

/// Index `items` by uuid. The first item wins on a duplicate uuid, as a linear
/// `find` would.
fn by_uuid<T>(items: &[T], uuid_of: impl Fn(&T) -> Uuid) -> HashMap<Uuid, &T> {
    let mut index = HashMap::with_capacity(items.len());
    for item in items {
        index.entry(uuid_of(item)).or_insert(item);
    }
    index
}

/// The world position of every connection-point marker for the dragged
/// `selected` items, in selection order, with the `(dx, dy)` drag applied.
///
/// Each item kind present in the selection is indexed once, so the cost is
/// the sheet size plus the selection size, not their product.
pub(super) fn dragged_connection_points(
    snap: &SchematicSheet,
    selected: &[SelectedItem],
    dx: f64,
    dy: f64,
) -> Vec<(f64, f64)> {
    let wires = OnceCell::new();
    let buses = OnceCell::new();
    let junctions = OnceCell::new();
    let labels = OnceCell::new();
    let no_connects = OnceCell::new();
    let no_erc = OnceCell::new();
    let symbols = OnceCell::new();
    let sheet_pins = OnceCell::new();

    let moved = |p: Point| (p.x + dx, p.y + dy);
    let mut points = Vec::new();
    for sel in selected {
        match sel.kind {
            SelectedKind::Wire => {
                let index = wires.get_or_init(|| by_uuid(&snap.wires, |w| w.uuid));
                if let Some(w) = index.get(&sel.uuid) {
                    points.extend([moved(w.start), moved(w.end)]);
                }
            }
            SelectedKind::Bus => {
                let index = buses.get_or_init(|| by_uuid(&snap.buses, |b| b.uuid));
                if let Some(b) = index.get(&sel.uuid) {
                    points.extend([moved(b.start), moved(b.end)]);
                }
            }
            SelectedKind::Junction => {
                let index = junctions.get_or_init(|| by_uuid(&snap.junctions, |j| j.uuid));
                if let Some(j) = index.get(&sel.uuid) {
                    points.push(moved(j.position));
                }
            }
            SelectedKind::Label => {
                let index = labels.get_or_init(|| by_uuid(&snap.labels, |l| l.uuid));
                if let Some(l) = index.get(&sel.uuid) {
                    points.push(moved(l.position));
                }
            }
            SelectedKind::NoConnect => {
                let index = no_connects.get_or_init(|| by_uuid(&snap.no_connects, |n| n.uuid));
                if let Some(n) = index.get(&sel.uuid) {
                    points.push(moved(n.position));
                }
            }
            SelectedKind::NoErcDirective => {
                let index = no_erc.get_or_init(|| by_uuid(&snap.no_erc_directives, |d| d.uuid));
                if let Some(d) = index.get(&sel.uuid) {
                    points.push(moved(d.position));
                }
            }
            SelectedKind::SheetPin => {
                let index = sheet_pins.get_or_init(|| {
                    let mut index = HashMap::new();
                    for pin in snap.child_sheets.iter().flat_map(|cs| &cs.pins) {
                        index.entry(pin.uuid).or_insert(pin.position);
                    }
                    index
                });
                if let Some(position) = index.get(&sel.uuid) {
                    points.push(moved(*position));
                }
            }
            SelectedKind::Symbol => {
                let index = symbols.get_or_init(|| by_uuid(&snap.symbols, |s| s.uuid));
                if let Some(sym) = index.get(&sel.uuid)
                    && let Some(lib_sym) = snap.lib_symbols.get(&sym.lib_id)
                {
                    // Project the pins at the dragged position.
                    let mut shifted = (*sym).clone();
                    shifted.position = Point::new(sym.position.x + dx, sym.position.y + dy);
                    let xform = SymbolTransform::from_symbol(&shifted);
                    points.extend(instance_pins(lib_sym, sym).map(|lp| {
                        let Point { x, y } = xform.apply(lp.pin.position);
                        (x, y)
                    }));
                }
            }
            _ => {}
        }
    }
    points
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::{Junction, Wire};

    #[test]
    fn markers_follow_the_selection_order_and_the_drag_offset() {
        let mut sheet = SchematicSheet::default();
        let wire = Wire {
            uuid: Uuid::new_v4(),
            start: Point::new(0.0, 0.0),
            end: Point::new(4.0, 0.0),
            ..Wire::default()
        };
        let junction = Junction {
            uuid: Uuid::new_v4(),
            position: Point::new(9.0, 9.0),
            ..Junction::default()
        };
        let other_wire = Wire {
            uuid: Uuid::new_v4(),
            ..Wire::default()
        };
        let selected = [
            SelectedItem::new(junction.uuid, SelectedKind::Junction),
            SelectedItem::new(wire.uuid, SelectedKind::Wire),
            SelectedItem::new(Uuid::new_v4(), SelectedKind::Wire),
        ];
        sheet.wires.extend([other_wire, wire]);
        sheet.junctions.push(junction);

        let points = dragged_connection_points(&sheet, &selected, 1.0, 2.0);
        assert_eq!(points, vec![(10.0, 11.0), (1.0, 2.0), (5.0, 2.0)]);
    }

    #[test]
    fn a_sheet_pin_is_found_through_its_child_sheet() {
        let mut sheet = SchematicSheet::default();
        let pin_uuid = Uuid::new_v4();
        let child: signex_types::schematic::ChildSheet =
            serde_json::from_value(serde_json::json!({
                "uuid": Uuid::new_v4(),
                "name": "Sub",
                "filename": "sub.snxsch",
                "position": { "x": 0.0, "y": 0.0 },
                "size": [10.0, 10.0],
                "pins": [{
                    "uuid": pin_uuid,
                    "name": "P",
                    "position": { "x": 7.0, "y": 8.0 },
                }],
            }))
            .expect("child sheet");
        sheet.child_sheets.push(child);
        let selected = [SelectedItem::new(pin_uuid, SelectedKind::SheetPin)];
        assert_eq!(
            dragged_connection_points(&sheet, &selected, 1.0, 1.0),
            vec![(8.0, 9.0)]
        );
    }
}
