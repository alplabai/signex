//! #832 — saving a symbol in the editor bumps the component rows that use
//! it. Only `LocalGitAdapter::save_*` ran the cascade, and the editor does
//! not save through that, so a row kept the old `symbol_version` while the
//! symbol's version moved on and placed instances missed the change.
//!
//! In Personal mode a save also patch-bumps the primitives it changed, in
//! the same write, and then moves the rows onto that version.

use signex_app::app::{
    FileMsg, FootprintEditorState, Message, Signex, SymbolEditorState, TabInfo, TabKind,
};
use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::component::{ComponentRow, DatasheetRef, PlmReserved};
use signex_library::identity::{ComponentClass, InternalPn};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::lifecycle::LifecycleState;
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig, WorkflowMode};
use signex_library::manufacturer::ManufacturerPart;
use signex_library::param::ParamMap;
use signex_library::primitive::PrimitiveRef;
use signex_library::{Footprint, FootprintFile, Symbol, SymbolFile};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// How a test's library is set up.
#[derive(Clone, Copy)]
struct Setup {
    mode: WorkflowMode,
    /// Mounted in the app (`open_library`), or only on disk.
    mounted: bool,
    /// Name of symbol `A`; the Messages ring buffer is process-global, so
    /// a test that reads it uses a name no other test does.
    name: &'static str,
}

const PERSONAL: Setup = Setup {
    mode: WorkflowMode::Personal,
    mounted: true,
    name: "A",
};

struct Fixture {
    app: Signex,
    _tmp: tempfile::TempDir,
    library_file: PathBuf,
    symbol_path: PathBuf,
    users: Uuid,
    bystander: Uuid,
    footprint_path: PathBuf,
    footprint_row: Uuid,
}

fn row(pn: &str, library_id: Uuid, symbol: Uuid) -> ComponentRow {
    let now = chrono::Utc::now();
    ComponentRow {
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new(pn),
        class: ComponentClass::new("resistor"),
        datasheet: DatasheetRef::default(),
        state: LifecycleState::Draft,
        symbol_ref: PrimitiveRef::new(library_id, symbol),
        footprint_ref: None,
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Acme", pn),
        alternates: Vec::new(),
        supply: Vec::new(),
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        version: "0.0.1".into(),
        released: false,
        symbol_version: "0.0.1".into(),
        footprint_version: String::new(),
        sim_version: String::new(),
        created: now,
        updated: now,
        content_hash: [0u8; 32],
    }
}

/// A mounted library with a symbol `A` (used by row `R-USES`) and a
/// symbol `B` (used by row `R-OTHER`), and an editor open on the file.
fn fixture() -> Fixture {
    fixture_with(PERSONAL)
}

