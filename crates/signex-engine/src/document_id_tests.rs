//! #2172: a sheet's own uuid is its document id. Repairing it is not an edit,
//! and a Save As encodes the copy under a new one without touching the
//! document being saved.

use signex_types::format::{SnxSchematic, document_id_in_header};
use uuid::Uuid;

use crate::test_support::test_sheet;
use crate::{Command, Engine};

fn engine() -> Engine {
    let mut sheet = test_sheet();
    sheet.uuid = Uuid::from_u128(7);
    Engine::new(sheet).unwrap()
}

#[test]
fn reassigning_the_id_is_not_an_edit() {
    let mut engine = engine();
    let revision = engine.revision();
    engine.reassign_document_id(Uuid::from_u128(8));
    assert_eq!(engine.document().uuid, Uuid::from_u128(8));
    assert_eq!(engine.revision(), revision, "no revision moves");
    assert!(!engine.can_undo(), "no history entry");
}

#[test]
fn an_edit_after_a_reassignment_keeps_the_new_id() {
    let mut engine = engine();
    engine.reassign_document_id(Uuid::from_u128(8));
    engine
        .execute(Command::SetNetColors {
            colors: Default::default(),
        })
        .unwrap();
    assert_eq!(engine.document().uuid, Uuid::from_u128(8));
}

#[test]
fn a_copy_is_encoded_under_a_new_id_and_the_document_keeps_its_own() {
    let engine = engine();
    let text = engine
        .serialize_as_copy_with_extents(Uuid::from_u128(9), |_| None)
        .unwrap();
    assert_eq!(
        document_id_in_header("snxsch", text.as_bytes()).map(|id| id.as_uuid()),
        Some(Uuid::from_u128(9))
    );
    assert_eq!(
        SnxSchematic::parse(&text).unwrap().sheet.uuid,
        Uuid::from_u128(9)
    );
    assert_eq!(engine.document().uuid, Uuid::from_u128(7), "the original");
    let plain = engine.serialize_with_extents(|_| None).unwrap();
    assert_eq!(
        document_id_in_header("snxsch", plain.as_bytes()).map(|id| id.as_uuid()),
        Some(Uuid::from_u128(7))
    );
}

fn edited_then_reassigned() -> Engine {
    let mut engine = engine();
    let junction = signex_types::schematic::Junction {
        uuid: Uuid::now_v7(),
        ..Default::default()
    };
    assert!(
        engine
            .execute(Command::PlaceJunction { junction })
            .unwrap()
            .changed
    );
    engine.reassign_document_id(Uuid::from_u128(8));
    engine
}

#[test]
fn undo_after_a_reassignment_keeps_the_new_id() {
    let mut engine = edited_then_reassigned();
    engine.undo().unwrap();
    assert_eq!(engine.document().uuid, Uuid::from_u128(8));
}

#[test]
fn redo_after_a_reassignment_keeps_the_new_id() {
    let mut engine = edited_then_reassigned();
    engine.undo().unwrap();
    engine.redo().unwrap();
    assert_eq!(engine.document().uuid, Uuid::from_u128(8));
    // And the other way round: the id moved while the edit sits on redo.
    engine.undo().unwrap();
    engine.reassign_document_id(Uuid::from_u128(9));
    engine.redo().unwrap();
    assert_eq!(engine.document().uuid, Uuid::from_u128(9));
}
