//! Mockup row 114, "Automation": a save of a watched file runs its profile
//! after the wait; keep outputs up to date runs only the outputs made from
//! the saved file; a newer save supersedes an older wait; before close runs
//! the profiles with out-of-date outputs (Always) or names them (Ask).

use signex_types::output_job::{
    BeforeClose, JobOutput, JobProfile, OutputJob, OutputKind, RunTrigger, read_output_job,
};

use super::auto::AutoMsg;
use super::pipeline_tests::{tree, write_job};
use super::tests::project_app;
use crate::app::handlers::menu::export::settle;
use crate::app::{Message, Signex};

/// A PDF and a Gerber output; profile `Auto` ticks both, with no Validate
/// and no wait.
fn auto_job(set: impl FnOnce(&mut JobProfile)) -> OutputJob {
    let mut auto = JobProfile::new("Auto", vec!["PDF".into(), "Gerber".into()]);
    auto.validate = false;
    auto.triggers.wait_seconds = 0;
    set(&mut auto);
    OutputJob {
        outputs: vec![
            JobOutput::new("PDF", OutputKind::SchematicPdf),
            JobOutput::new("Gerber", OutputKind::Gerber),
        ],
        profiles: vec![auto],
        ..OutputJob::default()
    }
}

fn saved(app: &mut Signex, file: &std::path::Path) {
    let task = app.auto_runs_after_save(file);
    settle(app, task);
}

/// Keep outputs up to date: a schematic save runs the profile's schematic
/// outputs only, with nothing to ask, and the run is kept as an auto run.
#[test]
fn keep_up_to_date_runs_only_the_outputs_of_the_saved_file() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    let job = write_job(dir.path(), &auto_job(|p| p.triggers.keep_up_to_date = true));
    saved(&mut app, &dir.path().join("main.snxsch"));
    let files: Vec<String> = tree(&dir.path().join("out")).into_keys().collect();
    assert_eq!(
        files,
        ["ProjectA.pdf"],
        "the board output waits for a board save"
    );
    let history = read_output_job(&job).unwrap().history;
    assert_eq!(history[0].trigger, RunTrigger::KeepUpToDate);
    assert_eq!(history[0].outputs, 1);
    let stale = app
        .output_jobs()
        .auto
        .stale_count(&job, &["PDF".into(), "Gerber".into()]);
    assert_eq!(stale, 0, "the PDF is up to date again");
}

/// Run on save: a save of a file the profile watches runs the whole
/// profile; a save of another file runs nothing.
#[test]
fn run_on_save_watches_the_files_it_names() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    let watching = |file: &str| {
        auto_job(|p| {
            p.triggers.on_save = true;
            p.triggers.sources = vec![file.to_string()];
            p.outputs = vec!["PDF".into()];
        })
    };
    write_job(dir.path(), &watching("other.snxsch"));
    saved(&mut app, &dir.path().join("main.snxsch"));
    assert!(
        !dir.path().join("out").exists(),
        "main.snxsch is not watched"
    );
    let job = write_job(dir.path(), &watching("MAIN.SNXSCH"));
    saved(&mut app, &dir.path().join("main.snxsch"));
    assert!(dir.path().join("out").join("ProjectA.pdf").is_file());
    let history = read_output_job(&job).unwrap().history;
    assert_eq!(history[0].trigger, RunTrigger::OnSave);
}

/// A wait a newer save replaced starts nothing.
#[test]
fn a_superseded_wait_starts_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    let job = write_job(dir.path(), &auto_job(|p| p.triggers.on_save = true));
    let task = app.update(Message::from(AutoMsg::WaitEnded {
        job: job.clone(),
        profile: "Auto".into(),
        generation: 7,
        among: None,
        trigger: RunTrigger::OnSave,
    }));
    settle(&mut app, task);
    assert!(app.output_jobs().runs.latest(&job).is_none());
    assert!(!dir.path().join("out").exists());
}

/// Before closing the project: Always runs the profile whose ticked
/// outputs are out of date; Ask names them in a notice and runs nothing.
#[test]
fn before_close_runs_or_names_the_out_of_date_profiles() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    let job = write_job(
        dir.path(),
        &auto_job(|p| {
            p.outputs = vec!["PDF".into()];
            p.triggers.before_close = BeforeClose::Always;
        }),
    );
    saved(&mut app, &dir.path().join("main.snxsch"));
    assert!(!dir.path().join("out").exists(), "no save trigger");
    let project = app.document_state.projects[0].id;
    let task = app.auto_runs_before_close(project);
    settle(&mut app, task);
    assert!(dir.path().join("out").join("ProjectA.pdf").is_file());
    let history = read_output_job(&job).unwrap().history;
    assert_eq!(history[0].trigger, RunTrigger::BeforeClose);

    let other = tempfile::tempdir().unwrap();
    let mut app = project_app(other.path());
    write_job(
        other.path(),
        &auto_job(|p| {
            p.outputs = vec!["PDF".into()];
            p.triggers.before_close = BeforeClose::Ask;
        }),
    );
    saved(&mut app, &other.path().join("main.snxsch"));
    let project = app.document_state.projects[0].id;
    let task = app.auto_runs_before_close(project);
    settle(&mut app, task);
    assert!(!other.path().join("out").exists());
    let notice = app.document_view.error_notice.as_ref().expect("a notice");
    use signex_i18n::RenderTranslatable as _;
    assert!(notice.detail.render_en_us().contains("out of date"));
}
