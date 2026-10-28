//! #364 — placing a `.snxlib` row puts a resolvable `LibSymbol` and a
//! `Symbol` on the active sheet, as one undo step.
//!
//! Driven through the real `LibraryMessage::PlaceLibraryComponent` path
//! against a real temp `.snxlib` holding a two-unit symbol: unit 1 owns
//! pin "1", unit 2 owns pin "2", and pin "3" is Part Zero (shared).

use futures::StreamExt;
use iced::Task;
use signex_app::app::{EditMsg, Message, Signex, TabInfo, TabKind};
use signex_app::library::LibraryMessage;
use signex_app::library::messages::ResolvedPlacement;
use signex_app::library::place::resolve_placement;
use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::component::{ComponentRow, DatasheetRef, PlmReserved};
use signex_library::identity::{ComponentClass, InternalPn};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::lifecycle::LifecycleState;
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig, WorkflowMode};
use signex_library::manufacturer::ManufacturerPart;
use signex_library::param::ParamMap;
use signex_library::primitive::{
    PinDirection, PrimitiveRef, Symbol, SymbolFile, SymbolGraphic, SymbolGraphicKind, SymbolPin,
};
use signex_types::schematic::{NoConnect, Point, SchematicSheet, SymbolTransform, Wire};
use std::path::PathBuf;
use uuid::Uuid;

pub(crate) struct Fixture {
    _tmp: tempfile::TempDir,
    pub(crate) snxlib: PathBuf,
    pub(crate) library_id: Uuid,
    pub(crate) symbol_id: Uuid,
    pub(crate) row_id: Uuid,
}

pub(crate) fn two_unit_symbol() -> Symbol {
    let mut s = Symbol::empty("TWO-UNIT");
    s.designator = "U?".into();
    s.part_count = 2;
    for (number, part, y) in [("1", 1u8, 0.0), ("2", 2, 5.08), ("3", 0, 10.16)] {
        let mut p = SymbolPin::new(number, format!("P{number}"));
        p.position = [0.0, y];
        p.part_number = part;
        s.pins.push(p);
    }
    for part in [1u8, 2] {
        s.graphics.push(SymbolGraphic {
            kind: SymbolGraphicKind::Line {
                from: [0.0, 0.0],
                to: [f64::from(part), 0.0],
            },
            stroke_width: 0.1,
            fill: None,
            part_number: part,
        });
    }
    s
}

/// A library with a two-unit symbol and one row bound to it.
pub(crate) fn fixture() -> Fixture {
    build("lib", None)
}

/// Row in library A whose symbol lives in library B.
pub(crate) fn cross_library() -> (Fixture, Fixture) {
    let b = build("libb", None);
    let a = build("liba", Some((b.library_id, b.symbol_id)));
    (a, b)
}

/// `symbol_ref` overrides where the row's symbol lives; every library also
/// carries its own copy of the symbol.
fn build(name: &str, symbol_ref: Option<(Uuid, Uuid)>) -> Fixture {
    build_with(name, symbol_ref, two_unit_symbol())
}

fn build_with(name: &str, symbol_ref: Option<(Uuid, Uuid)>, symbol: Symbol) -> Fixture {
    build_with_class(name, symbol_ref, symbol, "IC")
}

/// A library whose single row has component class `class` and a `D?` symbol.
pub(crate) fn fixture_of_class(class: &str) -> Fixture {
    let mut symbol = two_unit_symbol();
    symbol.designator = "D?".into();
    build_with_class("lib", None, symbol, class)
}

fn build_with_class(
    name: &str,
    symbol_ref: Option<(Uuid, Uuid)>,
    symbol: Symbol,
    class: &str,
) -> Fixture {
    build_in_mode(name, symbol_ref, symbol, class, WorkflowMode::Personal)
}

/// A library named `name` whose manifest runs the Team workflow.
pub(crate) fn team_fixture(name: &str) -> Fixture {
    build_in_mode(name, None, two_unit_symbol(), "IC", WorkflowMode::Team)
}

