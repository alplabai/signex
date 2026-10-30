//! #813 — opening a stock symbol or footprint edits a copy in a writable
//! library; the read-only stock original is never opened or changed.
//!
//! The stock library is a copy of `assets/stock-library` in a temp dir,
//! mounted read-only the way the app mounts it, so byte-identity of the
//! stock files can be asserted. Opening goes through the real
//! `FileMsg::Opened` message; the copy runs on a real worker.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use signex_app::app::{FileMsg, Message, Signex, TabKind};
use signex_app::library::LibraryMessage;
use signex_app::library::stock_copy::{StockCopyMsg, StockPrimitiveKind};
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::{Footprint, FootprintFile, StepAttachment, Symbol, SymbolFile};
use tempfile::TempDir;
use uuid::Uuid;

use crate::library_place_symbol::run;

struct World {
    _serial: std::sync::MutexGuard<'static, ()>,
    tmp: TempDir,
    stock: PathBuf,
    target: PathBuf,
    app: Signex,
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("mkdir");
    for entry in std::fs::read_dir(from).expect("read_dir") {
        let entry = entry.expect("entry");
        let dest = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &dest);
        } else {
            std::fs::copy(entry.path(), &dest).expect("copy");
        }
    }
}

fn new_library(path: &Path, name: &str) {
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: Uuid::now_v7(),
        library: LibrarySection {
            name: name.into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    LocalGitAdapter::init(
        path,
        manifest,
        LibraryInitOptions {
            enable_git: false,
            use_lfs: false,
        },
    )
    .expect("init library");
}

fn world() -> World {
    let serial = crate::shared_locks::global_list_file();
    let tmp = TempDir::new().expect("tempdir");
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/stock-library");
    copy_dir(&source, &tmp.path().join("stock"));
    let stock = tmp.path().join("stock/Stock.snxlib");
    let target = tmp.path().join("mine/Mine.snxlib");
    new_library(&target, "Mine");

    let (mut app, _boot) = Signex::new();
    // Prefs are shared by the process: start without a remembered target.
    app.library.stock_copy_target = None;
    app.library.stock_library_root = Some(stock.clone());
    app.library
        .open_library_read_only(stock.clone())
        .expect("mount stock");
    app.library
        .open_library(target.clone())
        .expect("mount mine");
    World {
        _serial: serial,
        tmp,
        stock,
        target,
        app,
    }
}

impl World {
    fn stock_file(&self, dir: &str, name: &str) -> PathBuf {
        self.stock.parent().unwrap().join(dir).join(name)
    }

    fn stock_r(&self) -> PathBuf {
        self.stock_file("symbols", "R.snxsym")
    }

    fn target_dir(&self) -> PathBuf {
        self.target.parent().unwrap().to_path_buf()
    }

    /// Apply `message` and every message its tasks lead to.
    fn settle(&mut self, message: Message) {
        settle_message(&mut self.app, message);
    }

    fn open(&mut self, path: &Path) {
        self.settle(Message::File(FileMsg::Opened(Some(path.to_path_buf()))));
    }

    fn pick(&mut self, library: &Path) {
        self.settle(Message::Library(LibraryMessage::StockCopy(
            StockCopyMsg::TargetPicked(library.to_path_buf()),
        )));
    }

    fn editor_paths(&self) -> Vec<PathBuf> {
        self.app
            .document_state
            .tabs
            .iter()
            .filter(|t| {
                matches!(
                    t.kind,
                    TabKind::SymbolEditor(_) | TabKind::FootprintEditor(_)
                )
            })
            .map(|t| t.path.clone())
            .collect()
    }

    fn stock_bytes(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        bytes_of(self.stock.parent().unwrap())
    }
}

fn settle_message(app: &mut Signex, message: Message) {
    let mut pending = vec![message];
    while let Some(next) = pending.pop() {
        let task = app.update(next);
        pending.extend(run(task));
    }
}

/// A new app as a restart builds it: prefs and the global library table are
/// read, and the boot task mounts what it lists. The stock library is the
/// world's copy, mounted the way the app mounts it.
fn restarted(w: &World) -> Signex {
    let (mut app, boot) = Signex::new();
    let mut pending = run(boot);
    while let Some(next) = pending.pop() {
        let task = app.update(next);
        pending.extend(run(task));
    }
    app.library.stock_library_root = Some(w.stock.clone());
    app.library
        .open_library_read_only(w.stock.clone())
        .expect("mount stock");
    app
}

fn bytes_of(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(dir: &Path, root: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).expect("read_dir") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                walk(&path, root, out);
            } else {
                out.insert(
                    path.strip_prefix(root).expect("under root").to_path_buf(),
                    std::fs::read(&path).expect("read"),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

fn symbol_in(path: &Path) -> SymbolFile {
    SymbolFile::from_bytes(&std::fs::read(path).expect("read copy")).expect("parse copy")
}

fn file_count(dir: &Path) -> usize {
    std::fs::read_dir(dir).map_or(0, |entries| entries.count())
}

#[test]
fn opening_a_stock_symbol_with_no_remembered_target_asks_which_library() {
    let mut w = world();
    let stock_r = w.stock_r();
    let before = w.stock_bytes();

    w.open(&stock_r);

    let dialog = w.app.library.stock_copy.as_ref().expect("the dialog opens");
    assert_eq!(dialog.source, stock_r);
    assert_eq!(dialog.kind, StockPrimitiveKind::Symbol);
    assert!(w.editor_paths().is_empty(), "no editor on the stock file");
    assert_eq!(file_count(&w.target_dir().join("symbols")), 0);
    assert_eq!(w.stock_bytes(), before);
}

#[test]
fn picking_a_library_copies_the_symbol_there_and_opens_the_copy() {
    let mut w = world();
    let stock_r = w.stock_r();
    let before = w.stock_bytes();
    let stock_symbol = symbol_in(&stock_r);

    w.open(&stock_r);
    let target = w.target.clone();
    w.pick(&target);

    let copy = w.target_dir().join("symbols/R.snxsym");
    assert!(copy.is_file(), "the copy is written to the picked library");
    let copied = symbol_in(&copy);
    assert_eq!(copied.symbols[0].name, stock_symbol.symbols[0].name);
    assert_ne!(copied.symbols[0].uuid, stock_symbol.symbols[0].uuid);
    assert_eq!(
        copied.symbols[0].pins.len(),
        stock_symbol.symbols[0].pins.len()
    );
    assert_eq!(w.editor_paths(), vec![copy], "the editor is on the copy");
    assert!(w.app.library.stock_copy.is_none());
    assert_eq!(
        w.app.library.stock_copy_target.as_deref(),
        Some(target.as_path())
    );
    assert_eq!(w.stock_bytes(), before, "the stock library is untouched");
    assert!(w.app.document_state.error_notice.is_none());
}

#[test]
fn a_second_open_goes_straight_to_the_remembered_library() {
    let mut w = world();
    let first = w.stock_r();
    let second = w.stock_file("symbols", "C.snxsym");
    w.open(&first);
    let target = w.target.clone();
    w.pick(&target);

    w.open(&second);

    assert!(w.app.library.stock_copy.is_none(), "no second question");
    let copy = w.target_dir().join("symbols/C.snxsym");
    assert!(copy.is_file());
    assert!(w.editor_paths().contains(&copy));
}

#[test]
fn a_name_collision_in_the_target_gets_a_suffix() {
    let mut w = world();
    // The user already has a symbol called R, and a file named R.snxsym.
    let own = w.target_dir().join("symbols/R.snxsym");
    std::fs::create_dir_all(own.parent().unwrap()).unwrap();
    std::fs::write(
        &own,
        SymbolFile::from_symbol(Symbol::empty("R"))
            .to_toml_string()
            .unwrap(),
    )
    .unwrap();
    let own_before = std::fs::read(&own).unwrap();
    let stock_r = w.stock_r();
    w.open(&stock_r);
    let target = w.target.clone();

    w.pick(&target);

    let copy = w.target_dir().join("symbols/R (2).snxsym");
    assert!(copy.is_file(), "the copy is suffixed");
    let copied = symbol_in(&copy);
    assert_eq!(copied.symbols[0].name, "R (2)");
    assert_eq!(
        copied.display_name, "R (2)",
        "the container is named after the copy"
    );
    assert_eq!(
        std::fs::read(&own).unwrap(),
        own_before,
        "the user's R is untouched"
    );
}

#[test]
fn a_second_open_reuses_the_copy_and_does_not_make_another() {
    let mut w = world();
    let stock_r = w.stock_r();
    w.open(&stock_r);
    let target = w.target.clone();
    w.pick(&target);
    let copy = w.target_dir().join("symbols/R.snxsym");
    assert_eq!(
        symbol_in(&copy).copied_from,
        Some(symbol_in(&stock_r).file_uuid),
        "the copy records the stock file it came from"
    );
    w.app.document_state.tabs.clear();
    w.app.document_state.symbol_editors.clear();
    let before = bytes_of(&w.target_dir());

    w.open(&stock_r);

    assert!(w.app.library.stock_copy.is_none());
    assert_eq!(w.editor_paths(), vec![copy], "the existing copy opens");
    assert_eq!(bytes_of(&w.target_dir()), before, "nothing new was written");
}

#[test]
fn an_open_copy_tab_is_activated_by_the_next_open() {
    let mut w = world();
    let stock_r = w.stock_r();
    let stock_c = w.stock_file("symbols", "C.snxsym");
    w.open(&stock_r);
    let target = w.target.clone();
    w.pick(&target);
    w.open(&stock_c);
    let copy_r = w.target_dir().join("symbols/R.snxsym");
    let copy_c = w.target_dir().join("symbols/C.snxsym");
    let active = |w: &World| {
        w.app.document_state.tabs[w.app.document_state.active_tab]
            .path
            .clone()
    };
    assert_eq!(active(&w), copy_c);

    w.open(&stock_r);

    assert_eq!(active(&w), copy_r, "the R tab came forward");
    assert_eq!(w.editor_paths().len(), 2, "no third tab");
}

#[test]
fn copy_again_makes_a_fresh_suffixed_copy() {
    let mut w = world();
    let stock_r = w.stock_r();
    w.open(&stock_r);
    let target = w.target.clone();
    w.pick(&target);
    let first = w.target_dir().join("symbols/R.snxsym");

    w.settle(Message::Library(LibraryMessage::StockCopy(
        StockCopyMsg::CopyAgainRequested(first.clone()),
    )));

    let second = w.target_dir().join("symbols/R (2).snxsym");
    assert!(second.is_file());
    let copied = symbol_in(&second);
    assert_eq!(copied.symbols[0].name, "R (2)");
    assert_eq!(copied.copied_from, symbol_in(&first).copied_from);
    assert_ne!(copied.symbols[0].uuid, symbol_in(&first).symbols[0].uuid);
    assert!(w.editor_paths().contains(&second));
    assert!(first.is_file(), "the first copy stays");
}

#[test]
fn an_open_after_a_restart_reuses_the_copy() {
    let mut w = world();
    let stock_r = w.stock_r();
    w.open(&stock_r);
    let target = w.target.clone();
    w.pick(&target);
    // `Mine` is a library the user made earlier and keeps in the global table.
    signex_app::panels::components_panel::global_prefs::add_path(target.clone()).unwrap();
    let before = bytes_of(&w.target_dir());

    let mut app = restarted(&w);
    assert_eq!(
        app.library.usable_stock_copy_target(),
        Some(target.as_path()),
        "the remembered target is mounted and usable again"
    );
    settle_message(
        &mut app,
        Message::File(FileMsg::Opened(Some(stock_r.clone()))),
    );

    let copy = w.target_dir().join("symbols/R.snxsym");
    assert!(app.library.stock_copy.is_none());
    let editors: Vec<PathBuf> = app
        .document_state
        .tabs
        .iter()
        .filter(|t| matches!(t.kind, TabKind::SymbolEditor(_)))
        .map(|t| t.path.clone())
        .collect();
    assert_eq!(editors, vec![copy]);
    assert_eq!(bytes_of(&w.target_dir()), before);
}

#[test]
fn a_library_made_in_the_dialog_survives_a_restart() {
    let mut w = world();
    let stock_r = w.stock_r();
    let fresh = w.tmp.path().join("fresh/Fresh.snxlib");
    w.open(&stock_r);
    w.settle(Message::Library(LibraryMessage::StockCopy(
        StockCopyMsg::NewLibraryPathChosen(Some(fresh.clone())),
    )));
    assert!(
        w.app
            .library
            .global_libraries
            .iter()
            .any(|entry| entry.path == fresh),
        "the new library is recorded as global"
    );

    let mut app = restarted(&w);

    assert!(
        app.library.library_at(&fresh).is_some(),
        "it is mounted again"
    );
    assert_eq!(
        app.library.usable_stock_copy_target(),
        Some(fresh.as_path())
    );
    let copy = fresh.parent().unwrap().join("symbols/R.snxsym");
    settle_message(&mut app, Message::File(FileMsg::Opened(Some(stock_r))));
    assert!(app.library.stock_copy.is_none(), "no second question");
    assert!(
        app.document_state.tabs.iter().any(|t| t.path == copy),
        "the user's copy is reachable"
    );
}

#[test]
fn a_second_stock_open_while_the_dialog_is_up_is_refused_not_dropped() {
    // The Messages ring only records once logging is set up; a test process
    // of its own (nextest) has not done it.
    let _ = signex_app::diagnostics::init_logging();
    let mut w = world();
    let stock_r = w.stock_r();
    let stock_c = w.stock_file("symbols", "C.snxsym");
    w.open(&stock_r);

    w.open(&stock_c);

    let dialog = w.app.library.stock_copy.as_ref().expect("still asking");
    assert_eq!(dialog.source, stock_r, "the first question stays");
    assert!(
        signex_app::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains("Finish the Copy to Library dialog")),
        "the refusal is reported"
    );
}

#[test]
fn a_read_only_library_that_is_not_stock_never_receives_a_copy() {
    let mut w = world();
    let stock_r = w.stock_r();
    let ro = w.tmp.path().join("ro/Ro.snxlib");
    new_library(&ro, "Ro");
    w.app
        .library
        .open_library_read_only(ro.clone())
        .expect("mount ro");
    assert!(
        w.app
            .library
            .writable_libraries()
            .iter()
            .all(|(root, _)| *root != ro),
        "it is not offered"
    );
    w.app.library.stock_copy_target = Some(ro.clone());
    w.open(&stock_r);
    assert!(
        w.app.library.stock_copy.is_some(),
        "a read-only target asks again"
    );

    w.pick(&ro);

    assert!(
        w.app
            .library
            .stock_copy
            .as_ref()
            .is_some_and(|d| d.error.is_some())
    );
    assert!(w.editor_paths().is_empty());
    assert_eq!(file_count(&ro.parent().unwrap().join("symbols")), 0);
}

#[test]
fn cancelling_the_dialog_opens_nothing_and_writes_nothing() {
    let mut w = world();
    let stock_r = w.stock_r();
    let before = w.stock_bytes();
    let target_before = bytes_of(&w.target_dir());
    w.open(&stock_r);

    w.settle(Message::Library(LibraryMessage::StockCopy(
        StockCopyMsg::Cancelled,
    )));

    assert!(w.app.library.stock_copy.is_none());
    assert!(w.editor_paths().is_empty());
    assert!(
        w.app.library.stock_copy_target.is_none(),
        "nothing remembered"
    );
    assert_eq!(bytes_of(&w.target_dir()), target_before);
    assert_eq!(w.stock_bytes(), before);
}

#[test]
fn a_remembered_library_that_is_gone_asks_again() {
    let mut w = world();
    let stock_r = w.stock_r();
    let gone = w.tmp.path().join("gone/Gone.snxlib");
    w.app.library.stock_copy_target = Some(gone.clone());

    w.open(&stock_r);

    let dialog = w.app.library.stock_copy.as_ref().expect("asks again");
    assert_eq!(dialog.previous_unavailable.as_deref(), Some(gone.as_path()));
    assert!(w.editor_paths().is_empty());
}

#[test]
fn a_remembered_library_that_became_read_only_asks_again() {
    let mut w = world();
    let stock_r = w.stock_r();
    let stock = w.stock.clone();
    w.app.library.stock_copy_target = Some(stock);

    w.open(&stock_r);

    assert!(w.app.library.stock_copy.is_some());
}

#[test]
fn a_stock_footprint_behaves_the_same() {
    let mut w = world();
    let stock_fpt = std::fs::read_dir(w.stock.parent().unwrap().join("footprints"))
        .expect("stock footprints")
        .map(|e| e.expect("entry").path())
        .find(|p| p.extension().is_some_and(|e| e == "snxfpt"))
        .expect("a stock footprint");
    let before = w.stock_bytes();
    let stock_name = FootprintFile::from_bytes(&std::fs::read(&stock_fpt).unwrap())
        .unwrap()
        .footprints[0]
        .name
        .clone();

    w.open(&stock_fpt);
    let dialog = w.app.library.stock_copy.as_ref().expect("the dialog opens");
    assert_eq!(dialog.kind, StockPrimitiveKind::Footprint);
    assert!(w.editor_paths().is_empty());

    let target = w.target.clone();
    w.pick(&target);

    let copies: Vec<PathBuf> = std::fs::read_dir(w.target_dir().join("footprints"))
        .expect("footprints dir")
        .map(|e| e.expect("entry").path())
        .collect();
    assert_eq!(copies.len(), 1);
    let copied = FootprintFile::from_bytes(&std::fs::read(&copies[0]).unwrap()).unwrap();
    assert_eq!(copied.footprints[0].name, stock_name);
    assert_eq!(w.editor_paths(), copies);
    assert_eq!(w.stock_bytes(), before);

    let tab_title = &w.app.document_state.tabs[w.app.document_state.active_tab].title;
    assert_eq!(
        tab_title, &stock_name,
        "the tab shows the footprint's name, not its uuid file"
    );

    // Second open: straight to the existing copy, nothing new written.
    w.open(&stock_fpt);
    assert!(w.app.library.stock_copy.is_none());
    assert_eq!(file_count(&w.target_dir().join("footprints")), 1);

    // Copy Again makes a suffixed one.
    w.settle(Message::Library(LibraryMessage::StockCopy(
        StockCopyMsg::CopyAgainRequested(copies[0].clone()),
    )));
    let names: Vec<String> = std::fs::read_dir(w.target_dir().join("footprints"))
        .unwrap()
        .map(|e| {
            FootprintFile::from_bytes(&std::fs::read(e.unwrap().path()).unwrap())
                .unwrap()
                .footprints[0]
                .name
                .clone()
        })
        .collect();
    assert!(names.contains(&format!("{stock_name} (2)")), "{names:?}");
}

#[test]
fn a_footprint_copy_keeps_its_3d_model() {
    let mut w = world();
    let hash = "0123456789abcdef";
    let mut footprint = Footprint::empty("WithModel");
    footprint.step_attachment = Some(StepAttachment {
        content_hash: hash.into(),
        filename: "m.step".into(),
        offset_xyz: [0.0; 3],
        rotation_xyz: [0.0; 3],
    });
    let stock_fpt = w.stock_file("footprints", "with-model.snxfpt");
    std::fs::write(
        &stock_fpt,
        FootprintFile::from_footprint(footprint)
            .to_toml_string()
            .unwrap(),
    )
    .unwrap();
    std::fs::create_dir_all(w.stock.parent().unwrap().join("step")).unwrap();
    std::fs::write(
        w.stock.parent().unwrap().join(format!("step/{hash}.step")),
        b"STEP-DATA",
    )
    .unwrap();

    w.open(&stock_fpt);
    let target = w.target.clone();
    w.pick(&target);

    let copy = std::fs::read_dir(w.target_dir().join("footprints"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let copied = FootprintFile::from_bytes(&std::fs::read(&copy).unwrap()).unwrap();
    assert_eq!(
        copied.footprints[0]
            .step_attachment
            .as_ref()
            .map(|a| a.content_hash.as_str()),
        Some(hash)
    );
    assert_eq!(
        std::fs::read(w.target_dir().join(format!("step/{hash}.step"))).unwrap(),
        b"STEP-DATA",
        "the model file comes along"
    );
}

#[test]
fn a_primitive_outside_the_stock_library_opens_directly() {
    let mut w = world();
    let own = w.target_dir().join("symbols/Own.snxsym");
    std::fs::create_dir_all(own.parent().unwrap()).unwrap();
    std::fs::write(
        &own,
        SymbolFile::from_symbol(Symbol::empty("Own"))
            .to_toml_string()
            .unwrap(),
    )
    .unwrap();

    w.open(&own);

    assert!(w.app.library.stock_copy.is_none());
    assert_eq!(w.editor_paths(), vec![own]);
}

#[test]
fn a_new_library_can_receive_the_copy() {
    let mut w = world();
    let stock_r = w.stock_r();
    let fresh = w.tmp.path().join("fresh/Fresh.snxlib");
    w.open(&stock_r);

    w.settle(Message::Library(LibraryMessage::StockCopy(
        StockCopyMsg::NewLibraryPathChosen(Some(fresh.clone())),
    )));

    let copy = fresh.parent().unwrap().join("symbols/R.snxsym");
    assert!(copy.is_file());
    assert_eq!(w.editor_paths(), vec![copy]);
    assert!(
        w.app.library.library_at(&fresh).is_some(),
        "the new library is mounted"
    );
    assert_eq!(
        w.app.library.stock_copy_target.as_deref(),
        Some(fresh.as_path())
    );
}
