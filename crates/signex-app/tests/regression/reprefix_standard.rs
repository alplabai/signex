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
        project: w
            .app
            .document_state
            .projects
            .first()
            .map(|p| p.path.clone()),
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

/// Press and let the libraries answer: the confirm is open, nothing changed.
fn open_confirm(w: &mut World) {
    let generation = press(&mut w.app);
    let found = lookup(w, generation);
    deliver(&mut w.app, generation, found);
}

fn confirm(app: &mut Signex) {
    let _ = app.update(Message::Annotate(AnnotateMsg::ReprefixConfirmed));
}

/// The whole flow: press, answer, Confirm.
fn reprefix(w: &mut World) {
    open_confirm(w);
    confirm(&mut w.app);
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
    confirm(&mut w.app);
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

#[test]
fn reprefix_without_a_standard_changes_nothing_and_says_so() {
    let f = fixture_of_class("diode");
    let root = vec![linked(&f, "D1", 0.0)];
    let mut w = world(f, root, vec![], false);
    w.app.document_state.projects[0]
        .data
        .set_designator_standard(None);

    reprefix(&mut w);

    assert_eq!(refs(&w, ROOT), ["D1"]);
    assert!(
        messages(&w).contains("no designator standard"),
        "{}",
        messages(&w)
    );
}

#[test]
fn reprefix_confirm_cancel_changes_nothing() {
    let f = fixture_of_class("diode");
    let root = vec![linked(&f, "D1", 0.0)];
    let mut w = world(f, root, vec![], false);

    open_confirm(&mut w);
    let open = w
        .app
        .ui_state
        .reprefix_confirm
        .as_ref()
        .expect("confirm is open");
    assert_eq!(open.change_lines.len(), 1);
    assert_eq!(refs(&w, ROOT), ["D1"], "nothing applies before Confirm");

    let _ = w
        .app
        .update(Message::Annotate(AnnotateMsg::ReprefixCancelled));

    assert!(w.app.ui_state.reprefix_confirm.is_none());
    assert_eq!(refs(&w, ROOT), ["D1"]);
    assert!(!w.app.document_state.engines[&PathBuf::from(ROOT)].can_undo());
    // A late Confirm after the cancel does nothing either.
    confirm(&mut w.app);
    assert_eq!(refs(&w, ROOT), ["D1"]);
}

#[test]
fn reprefix_result_for_another_project_is_dropped() {
    let f = fixture_of_class("diode");
    let root = vec![linked(&f, "D1", 0.0)];
    let mut w = world(f, root, vec![], false);

    let generation = press(&mut w.app);
    let mut found = lookup(&w, generation);
    found.project = Some(PathBuf::from("Other.snxprj"));
    deliver(&mut w.app, generation, found);

    assert!(w.app.ui_state.reprefix_confirm.is_none());
    assert_eq!(refs(&w, ROOT), ["D1"]);
    assert!(messages(&w).contains("another project"), "{}", messages(&w));
}

#[test]
fn reprefix_multi_unit_part_across_two_open_sheets_undoes_as_one() {
    let f = fixture_of_class("diode");
    let mut unit1 = linked(&f, "D1", 0.0);
    unit1.unit = 1;
    let mut unit2 = linked(&f, "D1", 0.0);
    unit2.unit = 2;
    let mut w = world(f, vec![unit1], vec![unit2], false);

    reprefix(&mut w);

    let (a, b) = (refs(&w, ROOT), refs(&w, CHILD));
    assert!(a[0].starts_with('V'), "{a:?}");
    assert_eq!(a, b, "both units of the part share one designator");

    let _ = w.app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(
        (refs(&w, ROOT), refs(&w, CHILD)),
        (vec!["D1".to_string()], vec!["D1".to_string()])
    );
}

#[test]
fn reprefix_overflow_does_not_split_a_part() {
    let f = fixture_of_class("diode");
    // The child (walked first) holds a part of a second row (LED -> `H`); the
    // root holds a diode that must become `V`, but `V` is saturated.
    let led_row = Uuid::new_v4();
    let mut led = linked(&f, "D5", 0.0);
    led.row_id = Some(led_row);
    let root = vec![linked(&f, "D1", 0.0), linked(&f, "V4294967295", 10.0)];
    let mut w = world(f, root, vec![led], false);

    let generation = press(&mut w.app);
    let mut found = lookup(&w, generation);
    found
        .libraries
        .get_mut(&w.f.library_id)
        .expect("library read")
        .insert(
            led_row,
            signex_app::library::reprefix_lookup::RowFacts {
                internal_pn: "LED".into(),
                symbol: (Uuid::nil(), Uuid::nil()),
                class: "led".into(),
                prefix_override: None,
            },
        );
    deliver(&mut w.app, generation, found);
    confirm(&mut w.app);

    assert_eq!(refs(&w, CHILD), ["D5"], "the converted sheet was put back");
    assert_eq!(refs(&w, ROOT), ["D1", "V4294967295"]);
    assert!(
        messages(&w).contains("whole run was undone"),
        "{}",
        messages(&w)
    );
    assert!(
        !w.app
            .document_state
            .dirty_paths
            .contains(&PathBuf::from(CHILD))
    );
    // Rolled back, not undone: Ctrl+Y must not bring half of the run back.
    let child = &w.app.document_state.engines[&PathBuf::from(CHILD)];
    assert!(!child.can_undo() && !child.can_redo());
    let _ = w.app.update(Message::Edit(EditMsg::Redo));
    assert_eq!(refs(&w, CHILD), ["D5"]);
    assert_eq!(refs(&w, ROOT), ["D1", "V4294967295"]);
}

#[test]
fn reprefix_reports_invalid_row_override() {
    use signex_library::{DESIGNATOR_PREFIX_PARAM, ParamMap, ParamValue};
    let params: ParamMap = [(
        DESIGNATOR_PREFIX_PARAM.to_owned(),
        ParamValue::Text("R1".into()),
    )]
    .into_iter()
    .collect();
    let f = crate::library_place_symbol::fixture_with_parameters("diode", params);
    let root = vec![linked(&f, "D1", 0.0)];
    let mut w = world(f, root, vec![], false);

    reprefix(&mut w);

    // Ignored and reported; the standard's `V` still applies.
    assert!(refs(&w, ROOT)[0].starts_with('V'));
    let log = messages(&w);
    assert!(log.contains("designator prefix override"), "{log}");
    assert!(log.contains("is ignored"), "{log}");
}

#[test]
fn reprefix_lists_parts_whose_row_is_gone() {
    let f = fixture_of_class("diode");
    let root = vec![linked(&f, "D1", 0.0)];
    let mut w = world(f, root, vec![], false);

    let generation = press(&mut w.app);
    let mut found = lookup(&w, generation);
    found
        .libraries
        .get_mut(&w.f.library_id)
        .expect("library read")
        .clear();
    deliver(&mut w.app, generation, found);

    assert!(w.app.ui_state.reprefix_confirm.is_none());
    assert_eq!(refs(&w, ROOT), ["D1"]);
    assert!(
        messages(&w).contains("whose library row no longer exists"),
        "{}",
        messages(&w)
    );
}

/// A project whose child sheet exists only on disk (the root is open).
/// Returns the world, the temp dir, both sheet paths and the child's bytes.
fn disk_world() -> (World, tempfile::TempDir, PathBuf, PathBuf, Vec<u8>) {
    let _ = signex_app::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let (root_path, child_path) = (
        dir.path().join("root.snxsch"),
        dir.path().join("child.snxsch"),
    );
    let f = fixture_of_class("diode");
    // The child exists only on disk.
    let mut child = signex_engine::Engine::new(sheet_of(&f, vec![linked(&f, "D2", 0.0)]))
        .expect("child engine");
    child.set_path(Some(child_path.clone()));
    child.save().expect("write the child sheet");
    let bytes_before = std::fs::read(&child_path).expect("read child");

    let mut app = app_with_library(&f);
    app.document_state.engines.clear();
    let root_engine =
        signex_engine::Engine::new(sheet_of(&f, vec![linked(&f, "D1", 0.0)])).expect("root engine");
    app.document_state
        .engines
        .insert(root_path.clone(), root_engine);
    app.document_state.tabs[0].path = root_path.clone();
    app.document_state.active_path = Some(root_path.clone());
    project(
        &mut app,
        "P",
        &["root.snxsch", "child.snxsch"],
        Some(DesignatorStandard::Din40719_2),
    );
    app.document_state.projects[0].path = dir.path().join("P.snxprj");
    (World { app, f }, dir, root_path, child_path, bytes_before)
}

#[test]
fn reprefix_unopened_sheets_stay_dirty_unsaved_and_undo_as_one() {
    let (mut w, _dir, root_path, child_path, bytes_before) = disk_world();

    open_confirm(&mut w);
    let lines = &w
        .app
        .ui_state
        .reprefix_confirm
        .as_ref()
        .expect("confirm")
        .change_lines;
    assert!(
        !lines.iter().any(|l| l.contains("matched by symbol")),
        "the reopened part carries its stored row link (#1417): {lines:?}"
    );
    confirm(&mut w.app);

    assert_eq!(
        std::fs::read(&child_path).expect("read child"),
        bytes_before,
        "nothing reaches disk until the user saves"
    );
    let parked = &w.app.document_state.engines[&child_path];
    assert!(parked.document().symbols[0].reference.starts_with('V'));
    assert!(w.app.document_state.dirty_paths.contains(&child_path));
    assert!(
        w.app.document_state.engines[&root_path].document().symbols[0]
            .reference
            .starts_with('V')
    );

    let _ = w.app.update(Message::Edit(EditMsg::Undo));
    let engines = &w.app.document_state.engines;
    assert_eq!(engines[&root_path].document().symbols[0].reference, "D1");
    // Back at what is on disk: nothing to save, no unsaved-edits prompt, and
    // no tabless engine left to shadow the file.
    assert!(!w.app.document_state.dirty_paths.contains(&child_path));
    assert!(!engines.contains_key(&child_path));

    // Redo is refused whole: re-applying the open sheet alone would split
    // parts across two prefixes.
    let _ = w.app.update(Message::Edit(EditMsg::Redo));
    let engines = &w.app.document_state.engines;
    assert_eq!(engines[&root_path].document().symbols[0].reference, "D1");
    let log = messages(&w);
    assert!(
        log.contains("child.snxsch went back to its saved state"),
        "{log}"
    );
}

/// Unit A on the open root, unit B on an unopened child: undo then redo must
/// never leave the one part under two designators.
#[test]
fn redo_of_a_reprefix_with_a_dropped_parked_sheet_changes_nothing() {
    let (mut w, _dir, root_path, child_path, _bytes) = disk_world();
    // Make D1 a two-unit part: unit 2 sits on the child (same designator).
    let unit = |s: &Symbol, n: u32| Symbol {
        unit: n,
        ..s.clone()
    };
    let root_unit = unit(
        &w.app.document_state.engines[&root_path].document().symbols[0],
        1,
    );
    let mut root_engine =
        signex_engine::Engine::new(sheet_of(&w.f, vec![root_unit.clone()])).expect("root");
    root_engine.set_path(Some(root_path.clone()));
    w.app
        .document_state
        .engines
        .insert(root_path.clone(), root_engine);
    let mut child = signex_engine::Engine::new(sheet_of(
        &w.f,
        vec![Symbol {
            uuid: Uuid::new_v4(),
            ..unit(&root_unit, 2)
        }],
    ))
    .expect("child");
    child.set_path(Some(child_path.clone()));
    child.save().expect("write the child");

    reprefix(&mut w);
    let _ = w.app.update(Message::Edit(EditMsg::Undo));
    let _ = w.app.update(Message::Edit(EditMsg::Redo));

    let root_ref = w.app.document_state.engines[&root_path].document().symbols[0]
        .reference
        .clone();
    assert_eq!(root_ref, "D1", "redo must not touch the root alone");
    assert!(!w.app.document_state.engines.contains_key(&child_path));
}

/// Undo of a re-prefix on an unopened sheet must not leave a clean copy that
/// a later Annotate edits instead of what changed on disk meanwhile.
#[test]
fn undone_reprefix_does_not_shadow_a_sheet_rewritten_on_disk() {
    let (mut w, _dir, _root_path, child_path, _bytes) = disk_world();
    reprefix(&mut w);
    let _ = w.app.update(Message::Edit(EditMsg::Undo));

    // Another program (a pull, another instance) rewrites the child.
    let mut rewritten =
        signex_engine::Engine::new(sheet_of(&w.f, vec![symbol("R?", 0.0)])).expect("engine");
    rewritten.set_path(Some(child_path.clone()));
    rewritten.save().expect("rewrite the child on disk");

    let _ = w.app.update(Message::Annotate(AnnotateMsg::Run(
        signex_engine::AnnotateMode::Incremental,
    )));

    // An unopened sheet is annotated and written straight to disk.
    let on_disk = signex_engine::Engine::open(&child_path).expect("reopen the child");
    assert_eq!(
        on_disk.document().symbols[0].reference,
        "R1",
        "Annotate must read the rewritten file, not the pre-undo copy"
    );
}

#[test]
fn reprefix_names_the_sheets_where_undo_reverts_it() {
    let f = fixture_of_class("diode");
    // The active (root) sheet has nothing to re-prefix.
    let root = vec![linked(&f, "X9", 0.0)];
    let child = vec![linked(&f, "D2", 0.0)];
    let mut w = world(f, root, child, false);

    reprefix(&mut w);

    let log = messages(&w);
    assert!(
        log.contains("Ctrl+Z on any of these sheets undoes the whole run"),
        "{log}"
    );
    assert!(log.contains("child-1315.snxsch"), "{log}");
    assert!(log.contains("nothing to re-prefix"), "{log}");
}

#[test]
fn closing_the_annotate_dialog_drops_an_open_reprefix_confirm() {
    let f = fixture_of_class("diode");
    let root = vec![linked(&f, "D1", 0.0)];
    let mut w = world(f, root, vec![], false);
    open_confirm(&mut w);
    assert!(w.app.ui_state.reprefix_confirm.is_some());

    let _ = w.app.update(Message::Annotate(AnnotateMsg::CloseDialog));

    assert!(w.app.ui_state.reprefix_confirm.is_none());
    confirm(&mut w.app);
    assert_eq!(refs(&w, ROOT), ["D1"]);
}

#[test]
fn undo_during_a_save_of_a_parked_sheet_keeps_it_dirty() {
    let (mut w, _dir, _root_path, child_path, _bytes) = disk_world();
    reprefix(&mut w);
    assert!(w.app.document_state.dirty_paths.contains(&child_path));

    // The save of the re-prefixed bytes is in flight when Undo is pressed.
    let _job = w
        .app
        .document_state
        .document_saves
        .begin(child_path.clone(), Vec::new());
    let _ = w.app.update(Message::Edit(EditMsg::Undo));

    assert_eq!(
        w.app.document_state.engines[&child_path].document().symbols[0].reference,
        "D2"
    );
    assert!(
        w.app.document_state.dirty_paths.contains(&child_path),
        "the landing save would write the re-prefixed bytes; the sheet must stay dirty"
    );
}

/// The confirm lists only the re-prefixed parts; an unrelated `R?` that the
/// list never showed must not be numbered by the run.
#[test]
fn reprefix_does_not_number_parts_the_confirm_never_showed() {
    let f = fixture_of_class("diode");
    let root = vec![linked(&f, "D1", 0.0), symbol("R?", 10.0)];
    let mut w = world(f, root, vec![], false);

    reprefix(&mut w);

    let root_refs = refs(&w, ROOT);
    assert!(root_refs[0].starts_with('V'), "{root_refs:?}");
    assert_eq!(root_refs[1], "R?", "{root_refs:?}");
}
