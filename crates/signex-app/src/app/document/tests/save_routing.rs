//! Guards for what Ctrl+S, Save As and the File menu rows do per document
//! kind, in the main window and in an undocked one. They pin today's answers
//! so that routing the save through the `DocumentKey` (#661) is provably
//! behaviour-preserving.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::{Kind, fixture, tab};
use crate::app::command::{CommandArgs, is_enabled};
use crate::app::path_ops::settle_task;
use crate::app::state::WindowKind;
use crate::app::{FileMsg, Message, Signex, TabDocument, TabInfo, TabKind};
use crate::keymap::AppCommandId;

const OLD: &[u8] = b"OLD";

fn command(id: &str) -> AppCommandId {
    AppCommandId::new(id).unwrap()
}

pub(super) fn board() -> signex_types::pcb::PcbBoard {
    signex_types::pcb::PcbBoard {
        unknown: Default::default(),
        uuid: uuid::Uuid::nil(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: vec![],
        layers: vec![],
        setup: None,
        nets: vec![],
        footprints: vec![],
        segments: vec![],
        vias: vec![],
        zones: vec![],
        graphics: vec![],
        texts: vec![],
    }
}

/// The paths a fixture of `kind` keeps its document at, and the project file.
fn doc_path(app: &Signex, t: &TabInfo) -> PathBuf {
    match &t.kind {
        TabKind::Project(_) => app.document_state.projects[0].path.clone(),
        _ => t.path.clone(),
    }
}

/// What the document's file holds before the save: a primitive must parse.
fn seed_bytes(kind: Kind) -> Vec<u8> {
    match kind {
        Kind::Symbol => signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("s"))
            .to_toml_string()
            .unwrap()
            .into_bytes(),
        Kind::Footprint => {
            signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("f"))
                .to_toml_string()
                .unwrap()
                .into_bytes()
        }
        _ => OLD.to_vec(),
    }
}

/// A dirty document of `kind` (with its file holding [`OLD`] unless
/// `missing`), plus the project file, dirty or clean. The document is the
/// main window's active tab.
struct Fx {
    app: Signex,
    tab_path: PathBuf,
    project: PathBuf,
    dir: tempfile::TempDir,
}

fn setup(kind: Kind, project_dirty: bool, missing: bool) -> Fx {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("lib.snxlib")).unwrap();
    let (mut app, mut t) = fixture(dir.path(), kind, true);
    if matches!(kind, Kind::Pcb) {
        t.cached_document = Some(TabDocument::Pcb(board()));
    }
    let project = app.document_state.projects[0].path.clone();
    std::fs::write(&project, OLD).unwrap();
    let file = doc_path(&app, &t);
    if !missing && !matches!(kind, Kind::Row) {
        std::fs::write(&file, seed_bytes(kind)).unwrap();
    }
    // The editors' dirty drafts differ from the seeded file.
    if let Some(editor) = app.document_state.symbol_editors.get_mut(&t.path) {
        editor.primitive_mut().name = "edited".into();
    }
    if let Some(editor) = app.document_state.footprint_editors.get_mut(&t.path) {
        editor.primitive_mut().name = "edited".into();
    }
    if project_dirty && !matches!(kind, Kind::Project) {
        app.document_state.mark_document_dirty(&project);
    }
    let id = app.document_state.projects[0].id;
    app.document_state.active_project = Some(id);
    app.document_state.active_path = matches!(kind, Kind::Schematic).then(|| t.path.clone());
    let tab_path = t.path.clone();
    app.document_state.tabs.push(t);
    app.document_state.active_tab = 0;
    Fx {
        app,
        tab_path,
        project,
        dir,
    }
}

fn bytes(path: &Path) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

/// The files that no longer hold [`OLD`] (or exist at all) after `run`.
fn written(f: &mut Fx, run: impl FnOnce(&mut Signex)) -> Vec<String> {
    let files = [
        f.dir.path().join("a.snxsch"),
        f.dir.path().join("b.snxpcb"),
        f.dir.path().join("lib.snxlib").join("s.snxsym"),
        f.dir.path().join("lib.snxlib").join("f.snxfpt"),
        f.project.clone(),
    ];
    let before: BTreeMap<_, _> = files.iter().map(|p| (p.clone(), bytes(p))).collect();
    run(&mut f.app);
    files
        .iter()
        .filter(|p| bytes(p) != before[*p])
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect()
}

