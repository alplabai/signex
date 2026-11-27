//! #1299: a Gerber output's settings (units, format, zeros, layers and
//! apertures) on both paths. The commands write a job no tab holds and
//! answer with the output's settings; the tab's fields edit the draft,
//! storing a setting picked back to its default as left out, so the job
//! file names only what differs from the writer's own settings; the form
//! shows the fields.

use std::path::Path;

use iced::Size;
use signex_types::output_job::{
    GerberApertures, GerberFormat, GerberSettings, GerberUnits, GerberZeros, JobOutput, OutputJob,
    OutputKind, read_output_job,
};

use super::pipeline_tests::{profile_job, write_job};
use super::tab::{GerberEdit, JobTabMsg};
use super::tests::{headless, project_app, refusal};
use crate::app::command::CommandOutcome;
use crate::app::handlers::menu::export::settle;
use crate::app::{Message, Signex};
use crate::layout_probe::lay_out_texts;

/// The sample job with a Gerber output named "Gerber".
fn gerber_job() -> OutputJob {
    let mut job = profile_job();
    job.outputs
        .push(JobOutput::new("Gerber", OutputKind::Gerber));
    job
}

fn answered(outcome: CommandOutcome) -> serde_json::Value {
    match outcome {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    }
}

fn run(app: &mut Signex, id: &str, job: &Path, operands: &[&str]) -> CommandOutcome {
    let path = format!("path={}", job.display());
    let mut all = vec![path.as_str()];
    all.extend_from_slice(operands);
    headless(app, id, &all)
}

fn gerber_of(job: &Path) -> JobOutput {
    read_output_job(job)
        .expect("the job file")
        .output("Gerber")
        .expect("the Gerber output")
        .clone()
}

#[test]
fn each_gerber_setting_is_a_command_that_writes_the_job_file() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    let job = write_job(dir.path(), &gerber_job());
    let answer = answered(run(
        &mut app,
        "set_job_gerber_units",
        &job,
        &["name=Gerber", "units=inch"],
    ));
    assert_eq!(answer["saved"], true);
    assert_eq!(answer["result"]["units"], "inch");
    assert_eq!(answer["result"]["format"], "4:6", "the others stay");
    for (id, operand) in [
        ("set_job_gerber_format", "format=2:5"),
        ("set_job_gerber_zeros", "zeros=trailing"),
        ("set_job_gerber_layers", "layers=copper, profile"),
        ("set_job_gerber_apertures", "apertures=macros"),
    ] {
        answered(run(&mut app, id, &job, &["name=Gerber", operand]));
    }
    let settings = gerber_of(&job).gerber_settings();
    assert_eq!(
        settings,
        GerberSettings {
            units: GerberUnits::Inch,
            format: GerberFormat {
                integer: 2,
                decimal: 5
            },
            zeros: GerberZeros::Trailing,
            layers: Some(vec!["copper".into(), "profile".into()]),
            apertures: GerberApertures::Macros,
        }
    );
    let all = answered(run(
        &mut app,
        "set_job_gerber_layers",
        &job,
        &["name=Gerber", "layers=all"],
    ));
    assert_eq!(all["result"]["layers"], "all");
    assert_eq!(gerber_of(&job).gerber_layers, None);
}

#[test]
fn a_bad_value_or_an_output_that_is_not_gerber_is_refused_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    let job = write_job(dir.path(), &gerber_job());
    let before = std::fs::read(&job).unwrap();
    for (id, operand, word) in [
        ("set_job_gerber_units", "units=mil", "`units`"),
        ("set_job_gerber_format", "format=9:9", "`format`"),
        ("set_job_gerber_zeros", "zeros=some", "`zeros`"),
        (
            "set_job_gerber_layers",
            "layers=Top Copper",
            "Gerber layer id",
        ),
        ("set_job_gerber_apertures", "apertures=round", "`apertures`"),
    ] {
        let outcome = run(&mut app, id, &job, &["name=Gerber", operand]);
        assert!(refusal(&outcome).contains(word), "{id}: {outcome:?}");
    }
    let pdf = run(
        &mut app,
        "set_job_gerber_units",
        &job,
        &["name=PDF", "units=inch"],
    );
    assert!(refusal(&pdf).contains("not a Gerber output"), "{pdf:?}");
    let none = run(
        &mut app,
        "set_job_gerber_units",
        &job,
        &["name=Nope", "units=inch"],
    );
    assert!(refusal(&none).contains("no output named"), "{none:?}");
    assert_eq!(
        std::fs::read(&job).unwrap(),
        before,
        "the file is as it was"
    );
}

