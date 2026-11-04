//! #2021 - an undocked window on a child sheet of another project, opened
//! from the Projects tree (no recorded placement), shows and edits the
//! default placement of that project's hierarchy, as the main window does.

use iced::Task;
use uuid::Uuid;

use crate::app::Signex;
use crate::app::handlers::menu::export::tests::{app_workspace, sheet_with_net};
use crate::app::state::WindowKind;
use signex_types::schematic::InstancePath;

const A: u128 = 1;
const B: u128 = 2;

fn placement(n: u128) -> InstancePath {
    InstancePath::from_chain(vec![Uuid::from_u128(n)])
}

fn write_sheet(path: &std::path::Path, sheet: signex_types::schematic::SchematicSheet) {
    let text = signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .expect("encode");
    std::fs::write(path, text).expect("write sheet");
}

struct Fixture {
    app: Signex,
    window: iced::window::Id,
    child: std::path::PathBuf,
    _dirs: [tempfile::TempDir; 2],
}

/// Project B places `child` twice (A, B), with no per-placement entries so
/// every placement reads the shared reference `R1`. Project A is focused in
/// the main window; `child` is open in an undocked window, opened by path.
fn fixture() -> Fixture {
    fixture_with(Opts::default())
}

#[derive(Default)]
struct Opts {
    /// Leave the hierarchies unread, as before their assemblies land.
    pending: bool,
    /// The declared project has no root sheet.
    rootless: bool,
    /// Placement A reads `R7` from an instance entry, not `reference`.
    entry_for_a: bool,
}

