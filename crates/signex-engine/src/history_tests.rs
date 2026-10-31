//! Undo history memory: unchanged collections are shared between entries and
//! restoring a snapshot reproduces the sheet exactly.

use signex_types::schematic::{Junction, Point, Wire};

use crate::test_support::test_sheet;
use crate::{Command, Engine};

fn wire(i: usize) -> Wire {
    Wire {
        uuid: uuid::Uuid::now_v7(),
        start: Point::new(i as f64, 0.0),
        end: Point::new(i as f64, 5.0),
        stroke_width: 0.0,
        unknown_columns: Default::default(),
        locked: false,
    }
}

fn junction(i: usize) -> Junction {
    Junction {
        uuid: uuid::Uuid::now_v7(),
        position: Point::new(i as f64, 5.0),
        diameter: 0.0,
        ..Default::default()
    }
}

fn state(engine: &Engine) -> String {
    format!("{:?}", engine.document())
}

#[test]
fn long_edit_sequence_round_trips_through_undo_and_redo() {
    let mut engine = Engine::new(test_sheet()).unwrap();
    let mut states = vec![state(&engine)];
    // More edits than the history cap, mixing two collections.
    let edits = crate::history::MAX_HISTORY_ENTRIES + 20;
    for i in 0..edits {
        let command = if i % 3 == 2 {
            Command::PlaceJunction {
                junction: junction(i),
            }
        } else {
            Command::PlaceWireSegment { wire: wire(i) }
        };
        assert!(engine.execute(command).unwrap().changed);
        states.push(state(&engine));
    }

    let depth = crate::history::MAX_HISTORY_ENTRIES;
    for step in 1..=depth {
        engine.undo().unwrap();
        assert_eq!(state(&engine), states[edits - step]);
    }
    assert!(!engine.can_undo());
    for step in 1..=depth {
        engine.redo().unwrap();
        assert_eq!(state(&engine), states[edits - depth + step]);
    }
    assert!(!engine.can_redo());
}

#[test]
fn consecutive_entries_share_collections_the_step_did_not_touch() {
    let mut engine = Engine::new(test_sheet()).unwrap();
    for i in 0..3 {
        engine
            .execute(Command::PlaceWireSegment { wire: wire(i) })
            .unwrap();
    }
    engine
        .execute(Command::PlaceJunction {
            junction: junction(9),
        })
        .unwrap();

    let last = &engine.history[3].snapshot;
    let before = &engine.history[2].snapshot;
    let shared = last.shared_with(before);
    assert!(!shared.contains(&"wires"), "wires differ between the steps");
    for untouched in ["symbols", "junctions", "labels", "drawings", "buses"] {
        assert!(shared.contains(&untouched), "{untouched} should be shared");
    }
}

mod field_edit_runs {
    use signex_types::schematic::Symbol;

    use crate::test_support::test_sheet;
    use crate::{Command, Engine};

    fn set(engine: &mut Engine, id: uuid::Uuid, key: &str, value: &str) {
        engine
            .execute(Command::SetSymbolField {
                symbol_id: id,
                key: key.into(),
                value: value.into(),
            })
            .unwrap();
    }

    fn engine_with_symbol() -> (Engine, uuid::Uuid) {
        let id = uuid::Uuid::now_v7();
        let mut sheet = test_sheet();
        sheet.symbols.push(Symbol {
            uuid: id,
            ..Default::default()
        });
        (Engine::new(sheet).unwrap(), id)
    }

    fn mpn(engine: &Engine) -> Option<String> {
        engine.document().symbols[0].fields.get("MPN").cloned()
    }

    #[test]
    fn keystrokes_in_one_cell_are_one_undo_step() {
        let (mut engine, id) = engine_with_symbol();
        for text in ["A", "AB", "ABC"] {
            set(&mut engine, id, "MPN", text);
        }
        assert_eq!(mpn(&engine).as_deref(), Some("ABC"));

        engine.undo().unwrap();
        assert_eq!(mpn(&engine), None);
        assert!(!engine.can_undo());
    }

    #[test]
    fn another_cell_or_ending_the_run_starts_a_new_step() {
        let (mut engine, id) = engine_with_symbol();
        set(&mut engine, id, "MPN", "A");
        set(&mut engine, id, "Note", "n");
        set(&mut engine, id, "MPN", "AB");
        engine.end_field_edit_run();
        set(&mut engine, id, "MPN", "ABC");

        engine.undo().unwrap();
        assert_eq!(mpn(&engine).as_deref(), Some("AB"));
        engine.undo().unwrap();
        assert_eq!(mpn(&engine).as_deref(), Some("A"));
    }

    #[test]
    fn an_edit_after_undo_does_not_fold_into_the_undone_step() {
        let (mut engine, id) = engine_with_symbol();
        set(&mut engine, id, "MPN", "A");
        set(&mut engine, id, "MPN", "AB");
        engine.undo().unwrap();
        set(&mut engine, id, "MPN", "Z");

        assert!(!engine.can_redo());
        engine.undo().unwrap();
        assert_eq!(mpn(&engine), None);
    }

    #[test]
    fn undo_ends_the_run_even_when_the_position_comes_back() {
        let (mut engine, id) = engine_with_symbol();
        set(&mut engine, id, "MPN", "A");
        engine.undo().unwrap();
        // An unrelated edit lands the history on the position the run saw.
        engine
            .execute(Command::SetSymbolDnp {
                symbol_id: id,
                dnp: true,
            })
            .unwrap();
        set(&mut engine, id, "MPN", "B");

        engine.undo().unwrap();
        assert_eq!(mpn(&engine), None, "typing after the undo is its own step");
        assert!(engine.document().symbols[0].dnp, "the unrelated edit stays");
    }
}
