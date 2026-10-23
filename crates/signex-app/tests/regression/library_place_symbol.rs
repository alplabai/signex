//! #364 — placing a `.snxlib` row puts a resolvable `LibSymbol` and a
//! `Symbol` on the active sheet, as one undo step.
//!
//! Driven through the real `LibraryMessage::PlaceLibraryComponent` path
//! against a real temp `.snxlib` holding a two-unit symbol: unit 1 owns
//! pin "1", unit 2 owns pin "2", and pin "3" is Part Zero (shared).

use signex_app::app::{Message, Signex, TabInfo, TabKind};
use signex_app::library::LibraryMessage;
use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::component::{ComponentRow, DatasheetRef, PlmReserved};
use signex_library::identity::{ComponentClass, InternalPn};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::lifecycle::LifecycleState;
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::manufacturer::ManufacturerPart;
use signex_library::param::ParamMap;
use signex_library::primitive::{PrimitiveRef, Symbol, SymbolFile, SymbolPin};
use signex_types::schematic::{NoConnect, SchematicSheet, SymbolTransform};
use std::path::PathBuf;
use uuid::Uuid;

struct Fixture {
    _tmp: tempfile::TempDir,
    snxlib: PathBuf,
    row_id: Uuid,
}

fn two_unit_symbol() -> Symbol {
    let mut s = Symbol::empty("TWO-UNIT");
    s.designator = "U?".into();
    s.part_count = 2;
    for (number, part, y) in [("1", 1u8, 0.0), ("2", 2, 5.08), ("3", 0, 10.16)] {
        let mut p = SymbolPin::new(number, format!("P{number}"));
        p.position = [0.0, y];
        p.part_number = part;
        s.pins.push(p);
    }
    s
}

fn fixture() -> Fixture {
    signex_library::git_identity::test_identity::install();
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().join("lib");
    let snxlib = dir.join("lib.snxlib");
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
    let adapter = LocalGitAdapter::init(
        &snxlib,
        manifest,
        LibraryInitOptions {
            enable_git: false,
            use_lfs: false,
        },
    )
    .expect("init");
    let library_id = adapter.library_id();

    let symbol = two_unit_symbol();
    let sym_uuid = symbol.uuid;
    std::fs::create_dir_all(dir.join("symbols")).expect("symbols dir");
    std::fs::write(
        dir.join("symbols").join("two-unit.snxsym"),
        SymbolFile::from_symbol(symbol)
            .to_toml_string()
            .expect("emit"),
    )
    .expect("write symbol");

    let now = chrono::Utc::now();
    let row_id = Uuid::now_v7();
    let row = ComponentRow {
        row_id,
        internal_pn: InternalPn::new("SNX-TWO"),
        class: ComponentClass::new("IC"),
        datasheet: DatasheetRef::default(),
        state: LifecycleState::Draft,
        symbol_ref: PrimitiveRef::new(library_id, sym_uuid),
        footprint_ref: None,
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Acme", "TWO-1"),
        alternates: Vec::new(),
        supply: Vec::new(),
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        version: "1.2.3".into(),
        released: false,
        symbol_version: String::new(),
        footprint_version: String::new(),
        sim_version: String::new(),
        created: now,
        updated: now,
        content_hash: [0u8; 32],
    };
    adapter.insert_row("ICs", row, "seed").expect("insert row");
    drop(adapter);
    drop(LocalGitAdapter::recover_init(&snxlib).expect("recover_init"));
    Fixture {
        _tmp: tmp,
        snxlib,
        row_id,
    }
}

