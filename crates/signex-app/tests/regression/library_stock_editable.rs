//! #1892 — with the maintainer mode on (`stock_editable`) a save into the
//! stock library lands on disk; with it off the same save is refused.
//!
//! The stock library is a copy of `assets/stock-library` in a temp dir, so
//! the checkout is never written.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use signex_app::app::{FileMsg, Message, Signex};
use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use tempfile::TempDir;

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

struct World {
    _tmp: TempDir,
    dir: PathBuf,
    stock: PathBuf,
    app: Signex,
}

/// The stock library mounted as the app mounts it, locked or editable.
fn world(editable: bool) -> World {
    let tmp = TempDir::new().expect("tempdir");
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/stock-library");
    let dir = tmp.path().join("stock");
    copy_dir(&source, &dir);
    let stock = dir.join("Stock.snxlib");
    let (mut app, _boot) = Signex::new();
    app.library.pending_mounts.clear();
    app.library.stock_library_root = Some(stock.clone());
    app.library.stock_editable = editable;
    if editable {
        app.library.open_library(stock.clone()).expect("mount");
    } else {
        app.library
            .open_library_read_only(stock.clone())
            .expect("mount");
    }
    assert_eq!(app.library.is_read_only(&stock), !editable);
    World {
        _tmp: tmp,
        dir,
        stock,
        app,
    }
}

fn add_row(w: &mut World) {
    let table = w
        .app
        .library
        .library_at(&w.stock)
        .and_then(|lib| lib.tables.keys().next().cloned())
        .expect("a stock table");
    w.app.document_state.error_notice = None;
    let task = w
        .app
        .update(Message::Library(LibraryMessage::BrowserAddComponent {
            library_path: w.stock.clone(),
            table: Some(table),
        }));
    crate::drive::drive(&mut w.app, task);
}

fn first_footprint(w: &World) -> PathBuf {
    let mut files: Vec<PathBuf> = std::fs::read_dir(w.dir.join("footprints"))
        .expect("footprints")
        .map(|e| e.expect("entry").path())
        .collect();
    files.sort();
    files.remove(0)
}

fn open_rename_save(w: &mut World, path: &Path) {
    crate::settle_support::settle(
        &mut w.app,
        Message::File(FileMsg::Opened(Some(path.to_path_buf()))),
    );
    if !w.app.document_state.footprint_editors.contains_key(path) {
        return;
    }
    let task = w
        .app
        .update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
            path: path.to_path_buf(),
            msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SetName("renamed".into())),
        }));
    crate::drive::drive(&mut w.app, task);
    crate::settle_support::settle(
        &mut w.app,
        Message::Library(LibraryMessage::PrimitiveEditorEvent {
            path: path.to_path_buf(),
            msg: PrimitiveEdit::Save,
        }),
    );
}

#[test]
fn an_editable_stock_library_saves_a_row_to_disk() {
    let _notes = crate::shared_locks::new_row_notes();
    let mut w = world(true);
    let before = bytes_of(&w.dir);

    add_row(&mut w);

    assert!(w.app.document_state.error_notice.is_none());
    assert_ne!(before, bytes_of(&w.dir), "the row must reach the disk");
}

#[test]
fn an_editable_stock_library_saves_a_footprint_to_disk() {
    let mut w = world(true);
    let path = first_footprint(&w);
    let before = std::fs::read(&path).expect("read");

    open_rename_save(&mut w, &path);

    assert!(
        w.app.document_state.footprint_editors.contains_key(&path),
        "an editable stock footprint opens in place"
    );
    assert_ne!(
        before,
        std::fs::read(&path).expect("read"),
        "the edit lands"
    );
}

#[test]
fn a_locked_stock_library_refuses_the_same_row_save() {
    let mut w = world(false);
    let before = bytes_of(&w.dir);

    add_row(&mut w);

    let notice = w
        .app
        .document_state
        .error_notice
        .as_ref()
        .expect("the refusal reaches the user");
    assert_eq!(notice.title, "Library Is Read-Only");
    assert_eq!(before, bytes_of(&w.dir));
}

#[test]
fn a_locked_stock_library_never_opens_its_footprint_for_a_save() {
    let mut w = world(false);
    let path = first_footprint(&w);
    let before = bytes_of(&w.dir);

    open_rename_save(&mut w, &path);

    assert!(!w.app.document_state.footprint_editors.contains_key(&path));
    assert_eq!(before, bytes_of(&w.dir));
}
