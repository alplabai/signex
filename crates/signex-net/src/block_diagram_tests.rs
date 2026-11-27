//! #2968: the block diagram the multi-channel buck project implies, and the
//! diagram generated and regenerated from it.

use std::collections::HashMap;

use signex_types::block_diagram::{
    BlockDiagram, DerivedDiagram, DiagramChange, LinkState, PortDirection, generate_block_diagram,
    regenerate_block_diagram,
};
use signex_types::document_id::DocumentId;
use signex_types::object_id::SequentialIds;
use signex_types::project::ChannelNaming;
use uuid::Uuid;

use crate::SheetKey;
use crate::derive_block_diagram;
use crate::fixtures::multi_channel::{BUCK, MCU, POWER_IN, Project, TOP, entry, project};
use crate::fixtures::pt;

fn id_of(n: u128) -> DocumentId {
    DocumentId::from_uuid(Uuid::from_u128(0x5000 + n)).unwrap()
}

/// The fixture with a document id on every sheet, as an opened project has.
fn linked_project() -> Project {
    let mut p = project();
    for (n, key) in [TOP, POWER_IN, BUCK, MCU].into_iter().enumerate() {
        p.sheet_mut(key).uuid = id_of(n as u128).as_uuid();
    }
    p
}

fn paths(p: &Project) -> HashMap<SheetKey, String> {
    p.sheets
        .keys()
        .map(|k| (k.clone(), format!("sheets/{}", k.as_str())))
        .collect()
}

fn derived(p: &Project) -> DerivedDiagram {
    derive_block_diagram(&p.graph(), &ChannelNaming::default(), &paths(p))
}

/// The golden block list: one block per sheet with parts, the top sheet
/// being only the frame they sit in.
#[test]
fn the_multi_sheet_fixture_gives_the_golden_block_list() {
    let d = derived(&linked_project());
    let blocks: Vec<(&str, &str, DocumentId, usize)> = d
        .blocks
        .iter()
        .map(|b| (b.name.as_str(), b.path.as_str(), b.sheet, b.depth))
        .collect();
    assert_eq!(
        blocks,
        [
            ("power_in", "sheets/power_in.snxsch", id_of(1), 1),
            ("buck_channel", "sheets/buck_channel.snxsch", id_of(2), 1),
            ("mcu", "sheets/mcu.snxsch", id_of(3), 1),
        ]
    );
    let ports: Vec<Vec<&str>> = d
        .blocks
        .iter()
        .map(|b| b.ports.iter().map(|p| p.name.as_str()).collect())
        .collect();
    assert_eq!(
        ports,
        [
            vec!["VIN_12V", "EN", "GND"],
            vec!["VIN_12V", "EN", "GND", "VOUT_3V3"],
            vec!["VOUT_3V3[1..4]"],
        ]
    );
    assert_eq!(d.blocks[0].rails, ["GND"]);
    assert_eq!(d.blocks[1].rails, ["GND"]);
    assert!(d.blocks[2].rails.is_empty());
}

/// One connection per signal group crossing sheets: the shared power and
/// signal nets, and the four repeated outputs as one group.
#[test]
fn crossing_nets_give_one_connection_per_group() {
    let d = derived(&linked_project());
    let sheet = |id: DocumentId| {
        let at = d.blocks.iter().position(|b| b.sheet == id).unwrap();
        d.blocks[at].name.clone()
    };
    let end = |id: DocumentId, port: &Option<String>| {
        format!("{}.{}", sheet(id), port.as_deref().unwrap_or("-"))
    };
    let mut links: Vec<String> = d
        .connections
        .iter()
        .map(|c| {
            format!(
                "{} {} '{}' {} -> {}",
                c.group,
                c.interface.token(),
                c.label,
                end(c.from.sheet, &c.from.port),
                end(c.to.sheet, &c.to.port),
            )
        })
        .collect();
    links.sort();
    assert_eq!(
        links,
        [
            "EN signal 'EN' power_in.EN -> buck_channel.EN",
            "GND power 'GND' power_in.GND -> buck_channel.GND",
            "VIN_12V signal 'VIN_12V' power_in.VIN_12V -> buck_channel.VIN_12V",
            "VOUT_3V3 signal 'VOUT_3V3' buck_channel.VOUT_3V3 -> mcu.VOUT_3V3[1..4]",
        ]
    );
}

#[test]
fn the_same_design_always_derives_the_same_diagram() {
    let p = linked_project();
    assert_eq!(derived(&p), derived(&p));
}

#[test]
fn an_output_entry_starts_the_flow() {
    let mut p = linked_project();
    let top = p.sheet_mut(TOP);
    top.child_sheets[0].pins[1].direction = "input".into();
    top.child_sheets[1].pins[1].direction = "output".into();
    let d = derived(&p);
    let en = d.connections.iter().find(|c| c.group == "EN").unwrap();
    assert_eq!(en.from.sheet, id_of(2));
    assert_eq!(en.to.sheet, id_of(1));
    assert_eq!(d.blocks[1].ports[1].direction, PortDirection::Output);
}

#[test]
fn a_sheet_without_a_document_id_is_left_out_and_listed() {
    let mut p = linked_project();
    p.sheet_mut(MCU).uuid = Uuid::nil();
    let d = derived(&p);
    assert_eq!(d.blocks.len(), 2);
    assert!(d.connections.iter().all(|c| c.group != "VOUT_3V3"));
    assert_eq!(d.unidentified, ["sheets/mcu.snxsch"], "named, not dropped");
}

