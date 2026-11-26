//! #1299: the Export Job file round-trips under its token with foreign keys
//! kept, refuses what it cannot run, and plans the outputs a run writes.

use super::*;

fn sample() -> OutputJob {
    let mut png = JobOutput::new("Sheets PNG", OutputKind::SchematicPng);
    png.dpi = Some(150);
    let mut bom = JobOutput::new("BOM", OutputKind::Bom);
    bom.file = Some("parts.html".into());
    OutputJob {
        folder: "out/rev-b".into(),
        variant: Some("Lite".into()),
        outputs: vec![
            JobOutput::new("Schematic PDF", OutputKind::SchematicPdf),
            png,
            bom,
            JobOutput::new("Netlist", OutputKind::Netlist),
        ],
        unknown: UnknownFields::default(),
    }
}

#[test]
fn a_job_round_trips_under_its_token() {
    let job = sample();
    let text = encode_output_job(&job).unwrap();
    assert!(
        text.starts_with(&format!("format = \"{SNXOUT_FORMAT_V1}\"")),
        "{text}"
    );
    assert_eq!(parse_output_job(&text).unwrap(), job);
}

#[test]
fn keys_this_build_does_not_know_are_kept() {
    let text = format!(
        "format = \"{SNXOUT_FORMAT_V1}\"\nfolder = \"out\"\nprofile = \"Release\"\n\n\
         [[outputs]]\nname = \"Gerber\"\nkind = \"gerber_x2\"\nenabled = true\nunits = \"mm\"\n\n\
         [[outputs]]\nname = \"PDF\"\nkind = \"schematic_pdf\"\nenabled = true\n\n\
         [automation]\non_save = true\n"
    );
    let job = parse_output_job(&text).unwrap();
    assert!(job.unknown.contains_key("profile"));
    assert!(job.unknown.contains_key("automation"));
    assert!(job.outputs[0].unknown.contains_key("units"));
    assert_eq!(job.outputs[0].kind, "gerber_x2");
    let again = parse_output_job(&encode_output_job(&job).unwrap()).unwrap();
    assert_eq!(again, job, "a save writes the foreign keys back");
}

#[test]
fn a_missing_newer_or_garbled_token_is_refused() {
    let text = encode_output_job(&sample()).unwrap();
    let newer = text.replacen(SNXOUT_FORMAT_V1, "snxout/2", 1);
    assert!(matches!(
        parse_output_job(&newer),
        Err(OutputJobError::Version(_))
    ));
    let missing = text.replacen(&format!("format = \"{SNXOUT_FORMAT_V1}\"\n"), "", 1);
    assert!(matches!(
        parse_output_job(&missing),
        Err(OutputJobError::Version(_))
    ));
    let number = text.replacen(&format!("\"{SNXOUT_FORMAT_V1}\""), "1", 1);
    assert!(matches!(
        parse_output_job(&number),
        Err(OutputJobError::Parse(_))
    ));
    assert!(matches!(
        parse_output_job("not toml ["),
        Err(OutputJobError::Parse(_))
    ));
}

#[test]
fn a_folder_or_file_that_leaves_the_job_folder_is_a_fault() {
    for folder in [
        "../out",
        "/tmp/out",
        "C:\\out",
        "out/../..",
        "out//x",
        "out/ x",
    ] {
        let job = OutputJob {
            folder: folder.into(),
            ..sample()
        };
        assert_eq!(
            job.fault(),
            Some(OutputJobFault::BadFolder(folder.into())),
            "{folder}"
        );
        assert!(encode_output_job(&job).is_err(), "{folder} is not saved");
    }
    let here = OutputJob {
        folder: String::new(),
        ..sample()
    };
    assert_eq!(here.fault(), None);
    assert_eq!(
        here.output_folder(Path::new("/p")).unwrap(),
        PathBuf::from("/p")
    );
    assert_eq!(
        sample().output_folder(Path::new("/p")).unwrap(),
        Path::new("/p").join("out").join("rev-b")
    );
    for file in ["../x.pdf", "a/b.pdf", "", "..", "c:x.pdf"] {
        let mut job = sample();
        job.outputs[0].file = Some(file.into());
        assert!(
            matches!(job.fault(), Some(OutputJobFault::BadFile { .. })),
            "{file:?}"
        );
    }
}

