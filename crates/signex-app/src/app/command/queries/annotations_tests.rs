//! `list_annotations` (mockup row 198): the project's notebook blocks as
//! review records cited by typed id, read off the UI thread and answered
//! once read, narrowed by id, kind or part.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use signex_i18n::RenderTranslatable;

use super::annotations::{AnnotationQuery, annotation_query, annotations_answer};
use crate::app::Signex;
use crate::app::command::project_name_tests::project_app;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::keymap::AppCommandId;
use crate::panels::annotations::{FileReadError, NotebookFile, NotebookScan};

const DESIGN: &str = "+++\nformat = \"snxnbk/1\"\n+++\n= Notebook\n\
#measurement(id: \"MEAS-010\", target: net(\"VIN\"), value: 310, unit: \"mV\", limit: (max: 250))\n\
#decision(id: \"DEC-002\", status: \"accepted\", target: component(\"L1\"), supersedes: \"DEC-001\")[Use A.]\n\
#include \"chapters/power.snxnbk\"\n";

const CHAPTER: &str = "#issue(id: \"ISS-004\", status: \"open\", from: \"MEAS-3.2\", target: component(\"R7\"))[Above +2 %.]\n\
#measurement(id: \"MEAS-3.2\", rows: ((net(\"VBUS\"), 5.0, 5%, 4.78),))\n\
#note(id: \"DEC-9\")[A wrong id.]\n";

fn write_notebook(dir: &tempfile::TempDir) {
    let folder = dir.path().join("notebook");
    std::fs::create_dir_all(folder.join("chapters")).expect("folders");
    std::fs::write(folder.join("design.snxnbk"), DESIGN).expect("notebook");
    std::fs::write(folder.join("chapters/power.snxnbk"), CHAPTER).expect("chapter");
}

fn project_path(app: &Signex) -> PathBuf {
    app.document_state.projects[0].path.clone()
}

fn list(app: &mut Signex, document: &Path, extra: &[&str]) -> CommandOutcome {
    let id = AppCommandId::new("list_annotations").expect("a catalog id");
    let document = format!("document={}", document.display());
    let mut operands = vec![document.as_str()];
    operands.extend_from_slice(extra);
    let args = CommandArgs::parse_operands(&id, &operands).expect("operands parse");
    match app.run_query(&id, &args).expect("a query") {
        (CommandOutcome::Deferred { ticket }, task) => app.settle_deferred(ticket, task).0,
        (outcome, _) => panic!("answered before the files were read: {outcome:?}"),
    }
}

fn answer(app: &mut Signex, document: &Path, extra: &[&str]) -> Value {
    match list(app, document, extra) {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    }
}

fn ids(value: &Value) -> Vec<String> {
    value["files"]
        .as_array()
        .expect("files")
        .iter()
        .flat_map(|file| file["blocks"].as_array().expect("blocks").iter())
        .map(|block| block["id"].as_str().unwrap_or("-").to_string())
        .collect()
}

fn args(operands: &[&str]) -> CommandArgs {
    let id = AppCommandId::new("list_annotations").expect("a catalog id");
    CommandArgs::parse_operands(&id, operands).expect("operands parse")
}

#[test]
fn every_block_of_the_notebook_and_its_chapters_is_listed_by_typed_id() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    write_notebook(&dir);
    let project = project_path(&app);
    let value = answer(&mut app, &project, &[]);
    assert_eq!(value["notebook_found"], true);
    assert_eq!(value["count"], 5);
    let files: Vec<&str> = value["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["file"].as_str().unwrap())
        .collect();
    assert_eq!(
        files,
        ["notebook/chapters/power.snxnbk", "notebook/design.snxnbk"],
        "a chapter in a subfolder is read too"
    );
    assert_eq!(
        ids(&value),
        ["ISS-004", "MEAS-3.2", "-", "MEAS-010", "DEC-002"]
    );
    let measurement = &value["files"][0]["blocks"][1];
    assert_eq!(
        measurement["cited_by"],
        json!([{"id": "ISS-004", "citation": "from"}]),
        "MEAS-3.2 opens ISS-004, from another file"
    );
    assert_eq!(measurement["record"]["verdict"], "warn");
    assert_eq!(value["unopened_failures"], json!(["MEAS-010"]));
    let problem = &value["files"][0]["problems"][0];
    assert_eq!(problem["problem"], "bad_id");
    assert_eq!(problem["id"], "DEC-9");
    assert_eq!(problem["line"], 3);
}