/// An app with an empty active sheet and the fixture library mounted.
fn app_with_library(f: &Fixture) -> Signex {
    let path = PathBuf::from("place-364.snxsch");
    let sheet = SchematicSheet {
        uuid: Uuid::new_v4(),
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    let (mut app, _t) = Signex::new();
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet).expect("engine"),
    );
    app.document_state.tabs.push(TabInfo {
        title: "place-364".to_string(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app.library
        .open_library(f.snxlib.clone())
        .expect("mount library");
    app
}

fn place(app: &mut Signex, f: &Fixture) {
    let _ = app.update(Message::Library(LibraryMessage::PlaceLibraryComponent {
        library_path: f.snxlib.clone(),
        table: "ICs".into(),
        row_id: signex_library::RowId(f.row_id),
    }));
}

fn doc(app: &Signex) -> &SchematicSheet {
    app.document_state
        .active_engine()
        .expect("engine")
        .document()
}

/// Terminal pin ids the netlist reports for unit `unit` of the first
/// placed symbol, with a no-connect on every pin tip so each is "connected".
fn terminal_pins(app: &Signex, unit: u32) -> Vec<String> {
    let mut sheet = doc(app).clone();
    sheet.symbols[0].unit = unit;
    let xform = SymbolTransform::from_symbol(&sheet.symbols[0]);
    let lib = sheet
        .lib_symbols
        .get(&sheet.symbols[0].lib_id)
        .expect("lib");
    let tips: Vec<_> = lib
        .pins
        .iter()
        .map(|p| xform.apply(p.pin.position))
        .collect();
    for position in tips {
        sheet.no_connects.push(NoConnect {
            uuid: Uuid::new_v4(),
            position,
        });
    }
    let mut pins: Vec<String> = signex_net::build_netlist(&sheet)
        .nets
        .iter()
        .flat_map(|n| n.terminals.iter().map(|t| t.pin.clone()))
        .collect();
    pins.sort();
    pins
}

#[test]
fn placing_registers_one_lib_symbol_that_the_symbol_resolves() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);

    let sheet = doc(&app);
    assert_eq!(sheet.symbols.len(), 1);
    assert_eq!(sheet.lib_symbols.len(), 1);
    let placed = &sheet.symbols[0];
    let lib = sheet
        .lib_symbols
        .get(&placed.lib_id)
        .expect("lib_id resolves");
    assert_eq!(lib.id, placed.lib_id);
    assert_eq!(placed.unit, 1);
    assert_eq!(placed.reference, "U1");
    assert_eq!(placed.row_id, Some(f.row_id));
    assert!(placed.library_id.is_some());
    assert_eq!(placed.library_version, "1.2.3");
    assert!(app.document_state.error_notice.is_none());
}

#[test]
fn each_unit_gets_its_own_pins_plus_part_zero() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);

    assert_eq!(terminal_pins(&app, 1), ["1", "3"]);
    assert_eq!(terminal_pins(&app, 2), ["2", "3"]);
}

#[test]
fn placing_twice_makes_two_symbols_and_one_lib_symbol() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    place(&mut app, &f);

    let sheet = doc(&app);
    assert_eq!(sheet.symbols.len(), 2);
    assert_eq!(sheet.lib_symbols.len(), 1);
    assert_eq!(sheet.symbols[0].lib_id, sheet.symbols[1].lib_id);
    assert_ne!(sheet.symbols[0].reference, sheet.symbols[1].reference);
}

#[test]
fn one_undo_removes_the_placement_and_its_new_lib_symbol() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    place(&mut app, &f);

    let engine = app.document_state.active_engine_mut().expect("engine");
    engine.undo().expect("undo");
    // Second placement reused the entry: undoing it keeps the first part.
    assert_eq!(engine.document().symbols.len(), 1);
    assert_eq!(engine.document().lib_symbols.len(), 1);

    engine.undo().expect("undo");
    assert!(engine.document().symbols.is_empty());
    assert!(engine.document().lib_symbols.is_empty());
}

#[test]
fn failures_reach_the_user_and_leave_the_sheet_alone() {
    let f = fixture();
    let mut app = app_with_library(&f);

    // Unknown row.
    let _ = app.update(Message::Library(LibraryMessage::PlaceLibraryComponent {
        library_path: f.snxlib.clone(),
        table: "ICs".into(),
        row_id: signex_library::RowId(Uuid::now_v7()),
    }));
    assert!(app.document_state.error_notice.is_some(), "bad row");
    app.document_state.error_notice = None;

    // Library that is not open.
    let _ = app.update(Message::Library(LibraryMessage::PlaceLibraryComponent {
        library_path: PathBuf::from("nowhere.snxlib"),
        table: "ICs".into(),
        row_id: signex_library::RowId(f.row_id),
    }));
    assert!(app.document_state.error_notice.is_some(), "closed library");
    app.document_state.error_notice = None;

    // No active sheet.
    app.document_state.clear_active_engine();
    place(&mut app, &f);
    assert!(app.document_state.error_notice.is_some(), "no sheet");
    assert!(app.library.picker.is_none());
}
