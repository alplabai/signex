//! #1801: Save As and the first Save of an undocked Symbol or Footprint
//! editor window re-key the window with the editor, so its per-window
//! commands still find the document afterwards.

use std::path::{Path, PathBuf};

use crate::app::command::CommandArgs;
use crate::app::state::WindowKind;
use crate::app::{FootprintDocument, Signex, SymbolDocument, TabInfo, TabKind};
use crate::keymap::AppCommandId;

#[derive(Clone, Copy, Debug)]
enum Primitive {
    Symbol,
    Footprint,
}

struct Fx {
    app: Signex,
    window: iced::window::Id,
    from: PathBuf,
    to: PathBuf,
    _dir: tempfile::TempDir,
}

/// An in-memory editor of `kind` (no file yet) shown by an undocked window.
fn fixture(kind: Primitive) -> Fx {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _boot) = Signex::new();
    let ext = match kind {
        Primitive::Symbol => "snxsym",
        Primitive::Footprint => "snxfpt",
    };
    let from = dir.path().join(format!("new.{ext}"));
    let to = dir.path().join(format!("saved.{ext}"));
    let tab_kind = match kind {
        Primitive::Symbol => {
            let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("s"));
            app.document_state
                .symbol_editors
                .insert(from.clone(), SymbolDocument::new(from.clone(), file));
            TabKind::SymbolEditor(from.clone())
        }
        Primitive::Footprint => {
            let file = signex_library::FootprintFile::from_footprint(
                signex_library::Footprint::empty("f"),
            );
            app.document_state
                .footprint_editors
                .insert(from.clone(), FootprintDocument::new(from.clone(), file));
            TabKind::FootprintEditor(from.clone())
        }
    };
    app.document_state.tabs.push(TabInfo {
        title: "new".to_string(),
        path: from.clone(),
        cached_document: None,
        project_id: None,
        kind: tab_kind,
    });
    app.document_state.active_tab = 0;
    let window = iced::window::Id::unique();
    app.ui_state.windows.insert(
        window,
        WindowKind::UndockedTab {
            path: from.clone(),
            title: "new".to_string(),
        },
    );
    app.interaction_state
        .canvases
        .insert(window, crate::canvas::CanvasSlot::new());
    Fx {
        app,
        window,
        from,
        to,
        _dir: dir,
    }
}

fn window_path(f: &Fx) -> PathBuf {
    match f.app.ui_state.windows.get(&f.window) {
        Some(WindowKind::UndockedTab { path, .. }) => path.clone(),
        other => panic!("not an undocked tab window: {other:?}"),
    }
}

fn command(id: &str) -> AppCommandId {
    AppCommandId::new(id).unwrap()
}

fn save_as_enables_the_window(kind: Primitive) {
    let mut f = fixture(kind);
    let (from, to) = (f.from.clone(), f.to.clone());
    let _write = f.app.handle_save_primitive_as(&from, &to).expect("starts");
    assert_eq!(window_path(&f), to, "{kind:?}: the window follows the file");
    let ctx = f.app.command_context(Some(f.window));
    assert!(ctx.document_is_editor, "{kind:?}: the window finds its tab");
    assert!(ctx.document_has_file);
}

#[test]
fn save_as_in_an_undocked_symbol_window_re_keys_the_window() {
    save_as_enables_the_window(Primitive::Symbol);
}

#[test]
fn save_as_in_an_undocked_footprint_window_re_keys_the_window() {
    save_as_enables_the_window(Primitive::Footprint);
}

/// Save in the window writes the re-keyed document: once the Save As write
/// has landed, a Save typed in the window logs its own write of the new
/// file and leaves the document clean. (The write itself may skip an
/// unchanged container, so the log line is the observable.)
fn save_acts_after_save_as(kind: Primitive) {
    let mut f = fixture(kind);
    let (from, to) = (f.from.clone(), f.to.clone());
    let write = f.app.handle_save_primitive_as(&from, &to).expect("starts");
    crate::app::path_ops::settle_task(&mut f.app, write);
    let needle = format!("[save] Wrote {}", to.display());
    let before = crate::diagnostics::logged_count(&needle);
    assert_eq!(before, 0, "{kind:?}: Save As logs as [save-as]");
    f.app.document_state.mark_document_dirty(&to);
    let window = f.window;
    let task = f
        .app
        .dispatch_command(&command("save_document"), CommandArgs::none(), Some(window));
    crate::app::path_ops::settle_task(&mut f.app, task);
    assert_eq!(
        crate::diagnostics::logged_count(&needle),
        1,
        "{kind:?}: Save in the window wrote its document"
    );
    assert!(
        !f.app.document_state.is_document_dirty(&to),
        "{kind:?}: the saved document is clean"
    );
}

#[test]
fn save_in_an_undocked_symbol_window_acts_after_save_as() {
    save_acts_after_save_as(Primitive::Symbol);
}

#[test]
fn save_in_an_undocked_footprint_window_acts_after_save_as() {
    save_acts_after_save_as(Primitive::Footprint);
}

/// Close Document in the window closes its own re-keyed tab; with the old
/// path the window found no tab and the keystroke did nothing.
fn close_document_acts_after_save_as(kind: Primitive) {
    let mut f = fixture(kind);
    let (from, to) = (f.from.clone(), f.to.clone());
    let _write = f.app.handle_save_primitive_as(&from, &to).expect("starts");
    let window = f.window;
    let _ = f.app.dispatch_command(
        &command("close_active_document"),
        CommandArgs::none(),
        Some(window),
    );
    assert!(
        f.app.document_state.tabs.iter().all(|t| t.path != to),
        "{kind:?}: the window's document closed"
    );
}

#[test]
fn close_document_in_an_undocked_symbol_window_acts_after_save_as() {
    close_document_acts_after_save_as(Primitive::Symbol);
}

#[test]
fn close_document_in_an_undocked_footprint_window_acts_after_save_as() {
    close_document_acts_after_save_as(Primitive::Footprint);
}

/// A failed Save As rolls the editor back to `from`; the window goes with it.
#[test]
fn a_rolled_back_save_as_returns_the_window_to_the_old_path() {
    let mut f = fixture(Primitive::Symbol);
    let (from, to) = (f.from.clone(), f.to.clone());
    f.app.retarget_primitive(&from, &to, None).expect("re-key");
    assert_eq!(window_path(&f), to);
    f.app
        .retarget_primitive(&to, &from, None)
        .expect("roll back");
    assert_eq!(window_path(&f), from);
}

/// A window on another document keeps its path.
#[test]
fn save_as_leaves_another_window_alone() {
    let mut f = fixture(Primitive::Symbol);
    let other = iced::window::Id::unique();
    let other_path = Path::new("other.snxsch").to_path_buf();
    f.app.ui_state.windows.insert(
        other,
        WindowKind::UndockedTab {
            path: other_path.clone(),
            title: "other".to_string(),
        },
    );
    let (from, to) = (f.from.clone(), f.to.clone());
    f.app.retarget_primitive(&from, &to, None).expect("re-key");
    assert!(matches!(
        f.app.ui_state.windows.get(&other),
        Some(WindowKind::UndockedTab { path, .. }) if *path == other_path
    ));
}
