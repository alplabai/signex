//! Mockup row 210: a regenerate that leaves the unlinking for review, the
//! review's Apply selected, and a block name that follows a renamed sheet
//! only while it is the name the old file gave it.

use uuid::Uuid;

use super::*;
use crate::object_id::SequentialIds;

fn doc(n: u128) -> DocumentId {
    DocumentId::from_uuid(Uuid::from_u128(0xd000 + n)).unwrap()
}

fn sheet(n: u128, path: &str, name: &str, ports: &[(&str, PortDirection)]) -> DerivedBlock {
    DerivedBlock {
        sheet: doc(n),
        path: path.into(),
        name: name.into(),
        depth: 1,
        rails: Vec::new(),
        ports: ports
            .iter()
            .map(|(name, direction)| DerivedPort {
                name: (*name).into(),
                direction: *direction,
            })
            .collect(),
    }
}

/// Power In feeds the LEDs sheet over LED; `leds.snxsch` has no symbol
/// name, so its block is named after the file.
fn two_sheets() -> DerivedDiagram {
    DerivedDiagram {
        blocks: vec![
            sheet(
                1,
                "power_in.snxsch",
                "Power In",
                &[("LED", PortDirection::Output)],
            ),
            sheet(2, "leds.snxsch", "", &[("LED", PortDirection::Input)]),
        ],
        connections: vec![DerivedConnection {
            group: "LED".into(),
            interface: Interface::Signal,
            label: "LED".into(),
            from: DerivedEnd {
                sheet: doc(1),
                port: Some("LED".into()),
            },
            to: DerivedEnd {
                sheet: doc(2),
                port: Some("LED".into()),
            },
        }],
    }
}

fn generated(ids: &mut SequentialIds) -> BlockDiagram {
    generate_block_diagram(&two_sheets(), ids, doc(0x100)).unwrap()
}

fn is_linked_to(block: &Block, sheet: DocumentId) -> bool {
    block.sheet.as_ref().is_some_and(|link| link.sheet == sheet)
}

fn leds_block(d: &BlockDiagram) -> &Block {
    d.blocks.iter().find(|b| is_linked_to(b, doc(2))).unwrap()
}

#[test]
fn without_unlink_a_deleted_sheet_is_skipped_and_its_block_and_connection_stay() {
    let mut ids = SequentialIds::default();
    let d = generated(&mut ids);
    let mut derived = two_sheets();
    derived.blocks.remove(1);
    derived.connections.clear();
    let options = RegenerateOptions { unlink: false };
    let kept = regenerate_block_diagram_with(&d, &derived, &mut ids, options).unwrap();
    assert_eq!(kept.diagram, d, "nothing changes");
    assert!(kept.changes.is_empty(), "{:?}", kept.changes);
    let leds = leds_block(&d).id;
    assert_eq!(
        kept.skipped,
        [DiagramChange::SheetRemoved {
            block: leds,
            sheet: doc(2),
            path: "leds.snxsch".into(),
        }]
    );

    // Apply selected: the same result a full regenerate gives.
    let full = regenerate_block_diagram(&d, &derived, &mut ids).unwrap();
    let applied = unlink_blocks(&kept.diagram, &[leds]);
    assert_eq!(applied.diagram, full.diagram);
    assert_eq!(applied.changes.len(), 2, "{:?}", applied.changes);
    assert!(unlink_blocks(&kept.diagram, &[]).changes.is_empty());
}

#[test]
fn a_generated_name_follows_a_renamed_sheet_and_a_typed_one_stays() {
    let mut ids = SequentialIds::default();
    let d = generated(&mut ids);
    assert_eq!(leds_block(&d).name, "leds");
    let mut derived = two_sheets();
    derived.blocks[1].path = "io/status_leds.snxsch".into();
    let again = regenerate_block_diagram(&d, &derived, &mut ids).unwrap();
    let leds = leds_block(&again.diagram);
    assert_eq!(leds.name, "status_leds");
    assert!(again.changes.contains(&DiagramChange::NameFollowed {
        block: leds.id,
        from: "leds".into(),
        to: "status_leds".into(),
    }));

    let mut typed = d.clone();
    let at = typed
        .blocks
        .iter()
        .position(|b| is_linked_to(b, doc(2)))
        .unwrap();
    typed.blocks[at].name = "Status LEDs".into();
    let again = regenerate_block_diagram(&typed, &derived, &mut ids).unwrap();
    assert_eq!(leds_block(&again.diagram).name, "Status LEDs");
    assert!(
        !again
            .changes
            .iter()
            .any(|c| matches!(c, DiagramChange::NameFollowed { .. }))
    );
}
