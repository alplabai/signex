//! m132 Library Document Options, m133 New Library, m134 Copy stock
//! primitive and m135 Library Updates.

use std::path::{Path, PathBuf};

use signex_app::app::{FileMsg, Message, Signex, TabKind};
use signex_app::library::messages::LibraryMessage;
use signex_app::library::part_sync::Choice;
use signex_app::library::stock_copy::StockCopyMsg;
use signex_app::library::updates_dialog::{
    BumpKind, ConflictChoice, LibraryUpdateEntry, LibraryUpdatesState,
};
use signex_app::panels::SheetColor;
use signex_library::RowId;
use signex_types::coord::Unit;
use signex_types::schematic::StampField;
use tempfile::TempDir;
use uuid::Uuid;

use super::support::{
    LibRig, files_under, fresh_app, library_on_disk, library_rig, load_project, open_sheet,
    part_number, project_data,
};
use crate::fixture::{self, Fixture};
use crate::settle_support::settle;

// ---- m132 ----

fn open_options(r: &mut LibRig) {
    r.send(LibraryMessage::OpenDocumentOptions {
        library_path: r.root.clone(),
    });
    assert!(r.app.library.document_options.is_some());
}

fn pick_options(r: &mut LibRig) {
    r.send(LibraryMessage::DocumentOptionsSetSheetColor(
        SheetColor::Cream,
    ));
    r.send(LibraryMessage::DocumentOptionsGridSpacingPicked(0.635));
    r.send(LibraryMessage::DocumentOptionsUnitPicked(Unit::Mil));
}

fn set_known_display(r: &mut LibRig) {
    let root = r.root.clone();
    let display = &mut r
        .app
        .library
        .library_at_mut(&root)
        .expect("mounted")
        .display;
    display.sheet_color = SheetColor::Black;
    display.grid_size_mm = 2.54;
    display.grid_visible = true;
    display.unit = Unit::Mm;
}

#[test]
fn document_options_apply_sets_the_library_display() {
    let mut r = library_rig(1);
    set_known_display(&mut r);
    open_options(&mut r);
    pick_options(&mut r);
    r.send(LibraryMessage::DocumentOptionsToggleGrid);

    r.send(LibraryMessage::DocumentOptionsApply);

    assert!(r.app.library.document_options.is_none());
    let display = r.app.library.library_at(&r.root).expect("mounted").display;
    assert_eq!(display.sheet_color, SheetColor::Cream);
    assert_eq!(display.grid_size_mm, 0.635);
    assert_eq!(display.unit, Unit::Mil);
    assert!(!display.grid_visible);
}

#[test]
fn document_options_cancel_leaves_the_library_display_unchanged() {
    let mut r = library_rig(1);
    set_known_display(&mut r);
    open_options(&mut r);
    pick_options(&mut r);

    r.send(LibraryMessage::DocumentOptionsCancel);

    assert!(r.app.library.document_options.is_none());
    let display = r.app.library.library_at(&r.root).expect("mounted").display;
    assert_eq!(display.sheet_color, SheetColor::Black);
    assert_eq!(display.grid_size_mm, 2.54);
    assert_eq!(display.unit, Unit::Mm);
}

#[test]
fn a_grid_spacing_that_is_no_length_is_refused() {
    let mut r = library_rig(1);
    set_known_display(&mut r);
    open_options(&mut r);

    for bad in [0.0, -1.27, f32::NAN, f32::INFINITY] {
        r.send(LibraryMessage::DocumentOptionsGridSpacingPicked(bad));
    }

    let draft = r.app.library.document_options.as_ref().expect("open").draft;
    assert_eq!(draft.grid_size_mm, 2.54);
}

// ---- m133 ----

struct NewLibrary {
    app: Signex,
    prj: PathBuf,
    lib: PathBuf,
    tmp: TempDir,
}

fn new_library_dialog() -> NewLibrary {
    let tmp = TempDir::new().expect("tempdir");
    let prj = tmp.path().join("Proj.snxprj");
    let data = project_data(tmp.path(), &[]);
    let mut app = fresh_app();
    load_project(&mut app, &prj, &data);
    let lib = tmp.path().join("Proj-lib").join("Proj-lib.snxlib");
    let _ = app.update(Message::Library(LibraryMessage::CreateLibraryAtPath {
        project_path: prj.clone(),
        lib_path: lib.clone(),
    }));
    assert!(app.library.create_options.is_some(), "the dialog opens");
    NewLibrary { app, prj, lib, tmp }
}

impl NewLibrary {
    fn send(&mut self, msg: LibraryMessage) {
        let _ = self.app.update(Message::Library(msg));
    }

    fn flags(&self) -> (bool, bool) {
        let state = self.app.library.create_options.as_ref().expect("open");
        (state.enable_git, state.use_lfs)
    }

    fn pending(&self) -> usize {
        self.app.document_state.projects[0].pending_libraries.len()
    }
}

#[test]
fn new_library_starts_with_version_control_and_lfs_off() {
    let d = new_library_dialog();
    assert_eq!(d.flags(), (false, false));
}

