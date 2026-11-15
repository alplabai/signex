use signex_types::pcb::{PcbBoard, Point, SymbolLink};
use signex_types::schematic::InstancePath;
use uuid::Uuid;

use super::*;
use crate::pcb::BoardEngine;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// U1 at (10, 10) on net GND by its pad, a GND track from (10, 10) to
/// (20, 10) and a VCC via at (20, 14), on a 1 mm grid.
fn board() -> PcbBoard {
    serde_json::from_value(serde_json::json!({
        "uuid": id(1),
        "setup": {"grid_size": 1.0},
        "nets": [{"number": 1, "name": "GND"}, {"number": 2, "name": "VCC"}],
        "footprints": [{
            "uuid": id(10), "reference": "U1", "position": {"x": 10.0, "y": 10.0},
            "layer": "TopCopper",
            "pads": [{
                "uuid": id(11), "number": "1", "pad_type": "smd", "shape": "rect",
                "position": {"x": 1.0, "y": 0.0}, "size": {"x": 1.0, "y": 0.5},
                "drill": null, "layers": ["TopCopper"], "net": {"number": 1, "name": "GND"}
            }]
        }],
        "segments": [{
            "uuid": id(20), "start": {"x": 10.0, "y": 10.0}, "end": {"x": 20.0, "y": 10.0},
            "width": 0.25, "layer": "TopCopper", "net": 1
        }],
        "vias": [{"uuid": id(30), "position": {"x": 20.0, "y": 14.0}, "diameter": 0.6, "net": 2}]
    }))
    .unwrap()
}

fn all() -> Vec<BoardItemId> {
    vec![
        BoardItemId::Footprint(id(10)),
        BoardItemId::Segment(id(20)),
        BoardItemId::Via(id(30)),
    ]
}

fn ids_from(start: u128) -> impl FnMut() -> Uuid {
    let mut next = start;
    move || {
        next += 1;
        id(next)
    }
}

fn keep(_: &str) -> Option<String> {
    None
}

#[test]
fn a_copy_is_held_about_its_grid_snapped_centre() {
    let copy = BoardClipboard::copy(&board(), &all(), 1.0)
        .unwrap()
        .unwrap();
    // Anchors span (10, 10)..(20, 14): the centre (15, 12) is on the grid.
    assert_eq!(copy.origin(), Point::new(15.0, 12.0));
    assert_eq!(copy.len(), 3);
    let preview = copy.preview_board(&board());
    assert_eq!(preview.footprints[0].position, Point::new(-5.0, -2.0));
    assert_eq!(preview.vias[0].position, Point::new(5.0, 2.0));
}

#[test]
fn nothing_carried_is_none_and_a_missing_item_is_refused() {
    assert_eq!(
        BoardClipboard::copy(&board(), &[BoardItemId::Zone(id(99))], 1.0),
        Ok(None)
    );
    assert_eq!(
        BoardClipboard::copy(&board(), &[BoardItemId::Via(id(99))], 1.0),
        Err(BoardCommandError::ItemNotFound(BoardItemId::Via(id(99))))
    );
}

#[test]
fn a_paste_lands_at_the_point_with_new_ids_and_is_one_undo_step() {
    let source = board();
    let copy = BoardClipboard::copy(&source, &all(), 1.0).unwrap().unwrap();
    let (command, added) = copy.paste_command(
        &source,
        Point::new(50.0, 50.0),
        &mut ids_from(100),
        &mut keep,
    );
    assert_eq!(added.len(), 3);
    let mut engine = BoardEngine::new(source, 1);
    engine.execute(command).expect("the paste applies");
    let pasted = engine.board();
    assert_eq!(pasted.footprints.len(), 2);
    let fp = &pasted.footprints[1];
    assert_eq!(fp.position, Point::new(45.0, 48.0));
    assert_ne!(fp.uuid, id(10));
    assert_ne!(fp.pads[0].uuid, id(11));
    assert_eq!(pasted.segments[1].start, Point::new(45.0, 48.0));
    assert_eq!(pasted.vias[1].position, Point::new(55.0, 52.0));
    engine.undo().expect("undo").expect("a step");
    assert_eq!(engine.board().footprints.len(), 1);
    assert_eq!(engine.board().segments.len(), 1);
    assert_eq!(engine.board().vias.len(), 1);
}

#[test]
fn copper_keeps_its_net_only_where_the_target_has_that_net() {
    let copy = BoardClipboard::copy(&board(), &all(), 1.0)
        .unwrap()
        .unwrap();
    let mut target = board();
    // GND is net 7 here and VCC does not exist.
    target.nets =
        serde_json::from_value(serde_json::json!([{"number": 7, "name": "GND"}])).unwrap();
    let (BoardCommand::AddItems { items }, _) =
        copy.paste_command(&target, Point::new(0.0, 0.0), &mut ids_from(100), &mut keep)
    else {
        panic!("a paste adds items");
    };
    let BoardItem::Footprint(fp) = &items[0] else {
        panic!("the footprint comes first")
    };
    let pad_net = fp.pads[0].net.as_ref().expect("GND is kept");
    assert_eq!((pad_net.number, pad_net.name.as_str()), (7, "GND"));
    assert!(matches!(&items[1], BoardItem::Segment(s) if s.net == 7));
    assert!(
        matches!(&items[2], BoardItem::Via(v) if v.net == 0),
        "VCC is gone"
    );

    target.nets.clear();
    let (BoardCommand::AddItems { items }, _) =
        copy.paste_command(&target, Point::new(0.0, 0.0), &mut ids_from(100), &mut keep)
    else {
        panic!("a paste adds items");
    };
    let BoardItem::Footprint(fp) = &items[0] else {
        panic!("the footprint comes first")
    };
    assert_eq!(fp.pads[0].net, None);
}

#[test]
fn a_footprint_takes_the_designator_it_is_given() {
    let copy = BoardClipboard::copy(&board(), &all(), 1.0)
        .unwrap()
        .unwrap();
    let (BoardCommand::AddItems { items }, _) = copy.paste_command(
        &board(),
        Point::new(0.0, 0.0),
        &mut ids_from(100),
        &mut |old: &str| (old == "U1").then(|| "U2".to_owned()),
    ) else {
        panic!("a paste adds items");
    };
    assert!(matches!(&items[0], BoardItem::Footprint(fp) if fp.reference == "U2"));
}

#[test]
fn a_schematic_link_is_kept_only_while_no_footprint_holds_it() {
    let link = SymbolLink::new(InstancePath::root(), [id(500)]);
    let mut source = board();
    source.footprints[0].schematic_link = Some(link.clone());
    let items = [BoardItemId::Footprint(id(10))];
    let copy = BoardClipboard::copy(&source, &items, 1.0).unwrap().unwrap();
    let linked = |target: &PcbBoard| {
        let (BoardCommand::AddItems { items }, _) =
            copy.paste_command(target, Point::new(0.0, 0.0), &mut ids_from(100), &mut keep)
        else {
            panic!("a paste adds items");
        };
        matches!(&items[0], BoardItem::Footprint(fp) if fp.schematic_link.is_some())
    };
    assert!(!linked(&source), "a copy does not claim the placed part");
    let mut cut = source.clone();
    cut.footprints.clear();
    assert!(linked(&cut), "a cut part pasted back stays linked");
}
