//! #1315 - "Re-prefix to standard": placed parts take their class prefix
//! under the project's designator standard, as one undo step across sheets,
//! never touching what a user typed, locked, or could not be looked up.

use std::path::PathBuf;
use std::sync::Arc;

use signex_app::annotate_undo::AnnotateUndo;
use signex_app::app::{AnnotateMsg, EditMsg, Message, Signex, TabInfo, TabKind};
use signex_app::library::drift::MountedLibrary;
use signex_app::library::reprefix_lookup::{ReprefixLookup, ReprefixLookupJob};
use signex_types::designator::DesignatorStandard;
use signex_types::schematic::{Point, SchematicSheet, Symbol};
use uuid::Uuid;

use crate::library_place_designator_standard::project;
use crate::library_place_symbol::{Fixture, app_with_library, fixture_of_class};

const ROOT: &str = "place-364.snxsch";
const CHILD: &str = "child-1315.snxsch";

struct World {
    app: Signex,
    f: Fixture,
}

fn symbol(reference: &str, x: f64) -> Symbol {
    Symbol {
        uuid: Uuid::new_v4(),
        lib_id: "Device:D".to_string(),
        reference: reference.to_string(),
        position: Point::new(x, 0.0),
        ..Default::default()
    }
}

/// A part placed from the fixture library's diode row.
fn linked(f: &Fixture, reference: &str, x: f64) -> Symbol {
    Symbol {
        lib_id: format!("{}:{}", f.library_id, f.symbol_id),
        library_id: Some(f.library_id),
        row_id: Some(f.row_id),
        ..symbol(reference, x)
    }
}

/// A part linked to a library that is not mounted.
fn from_unmounted(reference: &str, x: f64) -> Symbol {
    Symbol {
        library_id: Some(Uuid::new_v4()),
        row_id: Some(Uuid::new_v4()),
        ..symbol(reference, x)
    }
}

fn sheet_of(f: &Fixture, symbols: Vec<Symbol>) -> SchematicSheet {
    let mut sheet = SchematicSheet {
        uuid: Uuid::new_v4(),
        paper_size: "A4".to_string(),
        symbols,
        ..Default::default()
    };
    let lib_id = format!("{}:{}", f.library_id, f.symbol_id);
    let mut lib_symbol = signex_library::primitive::Symbol::empty("D");
    lib_symbol.designator = "D?".into();
    sheet
        .lib_symbols
        .insert(lib_id.clone(), lib_symbol.to_lib_symbol(lib_id));
    sheet
}

/// A two-sheet project under DIN 40719-2 (`diode` -> `V`) with the diode
/// library mounted. `child_read_only` makes the child a newer-format sheet.
fn world(f: Fixture, root: Vec<Symbol>, child: Vec<Symbol>, child_read_only: bool) -> World {
    let _ = signex_app::diagnostics::init_logging();
    let mut app = app_with_library(&f);
    project(
        &mut app,
        "P",
        &[ROOT, CHILD],
        Some(DesignatorStandard::Din40719_2),
    );
    let root_sheet = sheet_of(&f, root);
    let mut child_sheet = sheet_of(&f, child);
    if child_read_only {
        child_sheet.unknown.newer_minor_source = Some("snxsch/v1.99".into());
    }
    for (name, sheet) in [(ROOT, root_sheet), (CHILD, child_sheet)] {
        let path = PathBuf::from(name);
        app.document_state.engines.insert(
            path.clone(),
            signex_engine::Engine::new(sheet).expect("engine"),
        );
        if name == CHILD {
            app.document_state.tabs.push(TabInfo {
                title: "child".to_string(),
                path,
                cached_document: None,
                dirty: false,
                project_id: None,
                kind: TabKind::Schematic,
            });
        }
    }
    World { app, f }
}

fn refs(w: &World, name: &str) -> Vec<String> {
    w.app.document_state.engines[&PathBuf::from(name)]
        .document()
        .symbols
        .iter()
        .map(|s| s.reference.clone())
        .collect()
}

fn press(w: &mut Signex) -> u64 {
    let _ = w.update(Message::Annotate(AnnotateMsg::ReprefixPressed));
    w.ui_state.reprefix_generation
}

