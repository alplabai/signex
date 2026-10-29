//! Undo history memory: unchanged collections are shared between entries and
//! restoring a snapshot reproduces the sheet exactly.

use signex_types::schematic::{Junction, Point, Wire};

use crate::test_support::test_sheet;
use crate::{Command, Engine};

fn wire(i: usize) -> Wire {
    Wire {
        uuid: uuid::Uuid::new_v4(),
        start: Point::new(i as f64, 0.0),
        end: Point::new(i as f64, 5.0),
        stroke_width: 0.0,
        unknown_columns: Default::default(),
        locked: false,
    }
}

fn junction(i: usize) -> Junction {
    Junction {
        uuid: uuid::Uuid::new_v4(),
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
