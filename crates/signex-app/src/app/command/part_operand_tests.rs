//! #2199 PR C — `place_component` from operands: a library row placed at a
//! point through the click's commit, the unit and designator rules, the
//! connection rule and the refusals.

use std::path::PathBuf;

use serde_json::{Value, json};
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::primitive::{
    Symbol as LibrarySymbol, SymbolFile, SymbolGraphic, SymbolGraphicKind, SymbolPin,
};
use signex_library::{InternalPn, LibraryAdapter, ManufacturerPart, PrimitiveRef};
use signex_types::schematic::{Point, SchematicSheet, Wire};
use uuid::Uuid;

use super::{CommandArgs, CommandOutcome};
use crate::app::Signex;
use crate::app::handlers::canvas::placement_tests::{click, grid, sheet_shape};
use crate::app::handlers::editing_commands::net_color_driver::add_sheet_with;
use crate::keymap::AppCommandId;
use crate::library::LibraryMessage;

/// A library on disk with one two-unit symbol (`U?`; pin 1 on unit 1, pin 2
/// on unit 2, pin 3 shared) and two rows bound to it that share the
/// manufacturer part number `TWO-1`.
struct Library {
    _dir: tempfile::TempDir,
    snxlib: PathBuf,
    id: Uuid,
    row: Uuid,
}

