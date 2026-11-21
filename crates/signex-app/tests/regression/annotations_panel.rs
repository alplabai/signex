//! Mockup row 165 - View ▸ Annotations opens a dock panel that lists the annotation
//! blocks of the project's Design Notebook files (`notebook/*.snxnbk`),
//! grouped by file, newest first, filtered by kind, part, status, date and
//! text; a part chip goes to the part on its sheet, a reference to a part
//! the design no longer has is amber, and what a notebook could not be read
//! for reaches the Messages panel with its file and line.
//!
//! The project: `a.snxsch` (root, U1 and R4) and `b.snxsch` (U3), both open
//! with `a` focused, and two notebook files.

use signex_app::app::{CaseFold, FileMsg, LoadedProject, Message, Signex};
use signex_app::dock::DockMessage;
use signex_app::menu_bar::MenuMessage;
use signex_app::panels::annotations::{
    AnnotationKind, AnnotationsView, DateWindow, IsoDate, ListStatus, Reference, Status,
};
use signex_app::panels::panel_groups::{PanelGroup, grouped_panels};
use signex_app::panels::{AnnotationsMsg, PanelKind, PanelMsg};
use signex_app::task_driver::TaskDriver;
use signex_types::project::{ProjectData, SheetEntry};
use signex_types::schematic::{Point, SchematicSheet, SelectedItem, SelectedKind, Symbol};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use uuid::Uuid;

const U1: u128 = 0x0001_6501;
const U3: u128 = 0x0001_6503;
const R4: u128 = 0x0001_6504;

/// `settle_support::settle` without the dock layout save, which would write
/// the panel into the test run's shared config folder (see
/// `net_inspector.rs`).
fn settle(app: &mut Signex, message: Message) {
    let task = app.update(message);
    let mut driver = TaskDriver::new();
    let mut queue = std::collections::VecDeque::from([task]);
    let mut steps = 0;
    while let Some(task) = queue.pop_front() {
        for message in driver.outputs(task).expect("the task chain settled") {
            steps += 1;
            assert!(steps < 1_000, "the task chain settles");
            if !matches!(message, Message::DockLayout(_)) {
                queue.push_back(app.update(message));
            }
        }
    }
}

fn panel(app: &mut Signex, msg: AnnotationsMsg) {
    settle(
        app,
        Message::Dock(DockMessage::Panel(PanelMsg::Annotations(msg))),
    );
}

fn view(app: &Signex) -> &AnnotationsView {
    &app.document_state.panel_ctx.annotations
}

/// `YYYY-MM-DD` `days_ago` days before today (UTC), as the panel counts.
fn day(days_ago: i64) -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("a clock after 1970")
        .as_secs();
    let today = i64::try_from(seconds / 86_400).expect("days fit");
    IsoDate::from_days_since_epoch(today - days_ago)
        .expect("a date")
        .to_string()
}

fn symbol(id: u128, reference: &str, x: f64) -> Symbol {
    Symbol {
        uuid: Uuid::from_u128(id),
        lib_id: "Device:R".into(),
        reference: reference.into(),
        position: Point::new(x, 0.0),
        ..Default::default()
    }
}

fn write_sheet(dir: &Path, name: &str, symbols: Vec<Symbol>) -> PathBuf {
    let sheet = SchematicSheet {
        uuid: Uuid::now_v7(),
        version: 1,
        generator: "signex-test".into(),
        generator_version: "0.9".into(),
        paper_size: "A4".into(),
        symbols,
        ..Default::default()
    };
    let path = dir.join(name);
    let text = signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .expect("encode");
    fs::write(&path, text).expect("write");
    path
}