#[test]
fn turning_version_control_off_turns_lfs_off() {
    let mut d = new_library_dialog();
    d.send(LibraryMessage::LibraryCreateOptionsToggleGit);
    d.send(LibraryMessage::LibraryCreateOptionsToggleLfs);
    assert_eq!(d.flags(), (true, true));

    d.send(LibraryMessage::LibraryCreateOptionsToggleGit);

    assert_eq!(d.flags(), (false, false));
}

#[test]
fn create_library_registers_it_and_writes_nothing_until_the_project_is_saved() {
    let mut d = new_library_dialog();
    let before = files_under(d.tmp.path());

    d.send(LibraryMessage::LibraryCreateOptionsConfirm);

    assert!(d.app.library.create_options.is_none());
    assert_eq!(d.pending(), 1);
    assert!(d.app.document_state.is_document_dirty(&d.prj));
    assert!(!d.lib.exists(), "no library file before the project save");
    assert_eq!(files_under(d.tmp.path()), before);
}

#[test]
fn new_library_cancel_registers_nothing() {
    let mut d = new_library_dialog();

    d.send(LibraryMessage::LibraryCreateOptionsCancel);

    assert!(d.app.library.create_options.is_none());
    assert_eq!(d.pending(), 0);
    assert!(!d.app.document_state.is_document_dirty(&d.prj));
    assert!(!d.lib.exists());
}

/// Row m133: a library already at the target keeps the dialog open with the
/// refusal banner and its two ways out.
#[test]
fn a_library_already_at_the_path_is_refused_in_the_open_dialog() {
    let mut d = new_library_dialog();
    library_on_disk(&d.lib, 0);
    let before = files_under(d.tmp.path());

    d.send(LibraryMessage::LibraryCreateOptionsConfirm);

    assert_eq!(d.pending(), 0, "nothing is registered");
    assert_eq!(files_under(d.tmp.path()), before);
    let state = d
        .app
        .library
        .create_options
        .as_ref()
        .expect("the dialog stays open");
    assert!(state.refusal.is_some(), "the banner says why");
}

// ---- m134 ----

struct StockCopy {
    app: Signex,
    fx: Fixture,
    target: PathBuf,
    _tmp: TempDir,
}

fn stock_copy_dialog() -> StockCopy {
    let fx = fixture::fixture().expect("fixture");
    let tmp = TempDir::new().expect("tempdir");
    let target = tmp.path().join("mine").join("Mine.snxlib");
    library_on_disk(&target, 0);
    let mut app = fresh_app();
    app.library.stock_editable = false;
    app.library.stock_library_root = Some(fx.stock.clone());
    app.library
        .open_library_read_only(fx.stock.clone())
        .expect("mount stock");
    app.library
        .open_library(target.clone())
        .expect("mount target");
    settle(
        &mut app,
        Message::File(FileMsg::Opened(Some(fx.symbol.clone()))),
    );
    assert!(app.library.stock_copy.is_some(), "the dialog asks where");
    StockCopy {
        app,
        fx,
        target,
        _tmp: tmp,
    }
}

impl StockCopy {
    fn stock_dir(&self) -> &Path {
        self.fx.stock.parent().expect("stock folder")
    }

    fn target_symbols(&self) -> PathBuf {
        self.target.parent().expect("folder").join("symbols")
    }

    fn copies(&self) -> usize {
        std::fs::read_dir(self.target_symbols()).map_or(0, Iterator::count)
    }
}

#[test]
fn a_click_on_a_library_copies_there_and_opens_the_copy() {
    let mut c = stock_copy_dialog();
    let stock_before = files_under(c.stock_dir());
    let target = c.target.clone();

    settle(
        &mut c.app,
        Message::Library(LibraryMessage::StockCopy(StockCopyMsg::TargetPicked(
            target,
        ))),
    );

    assert!(c.app.library.stock_copy.is_none());
    assert_eq!(c.copies(), 1);
    let tab = &c.app.document_state.tabs[c.app.document_state.active_tab];
    assert!(
        matches!(&tab.kind, TabKind::SymbolEditor(path) if path.starts_with(c.target_symbols())),
        "the editor is on the copy"
    );
    assert_eq!(
        files_under(c.stock_dir()),
        stock_before,
        "the stock original is not changed"
    );
}

#[test]
fn copy_to_library_cancel_copies_nothing() {
    let mut c = stock_copy_dialog();
    let stock_before = files_under(c.stock_dir());

    settle(
        &mut c.app,
        Message::Library(LibraryMessage::StockCopy(StockCopyMsg::Cancelled)),
    );

    assert!(c.app.library.stock_copy.is_none());
    assert_eq!(c.copies(), 0);
    assert_eq!(files_under(c.stock_dir()), stock_before);
}

#[test]
fn the_read_only_stock_library_is_refused_as_a_copy_target() {
    let mut c = stock_copy_dialog();
    let stock_before = files_under(c.stock_dir());
    let stock = c.fx.stock.clone();

    settle(
        &mut c.app,
        Message::Library(LibraryMessage::StockCopy(StockCopyMsg::TargetPicked(stock))),
    );

    let dialog = c.app.library.stock_copy.as_ref().expect("the dialog stays");
    assert!(dialog.error.is_some(), "the dialog says why");
    assert_eq!(files_under(c.stock_dir()), stock_before);
}

