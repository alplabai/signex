//! #1283 — snippets and templates through the app: the headless path the CLI
//! and an agent take (save, list, place, a template for a new project), and
//! the GUI path (Save as Snippet on the selection, Place Snippet's dialog).

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use signex_i18n::RenderTranslatable;
use signex_types::schematic::{Point, SchematicSheet, SelectedItem, SelectedKind, Symbol, Wire};
use uuid::Uuid;

use super::{ListAnswer, listing_answer};
use crate::app::command::{CommandArgs, CommandOutcome, translator};
use crate::app::handlers::document_files::new_project::{
    NewProjectLayout, NewProjectPlan, RuleSeed, write_new_project_files_with,
};
use crate::app::handlers::editing_commands::net_color_driver::add_sheet_with;
use crate::app::handlers::menu::export::settle;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{EditMsg, Message, PlacementExtrasMsg, Signex};
use crate::keymap::AppCommandId;
use signex_types::reuse::{ReuseEntry, ReuseKind, ReuseListing, ReuseScope};

const PART: Uuid = Uuid::from_u128(0x51);
const WIRE: Uuid = Uuid::from_u128(0x52);

fn sheet() -> SchematicSheet {
    SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".to_string(),
        symbols: vec![Symbol {
            uuid: PART,
            lib_id: "Device:R".to_string(),
            reference: "R7".to_string(),
            position: Point::new(25.4, 25.4),
            ..Default::default()
        }],
        wires: vec![Wire {
            uuid: WIRE,
            start: Point::new(25.4, 25.4),
            end: Point::new(38.1, 25.4),
            ..Default::default()
        }],
        ..Default::default()
    }
}

/// A project in `dir` with `main.snxsch` open in it.
fn app_in(dir: &Path) -> Signex {
    let mut app = app_workspace(&dir.to_string_lossy(), &["main.snxsch"]);
    app.ui_state.first_run_tour_open = false;
    app.ui_state.snap_enabled = true;
    app.ui_state.grid_size_mm = 2.54;
    add_sheet_with(&mut app, &dir.join("main.snxsch"), sheet());
    app
}

/// `command` with `operands`, as a headless run gives it, settled.
fn run(app: &mut Signex, command: &str, operands: &[String]) -> CommandOutcome {
    let id = AppCommandId::new(command).expect("a catalog id");
    let refs: Vec<&str> = operands.iter().map(String::as_str).collect();
    let args = if refs.is_empty() {
        CommandArgs::none()
    } else {
        CommandArgs::parse_operands(&id, &refs).expect("operands")
    };
    let (outcome, task) = app.run_headless_command_in_test(&id, args);
    settle(app, task);
    outcome
}

fn answered(outcome: CommandOutcome) -> Value {
    match outcome {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    }
}

fn refusal(outcome: CommandOutcome) -> String {
    match outcome {
        CommandOutcome::Disabled { reason } => reason.render_en_us(),
        other => panic!("not refused: {other:?}"),
    }
}

fn parts_on_sheet(app: &Signex) -> usize {
    app.document_state
        .active_engine()
        .expect("engine")
        .document()
        .symbols
        .len()
}

fn ids() -> String {
    format!("ids={PART},{WIRE}")
}

#[test]
fn a_snippet_is_saved_listed_and_placed_as_one_undo_step() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_in(dir.path());
    let file = dir.path().join("snippets").join("divider.snxsch");

    let saved = answered(run(
        &mut app,
        "save_snippet",
        &["name=divider".into(), ids()],
    ));
    assert_eq!(saved["name"], "divider");
    assert_eq!(saved["scope"], "project");
    assert_eq!(saved["items"], 2);
    assert_eq!(saved["file"], json!(file));
    let text = std::fs::read_to_string(&file).expect("the snippet file");
    let stored = signex_types::format::SnxSchematic::parse(&text)
        .expect("a sheet")
        .sheet;
    assert_eq!(stored.symbols.len(), 1);
    assert_eq!(stored.wires.len(), 1);

    let listed = answered(run(&mut app, "list_snippets", &[]));
    let snippets = listed["snippets"].as_array().expect("a list");
    assert!(
        snippets
            .iter()
            .any(|s| s["name"] == "divider" && s["scope"] == "project"),
        "{listed}"
    );
    assert_eq!(listed["folders"]["project"], json!(dir.path()));

    let placed = answered(run(
        &mut app,
        "place_snippet",
        &["name=divider".into(), "at=50.8,50.8".into()],
    ));
    assert_eq!(placed["placed"], 2);
    assert_eq!(placed["scope"], "project");
    assert_eq!(parts_on_sheet(&app), 2);
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(parts_on_sheet(&app), 1, "one undo step takes it away");
}