#[test]
fn names_must_be_given_once_and_dpi_in_range() {
    let mut job = sample();
    job.outputs[1].name = " schematic pdf".into();
    assert_eq!(job.fault(), None, "names compare with case");
    job.outputs[1].name = "Schematic PDF ".into();
    assert_eq!(
        job.fault(),
        Some(OutputJobFault::NameTwice("Schematic PDF".into()))
    );
    job.outputs[1].name = "  ".into();
    assert_eq!(job.fault(), Some(OutputJobFault::NoName));
    let mut job = sample();
    job.outputs[1].dpi = Some(MAX_PNG_DPI + 1);
    assert!(matches!(job.fault(), Some(OutputJobFault::BadDpi { .. })));
    job.outputs[1].dpi = Some(MIN_PNG_DPI);
    assert_eq!(job.fault(), None);
}

#[test]
fn a_run_plans_every_enabled_output_with_default_files() {
    let mut job = sample();
    job.outputs[3].enabled = false;
    let plan = job.plan("demo", None).unwrap();
    let got: Vec<(&str, OutputKind, &str, u32)> = plan
        .outputs
        .iter()
        .map(|o| (o.name.as_str(), o.kind, o.file.as_str(), o.dpi))
        .collect();
    assert_eq!(
        got,
        [
            ("Schematic PDF", OutputKind::SchematicPdf, "demo.pdf", 300),
            ("Sheets PNG", OutputKind::SchematicPng, "demo.png", 150),
            ("BOM", OutputKind::Bom, "parts.html", 300),
        ]
    );
    assert!(plan.skipped.is_empty());
    assert_eq!(png_page_file("demo.png", 2), "demo-2.png");
    assert_eq!(png_page_file("sheets", 1), "sheets-1.png");
}

#[test]
fn a_run_of_one_output_names_it() {
    let mut job = sample();
    let plan = job.plan("demo", Some(" Netlist ")).unwrap();
    assert_eq!(plan.outputs.len(), 1);
    assert_eq!(plan.outputs[0].file, "demo.NET");
    assert_eq!(
        job.plan("demo", Some("Gerber")),
        Err(PlanProblem::NoSuchOutput("Gerber".into()))
    );
    job.outputs[3].enabled = false;
    assert_eq!(
        job.plan("demo", Some("Netlist")),
        Err(PlanProblem::Disabled("Netlist".into()))
    );
}

#[test]
fn an_unknown_kind_is_skipped_in_a_full_run_and_refused_by_name() {
    let mut job = sample();
    job.outputs.push(JobOutput {
        kind: "odb_plus_plus".into(),
        ..JobOutput::new("ODB", OutputKind::Bom)
    });
    let plan = job.plan("demo", None).unwrap();
    assert_eq!(plan.skipped, ["ODB"]);
    assert_eq!(plan.outputs.len(), 4);
    assert!(matches!(
        job.plan("demo", Some("ODB")),
        Err(PlanProblem::UnknownKind { .. })
    ));
}

#[test]
fn two_outputs_on_one_file_or_none_enabled_write_nothing() {
    let mut job = sample();
    job.outputs[2].file = Some("DEMO.pdf".into());
    assert!(matches!(
        job.plan("demo", None),
        Err(PlanProblem::SameFile { .. })
    ));
    let mut job = sample();
    for output in &mut job.outputs {
        output.enabled = false;
    }
    assert_eq!(job.plan("demo", None), Err(PlanProblem::Nothing));
    assert_eq!(
        OutputJob::default().plan("demo", None),
        Err(PlanProblem::Nothing)
    );
}

#[test]
fn the_file_is_written_atomically_and_read_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fab.snxout");
    assert!(matches!(read_output_job(&path), Err(OutputJobError::Io(_))));
    write_output_job(&path, &sample()).unwrap();
    assert_eq!(read_output_job(&path).unwrap(), sample());
}