fn build_in_mode(
    name: &str,
    symbol_ref: Option<(Uuid, Uuid)>,
    symbol: Symbol,
    class: &str,
    mode: WorkflowMode,
) -> Fixture {
    signex_library::git_identity::test_identity::install();
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().join(name);
    let snxlib = dir.join(format!("{name}.snxlib"));
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: Uuid::now_v7(),
        library: LibrarySection {
            name: name.into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig {
            mode,
            ..WorkflowConfig::default()
        },
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
        alternate_symbols: Vec::new(),
        row_id,
        internal_pn: InternalPn::new("SNX-TWO"),
        class: ComponentClass::new(class),
        datasheet: DatasheetRef::default(),
        datasheet_label: String::new(),
        more_datasheets: Vec::new(),
        state: LifecycleState::Draft,
        symbol_ref: match symbol_ref {
            Some((lib, sym)) => PrimitiveRef::new(lib, sym),
            None => PrimitiveRef::new(library_id, sym_uuid),
        },
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
        server_revision: None,
    };
    adapter.insert_row("ICs", row, "seed").expect("insert row");
    drop(adapter);
    drop(LocalGitAdapter::recover_init(&snxlib).expect("recover_init"));
    Fixture {
        _tmp: tmp,
        snxlib,
        library_id,
        symbol_id: sym_uuid,
        row_id,
    }
}

/// An app with an empty active sheet and the fixture library mounted.
pub(crate) fn app_with_library(f: &Fixture) -> Signex {
    app_with_libraries(&[f])
}

pub(crate) fn app_with_libraries(libs: &[&Fixture]) -> Signex {
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
    for f in libs {
        app.library
            .open_library(f.snxlib.clone())
            .expect("mount library");
    }
    app
}

/// The worker half, run directly (the `Task` wraps it in `spawn_blocking`).
fn resolve(f: &Fixture, row: Uuid) -> Result<Box<ResolvedPlacement>, String> {
    resolve_with(f, row, &[f])
}

fn resolve_with(
    f: &Fixture,
    row: Uuid,
    open: &[&Fixture],
) -> Result<Box<ResolvedPlacement>, String> {
    let open: Vec<_> = open
        .iter()
        .map(|l| (l.library_id, l.snxlib.clone()))
        .collect();
    resolve_placement(&f.snxlib, "ICs", signex_library::RowId(row), &open).map(Box::new)
}

/// Feed a worker result back as the completion message, targeting `sheet`.
fn complete(
    app: &mut Signex,
    f: &Fixture,
    sheet: &str,
    result: Result<Box<ResolvedPlacement>, String>,
) {
    let _ = app.update(Message::Library(LibraryMessage::LibraryComponentResolved {
        target_sheet: PathBuf::from(sheet),
        library_path: f.snxlib.clone(),
        result,
    }));
}

/// Place `a`'s row, whose symbol lives in `b`, on the active sheet.
pub(crate) fn place_across(app: &mut Signex, a: &Fixture, b: &Fixture) {
    let resolved = resolve_with(a, a.row_id, &[a, b]).expect("resolves across libraries");
    complete(app, a, "place-364.snxsch", Ok(resolved));
}

/// Worker + completion against the sheet `app_with_library` made active.
pub(crate) fn place(app: &mut Signex, f: &Fixture) {
    complete(app, f, "place-364.snxsch", resolve(f, f.row_id));
}

pub(crate) fn doc(app: &Signex) -> &SchematicSheet {
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

    let _ = app.update(Message::Edit(EditMsg::Undo));
    // Second placement reused the entry: undoing it keeps the first part.
    assert_eq!(doc(&app).symbols.len(), 1);
    assert_eq!(doc(&app).lib_symbols.len(), 1);

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert!(doc(&app).symbols.is_empty());
    assert!(doc(&app).lib_symbols.is_empty());
    assert!(
        app.document_state.tabs[0].dirty,
        "undo leaves the tab dirty"
    );
}

/// Run a task to completion the way the iced runtime would.
pub(crate) fn run(task: Task<Message>) -> Vec<Message> {
    let Some(mut stream) = iced_runtime::task::into_stream(task) else {
        return Vec::new();
    };
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let mut out = Vec::new();
        while let Some(action) = stream.next().await {
            if let iced_runtime::Action::Output(message) = action {
                out.push(message);
            }
        }
        out
    })
}