fn fixture_with(setup: Setup) -> Fixture {
    signex_library::git_identity::test_identity::install();
    let _ = signex_app::diagnostics::init_logging();
    let tmp = tempfile::tempdir().unwrap();
    let library_file = tmp.path().join("Lib").join("Lib.snxlib");
    let library_id = Uuid::now_v7();
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id,
        library: LibrarySection {
            name: "Lib".into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig {
            mode: setup.mode,
            ..WorkflowConfig::default()
        },
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    let seeding =
        LocalGitAdapter::init(&library_file, manifest, LibraryInitOptions::default()).unwrap();

    let symbol_path = library_file
        .parent()
        .unwrap()
        .join("symbols")
        .join("pair.snxsym");
    std::fs::create_dir_all(symbol_path.parent().unwrap()).unwrap();
    let mut file = SymbolFile::from_symbol(Symbol::empty(setup.name));
    file.symbols.push(Symbol::empty("B"));
    std::fs::write(&symbol_path, file.to_toml_string().unwrap()).unwrap();
    let (a, b) = (file.symbols[0].uuid, file.symbols[1].uuid);

    let (mut app, _t) = Signex::new();
    if setup.mounted {
        app.library.open_library(library_file.clone()).unwrap();
    }
    let adapter: &dyn LibraryAdapter = match setup.mounted {
        true => app.library.set.get(library_id).unwrap(),
        false => &seeding,
    };
    let uses = row("R-USES", library_id, a);
    let other = row("R-OTHER", library_id, b);
    let (users, bystander) = (uses.row_id, other.row_id);
    adapter.insert_row("R", uses, "seed").unwrap();
    adapter.insert_row("R", other, "seed").unwrap();

    let footprint_path = library_file
        .parent()
        .unwrap()
        .join("footprints")
        .join("pair.snxfpt");
    std::fs::create_dir_all(footprint_path.parent().unwrap()).unwrap();
    let fp_file = FootprintFile::from_footprint(Footprint::empty("F"));
    std::fs::write(&footprint_path, fp_file.to_toml_string().unwrap()).unwrap();
    let mut fp_row = row("R-FP", library_id, Uuid::now_v7());
    fp_row.footprint_ref = Some(PrimitiveRef::new(library_id, fp_file.footprints[0].uuid));
    fp_row.footprint_version = "0.0.1".into();
    let footprint_row = fp_row.row_id;
    adapter.insert_row("R", fp_row, "seed").unwrap();
    app.document_state.footprint_editors.insert(
        footprint_path.clone(),
        FootprintEditorState::new(footprint_path.clone(), fp_file),
    );

    app.document_state.symbol_editors.insert(
        symbol_path.clone(),
        SymbolEditorState::new(symbol_path.clone(), file),
    );
    app.document_state.tabs.push(TabInfo {
        title: "pair".into(),
        path: symbol_path.clone(),
        cached_document: None,
        dirty: true,
        project_id: None,
        kind: TabKind::SymbolEditor(symbol_path.clone()),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    Fixture {
        app,
        _tmp: tmp,
        library_file,
        symbol_path,
        users,
        bystander,
        footprint_path,
        footprint_row,
    }
}

/// Edit symbol `A`.
fn edit_a(f: &mut Fixture) {
    let editor = f
        .app
        .document_state
        .symbol_editors
        .get_mut(&f.symbol_path)
        .unwrap();
    editor.file.symbols[0].description = "edited".into();
    editor.dirty = true;
}

fn save(app: &mut Signex) {
    crate::settle_support::settle(app, Message::File(FileMsg::Save));
}

/// The row as a fresh process reads it from disk.
fn row_on_disk(library_file: &Path, id: Uuid) -> ComponentRow {
    LocalGitAdapter::open(library_file)
        .unwrap()
        .read_table("R")
        .unwrap()
        .into_iter()
        .find(|r| r.row_id == id)
        .unwrap()
}

#[test]
fn saving_a_symbol_bumps_the_component_row_that_uses_it() {
    let mut f = fixture();
    edit_a(&mut f);
    save(&mut f.app);

    let bumped = row_on_disk(&f.library_file, f.users);
    assert_eq!(bumped.symbol_version, "0.0.2");
    assert_eq!(bumped.version, "0.0.2");
}

#[test]
fn a_row_that_does_not_use_the_saved_symbol_is_untouched() {
    let mut f = fixture();
    edit_a(&mut f);
    save(&mut f.app);

    let other = row_on_disk(&f.library_file, f.bystander);
    assert_eq!(other.symbol_version, "0.0.1");
    assert_eq!(other.version, "0.0.1");
}

#[test]
fn a_refused_save_does_not_bump_any_row() {
    let mut f = fixture();
    // A row pinned behind the symbol: a cascade would move it.
    let library = LocalGitAdapter::open(&f.library_file).unwrap();
    let mut behind = row_on_disk(&f.library_file, f.users);
    behind.symbol_version = "0.0.0".into();
    library.update_row("R", behind, "behind").unwrap();
    edit_a(&mut f);
    // Another program broke the file since it was opened: the save is
    // refused rather than overwriting it.
    std::fs::write(&f.symbol_path, "not a symbol library").unwrap();
    save(&mut f.app);

    let untouched = row_on_disk(&f.library_file, f.users);
    assert_eq!(untouched.symbol_version, "0.0.0");
    assert_eq!(untouched.version, "0.0.1");
}

#[test]
fn the_messages_panel_reports_the_bumped_rows() {
    let mut f = fixture_with(Setup {
        name: "MSG-BUMPED-SYM",
        ..PERSONAL
    });
    edit_a(&mut f);
    save(&mut f.app);

    assert!(
        logged("Bumped 1 component row that uses MSG-BUMPED-SYM to v0.0.2"),
        "the Messages panel must say how many rows the save bumped"
    );
}

fn logged(line: &str) -> bool {
    signex_app::diagnostics::recent_entries()
        .iter()
        .any(|e| e.message.contains(line))
}

#[test]
fn saving_a_footprint_bumps_the_component_row_that_uses_it() {
    let mut f = fixture();
    let path = f.footprint_path.clone();
    let editor = f
        .app
        .document_state
        .footprint_editors
        .get_mut(&path)
        .unwrap();
    editor.file.footprints[0].description = "edited".into();
    editor.dirty = true;
    f.app.document_state.tabs.push(TabInfo {
        title: "fp".into(),
        path: path.clone(),
        cached_document: None,
        dirty: true,
        project_id: None,
        kind: TabKind::FootprintEditor(path),
    });
    f.app.document_state.active_tab = f.app.document_state.tabs.len() - 1;
    save(&mut f.app);

    let bumped = row_on_disk(&f.library_file, f.footprint_row);
    assert_eq!(bumped.footprint_version, "0.0.2");
    assert_eq!(bumped.version, "0.0.2");
}

fn symbol_on_disk(f: &Fixture) -> SymbolFile {
    SymbolFile::from_toml_str(&std::fs::read_to_string(&f.symbol_path).unwrap()).unwrap()
}

#[test]
fn a_personal_mode_save_patch_bumps_the_symbol_itself() {
    let mut f = fixture();
    edit_a(&mut f);
    save(&mut f.app);

    let file = symbol_on_disk(&f);
    assert_eq!(file.symbols[0].version, "0.0.2", "the edited symbol");
    assert_eq!(file.symbols[1].version, "0.0.1", "B was not edited");
}

#[test]
fn a_second_save_without_edits_bumps_nothing_and_the_next_edit_bumps_once_more() {
    let mut f = fixture_with(Setup {
        name: "MSG-TWICE-SYM",
        ..PERSONAL
    });
    edit_a(&mut f);
    save(&mut f.app);
    // Ctrl+S again, nothing edited.
    f.app
        .document_state
        .symbol_editors
        .get_mut(&f.symbol_path)
        .unwrap()
        .dirty = true;
    save(&mut f.app);
    let row = row_on_disk(&f.library_file, f.users);
    assert_eq!(symbol_on_disk(&f).symbols[0].version, "0.0.2");
    assert_eq!(row.symbol_version, "0.0.2");
    assert_eq!(row.version, "0.0.2", "the row is not bumped twice");

    // A real edit after the bump moves on by exactly one more, and the
    // bump the editor never saw is not reported as another window's change.
    f.app
        .document_state
        .symbol_editors
        .get_mut(&f.symbol_path)
        .unwrap()
        .file
        .symbols[0]
        .description = "edited again".into();
    save(&mut f.app);
    let row = row_on_disk(&f.library_file, f.users);
    assert_eq!(symbol_on_disk(&f).symbols[0].version, "0.0.3");
    assert_eq!(row.symbol_version, "0.0.3");
    assert!(
        !logged("another window's change to MSG-TWICE-SYM"),
        "the editor's own bump must not read as a foreign change"
    );
}

#[test]
fn save_as_neither_bumps_nor_cascades() {
    let mut f = fixture();
    let copy = f.symbol_path.with_file_name("copy.snxsym");
    // A row pinned behind the symbol: a cascade would move it.
    let library = LocalGitAdapter::open(&f.library_file).unwrap();
    let mut behind = row_on_disk(&f.library_file, f.users);
    behind.symbol_version = "0.0.0".into();
    library.update_row("R", behind, "behind").unwrap();
    edit_a(&mut f);
    crate::settle_support::settle(
        &mut f.app,
        Message::File(FileMsg::SavePrimitiveAs {
            from_path: f.symbol_path.clone(),
            to_path: copy.clone(),
        }),
    );

    let written = SymbolFile::from_toml_str(&std::fs::read_to_string(&copy).unwrap()).unwrap();
    assert_eq!(
        written.symbols[0].version, "0.0.1",
        "Save As writes it whole"
    );
    let row = row_on_disk(&f.library_file, f.users);
    assert_eq!(row.symbol_version, "0.0.0");
    assert_eq!(row.version, "0.0.1");
}

#[test]
fn a_library_that_is_not_mounted_still_gets_its_rows_bumped() {
    let mut f = fixture_with(Setup {
        mounted: false,
        ..PERSONAL
    });
    edit_a(&mut f);
    save(&mut f.app);

    assert_eq!(symbol_on_disk(&f).symbols[0].version, "0.0.2");
    let row = row_on_disk(&f.library_file, f.users);
    assert_eq!(row.symbol_version, "0.0.2");
    assert_eq!(row.version, "0.0.2");
}

#[test]
fn a_released_row_in_team_mode_is_left_behind_and_reported() {
    let mut f = fixture_with(Setup {
        mode: WorkflowMode::Team,
        name: "MSG-TEAM-SYM",
        ..PERSONAL
    });
    // The row was made against an older symbol and is released.
    let library = LocalGitAdapter::open(&f.library_file).unwrap();
    let mut released = row_on_disk(&f.library_file, f.users);
    released.released = true;
    released.symbol_version = "0.0.0".into();
    library.update_row("R", released, "release").unwrap();
    edit_a(&mut f);
    save(&mut f.app);

    let row = row_on_disk(&f.library_file, f.users);
    assert_eq!(row.symbol_version, "0.0.0", "a released row keeps its pin");
    assert_eq!(
        symbol_on_disk(&f).symbols[0].version,
        "0.0.1",
        "Team mode leaves the symbol's version to the user"
    );
    assert!(
        logged("1 released row uses MSG-TEAM-SYM and stay below v0.0.1"),
        "the Messages panel must name the rows the cascade left behind"
    );
}