fn send(app: &mut Signex, job: &Path, edit: GerberEdit) {
    let task = app.update(Message::from(JobTabMsg::GerberEdited {
        job: job.to_path_buf(),
        name: "Gerber".into(),
        edit,
    }));
    settle(app, task);
}

fn draft_gerber(app: &Signex, job: &Path) -> JobOutput {
    app.document_state.output_jobs[job]
        .output("Gerber")
        .expect("the Gerber output")
        .clone()
}

#[test]
fn the_tab_fields_edit_the_draft_and_a_default_is_left_out() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    let job = write_job(dir.path(), &gerber_job());
    let task = app.open_output_job_tab(job.clone());
    settle(&mut app, task);
    send(&mut app, &job, GerberEdit::UnitsPicked(GerberUnits::Inch));
    assert_eq!(
        draft_gerber(&app, &job).gerber_units,
        Some(GerberUnits::Inch)
    );
    assert!(app.document_state.is_document_dirty(&job), "unsaved");
    send(&mut app, &job, GerberEdit::UnitsPicked(GerberUnits::Mm));
    assert_eq!(draft_gerber(&app, &job).gerber_units, None, "the default");

    let format = GerberFormat {
        integer: 2,
        decimal: 4,
    };
    send(&mut app, &job, GerberEdit::FormatPicked(format));
    send(&mut app, &job, GerberEdit::ZerosPicked(GerberZeros::None));
    send(
        &mut app,
        &job,
        GerberEdit::AperturesPicked(GerberApertures::Macros),
    );
    let output = draft_gerber(&app, &job);
    assert_eq!(output.gerber_format, Some(format));
    assert_eq!(output.gerber_zeros, Some(GerberZeros::None));
    assert_eq!(output.gerber_apertures, Some(GerberApertures::Macros));

    // Every layer is unticked into a list of all copper; a layer tick adds
    // to it; unticking the last one is every layer again.
    send(&mut app, &job, GerberEdit::LayerToggled(None));
    assert_eq!(
        draft_gerber(&app, &job).gerber_layers,
        Some(vec!["copper".to_string()])
    );
    send(
        &mut app,
        &job,
        GerberEdit::LayerToggled(Some("profile".into())),
    );
    send(
        &mut app,
        &job,
        GerberEdit::LayerToggled(Some("copper".into())),
    );
    assert_eq!(
        draft_gerber(&app, &job).gerber_layers,
        Some(vec!["profile".to_string()])
    );
    send(
        &mut app,
        &job,
        GerberEdit::LayerToggled(Some("profile".into())),
    );
    assert_eq!(draft_gerber(&app, &job).gerber_layers, None);
    assert_eq!(
        gerber_of(&job).gerber_settings(),
        GerberSettings::default(),
        "nothing is written until Save"
    );
}

#[test]
fn the_gerber_form_shows_its_settings_fields() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    let job = write_job(dir.path(), &gerber_job());
    let task = app.open_output_job_tab(job.clone());
    settle(&mut app, task);
    let task = app.update(Message::from(JobTabMsg::OutputPicked {
        job: job.clone(),
        name: "Gerber".into(),
    }));
    settle(&mut app, task);
    let texts = lay_out_texts(
        app.view_output_job_tab(job.clone()),
        Size::new(1600.0, 1400.0),
    );
    // Without the isolation marks around a placeable.
    let plain = |text: &str| text.replace(['\u{2068}', '\u{2069}'], "");
    let shown: Vec<String> = texts.iter().map(|laid| plain(&laid.text)).collect();
    for word in [
        "Millimetres",
        "Inches",
        "4:6",
        "2:4",
        "Trailing",
        "Every layer",
        "Top copper",
        "Board outline",
        "Macros only",
    ] {
        assert!(
            shown.iter().any(|text| text == word),
            "{word} in {shown:#?}"
        );
    }
}
