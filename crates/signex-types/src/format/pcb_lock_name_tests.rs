//! The optional `net_locked` pad column, the optional `sch_link` footprint
//! column and the optional `[stackup.display_names]` table: written only
//! when used, so a board that uses none re-saves byte-identical and older
//! files load unchanged.

use super::tests::empty_board;
use super::*;
use crate::pcb::{Footprint, LayerDef, Pad, PadShape, PadType, Point as PcbPoint, SymbolLink};
use crate::schematic::InstancePath;
use uuid::Uuid;

fn pad(n: u128, locked: bool) -> Pad {
    Pad {
        uuid: Uuid::from_u128(0x100 + n),
        number: n.to_string(),
        pad_type: PadType::Smd,
        shape: PadShape::Rect,
        position: PcbPoint { x: 0.0, y: 0.0 },
        size: PcbPoint { x: 1.0, y: 1.0 },
        drill: None,
        layers: vec!["TopCopper".into()],
        net: None,
        roundrect_ratio: 0.0,
        net_locked: locked,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn board_with_pads(locked: &[bool]) -> PcbBoard {
    let mut board = empty_board();
    board.footprints.push(Footprint {
        uuid: Uuid::from_u128(1),
        reference: "R1".into(),
        value: String::new(),
        footprint_id: "x.snxfpt".into(),
        position: PcbPoint { x: 0.0, y: 0.0 },
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        pads: locked
            .iter()
            .enumerate()
            .map(|(i, l)| pad(i as u128 + 1, *l))
            .collect(),
        graphics: Vec::new(),
        properties: Vec::new(),
        schematic_link: None,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    });
    board
}

#[test]
fn unlocked_pads_write_no_net_locked_column() {
    let text = SnxPcb::new(board_with_pads(&[false, false]))
        .write_string()
        .unwrap();
    assert!(!text.contains("net_locked"), "{text}");
    let again = SnxPcb::parse(&text).unwrap();
    assert!(again.board.footprints[0].pads.iter().all(|p| !p.net_locked));
    assert_eq!(again.write_string().unwrap(), text);
}

#[test]
fn a_locked_pad_round_trips_and_its_neighbour_stays_unlocked() {
    let text = SnxPcb::new(board_with_pads(&[false, true]))
        .write_string()
        .unwrap();
    assert!(text.contains("net_locked"), "{text}");
    let parsed = SnxPcb::parse(&text).unwrap();
    let pads = &parsed.board.footprints[0].pads;
    assert!(!pads[0].net_locked);
    assert!(pads[1].net_locked);
    assert!(
        pads.iter().all(|p| p.unknown_columns.is_empty()),
        "the lock column is read, not kept as a foreign cell"
    );
    assert_eq!(parsed.write_string().unwrap(), text);
}

#[test]
fn an_unreadable_net_locked_cell_stays_a_foreign_cell() {
    let text = SnxPcb::new(board_with_pads(&[true]))
        .write_string()
        .unwrap()
        .replace("true", "maybe");
    let parsed = SnxPcb::parse(&text).unwrap();
    let pad = &parsed.board.footprints[0].pads[0];
    assert!(!pad.net_locked);
    assert_eq!(pad.unknown_columns.get("net_locked"), Some("maybe"));
    assert_eq!(parsed.write_string().unwrap(), text);
}

fn layer(id: u8, name: &str, display: &str) -> LayerDef {
    LayerDef {
        id,
        name: name.into(),
        layer_type: String::new(),
        display_name: display.into(),
    }
}

#[test]
fn mechanical_display_names_round_trip() {
    let mut board = empty_board();
    board.layers = vec![
        layer(0, "TopCopper", ""),
        layer(1, "Mechanical1", "Board outline 2"),
        layer(2, "Mechanical2", ""),
    ];
    let text = SnxPcb::new(board).write_string().unwrap();
    assert!(text.contains("[stackup.display_names]"), "{text}");
    let parsed = SnxPcb::parse(&text).unwrap();
    let layers = &parsed.board.layers;
    assert_eq!(layers[1].display_name, "Board outline 2");
    assert_eq!(layers[1].display_label(), "Board outline 2");
    assert_eq!(layers[2].display_name, "");
    assert_eq!(layers[2].display_label(), "Mechanical2");
    assert!(parsed.board.unknown.stackup.is_empty());
    assert_eq!(parsed.write_string().unwrap(), text);
}

#[test]
fn layers_without_display_names_write_no_key() {
    let mut board = empty_board();
    board.layers = vec![layer(0, "TopCopper", ""), layer(1, "Mechanical1", "")];
    let text = SnxPcb::new(board).write_string().unwrap();
    assert!(!text.contains("display_names"), "{text}");
    let parsed = SnxPcb::parse(&text).unwrap();
    assert!(
        parsed
            .board
            .layers
            .iter()
            .all(|l| l.display_name.is_empty())
    );
    assert_eq!(parsed.write_string().unwrap(), text);
}

#[test]
fn display_names_follow_their_layer_when_the_layer_list_is_reordered() {
    let mut board = empty_board();
    board.layers = vec![
        layer(0, "TopCopper", ""),
        layer(1, "Mechanical1", "Outline"),
        layer(2, "Mechanical2", "Fab notes"),
    ];
    let text = SnxPcb::new(board).write_string().unwrap();
    // An older build that dropped Mechanical1 and swapped the rest leaves
    // the table untouched: names still land on their own layers.
    let edited = text.replace(
        "    \"TopCopper\",\n    \"Mechanical1\",\n    \"Mechanical2\",\n",
        "    \"Mechanical2\",\n    \"TopCopper\",\n",
    );
    assert_ne!(edited, text, "{text}");
    let parsed = SnxPcb::parse(&edited).unwrap();
    let names: Vec<_> = parsed
        .board
        .layers
        .iter()
        .map(|l| (l.name.as_str(), l.display_name.as_str()))
        .collect();
    assert_eq!(names, [("Mechanical2", "Fab notes"), ("TopCopper", "")]);
}

fn link(symbol: u128, chain: &[u128]) -> SymbolLink {
    SymbolLink::new(
        InstancePath::from_chain(chain.iter().map(|c| Uuid::from_u128(*c)).collect()),
        [Uuid::from_u128(symbol)],
    )
}

fn two_footprints(first: Option<SymbolLink>, second: Option<SymbolLink>) -> PcbBoard {
    let mut board = board_with_pads(&[false]);
    let mut other = board.footprints[0].clone();
    other.uuid = Uuid::from_u128(2);
    other.reference = "R2".into();
    other.pads[0].uuid = Uuid::from_u128(0x200);
    board.footprints[0].schematic_link = first;
    other.schematic_link = second;
    board.footprints.push(other);
    board
}

#[test]
fn unlinked_footprints_write_no_sch_link_column() {
    let text = SnxPcb::new(two_footprints(None, None))
        .write_string()
        .unwrap();
    assert!(!text.contains("sch_link"), "{text}");
    assert_eq!(SnxPcb::parse(&text).unwrap().write_string().unwrap(), text);
}

#[test]
fn a_footprint_link_round_trips_and_its_neighbour_stays_unlinked() {
    let linked = link(0xa1, &[0xb1, 0xb2]);
    let text = SnxPcb::new(two_footprints(Some(linked.clone()), None))
        .write_string()
        .unwrap();
    assert!(text.contains("sch_link"), "{text}");
    let parsed = SnxPcb::parse(&text).unwrap();
    assert_eq!(parsed.board.footprints[0].schematic_link, Some(linked));
    assert_eq!(parsed.board.footprints[1].schematic_link, None);
    assert!(
        parsed
            .board
            .footprints
            .iter()
            .all(|f| f.unknown_columns.is_empty()),
        "the link column is read, not kept as a foreign cell"
    );
    assert_eq!(parsed.write_string().unwrap(), text);
}

#[test]
fn a_root_sheet_link_and_a_nested_one_both_round_trip() {
    let board = two_footprints(Some(link(0xa1, &[])), Some(link(0xa2, &[0xb1])));
    let text = SnxPcb::new(board.clone()).write_string().unwrap();
    let parsed = SnxPcb::parse(&text).unwrap();
    assert_eq!(
        parsed.board.footprints[0].schematic_link,
        board.footprints[0].schematic_link
    );
    assert_eq!(
        parsed.board.footprints[1].schematic_link,
        board.footprints[1].schematic_link
    );
}

#[test]
fn an_unreadable_sch_link_cell_stays_a_foreign_cell() {
    let text = SnxPcb::new(two_footprints(Some(link(0xa1, &[])), None))
        .write_string()
        .unwrap();
    let broken = text.replace("@/", "@not-a-path");
    assert_ne!(broken, text, "{text}");
    let parsed = SnxPcb::parse(&broken).unwrap();
    let fp = &parsed.board.footprints[0];
    assert_eq!(fp.schematic_link, None);
    assert_eq!(
        fp.unknown_columns.get("sch_link"),
        Some(&*format!("{}@not-a-path", Uuid::from_u128(0xa1)))
    );
    assert_eq!(parsed.write_string().unwrap(), broken);
}

#[test]
fn a_multi_unit_link_round_trips_with_its_units_in_ascending_order() {
    let linked = SymbolLink::new(
        InstancePath::root(),
        [
            Uuid::from_u128(0xa3),
            Uuid::from_u128(0xa1),
            Uuid::from_u128(0xa2),
        ],
    );
    assert_eq!(linked.units[0], Uuid::from_u128(0xa1), "sorted by new");
    let text = SnxPcb::new(two_footprints(Some(linked.clone()), None))
        .write_string()
        .unwrap();
    assert!(
        text.contains("00000000-0000-0000-0000-0000000000a1+"),
        "{text}"
    );
    let parsed = SnxPcb::parse(&text).unwrap();
    assert_eq!(parsed.board.footprints[0].schematic_link, Some(linked));
    assert_eq!(parsed.write_string().unwrap(), text);
}

#[test]
fn a_unit_list_out_of_order_stays_a_foreign_cell() {
    let text = SnxPcb::new(two_footprints(Some(link(0xa1, &[])), None))
        .write_string()
        .unwrap();
    let skewed = text.replace(
        "00000000-0000-0000-0000-0000000000a1@/",
        "00000000-0000-0000-0000-0000000000a2+00000000-0000-0000-0000-0000000000a1@/",
    );
    assert_ne!(skewed, text, "{text}");
    let parsed = SnxPcb::parse(&skewed).unwrap();
    assert_eq!(parsed.board.footprints[0].schematic_link, None);
    assert_eq!(parsed.write_string().unwrap(), skewed);
}

fn stackup_board() -> PcbBoard {
    let mut board = empty_board();
    board.layers = vec![
        layer(0, "TopCopper", ""),
        layer(1, "Mechanical1", "Outline"),
    ];
    board
}

#[test]
fn a_display_name_for_a_missing_layer_round_trips() {
    let text = SnxPcb::new(stackup_board()).write_string().unwrap();
    let with_orphan = text.replace(
        "Mechanical1 = \"Outline\"",
        "Mechanical1 = \"Outline\"
Mechanical9 = \"Gone\"",
    );
    assert_ne!(with_orphan, text, "{text}");
    let parsed = SnxPcb::parse(&with_orphan).unwrap();
    assert_eq!(parsed.board.layers[1].display_name, "Outline");
    assert!(!parsed.board.unknown.stackup_display_names.is_empty());
    assert_eq!(parsed.write_string().unwrap(), with_orphan);
}

#[test]
fn a_non_string_display_name_is_kept_instead_of_failing_the_load() {
    let text = SnxPcb::new(stackup_board()).write_string().unwrap();
    let odd = text.replace("Mechanical1 = \"Outline\"", "Mechanical1 = 7");
    assert_ne!(odd, text, "{text}");
    let parsed = SnxPcb::parse(&odd).unwrap();
    assert_eq!(parsed.board.layers[1].display_name, "");
    assert_eq!(parsed.write_string().unwrap(), odd);
}

#[test]
fn a_layer_named_by_the_user_replaces_a_kept_entry_of_the_same_token() {
    let text = SnxPcb::new(stackup_board()).write_string().unwrap();
    let odd = text.replace("Mechanical1 = \"Outline\"", "Mechanical1 = 7");
    let mut parsed = SnxPcb::parse(&odd).unwrap();
    parsed.board.layers[1].display_name = "Fab".into();
    let written = parsed.write_string().unwrap();
    assert!(written.contains("Mechanical1 = \"Fab\""), "{written}");
    assert!(!written.contains("Mechanical1 = 7"), "{written}");
}