fn write_notebooks(dir: &Path) {
    let folder = dir.join("notebook");
    fs::create_dir_all(&folder).expect("notebook folder");
    let bringup = format!(
        "+++\nids = {{ U3 = \"0190\" }}\n+++\n\
#annotation(kind: \"measurement\", status: \"done\", date: \"{}\", title: \"Output ripple\")[Ripple on #net(\"VOUT\"): 18 mV at #component(\"U1\").]\n\
\n\
#annotation(kind: \"issue\", status: \"open\", date: \"{}\")[U3 runs warm: #component(\"U3\") at 300 mA.]\n\
\n\
#annotation(kind: \"note\", date: \"{}\")[Rework: #component(\"R4\") replaced, #component(\"C7\") removed.]\n\
\n\
#annotation(kind: \"todo\")[Bad kind]\n",
        day(2),
        day(1),
        day(20)
    );
    fs::write(folder.join("bringup.snxnbk"), bringup).expect("write bringup");
    let notes = format!(
        "#annotation(kind: \"decision\", status: \"accepted\", date: \"{}\", title: \"Use the LDO\")[Chosen for #component(\"U3\").]\n\
#annotation(kind: \"calculation\", status: \"current\")[Divider for #component(\"U1\"): $ 0.8 (1 + 3.16) $]\n",
        day(5)
    );
    fs::write(folder.join("design-notes.snxnbk"), notes).expect("write notes");
}

/// The project, both sheets open with `a` focused; the notebook files are
/// written when `notebooks`.
fn open_project(dir: &Path, notebooks: bool) -> (Signex, PathBuf, PathBuf) {
    let a = write_sheet(
        dir,
        "a.snxsch",
        vec![symbol(U1, "U1", 0.0), symbol(R4, "R4", 20.0)],
    );
    let b = write_sheet(dir, "b.snxsch", vec![symbol(U3, "U3", 0.0)]);
    if notebooks {
        write_notebooks(dir);
    }
    let (mut app, _boot) = Signex::new();
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: dir.join("P.snxprj"),
        data: ProjectData {
            document_id: None,
            description: None,
            name: "P".into(),
            dir: dir.to_string_lossy().into_owned(),
            schematic_root: Some("a.snxsch".into()),
            pcb_file: None,
            sheets: ["a.snxsch", "b.snxsch"]
                .iter()
                .map(|f| SheetEntry {
                    name: (*f).into(),
                    filename: (*f).into(),
                    symbols_count: 0,
                    wires_count: 0,
                    labels_count: 0,
                    unknown: Default::default(),
                })
                .collect(),
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: false,
            net_classes: Vec::new(),
            rules: None,
            annotation: None,
            schematic_defaults: Default::default(),
            project_defaults: Default::default(),
            agent_journal: Default::default(),
            fab_outputs: None,
            requirements: Default::default(),
            parameters: Default::default(),
            unknown: Default::default(),
        },
        pending_libraries: HashMap::new(),
        case_fold: CaseFold::default(),
        case_probed: false,
        case_decided: false,
    });
    app.document_state.active_project = Some(id);
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    for path in [&b, &a] {
        let text = fs::read_to_string(path).expect("read");
        let sheet = signex_types::format::SnxSchematic::parse(&text)
            .expect("parse")
            .sheet;
        let _ = app.update(Message::File(FileMsg::SchematicOpenFinished {
            path: path.clone(),
            title: "t".into(),
            result: Ok(Box::new(sheet)),
            disk: None,
        }));
    }
    assert_eq!(app.document_state.active_path.as_deref(), Some(a.as_path()));
    (app, a, b)
}

/// The project with its notebooks and the panel opened from View.
fn opened() -> (TempDir, Signex, PathBuf) {
    let tmp = TempDir::new().expect("tempdir");
    let (mut app, _a, b) = open_project(tmp.path(), true);
    settle(&mut app, Message::Menu(MenuMessage::OpenAnnotationsPanel));
    (tmp, app, b)
}

fn titles(view: &AnnotationsView) -> Vec<(String, Vec<String>)> {
    view.groups
        .iter()
        .map(|g| {
            (
                g.label.clone(),
                g.rows.iter().map(|r| r.title.clone()).collect(),
            )
        })
        .collect()
}