#[test]
fn the_picker_message_places_end_to_end_with_no_io_in_update() {
    let f = fixture();
    let mut app = app_with_library(&f);
    // Only the placement's own messages are expected below.
    crate::settle_support::settle_history_panel(&mut app);
    let task = app.update(Message::Library(LibraryMessage::PlaceLibraryComponent {
        library_path: f.snxlib.clone(),
        table: "ICs".into(),
        row_id: signex_library::RowId(f.row_id),
    }));

    // Nothing was read or placed synchronously: the work is in the Task.
    assert!(doc(&app).symbols.is_empty());
    assert!(doc(&app).lib_symbols.is_empty());
    assert!(app.document_state.error_notice.is_none());

    for message in run(task) {
        assert!(matches!(
            message,
            Message::Library(LibraryMessage::LibraryComponentResolved { .. })
        ));
        let _ = app.update(message);
    }
    assert_eq!(doc(&app).symbols.len(), 1);
    assert_eq!(doc(&app).lib_symbols.len(), 1);
    assert_eq!(doc(&app).symbols[0].row_id, Some(f.row_id));
    assert!(app.document_state.error_notice.is_none());
}

#[test]
fn a_repeated_drop_is_offset_from_the_part_already_there() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    place(&mut app, &f);
    place(&mut app, &f);

    let s = &doc(&app).symbols;
    assert_ne!(s[0].position, s[1].position);
    assert_ne!(s[1].position, s[2].position);
    assert_ne!(s[0].position, s[2].position);
}

#[test]
fn replacing_after_the_library_symbol_changed_warns_and_keeps_the_sheet_copy() {
    let f = fixture();
    let mut app = app_with_library(&f);
    let first = resolve(&f, f.row_id).expect("resolves");
    let mut edited = first.clone();
    edited.lib_symbol.pins.clear();
    complete(&mut app, &f, "place-364.snxsch", Ok(first.clone()));
    let kept = doc(&app).lib_symbols.values().next().unwrap().pins.len();
    assert!(kept > 0);

    // Identical content: no warning.
    assert!(!warned(&mut app, &f, first));
    // Edited content: warning, sheet still holds the original copy.
    assert!(warned(&mut app, &f, edited));
    assert_eq!(
        doc(&app).lib_symbols.values().next().unwrap().pins.len(),
        kept
    );
}

/// Place `resolved` and report whether the drift warning was logged.
fn warned(app: &mut Signex, f: &Fixture, resolved: Box<ResolvedPlacement>) -> bool {
    let _ = signex_app::diagnostics::init_logging();
    let before = signex_app::diagnostics::newest_entry_id().unwrap_or(0);
    complete(app, f, "place-364.snxsch", Ok(resolved));
    signex_app::diagnostics::recent_entries().iter().any(|e| {
        e.id > before
            && e.message.contains("keeps its existing copy")
            && e.message.contains("Update from library")
    })
}

#[test]
fn worker_failures_reach_the_user_and_leave_the_sheet_alone() {
    let f = fixture();
    let mut app = app_with_library(&f);

    let bad_row = resolve(&f, Uuid::now_v7());
    assert!(bad_row.is_err());
    complete(&mut app, &f, "place-364.snxsch", bad_row);
    assert_eq!(
        app.document_state
            .error_notice
            .as_ref()
            .map(|n| n.title.as_str()),
        Some("Place Failed")
    );
    assert!(doc(&app).symbols.is_empty());
    app.document_state.error_notice = None;

    let missing = resolve_placement(
        &PathBuf::from("nowhere.snxlib"),
        "ICs",
        signex_library::RowId(f.row_id),
        &[],
    );
    assert!(missing.is_err());
}

#[test]
fn picker_refuses_a_closed_library_or_missing_sheet_up_front() {
    let f = fixture();
    let mut app = app_with_library(&f);
    let _ = app.update(Message::Library(LibraryMessage::PlaceLibraryComponent {
        library_path: PathBuf::from("nowhere.snxlib"),
        table: "ICs".into(),
        row_id: signex_library::RowId(f.row_id),
    }));
    assert!(app.document_state.error_notice.is_some(), "closed library");
    app.document_state.error_notice = None;

    app.document_state.clear_active_engine();
    let _ = app.update(Message::Library(LibraryMessage::PlaceLibraryComponent {
        library_path: f.snxlib.clone(),
        table: "ICs".into(),
        row_id: signex_library::RowId(f.row_id),
    }));
    assert!(app.document_state.error_notice.is_some(), "no sheet");
    assert!(app.library.picker.is_none());
}

