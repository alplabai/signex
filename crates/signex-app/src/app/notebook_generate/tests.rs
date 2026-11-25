//! #2945 (mockup row 201): the options dialog's state and events, the
//! operands read into options, and the answer of a finished run. The runs on
//! a project on disk are in `app/command/notebook_generate_tests.rs`.

use std::path::PathBuf;

use signex_notebook::generate::{ChecksSource, GenerateOptions, Section, Template, generate};

use super::request::options_from_args;
use super::update::answer;
use super::{GenerateDialog, NotebookGenerateMsg};
use crate::app::Signex;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::keymap::AppCommandId;

fn dialog() -> GenerateDialog {
    GenerateDialog {
        project: PathBuf::from("/work/buck-3v3/buck-3v3.snxprj"),
        project_file: "buck-3v3.snxprj".into(),
        revision: Some("B".into()),
        requirements: 6,
        variants: vec!["Base".into(), "Lite".into()],
        variant: "Base".into(),
        options: GenerateOptions::default(),
        document_number: String::new(),
        document_revision: String::new(),
        running: false,
        problem: None,
    }
}

fn app_with(dialog: GenerateDialog) -> Signex {
    let (mut app, _boot) = Signex::new();
    app.ui_state.notebook_generate.dialog = Some(dialog);
    app
}

fn open(app: &Signex) -> &GenerateDialog {
    app.ui_state
        .notebook_generate
        .dialog
        .as_ref()
        .expect("open")
}

#[test]
fn the_dialogs_request_is_never_saved_and_carries_the_typed_numbers() {
    let mut d = dialog();
    d.document_number = " DN-0043 ".into();
    d.document_revision = "A".into();
    d.variant = "Lite".into();
    let request = d.request();
    assert!(!request.save && !request.update);
    assert_eq!(request.options.document.as_deref(), Some("DN-0043"));
    assert_eq!(request.options.revision.as_deref(), Some("A"));
    assert_eq!(request.variant.as_deref(), Some("Lite"));
    assert_eq!(request.options.sections, Section::ALL.to_vec());
    assert_eq!(dialog().request().options.document, None);
}

#[test]
fn generate_needs_a_section_and_no_run_under_way() {
    let mut d = dialog();
    assert!(d.can_generate());
    d.running = true;
    assert!(!d.can_generate());
    d.running = false;
    d.options.sections.clear();
    assert!(!d.can_generate());
}

#[test]
fn the_dialogs_events_change_its_options() {
    let mut app = app_with(dialog());
    let events = [
        NotebookGenerateMsg::SectionToggled(Section::Bom),
        NotebookGenerateMsg::VariantPicked("Lite".into()),
        NotebookGenerateMsg::TemplatePicked(Template::DesignReportA4),
        NotebookGenerateMsg::DocumentNumberEdited("DN-1".into()),
        NotebookGenerateMsg::RevisionEdited("C".into()),
        NotebookGenerateMsg::ChecksSourcePicked(ChecksSource::LastResults),
    ];
    for event in events {
        let _ = app.update(event.into());
    }
    let d = open(&app);
    assert!(!d.options.has(Section::Bom));
    assert_eq!(d.variant, "Lite");
    assert_eq!(
        (d.document_number.as_str(), d.document_revision.as_str()),
        ("DN-1", "C")
    );
    assert_eq!(d.options.checks, ChecksSource::LastResults);
}

#[test]
fn a_run_under_way_keeps_its_options_and_cancel_closes() {
    let mut d = dialog();
    d.running = true;
    let mut app = app_with(d);
    let _ = app.update(NotebookGenerateMsg::SectionToggled(Section::Bom).into());
    assert!(open(&app).options.has(Section::Bom));
    let _ = app.update(NotebookGenerateMsg::DialogCancelled.into());
    assert!(app.ui_state.notebook_generate.dialog.is_none());
}

#[test]
fn an_edit_clears_the_last_problem() {
    let mut d = dialog();
    d.problem = Some("no root sheet".into());
    let mut app = app_with(d);
    let _ = app.update(NotebookGenerateMsg::RevisionEdited("D".into()).into());
    assert_eq!(open(&app).problem, None);
}

#[test]
fn the_dialog_does_not_open_without_a_project_in_view() {
    let (mut app, _boot) = Signex::new();
    let _ = app.update(NotebookGenerateMsg::DialogOpened.into());
    assert!(app.ui_state.notebook_generate.dialog.is_none());
}

fn args(operands: &[&str]) -> CommandArgs {
    let id = AppCommandId::new("generate_design_notebook").expect("a catalog id");
    CommandArgs::parse_operands(&id, operands).expect("operands parse")
}

#[test]
fn the_operands_read_into_options() {
    let options = options_from_args(&args(&[
        "sections=checks,summary",
        "template=design-report-a4",
        "checks=last",
        "document_number=DN-0043",
    ]))
    .expect("options");
    assert_eq!(options.sections, [Section::Summary, Section::Checks]);
    assert_eq!(options.checks, ChecksSource::LastResults);
    assert_eq!(options.document.as_deref(), Some("DN-0043"));
    assert_eq!(options.revision, None);
    assert_eq!(
        options_from_args(&args(&[])),
        Ok(GenerateOptions::default())
    );
    assert!(options_from_args(&args(&["template=letter"])).is_err());
}

#[test]
fn a_finished_run_answers_with_its_summary_or_its_refusal() {
    let facts = signex_notebook_facts();
    let notebook = generate(&facts, &GenerateOptions::default());
    let expected = serde_json::to_value(&notebook.summary).unwrap();
    let done = super::GenerateOutcome {
        notebook,
        ran: Vec::new(),
    };
    match answer(Ok(done)) {
        CommandOutcome::Answered(value) => assert_eq!(value, expected),
        other => panic!("not answered: {other:?}"),
    }
    assert!(matches!(
        answer(Err("no project".into())),
        CommandOutcome::Disabled { .. }
    ));
}

/// The smallest facts a report is built from: nothing read.
fn signex_notebook_facts() -> signex_notebook::generate::ProjectFacts {
    use signex_notebook::generate::{Fact, ProjectFacts};
    let none = || Fact::not_read("not in this test");
    ProjectFacts {
        project_file: "p.snxprj".into(),
        name: "p".into(),
        revision: None,
        variant: "Base".into(),
        sheets: Vec::new(),
        power_nets: Vec::new(),
        requirements: Vec::new(),
        blocks: none(),
        requirement_check: none(),
        erc: none(),
        drc: none(),
        bom: none(),
        simulation: none(),
        outputs: none(),
        changes: none(),
        stale_references: 0,
    }
}
