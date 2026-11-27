use std::collections::BTreeMap;

use uuid::Uuid;

use super::*;
use crate::object_id::{IdSource, SequentialIds};

fn at(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn end(block: Uuid, port: Option<Uuid>) -> Endpoint {
    Endpoint { block, port }
}

fn sheet(n: u128, path: &str) -> SheetLink {
    SheetLink {
        sheet: DocumentId::from_uuid(Uuid::from_u128(0xa000 + n)).unwrap(),
        path: path.into(),
        state: LinkState::Linked,
    }
}

/// Two blocks A and B, a port on each, and one connection port to port.
fn two_blocks(ids: &mut SequentialIds) -> (BlockDiagram, [Uuid; 5]) {
    let mut d = BlockDiagram::with_id(DocumentId::from_uuid(Uuid::from_u128(0x100)).unwrap());
    let a_fields = BlockFields::named("A", at(0.0, 0.0));
    let a = d.add_block(ids, a_fields).unwrap();
    let b_fields = BlockFields::named("B", at(60.0, 0.0));
    let b = d.add_block(ids, b_fields).unwrap();
    let pa = d
        .add_port(ids, a, "OUT", PortSide::Right, 5.0, PortDirection::Output)
        .unwrap();
    let pb = d
        .add_port(ids, b, "IN", PortSide::Left, 5.0, PortDirection::Input)
        .unwrap();
    let (from, to) = (end(a, Some(pa)), end(b, Some(pb)));
    let c = d.connect(ids, from, to, Interface::Spi, " SPI1 ").unwrap();
    (d, [a, b, pa, pb, c])
}

#[test]
fn a_block_is_added_with_trimmed_text_and_clean_rails_and_specs() {
    let mut ids = SequentialIds::default();
    let mut d = BlockDiagram::new();
    let mut fields = BlockFields::named("  Buck ", at(1.0, 2.0));
    fields.rails = vec![" VIN ".into(), "".into(), "VIN".into(), "VOUT".into()];
    fields.specs = BTreeMap::from([
        (" Vout ".to_string(), " 3.3 V ".to_string()),
        ("  ".to_string(), "dropped".to_string()),
    ]);
    let id = d.add_block(&mut ids, fields).unwrap();
    let block = d.block(id).unwrap();
    assert_eq!(block.name, "Buck");
    assert_eq!(block.rails, ["VIN", "VOUT"]);
    assert_eq!(
        block.specs,
        BTreeMap::from([("Vout".to_string(), "3.3 V".to_string())])
    );
    assert_eq!(block.width_mm, DEFAULT_BLOCK_WIDTH_MM);
    assert_eq!(block.sheet, None);
}

#[test]
fn a_block_without_a_name_or_size_is_refused_and_nothing_changes() {
    let mut ids = SequentialIds::default();
    let (mut d, _) = two_blocks(&mut ids);
    let before = d.clone();
    let blank = d.add_block(&mut ids, BlockFields::named("   ", at(0.0, 0.0)));
    assert!(matches!(
        blank,
        Err(BlockDiagramError::Fault(DiagramFault::BlockWithoutName(_)))
    ));
    let mut flat = BlockFields::named("Flat", at(0.0, 0.0));
    flat.height_mm = 0.0;
    assert!(matches!(
        d.add_block(&mut ids, flat),
        Err(BlockDiagramError::Fault(DiagramFault::BadBlockSize(_)))
    ));
    let mut lost = BlockFields::named("Lost", at(f64::NAN, 0.0));
    lost.width_mm = 10.0;
    assert!(d.add_block(&mut ids, lost).is_err());
    assert_eq!(d, before);
}

#[test]
fn set_block_changes_only_what_the_patch_names() {
    let mut ids = SequentialIds::default();
    let (mut d, [a, ..]) = two_blocks(&mut ids);
    let group = d.add_group(&mut ids, "Power", None).unwrap();
    let patch = BlockPatch {
        function: Some(" Converter ".into()),
        shape: Some(BlockShape::Hexagon),
        group: Some(Some(group)),
        color: Some(Some(Color::new(1, 2, 3, 255))),
        ..BlockPatch::default()
    };
    d.set_block(a, patch).unwrap();
    let block = d.block(a).unwrap();
    assert_eq!(block.name, "A");
    assert_eq!(block.function, "Converter");
    assert_eq!(block.shape, BlockShape::Hexagon);
    assert_eq!(block.group, Some(group));
    assert_eq!(block.position, at(0.0, 0.0));
    let clear = BlockPatch {
        color: Some(None),
        ..BlockPatch::default()
    };
    d.set_block(a, clear).unwrap();
    assert_eq!(d.block(a).unwrap().color, None);
    assert_eq!(d.block(a).unwrap().group, Some(group));
}

#[test]
fn set_block_refuses_an_unknown_block_or_group() {
    let mut ids = SequentialIds::default();
    let (mut d, [a, ..]) = two_blocks(&mut ids);
    let before = d.clone();
    let nowhere = Uuid::from_u128(0xdead);
    assert_eq!(
        d.set_block(nowhere, BlockPatch::default()),
        Err(BlockDiagramError::UnknownBlock(nowhere))
    );
    let patch = BlockPatch {
        group: Some(Some(nowhere)),
        ..BlockPatch::default()
    };
    assert_eq!(
        d.set_block(a, patch),
        Err(BlockDiagramError::Fault(DiagramFault::MissingGroup {
            block: a,
            group: nowhere
        }))
    );
    assert_eq!(d, before);
}

#[test]
fn removing_a_block_takes_its_connections_with_it() {
    let mut ids = SequentialIds::default();
    let (mut d, [a, b, _, _, c]) = two_blocks(&mut ids);
    let removed = d.remove_block(a).unwrap();
    assert_eq!(removed.block.id, a);
    assert_eq!(removed.connections.len(), 1);
    assert_eq!(removed.connections[0].id, c);
    assert!(d.connections.is_empty());
    assert!(d.block(b).is_some());
    assert_eq!(d.fault(), None);
    assert_eq!(d.remove_block(a), Err(BlockDiagramError::UnknownBlock(a)));
}

#[test]
fn removing_a_port_takes_only_the_connections_on_that_port() {
    let mut ids = SequentialIds::default();
    let (mut d, [a, b, pa, _, c]) = two_blocks(&mut ids);
    let (edge, to_b) = (end(a, None), end(b, None));
    let power = Interface::Power;
    let kept = d.connect(&mut ids, edge, to_b, power, "").unwrap();
    let removed = d.remove_port(pa).unwrap();
    assert_eq!(removed.iter().map(|c| c.id).collect::<Vec<_>>(), [c]);
    assert_eq!(d.connections.len(), 1);
    assert_eq!(d.connections[0].id, kept);
    assert!(d.block(a).unwrap().ports.is_empty());
    assert_eq!(d.remove_port(pa), Err(BlockDiagramError::UnknownPort(pa)));
}

#[test]
fn a_connection_must_end_on_blocks_and_ports_that_exist_and_differ() {
    let mut ids = SequentialIds::default();
    let (mut d, [a, b, pa, pb, _]) = two_blocks(&mut ids);
    let before = d.clone();
    let ghost = Uuid::from_u128(0xbeef);
    let on_a = end(a, Some(pa));
    let missing_block = end(ghost, None);
    assert!(matches!(
        d.connect(&mut ids, on_a, missing_block, Interface::Signal, ""),
        Err(BlockDiagramError::Fault(DiagramFault::MissingBlock { .. }))
    ));
    let wrong_port = end(b, Some(pa));
    assert!(matches!(
        d.connect(&mut ids, wrong_port, on_a, Interface::Signal, ""),
        Err(BlockDiagramError::Fault(DiagramFault::MissingPort { .. }))
    ));
    assert!(matches!(
        d.connect(&mut ids, on_a, on_a, Interface::Signal, ""),
        Err(BlockDiagramError::Fault(DiagramFault::ClosedConnection(_)))
    ));
    let to_b = end(b, Some(pb));
    let blank = Interface::Other("  ".into());
    assert!(matches!(
        d.connect(&mut ids, on_a, to_b, blank, ""),
        Err(BlockDiagramError::Fault(
            DiagramFault::InterfaceWithoutName(_)
        ))
    ));
    assert_eq!(d, before);
}

#[test]
fn a_connection_label_and_interface_change_and_it_can_be_removed() {
    let mut ids = SequentialIds::default();
    let (mut d, [.., c]) = two_blocks(&mut ids);
    assert_eq!(d.connection(c).unwrap().label, "SPI1");
    d.set_connection(c, Some(Interface::Lvds), Some(" LVDS0 "))
        .unwrap();
    let link = d.connection(c).unwrap();
    assert_eq!(link.interface, Interface::Lvds);
    assert_eq!(link.label, "LVDS0");
    d.set_connection(c, None, None).unwrap();
    assert_eq!(d.connection(c).unwrap().interface, Interface::Lvds);
    let spelled = Interface::Other(" UART ".into());
    d.set_connection(c, Some(spelled), None).unwrap();
    assert_eq!(d.connection(c).unwrap().interface, Interface::Uart);
    assert_eq!(d.disconnect(c).unwrap().id, c);
    let gone = BlockDiagramError::UnknownConnection(c);
    assert_eq!(d.disconnect(c), Err(gone));
}

#[test]
fn a_sheet_link_follows_a_move_and_is_marked_when_the_sheet_goes() {
    let mut ids = SequentialIds::default();
    let (mut d, [a, b, ..]) = two_blocks(&mut ids);
    let link = sheet(1, "power.snxsch");
    let id = link.sheet;
    d.link_sheet(a, link).unwrap();
    assert_eq!(d.block_for_sheet(id).map(|b| b.id), Some(a));
    assert_eq!(d.follow_sheet_move(id, " sub/power_in.snxsch "), Some(a));
    let path = |d: &BlockDiagram| d.block(a).unwrap().sheet.clone().unwrap().path;
    assert_eq!(path(&d), "sub/power_in.snxsch");
    assert_eq!(d.follow_sheet_move(id, "  "), None, "a blank path");
    assert_eq!(d.mark_sheet_removed(id), Some(a));
    let kept = d.block(a).unwrap().sheet.clone().unwrap();
    assert_eq!(kept.state, LinkState::Unlinked);
    assert_eq!(kept.path, "sub/power_in.snxsch");
    let other = sheet(2, "x.snxsch").sheet;
    assert_eq!(d.mark_sheet_removed(other), None);
    assert!(matches!(
        d.link_sheet(b, sheet(3, " ")),
        Err(BlockDiagramError::Fault(
            DiagramFault::SheetLinkWithoutPath(_)
        ))
    ));
    assert_eq!(d.unlink_sheet(a).unwrap(), Some(kept));
    assert_eq!(d.block(a).unwrap().sheet, None);
}

#[test]
fn removing_a_group_leaves_its_blocks_ungrouped() {
    let mut ids = SequentialIds::default();
    let (mut d, [a, b, ..]) = two_blocks(&mut ids);
    let g = d.add_group(&mut ids, " Power ", None).unwrap();
    assert_eq!(d.group(g).unwrap().name, "Power");
    for block in [a, b] {
        let patch = BlockPatch {
            group: Some(Some(g)),
            ..BlockPatch::default()
        };
        d.set_block(block, patch).unwrap();
    }
    d.set_group(g, Some("Supply"), Some(Some(Color::new(9, 9, 9, 255))))
        .unwrap();
    assert_eq!(d.group(g).unwrap().name, "Supply");
    assert!(matches!(
        d.set_group(g, Some(" "), None),
        Err(BlockDiagramError::Fault(DiagramFault::GroupWithoutName(_)))
    ));
    assert_eq!(d.remove_group(g).unwrap().id, g);
    assert!(d.blocks.iter().all(|b| b.group.is_none()));
    assert_eq!(d.fault(), None);
}

/// An id source that hands out the same id twice.
struct Repeating(Uuid);

impl IdSource for Repeating {
    fn next_id(&mut self) -> Uuid {
        self.0
    }
}

#[test]
fn an_id_already_in_the_diagram_is_refused() {
    let mut ids = SequentialIds::default();
    let (mut d, [a, ..]) = two_blocks(&mut ids);
    let mut same = Repeating(a);
    assert_eq!(
        d.add_block(&mut same, BlockFields::named("Twin", at(0.0, 0.0))),
        Err(BlockDiagramError::Fault(DiagramFault::DuplicateId(a)))
    );
    assert_eq!(
        d.add_group(&mut same, "Twin", None),
        Err(BlockDiagramError::Fault(DiagramFault::DuplicateId(a)))
    );
}

#[test]
fn an_interface_reads_its_token_in_any_case_and_keeps_other_names() {
    assert_eq!(Interface::from_token(" I2C "), Interface::I2c);
    assert_eq!(Interface::from_token("usb"), Interface::Usb);
    assert_eq!(Interface::from_token("CAN"), Interface::Other("CAN".into()));
    for token in [
        "signal", "bus", "power", "analog", "spi", "i2c", "uart", "usb", "lvds",
    ] {
        assert_eq!(Interface::from_token(token).token(), token);
    }
    assert_eq!(Interface::Other("MIPI CSI".into()).token(), "MIPI CSI");
}

#[test]
fn the_json_of_a_diagram_uses_the_file_tokens() {
    let mut ids = SequentialIds::default();
    let (d, _) = two_blocks(&mut ids);
    let json = serde_json::to_value(&d).unwrap();
    assert_eq!(json["blocks"][0]["shape"], "rectangle");
    assert_eq!(json["blocks"][0]["ports"][0]["side"], "right");
    assert_eq!(json["blocks"][0]["ports"][0]["direction"], "output");
    assert_eq!(json["connections"][0]["interface"], "spi");
    assert_eq!(json["connections"][0]["label"], "SPI1");
    assert_eq!(json["id"], "00000000-0000-0000-0000-000000000100");
}