#[test]
fn the_view_row_opens_the_panel_and_groups_each_files_blocks_newest_first() {
    let (_tmp, app, _b) = opened();
    assert!(
        app.document_state
            .dock
            .locate(PanelKind::Annotations)
            .is_some(),
        "the panel is docked"
    );
    let view = view(&app);
    assert_eq!(view.status, ListStatus::Ready);
    assert_eq!(
        titles(view),
        [
            (
                "notebook/bringup.snxnbk".to_string(),
                vec![
                    "U3 runs warm".to_string(),
                    "Output ripple".to_string(),
                    "Rework".to_string(),
                ]
            ),
            (
                "notebook/design-notes.snxnbk".to_string(),
                vec!["Use the LDO".to_string(), "Divider for U1".to_string()]
            ),
        ]
    );
    assert_eq!((view.shown, view.total, view.files), (5, 5, 2));
    let counts: Vec<(AnnotationKind, u32)> = view.kinds.iter().map(|k| (k.kind, k.count)).collect();
    assert_eq!(
        counts,
        AnnotationKind::ALL.map(|kind| (kind, 1)).to_vec(),
        "one block of each kind; the bad one is not counted"
    );
    let ripple = &view.groups[0].rows[1];
    assert_eq!(ripple.status, Some(Status::Done));
    assert_eq!(ripple.snippet, "Ripple on VOUT: 18 mV at U1.");
    assert_eq!(ripple.jump, Some(Reference::Component("U1".into())));
}

#[test]
fn what_a_notebook_could_not_be_read_for_reaches_messages_with_file_and_line() {
    let _ = signex_app::diagnostics::init_logging();
    let (_tmp, app, _b) = opened();
    let lines: Vec<String> = app
        .document_state
        .panel_ctx
        .diagnostics
        .iter()
        .map(|e| e.message.replace(['\u{2068}', '\u{2069}'], ""))
        .collect();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("notebook/bringup.snxnbk line 10") && l.contains("todo")),
        "the skipped block is reported: {lines:?}"
    );
}

#[test]
fn a_reference_to_a_part_the_design_lost_is_amber_and_the_others_are_not() {
    let (_tmp, app, _b) = opened();
    let rework = &view(&app).groups[0].rows[2];
    let gone: Vec<(String, bool)> = rework
        .chips
        .iter()
        .map(|c| (c.label.clone(), c.gone))
        .collect();
    assert_eq!(gone, [("R4".to_string(), false), ("C7".to_string(), true)]);
    let ripple = &view(&app).groups[0].rows[1];
    assert!(ripple.chips.iter().all(|c| !c.gone), "a net is never gone");
}

#[test]
fn the_kind_part_status_date_and_text_filters_narrow_the_list_and_the_counts_stay() {
    let (_tmp, mut app, _b) = opened();
    assert_eq!(view(&app).parts, ["C7", "R4", "U1", "U3"]);
    assert_eq!(
        view(&app).statuses,
        [
            Status::Open,
            Status::Accepted,
            Status::Done,
            Status::Current
        ]
    );

    panel(&mut app, AnnotationsMsg::KindToggled(AnnotationKind::Issue));
    let v = view(&app);
    assert_eq!((v.shown, v.total), (4, 5));
    assert!(
        !v.kinds
            .iter()
            .find(|k| k.kind == AnnotationKind::Issue)
            .expect("issue")
            .on
    );
    assert_eq!(
        v.kinds.iter().map(|k| k.count).sum::<u32>(),
        5,
        "counts ignore the filters"
    );
    panel(&mut app, AnnotationsMsg::KindToggled(AnnotationKind::Issue));
    assert_eq!(view(&app).shown, 5);

    panel(&mut app, AnnotationsMsg::PartPicked("U3".into()));
    let v = view(&app);
    assert_eq!(v.part.as_deref(), Some("U3"));
    assert_eq!(
        titles(v),
        [
            (
                "notebook/bringup.snxnbk".to_string(),
                vec!["U3 runs warm".to_string()]
            ),
            (
                "notebook/design-notes.snxnbk".to_string(),
                vec!["Use the LDO".to_string()]
            ),
        ]
    );
    panel(&mut app, AnnotationsMsg::PartCleared);

    panel(&mut app, AnnotationsMsg::StatusPicked(Status::Open));
    assert_eq!(view(&app).shown, 1);
    panel(&mut app, AnnotationsMsg::StatusCleared);

    panel(&mut app, AnnotationsMsg::DatePicked(DateWindow::Last7Days));
    assert_eq!(view(&app).shown, 3, "1, 2 and 5 days old");
    panel(&mut app, AnnotationsMsg::DatePicked(DateWindow::Last30Days));
    assert_eq!(view(&app).shown, 4, "the undated block stays out");
    panel(&mut app, AnnotationsMsg::DatePicked(DateWindow::Any));
    assert_eq!(view(&app).shown, 5);

    panel(&mut app, AnnotationsMsg::QueryChanged("RIPPLE".into()));
    assert_eq!(view(&app).shown, 1);
    panel(
        &mut app,
        AnnotationsMsg::QueryChanged("nothing like it".into()),
    );
    let v = view(&app);
    assert!(
        v.groups.is_empty() && v.total == 5,
        "no match, the total stays"
    );
}