/// A hierarchy whose top sheet places N sheet symbols gives N blocks, none
/// left unlisted; with no id on any sheet (a file no build has saved since
/// ids came in) it gives none, and every one is listed.
#[test]
fn n_sheet_symbols_give_n_blocks() {
    let p = linked_project();
    let symbols = p.sheets[&SheetKey::new(TOP)].child_sheets.len();
    assert_eq!(symbols, 3);
    let d = derived(&p);
    assert_eq!(d.blocks.len(), symbols);
    assert!(d.unidentified.is_empty());

    let d = derived(&project());
    assert!(d.blocks.is_empty());
    assert_eq!(d.unidentified.len(), symbols, "{:?}", d.unidentified);
}

#[test]
fn generated_blocks_link_their_sheets() {
    let d = derived(&linked_project());
    let mut ids = SequentialIds::default();
    let diagram = generate_block_diagram(&d, &mut ids, id_of(0x99)).unwrap();
    assert_eq!(diagram.blocks.len(), 3);
    for (block, want) in diagram.blocks.iter().zip(&d.blocks) {
        let link = block.sheet.as_ref().unwrap();
        assert_eq!(link.sheet, want.sheet);
        assert_eq!(link.state, LinkState::Linked);
    }
    assert_eq!(diagram.connections.len(), 4);
    assert!(diagram.fault().is_none());
}

/// Renaming a sheet's file keeps its document id: the block follows it.
#[test]
fn renaming_a_sheet_updates_its_block() {
    let p = linked_project();
    let mut ids = SequentialIds::default();
    let diagram = generate_block_diagram(&derived(&p), &mut ids, id_of(0x99)).unwrap();
    let mut moved = paths(&p);
    moved.insert(SheetKey::new(MCU), "boards/mcu_v2.snxsch".into());
    let renamed = derive_block_diagram(&p.graph(), &ChannelNaming::default(), &moved);
    let again = regenerate_block_diagram(&diagram, &renamed, &mut ids).unwrap();
    let mcu = again
        .diagram
        .block_for_sheet(id_of(3))
        .expect("the mcu block");
    assert_eq!(mcu.sheet.as_ref().unwrap().path, "boards/mcu_v2.snxsch");
    assert_eq!(again.changes.len(), 1, "{:?}", again.changes);
    assert!(matches!(again.changes[0], DiagramChange::SheetMoved { .. }));
}

/// Adding a sheet adds exactly one block and keeps every position.
#[test]
fn regenerate_after_adding_a_sheet_adds_exactly_one_block() {
    let p = linked_project();
    let mut ids = SequentialIds::default();
    let mut diagram: BlockDiagram =
        generate_block_diagram(&derived(&p), &mut ids, id_of(0x99)).unwrap();
    diagram.blocks[1].position = pt(250.0, 90.0);
    let before: Vec<_> = diagram.blocks.iter().map(|b| (b.id, b.position)).collect();

    let mut grown = linked_project();
    let connectors = "connectors.snxsch";
    let mut sheet = grown.sheets[&SheetKey::new(POWER_IN)].clone();
    sheet.uuid = id_of(4).as_uuid();
    grown.sheets.insert(SheetKey::new(connectors), sheet);
    let symbol = crate::fixtures::multi_channel::sheet_symbol(
        0x14,
        "connectors",
        connectors,
        vec![entry("USB_DP", pt(150.0, 10.0))],
    );
    grown.sheet_mut(TOP).child_sheets.push(symbol);
    grown
        .resolved
        .entry(SheetKey::new(TOP))
        .or_default()
        .insert(connectors.into(), SheetKey::new(connectors));

    let again = regenerate_block_diagram(&diagram, &derived(&grown), &mut ids).unwrap();
    let added: Vec<_> = again
        .changes
        .iter()
        .filter(|c| matches!(c, DiagramChange::BlockAdded { .. }))
        .collect();
    assert_eq!(added.len(), 1, "{:?}", again.changes);
    assert_eq!(again.diagram.blocks.len(), 4);
    for (id, at) in before {
        assert_eq!(again.diagram.block(id).unwrap().position, at);
    }
    let new = again.diagram.block_for_sheet(id_of(4)).unwrap();
    assert_eq!(new.name, "connectors");
    assert_eq!(new.sheet.as_ref().unwrap().path, "sheets/connectors.snxsch");
}

/// A deleted sheet leaves its block in place, marked unlinked.
#[test]
fn deleting_a_sheet_marks_its_block_unlinked() {
    let p = linked_project();
    let mut ids = SequentialIds::default();
    let diagram = generate_block_diagram(&derived(&p), &mut ids, id_of(0x99)).unwrap();
    let mut shrunk = linked_project();
    shrunk
        .sheet_mut(TOP)
        .child_sheets
        .retain(|s| s.filename != MCU);
    let again = regenerate_block_diagram(&diagram, &derived(&shrunk), &mut ids).unwrap();
    let mcu = again.diagram.block_for_sheet(id_of(3)).unwrap();
    assert_eq!(mcu.sheet.as_ref().unwrap().state, LinkState::Unlinked);
    assert_eq!(again.diagram.blocks.len(), 3);
}
