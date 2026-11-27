//! Mockup row 114: run profiles pick the ticked outputs, the one variant and
//! the folder; their triggers and the run history are kept in the file.

use super::*;

fn job() -> OutputJob {
    let mut odb = JobOutput::new("ODB++", OutputKind::Odb);
    odb.enabled = false;
    let mut release = JobProfile::new(
        "Production release",
        vec![
            "Gerber".into(),
            "Drill".into(),
            "PDF".into(),
            "ODB++".into(),
        ],
    );
    release.variant = Some("Base".into());
    release.folder = Some("out/release".into());
    release.zip = true;
    release.triggers.on_save = true;
    release.triggers.sources = vec!["board.snxpcb".into(), "*.snxsch".into()];
    let quick = JobProfile::new("Quick check", vec!["Gerber".into(), "Drill".into()]);
    OutputJob {
        default_profile: Some("production release".into()),
        outputs: vec![
            JobOutput::new("Gerber", OutputKind::Gerber),
            JobOutput::new("Drill", OutputKind::NcDrill),
            JobOutput::new("PDF", OutputKind::SchematicPdf),
            JobOutput::new("BOM", OutputKind::Bom),
            odb,
        ],
        profiles: vec![release, quick],
        ..OutputJob::default()
    }
}

fn record(n: u32) -> RunRecord {
    RunRecord {
        started: format!("2026-11-18 14:{n:02}:00"),
        profile: Some("Quick check".into()),
        variant: "Base".into(),
        trigger: RunTrigger::OnSave,
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
        viewable: vec!["board-F_Cu.gbr".into()],
        warning_lines: Vec::new(),
        unknown: UnknownFields::default(),
    }
}

fn names(plan: &RunPlan) -> Vec<&str> {
    plan.plan.outputs.iter().map(|o| o.name.as_str()).collect()
}

#[test]
fn profiles_triggers_and_history_round_trip() {
    let mut job = job();
    job.record_run(record(1));
    let text = encode_output_job(&job).unwrap();
    assert!(text.contains("[[profiles]]"), "{text}");
    assert!(text.contains("[[history]]"), "{text}");
    assert_eq!(parse_output_job(&text).unwrap(), job);
}

#[test]
fn keys_of_a_profile_its_triggers_and_a_run_that_this_build_does_not_know_are_kept() {
    let text = format!(
        "format = \"{SNXOUT_FORMAT_V1}\"\n\n\
         [[profiles]]\nname = \"P\"\noutputs = []\ncontainer = \"tar\"\n\n\
         [profiles.triggers]\non_save = true\nwait_seconds = 5\nwebhook = \"x\"\n\n\
         [[history]]\nstarted = \"2026-11-18 14:00:00\"\ntrigger = \"manual\"\nresult = \"complete\"\nsigned = true\n"
    );
    let job = parse_output_job(&text).unwrap();
    assert!(job.profiles[0].unknown.contains_key("container"));
    assert!(job.profiles[0].triggers.unknown.contains_key("webhook"));
    assert!(job.history[0].unknown.contains_key("signed"));
    let again = parse_output_job(&encode_output_job(&job).unwrap()).unwrap();
    assert_eq!(again, job);
}

#[test]
fn a_profile_runs_its_ticked_enabled_outputs_in_its_variant_and_folder() {
    let job = job();
    let plan = job.run_plan("board", &RunRequest::default()).unwrap();
    assert_eq!(plan.profile.as_deref(), Some("Production release"));
    assert_eq!(names(&plan), ["Gerber", "Drill", "PDF"], "ODB++ is off");
    assert_eq!(plan.variant.as_deref(), Some("Base"));
    assert_eq!(plan.folder, "out/release");
    assert!(plan.validate);
    assert!(plan.packaging.zip);
    assert_eq!(plan.on_errors, OnErrors::Stop);
    assert_eq!(plan.on_warnings, OnWarnings::Continue);

    let quick = RunRequest {
        profile: Some("QUICK CHECK"),
        ..RunRequest::default()
    };
    let plan = job.run_plan("board", &quick).unwrap();
    assert_eq!(names(&plan), ["Gerber", "Drill"]);
    assert_eq!(plan.folder, "out", "the job's folder");
    assert_eq!(plan.plan.outputs[0].file, "board", "a board output's stem");
}

#[test]
fn run_selected_skips_validate_and_the_zip_and_keep_up_to_date_runs_a_subset() {
    let job = job();
    let selected = RunRequest {
        selected: true,
        ..RunRequest::default()
    };
    let plan = job.run_plan("board", &selected).unwrap();
    assert!(!plan.validate);
    assert!(!plan.packaging.zip);
    let board_only = ["Gerber".to_string(), "BOM".to_string()];
    let among = RunRequest {
        among: Some(&board_only),
        ..RunRequest::default()
    };
    let plan = job.run_plan("board", &among).unwrap();
    assert_eq!(names(&plan), ["Gerber"], "BOM is not ticked");
}