#[test]
fn a_changed_sheet_or_closed_library_is_reported_not_placed() {
    let f = fixture();
    let mut app = app_with_library(&f);
    let resolved = resolve(&f, f.row_id);

    // The sheet the gesture started on is not the active one any more.
    complete(&mut app, &f, "other-sheet.snxsch", resolved.clone());
    assert!(app.document_state.error_notice.is_some(), "wrong sheet");
    assert!(doc(&app).symbols.is_empty());
    app.document_state.error_notice = None;

    // The library was unmounted while the worker ran.
    app.library.open_libraries.clear();
    complete(&mut app, &f, "place-364.snxsch", resolved);
    assert!(app.document_state.error_notice.is_some(), "closed library");
    assert!(doc(&app).symbols.is_empty());
}

#[test]
fn a_symbol_from_another_open_library_is_placed_keyed_on_its_library() {
    let (a, b) = cross_library();
    let mut app = app_with_libraries(&[&a, &b]);
    let resolved = resolve_with(&a, a.row_id, &[&a, &b]).expect("resolves across libraries");
    complete(&mut app, &a, "place-364.snxsch", Ok(resolved));

    let sheet = doc(&app);
    assert_eq!(sheet.symbols.len(), 1);
    let placed = &sheet.symbols[0];
    assert_eq!(placed.lib_id, format!("{}:{}", b.library_id, b.symbol_id));
    assert!(sheet.lib_symbols.contains_key(&placed.lib_id));
    // Identity slots name the row's own library.
    assert_eq!(placed.library_id, Some(a.library_id));
    assert_eq!(placed.row_id, Some(a.row_id));
}

#[test]
fn a_symbol_whose_library_is_not_open_says_so() {
    let (a, b) = cross_library();
    let err = resolve_with(&a, a.row_id, &[&a]).expect_err("B is not open");
    assert!(err.contains(&b.library_id.to_string()), "{err}");
    assert!(err.contains("not open"), "{err}");
}

#[test]
fn the_placement_survives_a_save_and_reload() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);

    let text = signex_types::format::SnxSchematic::new(doc(&app).clone())
        .write_string()
        .expect("write");
    let reloaded = signex_types::format::SnxSchematic::parse(&text)
        .expect("parse")
        .sheet;

    let key = format!("{}:{}", f.library_id, f.symbol_id);
    assert_eq!(reloaded.symbols[0].lib_id, key);
    let lib = reloaded.lib_symbols.get(&key).expect("entry survives");
    assert_eq!(lib.id, key);
    assert!(lib.pins.iter().any(|p| p.unit == 0), "Part Zero pin kept");
}

#[test]
fn each_unit_selects_its_own_body() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);

    let lib = doc(&app).lib_symbols.values().next().expect("lib symbol");
    let body_for = |unit: u32| -> Vec<u32> {
        lib.graphics
            .iter()
            .filter(|g| g.unit == 0 || g.unit == unit)
            .map(|g| g.unit)
            .collect()
    };
    assert_eq!(body_for(1), [1]);
    assert_eq!(body_for(2), [2]);
}

/// Stock-style power port: one `Power` pin named `GND`, `#PWR?`, comment `GND`.
fn gnd_symbol() -> Symbol {
    let mut s = Symbol::empty("GND");
    s.designator = "#PWR?".into();
    s.comment = "GND".into();
    let mut p = SymbolPin::new("1", "GND");
    p.electrical = PinDirection::Power;
    s.pins.push(p);
    s
}

#[test]
fn a_library_power_port_places_as_a_power_port() {
    let f = build_with("gnd", None, gnd_symbol());
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    place(&mut app, &f);
    assert!(app.document_state.error_notice.is_none());

    let mut sheet = doc(&app).clone();
    let (a, b) = (&sheet.symbols[0], &sheet.symbols[1]);
    assert!(a.is_power && b.is_power);
    assert_eq!(a.value, "GND");
    assert_eq!(a.reference, "#PWR1");
    assert_eq!(b.reference, "#PWR2");

    // A wire from the pin tip makes a net the port names.
    let lib = sheet.lib_symbols.get(&a.lib_id).expect("lib");
    let tip = SymbolTransform::from_symbol(a).apply(lib.pins[0].pin.position);
    sheet.wires.push(Wire {
        uuid: Uuid::new_v4(),
        start: tip,
        end: Point::new(tip.x + 10.16, tip.y),
        ..Default::default()
    });
    let nets = signex_net::build_netlist(&sheet).nets;
    assert!(nets.iter().any(|n| n.name == "GND"), "{nets:?}");

    let ctx = signex_output::ExportContext {
        sheets: vec![signex_output::SheetSnapshot {
            path: "gnd.snxsch".into(),
            schematic: sheet,
            sheet_name: "Main".into(),
            sheet_number: 1,
            sheet_count: 1,
            placement: Default::default(),
        }],
        metadata: Default::default(),
        netlist: None,
        netlist_sheets: Vec::new(),
        current_sheet: None,
        netlist_placements: Vec::new(),
    };
    let table = signex_output::bom::rollup(&ctx, &Default::default());
    assert!(table.rows.is_empty(), "power ports stay out of the BOM");
}