#[test]
fn the_list_narrows_to_one_id_one_kind_or_one_part() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    write_notebook(&dir);
    let project = project_path(&app);
    assert_eq!(
        ids(&answer(&mut app, &project, &["id=DEC-002"])),
        ["DEC-002"]
    );
    assert_eq!(
        ids(&answer(&mut app, &project, &["kind=measurement"])),
        ["MEAS-3.2", "MEAS-010"]
    );
    assert_eq!(ids(&answer(&mut app, &project, &["part=R7"])), ["ISS-004"]);
}

#[test]
fn a_sheet_of_the_project_names_its_notebook() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    write_notebook(&dir);
    let sheet = dir.path().join("top.snxsch");
    assert_eq!(answer(&mut app, &sheet, &[])["count"], 5);
}

#[test]
fn a_project_without_a_notebook_answers_with_none() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let project = project_path(&app);
    let value = answer(&mut app, &project, &[]);
    assert_eq!(value["notebook_found"], false);
    assert_eq!(value["count"], 0);
    assert_eq!(value["files"], json!([]));
}

#[test]
fn a_bad_filter_is_refused_before_anything_is_read() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let project = project_path(&app);
    let id = AppCommandId::new("list_annotations").expect("a catalog id");
    for (operand, said) in [
        ("id=DEC", "`id` is a block id"),
        ("kind=memo", "`kind` is note, calculation"),
    ] {
        let document = format!("document={}", project.display());
        let args = CommandArgs::parse_operands(&id, &[document.as_str(), operand]).unwrap();
        let (outcome, _) = app.run_query(&id, &args).expect("a query");
        let CommandOutcome::Disabled { reason } = outcome else {
            panic!("{operand}: not refused: {outcome:?}");
        };
        assert!(reason.render_en_us().contains(said), "{operand}");
    }
}

#[test]
fn the_query_reads_its_filters() {
    let query = annotation_query(&args(&[
        "document=/p.snxprj",
        "id=CALC-2.1",
        "kind=issue",
        "part=U3",
    ]))
    .expect("a query");
    let id = query.id.as_ref().map(ToString::to_string);
    assert_eq!(id.as_deref(), Some("CALC-2.1"));
    assert_eq!(query.filter.part.as_deref(), Some("U3"));
    let hidden = &query.filter.hidden_kinds;
    assert_eq!(hidden.len(), 4, "every kind but issue");
    assert!(!hidden.contains(&signex_notebook::AnnotationKind::Issue));
    let none = annotation_query(&args(&["document=/p.snxprj"])).expect("a query");
    assert_eq!(none, AnnotationQuery::default(), "no filter, none set");
}

#[test]
fn a_file_that_could_not_be_read_says_why() {
    let scan = NotebookScan {
        folder_found: true,
        files: vec![NotebookFile {
            path: PathBuf::from("/p/notebook/a.snxnbk"),
            label: "notebook/a.snxnbk".into(),
            annotations: Vec::new(),
            problems: Vec::new(),
            read_error: Some(FileReadError::NotUtf8),
        }],
        unlisted: vec!["denied".into()],
        parts: None,
    };
    let value = annotations_answer(Path::new("/p/p.snxprj"), &AnnotationQuery::default(), &scan);
    assert_eq!(value["files"][0]["read_error"], "not_utf8");
    assert_eq!(value["unlisted"], json!(["denied"]));
    assert_eq!(value["links"], json!([]));
}
