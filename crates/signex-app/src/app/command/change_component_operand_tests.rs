//! #2954: `change_component` from operands — the part swapped for a library
//! row in one undo step, the pins mapped by number or by hand, the answer's
//! pin table, `dry_run`, and the refusals.

use std::path::PathBuf;

use serde_json::{Value, json};
use signex_i18n::RenderTranslatable;
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::primitive::{Symbol as LibrarySymbol, SymbolFile, SymbolPin};
use signex_library::{InternalPn, LibraryAdapter, ManufacturerPart, PrimitiveRef};
use signex_types::schematic::{Point, SchematicSheet};
use uuid::Uuid;

use super::change_component_operands::pin_pairs;
use super::{CommandArgs, CommandOutcome};
use crate::app::Signex;
use crate::app::handlers::canvas::placement_tests::{grid, sheet_shape};
use crate::app::handlers::editing_commands::net_color_driver::add_sheet_with;
use crate::keymap::AppCommandId;

/// A library on disk with two rows: `SNX-A` drawn with pins 1 IN and 2 OUT,
/// `SNX-B` with pins 1 OUT, 2 IN and 3 EN, each pin a grid step lower.
struct Library {
    _dir: tempfile::TempDir,
    snxlib: PathBuf,
    id: Uuid,
}

fn symbol(name: &str, pins: &[(&str, &str)]) -> LibrarySymbol {
    let mut symbol = LibrarySymbol::empty(name);
    symbol.designator = "U?".into();
    for (at, (number, pin_name)) in pins.iter().enumerate() {
        let mut pin = SymbolPin::new(*number, *pin_name);
        pin.position = [0.0, 5.08 * at as f64];
        symbol.pins.push(pin);
    }
    symbol
}

fn library() -> Library {
    signex_library::git_identity::test_identity::install();
    let dir = tempfile::tempdir().expect("tempdir");
    let snxlib = dir.path().join("lib").join("lib.snxlib");
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: Uuid::now_v7(),
        library: LibrarySection {
            name: "lib".into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    let options = LibraryInitOptions {
        enable_git: false,
        use_lfs: false,
    };
    let adapter = LocalGitAdapter::init(&snxlib, manifest, options).expect("init");
    let id = adapter.library_id();
    let symbols = snxlib.parent().expect("folder").join("symbols");
    std::fs::create_dir_all(&symbols).expect("symbols dir");
    let parts = [
        ("SNX-A", "A-1", symbol("A", &[("1", "IN"), ("2", "OUT")])),
        (
            "SNX-B",
            "B-1",
            symbol("B", &[("1", "OUT"), ("2", "IN"), ("3", "EN")]),
        ),
    ];
    for (pn, mpn, drawing) in parts {
        let symbol_ref = PrimitiveRef::new(id, drawing.uuid);
        let file = format!("{}.snxsym", drawing.name.to_lowercase());
        let text = SymbolFile::from_symbol(drawing)
            .to_toml_string()
            .expect("emit");
        std::fs::write(symbols.join(file), text).expect("write symbol");
        let mut row = crate::library::component_preview::test_row();
        row.internal_pn = InternalPn::new(pn);
        row.primary_mpn = ManufacturerPart::draft("Acme", mpn);
        row.symbol_ref = symbol_ref;
        adapter.insert_row("ICs", row, "seed").expect("insert row");
    }
    drop(adapter);
    drop(LocalGitAdapter::recover_init(&snxlib).expect("recover_init"));
    Library {
        _dir: dir,
        snxlib,
        id,
    }
}

fn sheet_path() -> PathBuf {
    std::env::temp_dir().join("snx-2954").join("change.snxsch")
}

fn run(app: &mut Signex, id: &str, operands: &[String]) -> CommandOutcome {
    let id = AppCommandId::new(id).expect("a catalog id");
    let refs: Vec<&str> = operands.iter().map(String::as_str).collect();
    let args = CommandArgs::parse_operands(&id, &refs).expect("operands");
    app.run_operands_settled(&id, &args).0
}

#[track_caller]
fn answered(outcome: CommandOutcome) -> Value {
    match outcome {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    }
}

fn refusal(outcome: CommandOutcome) -> String {
    match outcome {
        CommandOutcome::Disabled { reason } => reason.render_en_us(),
        other => panic!("not refused: {other:?}"),
    }
}

fn pt(p: Point) -> String {
    format!("{},{}", p.x, p.y)
}

/// An app with `U1` (row `SNX-A`) placed and its pin 1 wired out to the
/// net label `SIG`; the part's uuid.
fn app_with_u1(lib: &Library) -> (Signex, Uuid) {
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.snap_enabled = true;
    app.ui_state.grid_size_mm = 1.27;
    add_sheet_with(&mut app, &sheet_path(), SchematicSheet::default());
    app.library
        .open_library(lib.snxlib.clone())
        .expect("mount library");
    let placed = run(
        &mut app,
        "place_component",
        &[
            format!("library={}", lib.id),
            "part=SNX-A".into(),
            format!("at={}", pt(grid(8.0, 16.0))),
            "designator=U1".into(),
        ],
    );
    answered(placed);
    let end = pt(grid(-8.0, 16.0));
    let wired = run(
        &mut app,
        "place_wire",
        &[
            "start=U1.1".into(),
            format!("end={end}"),
            "allow_open=true".into(),
        ],
    );
    answered(wired);
    // A label placement runs and answers nothing (`placement_operand_tests`).
    assert_eq!(
        run(
            &mut app,
            "place_net_label",
            &[format!("at={end}"), "text=SIG".into()],
        ),
        CommandOutcome::Ran
    );
    let id = app
        .document_state
        .active_engine()
        .expect("engine")
        .document()
        .symbols[0]
        .uuid;
    (app, id)
}

/// `change_component` of `id` to `part`, plus `extra`.
fn change(app: &mut Signex, lib: &Library, id: Uuid, part: &str, extra: &[&str]) -> CommandOutcome {
    let mut operands = vec![
        format!("id={id}"),
        format!("library={}", lib.id),
        format!("part={part}"),
    ];
    operands.extend(extra.iter().map(|s| (*s).to_string()));
    run(app, "change_component", &operands)
}

fn lib_id(app: &Signex) -> String {
    let doc = app
        .document_state
        .active_engine()
        .expect("engine")
        .document();
    doc.symbols[0].lib_id.clone()
}

/// The answer's pin row for old pin `old`.
fn pin_row<'a>(answer: &'a Value, old: &str) -> &'a Value {
    answer["pins"]
        .as_array()
        .expect("pins")
        .iter()
        .find(|row| row["old"] == old)
        .unwrap_or_else(|| panic!("no row for pin {old}: {answer}"))
}