// ---- m135 ----

fn entry(r: &LibRig, row_id: RowId, n: usize, bump_kind: BumpKind) -> LibraryUpdateEntry {
    LibraryUpdateEntry {
        symbol_uuid: Uuid::now_v7(),
        ref_des: format!("R{}", n + 1),
        internal_pn: part_number(n),
        lib_id: format!("{}:{}", r.library_id(), row_id.as_uuid()),
        library_id: r.library_id(),
        library_name: "parts".to_string(),
        row_id,
        library_path: r.root.clone(),
        current_version: "1.0.0".to_string(),
        latest_version: match bump_kind {
            BumpKind::Patch => "1.0.1".to_string(),
            BumpKind::Minor => "1.1.0".to_string(),
            BumpKind::Major => "2.0.0".to_string(),
        },
        bump_kind,
        selected: false,
        conflicts: Vec::new(),
    }
}

/// A sheet with the modal open on a patch and a major bump.
fn updates_modal() -> (LibRig, PathBuf, TempDir) {
    let mut r = library_rig(2);
    let (sheet, tmp) = open_sheet(&mut r.app);
    let mut entries = vec![
        entry(&r, r.rows[0], 0, BumpKind::Patch),
        entry(&r, r.rows[1], 1, BumpKind::Major),
    ];
    entries[1].conflicts.push(ConflictChoice {
        field: StampField::Mpn,
        mine: "MINE".to_string(),
        library: "THEIRS".to_string(),
        choice: Choice::KeepMine,
    });
    r.app.library.library_updates = Some(LibraryUpdatesState::new(sheet.clone(), entries));
    (r, sheet, tmp)
}

fn ticks(r: &LibRig) -> Vec<bool> {
    r.app
        .library
        .library_updates
        .as_ref()
        .expect("open")
        .entries
        .iter()
        .map(|e| e.selected)
        .collect()
}

#[test]
fn patch_bumps_start_ticked_and_major_bumps_do_not() {
    let (r, _sheet, _tmp) = updates_modal();
    assert_eq!(ticks(&r), [true, false]);
}

#[test]
fn a_row_checkbox_toggles_that_row() {
    let (mut r, _sheet, _tmp) = updates_modal();
    let uuid = r
        .app
        .library
        .library_updates
        .as_ref()
        .expect("open")
        .entries[1]
        .symbol_uuid;

    r.send(LibraryMessage::LibraryUpdatesToggleSelection(uuid));

    assert_eq!(ticks(&r), [true, true]);
}

#[test]
fn the_header_checkbox_ticks_all_then_clears_all() {
    let (mut r, _sheet, _tmp) = updates_modal();

    r.send(LibraryMessage::LibraryUpdatesSelectAllToggled);
    assert_eq!(ticks(&r), [true, true]);

    r.send(LibraryMessage::LibraryUpdatesSelectAllToggled);
    assert_eq!(ticks(&r), [false, false]);
}

#[test]
fn answering_a_conflict_records_it_and_ticks_the_part() {
    let (mut r, _sheet, _tmp) = updates_modal();
    let uuid = r
        .app
        .library
        .library_updates
        .as_ref()
        .expect("open")
        .entries[1]
        .symbol_uuid;

    r.send(LibraryMessage::LibraryUpdatesConflictAnswered {
        symbol_uuid: uuid,
        field: StampField::Mpn,
        choice: Choice::UseLibrary,
    });

    let state = r.app.library.library_updates.as_ref().expect("open");
    assert_eq!(state.entries[1].conflicts[0].choice, Choice::UseLibrary);
    assert!(state.entries[1].selected);
}

#[test]
fn library_updates_cancel_leaves_the_sheet_clean_and_records_no_skip() {
    let (mut r, sheet, _tmp) = updates_modal();

    r.send(LibraryMessage::LibraryUpdatesCancel);

    assert!(r.app.library.library_updates.is_none());
    assert!(!r.app.library.skipped_updates_for.contains(&sheet));
    assert!(!r.app.document_state.is_document_dirty(&sheet));
}

#[test]
fn skip_all_closes_and_remembers_the_sheet_without_editing_it() {
    let (mut r, sheet, _tmp) = updates_modal();

    r.send(LibraryMessage::LibraryUpdatesSkipAll);

    assert!(r.app.library.library_updates.is_none());
    assert!(r.app.library.skipped_updates_for.contains(&sheet));
    assert!(!r.app.document_state.is_document_dirty(&sheet));
}

#[test]
fn update_selected_with_nothing_ticked_changes_nothing() {
    let (mut r, sheet, _tmp) = updates_modal();
    r.send(LibraryMessage::LibraryUpdatesSelectAllToggled);
    r.send(LibraryMessage::LibraryUpdatesSelectAllToggled);
    assert_eq!(ticks(&r), [false, false]);

    r.run(LibraryMessage::LibraryUpdatesApply);

    assert!(r.app.library.library_updates.is_none());
    assert!(!r.app.document_state.is_document_dirty(&sheet));
}