/// The worker half, run for real (the `Task` wraps it in `spawn_blocking`).
fn lookup(w: &World, generation: u64) -> ReprefixLookup {
    ReprefixLookupJob {
        generation,
        libs: vec![MountedLibrary {
            id: w.f.library_id,
            path: w.f.snxlib.clone(),
            display_name: "lib".into(),
        }],
    }
    .run()
}

fn deliver(app: &mut Signex, generation: u64, lookup: ReprefixLookup) {
    let _ = app.update(Message::Annotate(AnnotateMsg::ReprefixRowsResolved {
        generation,
        result: Arc::new(Ok(lookup)),
    }));
}

fn reprefix(w: &mut World) {
    let generation = press(&mut w.app);
    let found = lookup(w, generation);
    deliver(&mut w.app, generation, found);
}

fn messages(w: &World) -> String {
    w.app
        .document_state
        .panel_ctx
        .diagnostics
        .iter()
        .map(|e| e.message.clone())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn reprefix_is_one_undo_step_across_sheets() {
    let f = fixture_of_class("diode");
    let root = vec![linked(&f, "D1", 0.0), linked(&f, "D2", 10.0)];
    let child = vec![linked(&f, "D3", 0.0)];
    let mut w = world(f, root, child, false);
    // Per-sheet undo must not leak into a re-prefix: it always groups.
    w.app.ui_state.annotate_undo = AnnotateUndo::PerSheet;

    reprefix(&mut w);

    let after: Vec<String> = refs(&w, ROOT).into_iter().chain(refs(&w, CHILD)).collect();
    assert!(after.iter().all(|r| r.starts_with('V')), "{after:?}");
    let distinct: std::collections::HashSet<_> = after.iter().collect();
    assert_eq!(distinct.len(), 3, "{after:?}");

    let _ = w.app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(refs(&w, ROOT), ["D1", "D2"]);
    assert_eq!(refs(&w, CHILD), ["D3"]);
}

#[test]
fn reprefix_skips_user_edited_references() {
    let f = fixture_of_class("diode");
    let root = vec![linked(&f, "D1", 0.0), linked(&f, "X9", 10.0)];
    let mut w = world(f, root, vec![], false);

    reprefix(&mut w);

    let after = refs(&w, ROOT);
    assert!(after[0].starts_with('V'), "{after:?}");
    assert_eq!(after[1], "X9");
    assert!(
        messages(&w).contains("hand-edited prefix"),
        "{}",
        messages(&w)
    );
}

#[test]
fn reprefix_lists_unmounted_and_unlinked_parts() {
    let f = fixture_of_class("diode");
    let root = vec![
        linked(&f, "D1", 0.0),
        symbol("D2", 10.0),
        from_unmounted("D3", 20.0),
    ];
    let mut w = world(f, root, vec![], false);

    reprefix(&mut w);

    let after = refs(&w, ROOT);
    assert!(after[0].starts_with('V'), "{after:?}");
    assert_eq!((after[1].as_str(), after[2].as_str()), ("D2", "D3"));
    let log = messages(&w);
    assert!(log.contains("not placed from a library row"), "{log}");
    assert!(log.contains("library that is not mounted"), "{log}");
}

#[test]
fn reprefix_drops_a_stale_lookup_result() {
    let f = fixture_of_class("diode");
    let root = vec![linked(&f, "D1", 0.0)];
    let mut w = world(f, root, vec![], false);

    let first = press(&mut w.app);
    let second = press(&mut w.app);
    assert_ne!(first, second);
    let stale = lookup(&w, first);
    deliver(&mut w.app, first, stale);

    assert_eq!(refs(&w, ROOT), ["D1"], "a replaced lookup must not apply");
    assert!(messages(&w).contains("dropped"), "{}", messages(&w));

    let fresh = lookup(&w, second);
    deliver(&mut w.app, second, fresh);
    assert!(refs(&w, ROOT)[0].starts_with('V'));
}

#[test]
fn reprefix_leaves_read_only_sheets_alone() {
    let f = fixture_of_class("diode");
    let root = vec![linked(&f, "D1", 0.0)];
    let child = vec![linked(&f, "D5", 0.0)];
    let mut w = world(f, root, child, true);

    reprefix(&mut w);

    assert!(refs(&w, ROOT)[0].starts_with('V'));
    assert_eq!(refs(&w, CHILD), ["D5"]);
}