#[test]
fn a_missing_profile_or_one_with_nothing_to_run_is_refused() {
    let mut job = job();
    let missing = RunRequest {
        profile: Some("Docs"),
        ..RunRequest::default()
    };
    assert_eq!(
        job.run_plan("board", &missing),
        Err(PlanProblem::NoSuchProfile("Docs".into()))
    );
    job.profiles[1].outputs = vec!["ODB++".into()];
    let quick = RunRequest {
        profile: Some("Quick check"),
        ..RunRequest::default()
    };
    assert_eq!(
        job.run_plan("board", &quick),
        Err(PlanProblem::NothingTicked("Quick check".into()))
    );
    let one = RunRequest {
        profile: Some("Quick check"),
        only: Some("BOM"),
        ..RunRequest::default()
    };
    assert_eq!(names(&job.run_plan("board", &one).unwrap()), ["BOM"]);
}

#[test]
fn with_no_profile_every_enabled_output_runs_without_validate() {
    let mut job = job();
    job.profiles.clear();
    job.default_profile = None;
    let plan = job.run_plan("board", &RunRequest::default()).unwrap();
    assert_eq!(plan.profile, None);
    assert_eq!(names(&plan), ["Gerber", "Drill", "PDF", "BOM"]);
    assert!(!plan.validate);
}

#[test]
fn profile_names_the_default_a_wait_and_a_folder_are_checked() {
    let mut job = job();
    job.profiles[1].name = "production RELEASE".into();
    assert_eq!(
        job.fault(),
        Some(OutputJobFault::ProfileTwice("production RELEASE".into()))
    );
    let mut job = self::job();
    job.default_profile = Some("Docs".into());
    assert_eq!(job.fault(), Some(OutputJobFault::NoDefault("Docs".into())));
    assert!(encode_output_job(&job).is_err());
    let mut job = self::job();
    job.profiles[0].triggers.wait_seconds = MAX_SAVE_WAIT_SECONDS + 1;
    assert!(matches!(job.fault(), Some(OutputJobFault::BadWait { .. })));
    let mut job = self::job();
    job.profiles[0].folder = Some("../out".into());
    assert!(matches!(job.fault(), Some(OutputJobFault::BadFolder(_))));
    let mut job = self::job();
    job.profiles[0].name = " ".into();
    assert_eq!(job.fault(), Some(OutputJobFault::NoProfileName));
}

#[test]
fn a_trigger_watches_the_files_it_names() {
    let job = job();
    let triggers = &job.profiles[0].triggers;
    assert!(triggers.watches("board.snxpcb"));
    assert!(triggers.watches("Power.SNXSCH"));
    assert!(!triggers.watches("other.snxpcb"));
    assert!(triggers.is_auto());
    let quick = &job.profiles[1].triggers;
    assert!(!quick.watches("board.snxpcb"), "no trigger on");
    assert!(!quick.is_auto());
    let any = JobTriggers {
        keep_up_to_date: true,
        ..JobTriggers::default()
    };
    assert!(any.watches("a.snxsch") && any.watches("b.snxpcb"));
    assert!(!any.watches("fab.snxout"));
}

#[test]
fn naming_and_release_folders_fill_their_tokens() {
    assert_eq!(
        expand_naming("{project}-{rev}", "dcdc", "B", "Base"),
        "dcdc-B"
    );
    assert_eq!(expand_naming("{project}-{rev}", "dcdc", "", "Base"), "dcdc");
    assert_eq!(expand_naming("{variant}", "dcdc", "B", ""), "dcdc");
    assert_eq!(
        release_folder_name("dcdc", "B", "2026-11-18"),
        "dcdc-revB-2026-11-18"
    );
    assert_eq!(
        release_folder_name("dcdc", "", "2026-11-18"),
        "dcdc-2026-11-18"
    );
}

#[test]
fn the_history_keeps_the_newest_runs_and_a_record_leaves_the_rest_of_the_file() {
    let mut job = job();
    for n in 0..(MAX_HISTORY as u32 + 5) {
        job.record_run(record(n));
    }
    assert_eq!(job.history.len(), MAX_HISTORY);
    assert_eq!(
        job.history[0].started,
        record(MAX_HISTORY as u32 + 4).started
    );

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fab.snxout");
    let saved = self::job();
    write_output_job(&path, &saved).unwrap();
    append_run_record(&path, record(7)).unwrap();
    let read = read_output_job(&path).unwrap();
    assert_eq!(read.history, vec![record(7)]);
    assert_eq!(read.profiles, saved.profiles);
    assert_eq!(read.outputs, saved.outputs);
}