#[test]
fn pin_pairs_read_old_new_and_a_dash_for_no_pin() {
    let pairs = pin_pairs(" 1:2, 3:- ,").expect("pairs");
    assert_eq!(
        pairs,
        vec![
            ("1".to_string(), Some("2".to_string())),
            ("3".to_string(), None)
        ]
    );
    for bad in ["1", "1:", ":2", "1:2,x"] {
        let reason = pin_pairs(bad).expect_err(bad);
        assert!(reason.contains("old:new"), "{bad}: {reason}");
    }
}

#[test]
fn a_dry_run_answers_the_pin_table_and_changes_nothing() {
    let lib = library();
    let (mut app, id) = app_with_u1(&lib);
    let before = sheet_shape(&app);
    let answer = answered(change(&mut app, &lib, id, "SNX-B", &["dry_run=true"]));
    assert_eq!(answer["dry_run"], true);
    assert_eq!(answer["designator"], "U1");
    assert_eq!(answer["part"], "SNX-B");
    assert_eq!(answer["mapped_by_hand"], false);
    // By number: pin 1 keeps its net on the new part's pin 1.
    let one = pin_row(&answer, "1");
    assert_eq!(
        (
            one["old_name"].clone(),
            one["new"].clone(),
            one["net"].clone()
        ),
        (json!("IN"), json!("1"), json!("SIG"))
    );
    assert_eq!(pin_row(&answer, "2")["net"], Value::Null);
    assert_eq!(sheet_shape(&app), before, "a dry run changes nothing");
}

#[test]
fn a_hand_mapping_is_followed_in_one_undo_step() {
    let lib = library();
    let (mut app, id) = app_with_u1(&lib);
    let before = sheet_shape(&app);
    let old_lib = lib_id(&app);
    let answer = answered(change(&mut app, &lib, id, "SNX-B", &["pins=1:2"]));
    assert_eq!(answer["dry_run"], false);
    assert_eq!(answer["mapped_by_hand"], true);
    let one = pin_row(&answer, "1");
    assert_eq!(
        (
            one["new"].clone(),
            one["new_name"].clone(),
            one["net"].clone()
        ),
        (json!("2"), json!("IN"), json!("SIG"))
    );
    // New pin 2 served old pin 2 by number; it now serves pin 1 only.
    assert_eq!(pin_row(&answer, "2")["new"], Value::Null);
    let carried = answer["carried"].as_array().expect("carried");
    assert!(carried.contains(&json!("1")), "{answer}");
    assert_ne!(lib_id(&app), old_lib, "the part is the new row's");

    let undo = AppCommandId::new("undo").expect("id");
    let _ = app.dispatch_command_reporting(&undo, CommandArgs::none(), None);
    assert_eq!(sheet_shape(&app), before, "one undo step");
}

#[test]
fn unknown_pins_parameters_parts_and_ids_are_refused() {
    let lib = library();
    let (mut app, id) = app_with_u1(&lib);
    let before = sheet_shape(&app);
    let cases: [(&[&str], &str); 4] = [
        (&["pins=9:1"], "the part has no pin 9"),
        (&["pins=1:9"], "the new part has no pin 9"),
        (&["pins=1"], "old:new"),
        (&["drop=Nope"], "`Nope` is not a parameter"),
    ];
    for (extra, needle) in cases {
        let reason = refusal(change(&mut app, &lib, id, "SNX-B", extra));
        assert!(reason.contains(needle), "{extra:?}: {reason}");
    }
    let reason = refusal(change(&mut app, &lib, id, "NOPE", &[]));
    assert!(reason.contains("has no part `NOPE`"), "{reason}");
    let reason = refusal(change(&mut app, &lib, Uuid::now_v7(), "SNX-B", &[]));
    assert!(reason.contains("no placed part"), "{reason}");
    let outcome = run(
        &mut app,
        "change_component",
        &[
            "id=U1".into(),
            format!("library={}", lib.id),
            "part=SNX-B".into(),
        ],
    );
    assert!(refusal(outcome).contains("must be a part id"));
    assert_eq!(sheet_shape(&app), before, "nothing refused was applied");
}
