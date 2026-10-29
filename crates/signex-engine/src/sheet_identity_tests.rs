//! `Command::ModifyChildSheet` (#794): rename / re-point a placed sheet symbol.

use signex_types::schematic::Point;

use crate::test_support::test_sheet;
use crate::{Command, DocumentPatch, Engine, EngineError, child_sheet_from_corners};

fn engine_with_two_sheets() -> (Engine, uuid::Uuid) {
    let mut engine = Engine::new(test_sheet()).unwrap();
    let mut first_id = uuid::Uuid::nil();
    for (i, stem) in ["sheet1", "sheet2"].iter().enumerate() {
        let x = 10.0 + 50.0 * i as f64;
        let child = child_sheet_from_corners(Point::new(x, 10.0), Point::new(x + 30.0, 40.0), stem)
            .expect("big enough");
        if i == 0 {
            first_id = child.uuid;
        }
        engine
            .execute(Command::PlaceChildSheet { child_sheet: child })
            .unwrap();
    }
    (engine, first_id)
}

fn modify(id: uuid::Uuid, name: Option<&str>, filename: Option<&str>) -> Command {
    Command::ModifyChildSheet {
        sheet_id: id,
        name: name.map(str::to_string),
        filename: filename.map(str::to_string),
    }
}

fn first(engine: &Engine) -> (String, String) {
    let c = &engine.document().child_sheets[0];
    (c.name.clone(), c.filename.clone())
}

#[test]
fn rename_and_repoint_are_undoable_and_flag_child_sheets() {
    let (mut engine, id) = engine_with_two_sheets();
    let result = engine
        .execute(modify(id, Some(" Power "), Some("sub/power.snxsch")))
        .unwrap();
    assert!(result.changed);
    assert!(
        result
            .patch_pair
            .expect("patch")
            .document
            .contains(DocumentPatch::CHILD_SHEETS)
    );
    assert_eq!(first(&engine), ("Power".into(), "sub/power.snxsch".into()));

    engine.undo().unwrap();
    assert_eq!(first(&engine), ("sheet1".into(), "sheet1.snxsch".into()));
    engine.redo().unwrap();
    assert_eq!(first(&engine), ("Power".into(), "sub/power.snxsch".into()));
}

#[test]
fn an_unchanged_value_records_nothing() {
    let (mut engine, id) = engine_with_two_sheets();
    let result = engine.execute(modify(id, Some("sheet1"), None)).unwrap();
    assert!(!result.changed);
}

#[test]
fn refused_edits_leave_the_sheet_and_history_untouched() {
    let (mut engine, id) = engine_with_two_sheets();
    let cases: [(Option<&str>, Option<&str>); 8] = [
        (None, Some("")),
        (None, Some("   ")),
        (None, Some("sheet9.txt")),
        (None, Some(".snxsch")),
        (None, Some("sheet2.snxsch")),
        (Some("  "), None),
        (Some("SHEET2"), None),
        // A valid name must not slip through alongside an invalid file.
        (Some("Fine"), Some("nope.txt")),
    ];
    for (name, file) in cases {
        assert!(engine.execute(modify(id, name, file)).is_err());
    }
    assert_eq!(first(&engine), ("sheet1".into(), "sheet1.snxsch".into()));
    // Only the two placements are in history.
    engine.undo().unwrap();
    engine.undo().unwrap();
    assert!(engine.undo().unwrap().is_none());
}

#[test]
fn error_kinds_are_distinct() {
    let (mut engine, id) = engine_with_two_sheets();
    assert!(matches!(
        engine.execute(modify(id, None, Some(""))),
        Err(EngineError::EmptyChildSheetFilename)
    ));
    assert!(matches!(
        engine.execute(modify(id, None, Some("a.txt"))),
        Err(EngineError::InvalidChildSheetFilename(_))
    ));
    assert!(matches!(
        engine.execute(modify(id, Some("sheet2"), None)),
        Err(EngineError::DuplicateChildSheetName(_))
    ));
    assert!(matches!(
        engine.execute(modify(id, None, Some("Sheet2.SNXSCH"))),
        Err(EngineError::DuplicateChildSheetFile(_))
    ));
}

#[test]
fn an_unknown_sheet_is_a_no_op() {
    let (mut engine, _) = engine_with_two_sheets();
    let result = engine
        .execute(modify(uuid::Uuid::now_v7(), Some("x"), None))
        .unwrap();
    assert!(!result.changed);
}