fn fixture_with(opts: Opts) -> Fixture {
    let _ = crate::diagnostics::init_logging();
    let (dir_a, dir_b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (top, child) = (
        dir_b.path().join("top.snxsch"),
        dir_b.path().join("child.snxsch"),
    );
    let mut top_sheet = sheet_with_net("R_TOP", "TOP_NET", &["child.snxsch", "child.snxsch"]);
    for (cs, (name, id)) in top_sheet.child_sheets.iter_mut().zip([("A", A), ("B", B)]) {
        cs.name = name.to_string();
        cs.uuid = Uuid::from_u128(id);
    }
    write_sheet(&top, top_sheet);
    let mut child_sheet = sheet_with_net("R1", "CHILD_NET", &[]);
    if opts.entry_for_a {
        child_sheet.symbols[0].instances = vec![signex_types::schematic::SymbolInstance {
            project: "p".into(),
            path: placement(A).to_string(),
            reference: "R7".into(),
            unit: 1,
        }];
    }
    write_sheet(&child, child_sheet);
    let main_sheet = dir_a.path().join("main.snxsch");
    write_sheet(&main_sheet, sheet_with_net("R_MAIN", "MAIN_NET", &[]));

    let mut app = app_workspace(&dir_b.path().to_string_lossy(), &["top.snxsch"]);
    if opts.rootless {
        app.document_state.projects[0].data.schematic_root = None;
    }
    let mut other = app_workspace(&dir_a.path().to_string_lossy(), &["main.snxsch"])
        .document_state
        .projects
        .remove(0);
    other.id = app.document_state.mint_project_id();
    other.path = dir_a.path().join("ProjectA.snxprj");
    app.document_state.projects.push(other);

    app.open_or_focus_sheet_path(top);
    app.open_or_focus_sheet_path(child.clone());
    let window = iced::window::Id::unique();
    app.ui_state.windows.insert(
        window,
        WindowKind::UndockedTab {
            path: child.clone(),
            title: "child".to_string(),
        },
    );
    app.interaction_state
        .canvases
        .insert(window, crate::canvas::CanvasSlot::new());
    app.open_or_focus_sheet_path(main_sheet);
    app.refresh_panel_ctx();
    if !opts.pending {
        app.settle_navigator();
    }
    assert_eq!(app.document_state.tab_placements.get(&child), None);
    Fixture {
        app,
        window,
        child,
        _dirs: [dir_a, dir_b],
    }
}

fn edit_in_window(f: &mut Fixture, value: &str) {
    let symbol = f.app.document_state.engines[&f.child].document().symbols[0].uuid;
    let value = value.to_string();
    let _ = f.app.run_in_window(f.window, true, |app| {
        let commands = app.designator_edit_commands(symbol, value);
        app.apply_engine_commands(commands, true, true);
        Task::none()
    });
}

#[test]
fn a_designator_edit_in_a_window_of_another_project_changes_its_placement_only() {
    let mut f = fixture();
    edit_in_window(&mut f, "R9");

    let symbol = f.app.document_state.engines[&f.child].document().symbols[0].clone();
    assert_eq!(symbol.reference, "R1", "the shared reference is untouched");
    assert_eq!(symbol.designator_in(&placement(A)), "R9");
    assert_eq!(symbol.designator_in(&placement(B)), "R1");
}

#[test]
fn the_window_canvas_shows_the_default_placements_designators() {
    let mut f = fixture();
    edit_in_window(&mut f, "R9");
    f.app.settle_navigator();

    let id = f.app.document_state.engines[&f.child].document().symbols[0].uuid;
    let canvas = &f.app.interaction_state.canvases[&f.window];
    assert_eq!(canvas.designators.get(&id).map(String::as_str), Some("R9"));
}

fn refusals_logged() -> usize {
    crate::diagnostics::logged_count("Designator edit refused")
}

#[test]
fn an_edit_refused_for_want_of_a_hierarchy_writes_nothing_and_says_why() {
    let mut f = fixture_with(Opts {
        pending: true,
        ..Opts::default()
    });
    let before = refusals_logged();
    edit_in_window(&mut f, "R9");

    let symbol = f.app.document_state.engines[&f.child].document().symbols[0].clone();
    assert_eq!(symbol.reference, "R1");
    assert_eq!(symbol.designator_in(&placement(A)), "R1");
    assert_eq!(symbol.designator_in(&placement(B)), "R1");
    assert!(
        refusals_logged() > before,
        "nothing reached the Messages panel"
    );
}

#[test]
fn a_project_with_no_root_sheet_keeps_writing_the_reference() {
    let mut f = fixture_with(Opts {
        rootless: true,
        ..Opts::default()
    });
    edit_in_window(&mut f, "R9");

    let symbol = f.app.document_state.engines[&f.child].document().symbols[0].clone();
    assert_eq!(symbol.reference, "R9");
    assert!(
        f.app.designator_write_refusal(&f.child).is_none(),
        "no hierarchy will come: no refusal"
    );
}

#[test]
fn the_root_sheet_is_never_refused() {
    let f = fixture_with(Opts {
        pending: true,
        ..Opts::default()
    });
    let top = f._dirs[1].path().join("top.snxsch");
    assert!(f.app.designator_write_refusal(&top).is_none());
    assert!(f.app.designator_write_refusal(&f.child).is_some());
}

#[test]
fn a_window_shows_its_default_placements_designator_before_any_edit() {
    let f = fixture_with(Opts {
        entry_for_a: true,
        ..Opts::default()
    });
    let id = f.app.document_state.engines[&f.child].document().symbols[0].uuid;
    let canvas = &f.app.interaction_state.canvases[&f.window];
    assert_eq!(canvas.designators.get(&id).map(String::as_str), Some("R7"));
}

#[test]
fn a_stale_assembly_is_dropped_and_the_current_one_redraws_the_window() {
    use crate::app::project_sheets::open_sheets_snapshot;
    use crate::app::runtime::navigator_ctx::assemble_navigator;
    let mut f = fixture_with(Opts {
        pending: true,
        entry_for_a: true,
        ..Opts::default()
    });
    let hierarchies = &mut f.app.document_state.window_hierarchies;
    hierarchies.abandon_running();
    let (generation, request) = hierarchies.begin_all().remove(0);
    let root = request.root.clone();
    let assembly = assemble_navigator(&open_sheets_snapshot(&f.app.document_state), request);
    let id = f.app.document_state.engines[&f.child].document().symbols[0].uuid;
    let deliver = |app: &mut Signex, generation: u64| {
        let message =
            crate::app::Message::Project(crate::app::ProjectMsg::WindowHierarchyAssembled(
                root.clone(),
                generation,
                Ok(Box::new(assembly.clone())),
            ));
        let _ = app.update(message);
    };
    let shown = |app: &Signex| {
        app.interaction_state.canvases[&f.window]
            .designators
            .get(&id)
            .cloned()
    };

    deliver(&mut f.app, generation + 1);
    assert_eq!(shown(&f.app), None, "a stale answer is dropped");
    deliver(&mut f.app, generation);
    assert_eq!(
        shown(&f.app).as_deref(),
        Some("R7"),
        "the current one lands"
    );
}

#[test]
fn find_replace_leaves_the_designators_of_a_sheet_it_cannot_place() {
    use crate::find_replace::FindReplaceMsg;
    let mut f = fixture_with(Opts {
        pending: true,
        ..Opts::default()
    });
    let _ = f.app.run_in_window(f.window, true, |app| {
        app.update(crate::app::Message::FindReplaceMsg(
            FindReplaceMsg::QueryChanged("R1".into()),
        ))
    });
    let kinds: Vec<_> = f
        .app
        .ui_state
        .find_replace
        .matches
        .iter()
        .map(|m| m.hit.kind)
        .collect();
    assert!(
        !kinds.contains(&signex_engine::FindKind::Designator),
        "{kinds:?}"
    );
}