fn ctrl_s(f: &mut Fx) -> Vec<String> {
    written(f, |app| {
        let task = app.update(Message::File(FileMsg::Save));
        settle_task(app, task);
    })
}

#[test]
fn ctrl_s_in_the_main_window_writes_the_active_documents_file() {
    for (kind, want) in [
        (Kind::Schematic, vec!["a.snxsch"]),
        (Kind::Pcb, vec!["b.snxpcb"]),
        (Kind::Symbol, vec!["s.snxsym"]),
        (Kind::Footprint, vec!["f.snxfpt"]),
        (Kind::Row, vec![]),
        (Kind::Project, vec!["p.snxprj"]),
    ] {
        let mut f = setup(kind, false, false);
        assert_eq!(ctrl_s(&mut f), want, "{kind:?}");
        assert!(f.app.document_state.error_notice.is_none(), "{kind:?}");
    }
}

/// Every tab but a Project tab also saves the active project when dirty.
#[test]
fn ctrl_s_also_saves_the_dirty_active_project_beside_the_document() {
    for (kind, want) in [
        (Kind::Schematic, vec!["a.snxsch", "p.snxprj"]),
        (Kind::Pcb, vec!["b.snxpcb", "p.snxprj"]),
        (Kind::Symbol, vec!["s.snxsym", "p.snxprj"]),
        (Kind::Footprint, vec!["f.snxfpt", "p.snxprj"]),
        (Kind::Row, vec!["p.snxprj"]),
    ] {
        let mut f = setup(kind, true, false);
        assert_eq!(ctrl_s(&mut f), want, "{kind:?}");
    }
}

/// A Project tab saves its own project, whichever project is the active one.
#[test]
fn ctrl_s_on_a_project_tab_saves_that_project_even_when_none_is_active() {
    let mut f = setup(Kind::Project, false, false);
    f.app.document_state.active_project = None;
    assert_eq!(ctrl_s(&mut f), ["p.snxprj"]);
}

#[test]
fn ctrl_s_on_a_library_browser_tab_saves_only_the_dirty_project() {
    for (dirty, want) in [(false, vec![]), (true, vec!["p.snxprj"])] {
        let mut f = setup(Kind::Row, false, false);
        let lib = f.dir.path().join("lib.snxlib");
        f.app.document_state.tabs[0] = tab(&lib, TabKind::LibraryBrowser(lib.clone()));
        f.app.document_state.active_path = None;
        if dirty {
            f.app.document_state.mark_document_dirty(&f.project);
        }
        assert_eq!(ctrl_s(&mut f), want, "dirty project: {dirty}");
    }
}

/// A new symbol or footprint with no file yet asks where to save (a dialog
/// task) and writes nothing; its dirty marker stays.
#[test]
fn ctrl_s_on_an_unwritten_primitive_opens_a_dialog_and_writes_nothing() {
    for kind in [Kind::Symbol, Kind::Footprint] {
        let mut f = setup(kind, true, true);
        let mut dialog = false;
        let wrote = written(&mut f, |app| {
            let task = app.save_active_document().expect("starts");
            dialog = iced_runtime::task::into_stream(task).is_some();
        });
        assert_eq!(wrote, Vec::<String>::new(), "{kind:?}");
        assert!(dialog, "{kind:?}: the save-as dialog task");
        // Saving the missing file instead would start a write job for it.
        assert!(f.app.document_state.error_notice.is_none(), "{kind:?}");
        let saving = f.app.document_state.document_saves.is_saving(&f.tab_path);
        assert!(!saving, "{kind:?}: no write was started");
        assert!(
            f.app.document_state.is_document_dirty(&f.tab_path),
            "{kind:?}"
        );
    }
}

fn enabled(f: &Fx, id: &str, window: Option<iced::window::Id>) -> bool {
    is_enabled(&command(id), &f.app.command_context(window))
}