#[test]
fn an_ordinary_part_is_not_a_power_port() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    assert!(!doc(&app).symbols[0].is_power);
}

/// A power flag: `#FLG?` designator, one pin that drives the rail.
fn flag_symbol() -> Symbol {
    let mut s = Symbol::empty("PWR_FLAG");
    s.designator = "#FLG?".into();
    s.comment = "PWR_FLAG".into();
    let mut p = SymbolPin::new("1", "PWR_FLAG");
    p.electrical = PinDirection::Power;
    s.pins.push(p);
    s
}

/// Place `f` twice, shift the second copy down, and give each a wire stub
/// off its pin tip; returns the sheet with two separate wired symbols.
fn two_wired_copies(f: &Fixture) -> SchematicSheet {
    let mut app = app_with_library(f);
    place(&mut app, f);
    place(&mut app, f);
    let mut sheet = doc(&app).clone();
    sheet.symbols[1].position.y += 50.8;
    for i in 0..2 {
        let sym = sheet.symbols[i].clone();
        let lib = sheet.lib_symbols.get(&sym.lib_id).expect("lib");
        let tip = SymbolTransform::from_symbol(&sym).apply(lib.pins[0].pin.position);
        sheet.wires.push(Wire {
            uuid: Uuid::new_v4(),
            start: tip,
            end: Point::new(tip.x + 10.16, tip.y),
            ..Default::default()
        });
    }
    sheet
}

#[test]
fn a_flag_is_power_but_never_names_its_net() {
    let f = build_with("flag", None, flag_symbol());
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let flag = &doc(&app).symbols[0];
    assert!(flag.is_power, "ERC still sees the flag");
    assert!(flag.value.is_empty(), "a flag carries no net name");
    assert_eq!(flag.reference, "#FLG1");
}

#[test]
fn two_flags_on_different_rails_do_not_merge_them() {
    let f = build_with("flag2", None, flag_symbol());
    let nets = signex_net::build_netlist(&two_wired_copies(&f)).nets;
    assert!(
        nets.iter().all(|n| n.name != "PWR_FLAG"),
        "flags must not name nets: {nets:?}"
    );
    let flag_nets = nets.iter().filter(|n| !n.terminals.is_empty()).count();
    assert_eq!(flag_nets, 2, "{nets:?}");
}

#[test]
fn a_hash_designator_without_a_power_pin_is_not_a_power_port() {
    let mut s = Symbol::empty("VIRT");
    s.designator = "#PWR?".into();
    s.comment = "VIRT".into();
    s.pins.push(SymbolPin::new("1", "VIRT"));
    let f = build_with("virt", None, s);
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    assert!(!doc(&app).symbols[0].is_power);
}

#[test]
fn an_empty_or_star_comment_falls_back_to_the_pin_name() {
    for comment in ["", "*", "  "] {
        let mut s = gnd_symbol();
        s.comment = comment.into();
        let f = build_with("fallback", None, s);
        assert_eq!(resolve(&f, f.row_id).expect("resolve").value, "GND");
    }
}

#[test]
fn two_same_name_ports_join_their_nets() {
    let f = build_with("gnd2", None, gnd_symbol());
    let nets = signex_net::build_netlist(&two_wired_copies(&f)).nets;
    let gnd: Vec<_> = nets.iter().filter(|n| n.name == "GND").collect();
    assert_eq!(gnd.len(), 1, "both wires are one GND net: {nets:?}");
    assert!(gnd[0].terminals.len() >= 2, "{nets:?}");
}