#[test]
fn a_snippet_of_a_taken_name_is_replaced_only_with_overwrite() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_in(dir.path());
    let file = dir.path().join("snippets").join("pair.snxsch");
    answered(run(&mut app, "save_snippet", &["name=pair".into(), ids()]));
    let first = std::fs::read(&file).expect("first write");

    let why = refusal(run(
        &mut app,
        "save_snippet",
        &["name=pair".into(), format!("ids={WIRE}")],
    ));
    assert!(why.contains("overwrite=true"), "{why}");
    assert_eq!(
        std::fs::read(&file).expect("kept"),
        first,
        "the file is kept"
    );

    let again = answered(run(
        &mut app,
        "save_snippet",
        &[
            "name=pair".into(),
            format!("ids={WIRE}"),
            "overwrite=true".into(),
        ],
    ));
    assert_eq!(again["items"], 1);
    assert_ne!(std::fs::read(&file).expect("replaced"), first);
}

#[test]
fn snippet_commands_refuse_what_they_cannot_name() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_in(dir.path());
    let why = refusal(run(&mut app, "save_snippet", &["name=../up".into(), ids()]));
    assert!(why.contains("cannot name a snippet"), "{why}");
    let why = refusal(run(
        &mut app,
        "save_snippet",
        &["name=x".into(), "scope=team".into(), ids()],
    ));
    assert!(why.contains("`project` or `library`"), "{why}");
    let why = refusal(run(
        &mut app,
        "place_snippet",
        &["name=missing".into(), "at=50.8,50.8".into()],
    ));
    assert!(why.contains("no snippet named missing"), "{why}");
    assert_eq!(parts_on_sheet(&app), 1, "nothing was placed");
    let why = refusal(run(&mut app, "list_templates", &["kind=board".into()]));
    assert!(why.contains("`sheet` or `project`"), "{why}");
}

/// The GUI's Save as Snippet: the selection, named after its designators,
/// saved through the operand handler.
#[test]
fn save_as_snippet_saves_the_selection_named_after_its_parts() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_in(dir.path());
    app.interaction_state.active_canvas_mut().selected = vec![
        SelectedItem::new(PART, SelectedKind::Symbol),
        SelectedItem::new(WIRE, SelectedKind::Wire),
    ];
    let task = app.update(Message::Edit(EditMsg::Extras(
        PlacementExtrasMsg::SnippetSaveRequested,
    )));
    settle(&mut app, task);
    assert!(translator::ran("save_snippet"));
    assert!(dir.path().join("snippets").join("R7.snxsch").is_file());
}

#[test]
fn save_as_snippet_with_nothing_selected_writes_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_in(dir.path());
    let task = app.update(Message::Edit(EditMsg::Extras(
        PlacementExtrasMsg::SnippetSaveRequested,
    )));
    settle(&mut app, task);
    assert!(!dir.path().join("snippets").exists());
}

/// The menu, key and palette rows reach the slice's own messages.
#[test]
fn the_gui_rows_open_the_snippet_dialog_and_save_the_selection() {
    let bridged = |id: &str| {
        let command = AppCommandId::new(id).expect("an id");
        crate::app::command::bridge::core_to_message(&command, &CommandArgs::none())
    };
    assert!(matches!(
        bridged("place_snippet"),
        Some(Message::Edit(EditMsg::Extras(
            PlacementExtrasMsg::SnippetRequested
        )))
    ));
    assert!(matches!(
        bridged("save_snippet"),
        Some(Message::Edit(EditMsg::Extras(
            PlacementExtrasMsg::SnippetSaveRequested
        )))
    ));
}