#[test]
fn a_row_click_shows_its_source_and_a_second_click_folds_it() {
    let (_tmp, mut app, _b) = opened();
    let key = view(&app).groups[1].rows[0].key.clone();
    panel(&mut app, AnnotationsMsg::RowToggled(key.clone()));
    let row = &view(&app).groups[1].rows[0];
    assert!(row.expanded);
    assert_eq!(
        (row.file_name.as_str(), row.line),
        ("design-notes.snxnbk", 1)
    );
    assert!(row.source.starts_with("#annotation(kind: \"decision\""));
    assert!(row.source.ends_with("#component(\"U3\").]"));
    panel(&mut app, AnnotationsMsg::RowToggled(key));
    assert!(!view(&app).groups[1].rows[0].expanded);
}

#[test]
fn a_part_chip_opens_the_sheet_holding_the_part_and_selects_it() {
    let (_tmp, mut app, b) = opened();
    panel(
        &mut app,
        AnnotationsMsg::ReferenceClicked(Reference::Component("U3".into())),
    );
    assert_eq!(app.document_state.active_path.as_deref(), Some(b.as_path()));
    assert_eq!(
        app.interaction_state.active_canvas().selected,
        vec![SelectedItem::new(Uuid::from_u128(U3), SelectedKind::Symbol)]
    );
}

#[test]
fn a_gone_part_chip_goes_nowhere() {
    let (_tmp, mut app, _b) = opened();
    let before = app.document_state.active_path.clone();
    panel(
        &mut app,
        AnnotationsMsg::ReferenceClicked(Reference::Component("C7".into())),
    );
    assert_eq!(app.document_state.active_path, before);
    assert!(app.interaction_state.active_canvas().selected.is_empty());
}

#[test]
fn a_project_without_notebook_files_shows_the_empty_state() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut app, _a, _b) = open_project(tmp.path(), false);
    settle(&mut app, Message::Menu(MenuMessage::OpenAnnotationsPanel));
    assert_eq!(view(&app).status, ListStatus::NoNotebook, "no folder");

    fs::create_dir_all(tmp.path().join("notebook")).expect("folder");
    fs::write(
        tmp.path().join("notebook").join("readme.txt"),
        "not a notebook",
    )
    .expect("write");
    panel(&mut app, AnnotationsMsg::RefreshRequested);
    assert_eq!(view(&app).status, ListStatus::NoNotebook, "no .snxnbk file");

    write_notebooks(tmp.path());
    panel(&mut app, AnnotationsMsg::RefreshRequested);
    assert_eq!(view(&app).status, ListStatus::Ready, "Refresh reads them");
    assert_eq!(view(&app).total, 5);
}

#[test]
fn the_panel_list_shows_annotations_under_project() {
    let grouped = grouped_panels(|_| true);
    let project = grouped
        .iter()
        .find(|(group, _)| *group == PanelGroup::Project)
        .map(|(_, kinds)| kinds.clone())
        .expect("the Project group");
    let at = |kind| project.iter().position(|&k| k == kind);
    assert_eq!(
        at(PanelKind::Annotations),
        at(PanelKind::Navigator).map(|i| i + 1),
        "right after Navigator"
    );
    assert_eq!(PanelKind::Annotations.label(), "Annotations");
}