#[test]
fn the_save_rows_enable_by_the_active_documents_kind() {
    // (kind, save, save as) with the project clean, then dirty.
    for (kind, clean, dirty) in [
        (Kind::Schematic, (true, true), (true, true)),
        (Kind::Pcb, (false, false), (true, false)),
        (Kind::Symbol, (true, true), (true, true)),
        (Kind::Footprint, (true, true), (true, true)),
        (Kind::Row, (false, false), (true, false)),
        (Kind::Project, (false, false), (true, false)),
    ] {
        for (project_dirty, want) in [(false, clean), (true, dirty)] {
            let f = setup(kind, project_dirty, false);
            let got = (
                enabled(&f, "save_document", None),
                enabled(&f, "save_document_as", None),
            );
            // A dirty Project tab is dirty by its own flag.
            let want = if matches!(kind, Kind::Project) && !project_dirty {
                (true, false)
            } else {
                want
            };
            assert_eq!(got, want, "{kind:?} project dirty: {project_dirty}");
        }
    }
}

fn save_as_dialog_opens(f: &mut Fx) -> bool {
    let task = f.app.save_as_dialog_for_active_tab();
    iced_runtime::task::into_stream(task).is_some()
}

#[test]
fn save_as_opens_a_dialog_for_a_sheet_a_symbol_and_a_footprint_only() {
    for (kind, want) in [
        (Kind::Schematic, true),
        (Kind::Symbol, true),
        (Kind::Footprint, true),
        (Kind::Pcb, false),
        (Kind::Row, false),
        (Kind::Project, false),
    ] {
        let mut f = setup(kind, false, false);
        assert_eq!(save_as_dialog_opens(&mut f), want, "{kind:?}");
    }
}

/// An undocked window of `kind`'s tab beside a dirty sheet in the main
/// window; Ctrl+S typed there writes only the window's own document.
fn undocked(kind: Kind) -> Fx {
    let mut f = setup(kind, false, false);
    let sheet = f.dir.path().join("main.snxsch");
    std::fs::write(&sheet, OLD).unwrap();
    f.app.document_state.insert_engine(
        sheet.clone(),
        signex_engine::Engine::new(Default::default()).unwrap(),
    );
    f.app.document_state.mark_document_dirty(&sheet);
    f.app
        .document_state
        .tabs
        .insert(0, tab(&sheet, TabKind::Schematic));
    f.app.document_state.active_tab = 0;
    f.app.document_state.active_path = Some(sheet);
    f
}

fn ctrl_s_in_window(f: &mut Fx, kind: Kind) -> (Vec<String>, Vec<u8>) {
    let window = iced::window::Id::unique();
    f.app.ui_state.windows.insert(
        window,
        WindowKind::UndockedTab {
            path: f.tab_path.clone(),
            title: "t".into(),
        },
    );
    f.app
        .interaction_state
        .canvases
        .insert(window, crate::canvas::CanvasSlot::new());
    let main = f.dir.path().join("main.snxsch");
    let wrote = written(f, |app| {
        let task =
            app.dispatch_command(&command("save_document"), CommandArgs::none(), Some(window));
        settle_task(app, task);
    });
    let _ = kind;
    (wrote, std::fs::read(main).unwrap())
}

#[test]
fn ctrl_s_in_an_undocked_window_writes_that_windows_document_not_the_main_one() {
    for (kind, want) in [
        (Kind::Schematic, vec!["a.snxsch"]),
        (Kind::Pcb, vec![]),
        (Kind::Symbol, vec!["s.snxsym"]),
        (Kind::Footprint, vec!["f.snxfpt"]),
        (Kind::Project, vec!["p.snxprj"]),
    ] {
        let mut f = undocked(kind);
        let (wrote, main) = ctrl_s_in_window(&mut f, kind);
        assert_eq!(wrote, want, "{kind:?}");
        assert_eq!(main, OLD, "{kind:?}: the main window's sheet is untouched");
        assert_eq!(f.app.document_state.active_tab, 0, "{kind:?}");
    }
}
