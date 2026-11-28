//! The Export Job file on disk: read back what is written, a missing file
//! refused, and a run record that leaves the rest of the file (#1299).

use signex_types::output_job::{
    JobOutput, OutputJob, OutputJobError, OutputKind, RunRecord, RunResult, RunTrigger,
};

use super::{append_run_record, read_output_job, write_output_job};

fn job() -> OutputJob {
    let mut png = JobOutput::new("Sheets PNG", OutputKind::SchematicPng);
    png.dpi = Some(150);
    OutputJob {
        folder: "out/rev-b".into(),
        variant: Some("Lite".into()),
        outputs: vec![
            JobOutput::new("Schematic PDF", OutputKind::SchematicPdf),
            png,
        ],
        ..OutputJob::default()
    }
}

#[test]
fn the_file_is_written_atomically_and_read_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fab.snxout");
    assert!(matches!(read_output_job(&path), Err(OutputJobError::Io(_))));
    write_output_job(&path, &job()).unwrap();
    assert_eq!(read_output_job(&path).unwrap(), job());
}

#[test]
fn a_run_record_leaves_the_rest_of_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fab.snxout");
    write_output_job(&path, &job()).unwrap();
    let record = RunRecord {
        started: "2026-11-18 14:07:00".into(),
        profile: None,
        variant: "Base".into(),
        trigger: RunTrigger::Manual,
        result: RunResult::Complete,
        outputs: 2,
        files: 3,
        warnings: 0,
        errors: 0,
        seconds: 3,
        folder: "out".into(),
        zip: None,
        stopped_at: None,
        reason: None,
        viewable: Vec::new(),
        warning_lines: Vec::new(),
        unknown: Default::default(),
    };
    append_run_record(&path, record.clone()).unwrap();
    let read = read_output_job(&path).unwrap();
    assert_eq!(read.history, vec![record]);
    assert_eq!(read.outputs, job().outputs);
}