fn two_unit_symbol() -> LibrarySymbol {
    let mut symbol = LibrarySymbol::empty("TWO");
    symbol.designator = "U?".into();
    symbol.part_count = 2;
    for (number, part, y) in [("1", 1u8, 0.0), ("2", 2, 5.08), ("3", 0, 10.16)] {
        let mut pin = SymbolPin::new(number, format!("P{number}"));
        pin.position = [0.0, y];
        pin.part_number = part;
        symbol.pins.push(pin);
    }
    for part in [1u8, 2] {
        symbol.graphics.push(SymbolGraphic {
            body_style: 1,
            kind: SymbolGraphicKind::Line {
                from: [0.0, 0.0],
                to: [-2.54, 0.0],
            },
            stroke_width: 0.1,
            fill: None,
            part_number: part,
        });
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
    let symbol = two_unit_symbol();
    let symbol_ref = PrimitiveRef::new(id, symbol.uuid);
    let symbols = snxlib.parent().expect("folder").join("symbols");
    std::fs::create_dir_all(&symbols).expect("symbols dir");
    let text = SymbolFile::from_symbol(symbol)
        .to_toml_string()
        .expect("emit");
    std::fs::write(symbols.join("two.snxsym"), text).expect("write symbol");
    let mut rows = Vec::new();
    for pn in ["SNX-TWO", "SNX-TWO-B"] {
        let mut row = crate::library::component_preview::test_row();
        row.internal_pn = InternalPn::new(pn);
        row.primary_mpn = ManufacturerPart::draft("Acme", "TWO-1");
        row.symbol_ref = symbol_ref;
        rows.push(row.row_id);
        adapter.insert_row("ICs", row, "seed").expect("insert row");
    }
    drop(adapter);
    drop(LocalGitAdapter::recover_init(&snxlib).expect("recover_init"));
    Library {
        _dir: dir,
        snxlib,
        id,
        row: rows[0],
    }
}

fn sheet_path() -> PathBuf {
    std::env::temp_dir().join("snx-2199c").join("parts.snxsch")
}

/// A grid-snapping app with `sheet` active and the library mounted.
fn app_on(lib: &Library, sheet: SchematicSheet) -> Signex {
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.snap_enabled = true;
    app.ui_state.grid_size_mm = 1.27;
    add_sheet_with(&mut app, &sheet_path(), sheet);
    app.library
        .open_library(lib.snxlib.clone())
        .expect("mount library");
    app
}

fn app(lib: &Library) -> Signex {
    app_on(lib, SchematicSheet::default())
}

fn pt(p: Point) -> String {
    format!("{},{}", p.x, p.y)
}

fn run(app: &mut Signex, id: &str, operands: &[String]) -> CommandOutcome {
    let id = AppCommandId::new(id).expect("a catalog id");
    let refs: Vec<&str> = operands.iter().map(String::as_str).collect();
    let args = CommandArgs::parse_operands(&id, &refs).expect("operands");
    app.run_operands(&id, &args).0
}

/// `place_component` of `lib`'s first row at `at`, plus `extra`.
fn place(app: &mut Signex, lib: &Library, at: Point, extra: &[&str]) -> CommandOutcome {
    let mut operands = vec![
        format!("library={}", lib.id),
        "part=SNX-TWO".to_string(),
        format!("at={}", pt(at)),
    ];
    operands.extend(extra.iter().map(|s| (*s).to_string()));
    run(app, "place_component", &operands)
}

fn answered(outcome: CommandOutcome) -> Value {
    match outcome {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    }
}

fn refusal(outcome: CommandOutcome) -> String {
    match outcome {
        CommandOutcome::Disabled { reason } => reason,
        other => panic!("not refused: {other:?}"),
    }
}

fn symbols(app: &Signex) -> &[signex_types::schematic::Symbol] {
    &app.document_state
        .active_engine()
        .expect("engine")
        .document()
        .symbols
}

#[test]
fn place_component_makes_the_document_a_pick_and_click_makes() {
    let lib = library();
    let at = grid(8.0, 16.0);

    let mut clicked = app(&lib);
    let resolved = crate::library::place::resolve_placement(
        &lib.snxlib,
        "ICs",
        signex_library::RowId(lib.row),
        &[],
    )
    .expect("resolve");
    let _ = clicked.update(crate::app::Message::Library(
        LibraryMessage::LibraryComponentResolved {
            target_sheet: sheet_path(),
            arm_epoch: clicked.interaction_state.placement_epoch,
            library_path: lib.snxlib.clone(),
            result: Ok(Box::new(resolved)),
        },
    ));
    click(&mut clicked, (at.x, at.y));

    let mut by_operands = app(&lib);
    let empty = sheet_shape(&by_operands);
    let answer = answered(place(&mut by_operands, &lib, at, &[]));
    assert_ne!(sheet_shape(&by_operands), empty);
    assert_eq!(sheet_shape(&by_operands), sheet_shape(&clicked));
    assert_eq!(answer["designator"], "U1");
    assert_eq!(answer["unit"], 1);
    // The tool and the cursor are left alone.
    assert!(by_operands.interaction_state.placement.is_none());
    assert_eq!(
        by_operands.interaction_state.current_tool,
        crate::app::Tool::Select
    );
}

#[test]
fn the_answer_lists_the_placed_units_pin_tips() {
    let lib = library();
    let at = grid(8.0, 16.0);
    let mut app = app(&lib);
    let answer = answered(place(&mut app, &lib, at, &[]));
    assert_eq!(answer["at"], json!([at.x, at.y]));
    assert_eq!(answer["lib_id"], symbols(&app)[0].lib_id);
    let pins = answer["pins"].as_array().expect("pins");
    let numbers: Vec<&str> = pins.iter().filter_map(|p| p["number"].as_str()).collect();
    // Unit 1 shows its own pin and the shared one, never unit 2's.
    assert_eq!(numbers, ["1", "3"]);
    // Pin 1's connection point is the symbol's origin.
    assert_eq!(pins[0]["at"], json!([at.x, at.y]));
    assert_eq!(pins[0]["name"], "P1");
}

#[test]
fn designator_value_rotation_and_mirror_come_from_the_operands() {
    let lib = library();
    let mut app = app(&lib);
    let extra = ["designator=U7", "value=Custom", "rotation=90", "mirror=x"];
    let answer = answered(place(&mut app, &lib, grid(8.0, 16.0), &extra));
    assert_eq!(answer["designator"], "U7");
    let symbol = &symbols(&app)[0];
    assert_eq!(
        (
            symbol.reference.as_str(),
            symbol.value.as_str(),
            symbol.rotation
        ),
        ("U7", "Custom", 90.0)
    );
    assert!(symbol.mirror_x && !symbol.mirror_y);
    // Without a designator the next free one is used, as a click numbers.
    let next = answered(place(&mut app, &lib, grid(40.0, 16.0), &[]));
    assert_eq!(next["designator"], "U8");
}

#[test]
fn unit_two_of_a_placed_part_shares_its_designator_once() {
    let lib = library();
    let mut app = app(&lib);
    answered(place(&mut app, &lib, grid(8.0, 16.0), &[]));
    let second = answered(place(
        &mut app,
        &lib,
        grid(40.0, 16.0),
        &["unit=2", "designator=U1"],
    ));
    assert_eq!(
        (second["designator"].clone(), second["unit"].clone()),
        (json!("U1"), json!(2))
    );
    let numbers: Vec<&str> = second["pins"]
        .as_array()
        .expect("pins")
        .iter()
        .filter_map(|p| p["number"].as_str())
        .collect();
    assert_eq!(numbers, ["2", "3"]);
    // Both units are one part: `U1.2` names unit 2's pin.
    let again = place(
        &mut app,
        &lib,
        grid(70.0, 16.0),
        &["unit=2", "designator=U1"],
    );
    assert!(refusal(again).contains("unit 2 of `U1` is already placed"));
    let out_of_range = place(&mut app, &lib, grid(70.0, 16.0), &["unit=3"]);
    assert!(refusal(out_of_range).contains("1 to 2"), "unit 3");
    // Another part's designator is not taken over.
    let other = run(
        &mut app,
        "place_component",
        &[
            format!("library={}", lib.id),
            "part=SNX-TWO-B".into(),
            format!("at={}", pt(grid(70.0, 16.0))),
            "unit=2".into(),
            "designator=U1".into(),
        ],
    );
    assert!(refusal(other).contains("is another part"));
    assert_eq!(symbols(&app).len(), 2);
}

#[test]
fn an_unknown_library_or_part_and_an_ambiguous_part_number_are_refused() {
    let lib = library();
    let mut app = app(&lib);
    let at = format!("at={}", pt(grid(8.0, 16.0)));
    let cases = [
        (
            format!("library={}", Uuid::now_v7()),
            "part=SNX-TWO",
            "is not open",
        ),
        (
            format!("library={}", lib.id),
            "part=NOPE",
            "has no part `NOPE`",
        ),
        (
            format!("library={}", lib.id),
            "part=TWO-1",
            "matches 2 rows",
        ),
    ];
    for (library, part, needle) in cases {
        let outcome = run(
            &mut app,
            "place_component",
            &[library, part.to_string(), at.clone()],
        );
        let reason = refusal(outcome);
        assert!(reason.contains(needle), "{part}: {reason}");
    }
    // A row id and the library's path name them too.
    let by_row = run(
        &mut app,
        "place_component",
        &[
            format!("library={}", lib.snxlib.display()),
            format!("part={}", lib.row),
            at,
        ],
    );
    assert_eq!(answered(by_row)["designator"], "U1");
}

#[test]
fn a_library_that_is_not_mounted_is_refused() {
    let lib = library();
    let (mut app, _task) = Signex::new();
    add_sheet_with(&mut app, &sheet_path(), SchematicSheet::default());
    let outcome = place(&mut app, &lib, Point::new(0.0, 0.0), &[]);
    assert!(refusal(outcome).contains("is not open"));
}

#[test]
fn an_off_grid_point_or_a_pin_reference_for_at_is_refused() {
    let lib = library();
    let mut app = app(&lib);
    let off = place(&mut app, &lib, Point::new(1.0, 1.0), &[]);
    assert!(refusal(off).contains("is not where a click lands"));
    answered(place(&mut app, &lib, grid(8.0, 16.0), &[]));
    let by_pin = run(
        &mut app,
        "place_component",
        &[
            format!("library={}", lib.id),
            "part=SNX-TWO".into(),
            "at=U1.1".into(),
        ],
    );
    assert!(refusal(by_pin).contains("not a pin reference"));
    assert_eq!(symbols(&app).len(), 1);
}

#[test]
fn a_pin_tip_landing_on_a_wire_end_is_refused_with_no_escape() {
    let lib = library();
    let at = grid(8.0, 16.0);
    // Pin 1's connection point is the symbol's origin.
    let tip = at;
    let sheet = SchematicSheet {
        wires: vec![Wire {
            uuid: Uuid::now_v7(),
            start: tip,
            end: Point::new(tip.x + 10.16, tip.y),
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut app = app_on(&lib, sheet);
    let reason = refusal(place(&mut app, &lib, at, &[]));
    assert!(reason.contains("would land on another wire"), "{reason}");
    assert!(symbols(&app).is_empty());
}

#[test]
fn one_undo_removes_a_part_placed_from_operands() {
    let lib = library();
    let mut app = app(&lib);
    let empty = sheet_shape(&app);
    answered(place(&mut app, &lib, grid(8.0, 16.0), &[]));
    let undo = AppCommandId::new("undo").expect("id");
    let _ = app.dispatch_command_reporting(&undo, CommandArgs::none(), None);
    assert_eq!(sheet_shape(&app), empty);
}

#[test]
fn a_part_and_a_wire_to_its_pin_are_one_edit_list() {
    let lib = library();
    let mut app = app(&lib);
    let path = sheet_path();
    let revision = app.document_state.engines[&path].revision();
    let at = grid(8.0, 16.0);
    let edits = json!([
        {"id": "place_component", "args": {"library": lib.id.to_string(), "part": "SNX-TWO", "at": pt(at)}},
        {"id": "place_wire", "args": {"start": "U1.1", "end": pt(grid(16.0, 16.0)), "allow_open": true}},
    ]);
    let outcome = run(
        &mut app,
        "apply_edits",
        &[
            format!("document={}", path.display()),
            format!("expect_revision={revision}"),
            format!("edits={edits}"),
        ],
    );
    let answer = answered(outcome);
    assert_eq!(answer["changed"], true, "{answer}");
    assert_eq!(answer["results"][0]["designator"], "U1");
    let sheet = app.document_state.engines[&path].document();
    assert_eq!((sheet.symbols.len(), sheet.wires.len()), (1, 1));
    assert_eq!(sheet.wires[0].start, at, "the wire starts on U1.1");
}
