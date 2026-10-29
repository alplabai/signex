//! Placing a generic No-ERC directive (issue #350): it lands in
//! `no_erc_directives` (never `no_connects`), undo/redo/delete work, a save
//! reopens with the entry intact, and ERC stays quiet under it.

use signex_engine::{Command, DocumentPatch, Engine};
use signex_erc::engine::run_all;
use signex_erc::{ErcContext, RuleId, RuleKind};
use signex_types::schematic::{NoConnect, Point, SchematicSheet, SelectedItem, SelectedKind, Wire};
use uuid::Uuid;

fn directive_at(x: f64, y: f64) -> NoConnect {
    NoConnect {
        uuid: Uuid::now_v7(),
        position: Point::new(x, y),
    }
}

fn empty_engine() -> Engine {
    let sheet = SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    Engine::new(sheet).expect("engine")
}

fn place(engine: &mut Engine, x: f64, y: f64) -> Uuid {
    let directive = directive_at(x, y);
    let uuid = directive.uuid;
    engine
        .execute(Command::PlaceNoErcDirective { directive })
        .expect("place directive");
    uuid
}

#[test]
fn place_lands_in_no_erc_directives_not_no_connects() {
    let mut engine = empty_engine();
    let uuid = place(&mut engine, 5.0, 7.0);

    let doc = engine.document();
    assert!(doc.no_connects.is_empty());
    assert_eq!(doc.no_erc_directives.len(), 1);
    assert_eq!(doc.no_erc_directives[0].uuid, uuid);
}

#[test]
fn place_reports_the_directive_document_patch() {
    let mut engine = empty_engine();
    let result = engine
        .execute(Command::PlaceNoErcDirective {
            directive: directive_at(0.0, 0.0),
        })
        .expect("place directive");
    let patch = result.patch_pair.expect("a placement changes the document");
    assert!(patch.document.contains(DocumentPatch::NO_ERC_DIRECTIVES));
    assert!(!patch.document.intersects(DocumentPatch::NO_CONNECTS));
}

#[test]
fn undo_removes_and_redo_restores() {
    let mut engine = empty_engine();
    let uuid = place(&mut engine, 1.0, 2.0);

    engine.undo().expect("undo");
    assert!(engine.document().no_erc_directives.is_empty());

    engine.redo().expect("redo");
    assert_eq!(engine.document().no_erc_directives.len(), 1);
    assert_eq!(engine.document().no_erc_directives[0].uuid, uuid);
}

#[test]
fn delete_selection_removes_the_directive() {
    let mut engine = empty_engine();
    let keep = place(&mut engine, 0.0, 0.0);
    let gone = place(&mut engine, 10.0, 0.0);

    engine
        .execute(Command::DeleteSelection {
            items: vec![SelectedItem::new(gone, SelectedKind::NoErcDirective)],
        })
        .expect("delete");

    let remaining: Vec<Uuid> = engine
        .document()
        .no_erc_directives
        .iter()
        .map(|d| d.uuid)
        .collect();
    assert_eq!(remaining, vec![keep]);

    engine.undo().expect("undo delete");
    assert_eq!(engine.document().no_erc_directives.len(), 2);
}

#[test]
fn save_then_reopen_round_trips_the_directive() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("sheet.snxsch");

    let mut engine = empty_engine();
    let uuid = place(&mut engine, 12.7, 25.4);
    engine.save_as(&path).expect("save");

    let reopened = Engine::open(&path).expect("reopen");
    let doc = reopened.document();
    assert!(doc.no_connects.is_empty());
    assert_eq!(doc.no_erc_directives.len(), 1);
    assert_eq!(doc.no_erc_directives[0].uuid, uuid);
    assert_eq!(doc.no_erc_directives[0].position, Point::new(12.7, 25.4));

    // A no-edit save of a file that already has directives is byte-stable.
    let first = std::fs::read(&path).expect("read first");
    let mut reopened = reopened;
    reopened.save().expect("re-save");
    assert_eq!(first, std::fs::read(&path).expect("read second"));
}

#[test]
fn erc_clears_the_dangling_wire_row_under_a_placed_directive() {
    let sheet = SchematicSheet {
        paper_size: "A4".to_string(),
        wires: vec![Wire {
            uuid: Uuid::now_v7(),
            start: Point::new(0.0, 0.0),
            end: Point::new(10.0, 0.0),
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut engine = Engine::new(sheet).expect("engine");

    let dangling_at_origin = |engine: &Engine| {
        run_all(&ErcContext::from_snapshot(engine.document()))
            .iter()
            .filter(|d| {
                d.rule_id == RuleId::builtin(RuleKind::DanglingWire)
                    && d.location.x.abs() < 1e-6
                    && d.location.y.abs() < 1e-6
            })
            .count()
    };

    assert_eq!(dangling_at_origin(&engine), 1);
    place(&mut engine, 0.0, 0.0);
    assert_eq!(dangling_at_origin(&engine), 0);
    engine.undo().expect("undo");
    assert_eq!(dangling_at_origin(&engine), 1);
}