#[test]
fn a_template_listing_names_each_kind_and_scope() {
    let entry = |name: &str, kind, scope| ReuseEntry {
        name: name.into(),
        kind,
        scope,
        file: PathBuf::from(name),
    };
    let listing = ReuseListing {
        entries: vec![
            entry("A4", ReuseKind::SheetTemplate, ReuseScope::Project),
            entry("power", ReuseKind::ProjectTemplate, ReuseScope::Library),
        ],
        unreadable: vec!["x: denied".into()],
    };
    let folders = [(ReuseScope::Library, PathBuf::from("lib"))];
    let answer = listing_answer(ListAnswer::Templates, &listing, &folders);
    assert_eq!(
        answer,
        json!({
            "templates": [
                {"name": "A4", "kind": "sheet", "scope": "project", "file": "A4"},
                {"name": "power", "kind": "project", "scope": "library", "file": "power"},
            ],
            "unreadable": ["x: denied"],
            "folders": {"project": null, "library": "lib"},
        })
    );
    let snippets = listing_answer(ListAnswer::Snippets, &listing, &folders);
    assert_eq!(snippets["snippets"], json!([]));
}

#[test]
fn list_templates_reads_the_projects_sheet_templates() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_in(dir.path());
    let folder = ReuseKind::SheetTemplate.folder(dir.path());
    std::fs::create_dir_all(&folder).expect("folder");
    std::fs::write(folder.join("A4 frame.snxsch"), "").expect("template");
    let listed = answered(run(&mut app, "list_templates", &["kind=sheet".into()]));
    let templates = listed["templates"].as_array().expect("a list");
    assert!(
        templates
            .iter()
            .any(|t| t["name"] == "A4 frame" && t["kind"] == "sheet" && t["scope"] == "project"),
        "{listed}"
    );
    assert!(templates.iter().all(|t| t["kind"] == "sheet"));
}

/// A project template: its settings and its first sheet's frame go to the
/// new project; a template that cannot be read makes nothing.
#[test]
fn a_new_project_from_a_template_takes_its_settings_and_frame() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let template = tmp.path().join("power");
    std::fs::create_dir_all(&template).expect("template folder");
    let file = template.join("power.snxprj");
    let mut data = signex_types::project::parse_project_bytes(
        &file,
        b"",
        signex_types::project::files_on_disk(&file),
    )
    .expect("blank");
    data.parameters
        .insert("company".to_string(), "Alp Lab".to_string());
    data.schematic_root = Some("power.snxsch".to_string());
    signex_types::project::write_project(&file, &data).expect("template project");
    let mut frame = SchematicSheet {
        uuid: Uuid::now_v7(),
        version: 1,
        paper_size: "A2".to_string(),
        ..Default::default()
    };
    frame
        .title_block
        .insert("title".to_string(), "Power stage".to_string());
    let text = signex_types::format::SnxSchematic::new(frame)
        .write_string()
        .expect("encodes");
    std::fs::write(template.join("power.snxsch"), text).expect("template sheet");

    let seed = RuleSeed {
        dir: Some(tmp.path().join("no-library-yet")),
        ..RuleSeed::default()
    };
    let plan = NewProjectPlan {
        layout: NewProjectLayout::NamedFolder,
        setup: None,
        template: Some(template.clone()),
    };
    let written = write_new_project_files_with(&tmp.path().join("Demo.snxprj"), &seed, &plan)
        .expect("the project is made");
    let made = signex_types::project::parse_project(&written).expect("reads");
    assert_eq!(made.parameters["company"], "Alp Lab");
    let sheet_file = written.with_extension("snxsch");
    let text = std::fs::read_to_string(&sheet_file).expect("the first sheet");
    let sheet = signex_types::format::SnxSchematic::parse(&text)
        .expect("a sheet")
        .sheet;
    assert_eq!(sheet.paper_size, "A2");
    assert_eq!(sheet.title_block["title"], "Power stage");

    let missing = NewProjectPlan {
        template: Some(tmp.path().join("none")),
        ..plan
    };
    let other = tmp.path().join("Other.snxprj");
    assert!(write_new_project_files_with(&other, &seed, &missing).is_err());
    assert!(!tmp.path().join("Other").exists(), "nothing is made");
}
