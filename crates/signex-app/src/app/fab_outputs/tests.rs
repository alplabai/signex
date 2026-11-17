use std::path::PathBuf;

use signex_output::fab::{FabError, FabOptions};
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use super::{FabOutput, FabOutputsState, FabProblem, write_fab_outputs};

fn board(outline: &[(f64, f64)]) -> PcbBoard {
    let mut board: PcbBoard =
        serde_json::from_value(serde_json::json!({ "uuid": Uuid::nil() })).expect("a board");
    board.outline = outline.iter().map(|&(x, y)| Point::new(x, y)).collect();
    board
}

#[test]
fn a_new_dialog_writes_everything_named_after_the_board_beside_it() {
    let path = PathBuf::from("projects").join("main.snxpcb");
    let state = FabOutputsState::new(path, "main.snxpcb".into());
    assert_eq!(state.options, FabOptions::all("main"));
    assert_eq!(
        PathBuf::from(&state.folder),
        PathBuf::from("projects").join("fabrication")
    );
    assert!(state.can_generate());
    assert!(state.notes().is_empty());
}

#[test]
fn generate_needs_a_kind_a_name_and_a_folder_and_no_write_in_flight() {
    let mut state = FabOutputsState::new(PathBuf::from("b.snxpcb"), "b".into());
    for kind in FabOutput::ALL {
        kind.toggle(&mut state.options);
        assert!(!kind.is_on(&state.options));
    }
    assert!(!state.can_generate(), "no kind chosen");
    FabOutput::Drill.toggle(&mut state.options);
    assert!(state.can_generate());
    state.folder = "  ".into();
    assert!(!state.can_generate(), "no folder");
    state.folder = "out".into();
    state.options.name = String::new();
    assert!(!state.can_generate(), "no name");
    state.options.name = "b".into();
    state.writing = Some(1);
    assert!(!state.can_generate(), "a write in flight");
}

#[test]
fn the_job_writes_every_file_and_creates_the_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let folder = tmp.path().join("nested").join("out");
    let board = board(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)]);
    let written = write_fab_outputs(&board, &FabOptions::all("b"), &folder).expect("written");
    assert_eq!(written.folder, folder);
    assert!(!written.files.is_empty());
    for name in &written.files {
        assert!(folder.join(name).is_file(), "{name}");
    }
}

#[test]
fn a_pour_and_a_solid_region_are_copper_in_the_gerber_with_no_note() {
    let tmp = tempfile::tempdir().unwrap();
    let mut board = board(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]);
    let zone = |id: u128,
                net: u32,
                name: &str,
                layer: &str,
                fill: &str,
                ring: &[(f64, f64)]|
     -> signex_types::pcb::Zone {
        let outline: Vec<_> = ring
            .iter()
            .map(|&(x, y)| serde_json::json!({"x": x, "y": y}))
            .collect();
        serde_json::from_value(serde_json::json!({
            "uuid": Uuid::from_u128(id), "net": net, "net_name": name, "layer": layer,
            "outline": outline, "fill_type": fill
        }))
        .expect("a zone")
    };
    let square = [(1.0, 1.0), (9.0, 1.0), (9.0, 9.0), (1.0, 9.0)];
    board.zones = vec![
        zone(0x80, 1, "GND", "TopCopper", "pour", &square),
        zone(0x81, 2, "VCC", "BottomCopper", "solid", &square),
    ];
    let written = write_fab_outputs(&board, &FabOptions::all("b"), tmp.path()).expect("written");
    assert!(written.notes.is_empty(), "{:?}", written.notes);
    let top = std::fs::read_to_string(tmp.path().join("b-copper-l1-top.gbr")).unwrap();
    assert!(top.contains("%TO.N,GND*%\nG36*"), "the pour's fill: {top}");
    let bottom = std::fs::read_to_string(tmp.path().join("b-copper-l2-bottom.gbr")).unwrap();
    assert!(
        bottom.contains("%TO.N,VCC*%\nG36*"),
        "the solid region: {bottom}"
    );
}

#[test]
fn a_refused_board_creates_no_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let folder = tmp.path().join("out");
    let result = write_fab_outputs(&board(&[]), &FabOptions::all("b"), &folder);
    assert_eq!(result, Err(FabProblem::Refused(FabError::NoOutline)));
    assert!(!folder.exists());
}

#[test]
fn older_files_of_the_same_stem_are_reported_and_left_in_place() {
    let tmp = tempfile::tempdir().unwrap();
    let folder = tmp.path().to_path_buf();
    let board = board(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)]);
    let first = write_fab_outputs(&board, &FabOptions::all("b"), &folder).expect("written");
    assert!(first.stale.is_empty(), "{:?}", first.stale);
    std::fs::write(folder.join("b-copper-l3-inner2.gbr"), "old").unwrap();
    std::fs::write(folder.join("b-readme.txt"), "not a fabrication file").unwrap();
    std::fs::write(folder.join("other-pth.drl"), "another stem").unwrap();
    let options = FabOptions {
        centroid: false,
        ..FabOptions::all("b")
    };
    let second = write_fab_outputs(&board, &options, &folder).expect("written");
    assert_eq!(second.stale, ["b-centroid.csv", "b-copper-l3-inner2.gbr"]);
    assert!(
        folder.join("b-copper-l3-inner2.gbr").is_file(),
        "never deleted"
    );
}

#[test]
fn a_set_with_notes_or_stale_files_is_a_warning_in_messages() {
    let i18n = signex_i18n::I18n::new(&signex_i18n::LanguagePref::Locale("en-US".into()));
    let clean = super::FabWritten {
        files: vec!["b-job.gbrjob".into()],
        folder: PathBuf::from("out"),
        notes: Vec::new(),
        stale: Vec::new(),
    };
    let (line, is_warning) = super::written_message(&clean, &i18n);
    assert!(!is_warning, "{line}");
    let zones = super::FabWritten {
        notes: vec![signex_output::fab::FabNote::ZonesNotFilled { count: 2 }],
        ..clean.clone()
    };
    let (line, is_warning) = super::written_message(&zones, &i18n);
    assert!(is_warning);
    assert!(line.contains(&super::note_text(zones.notes[0], &i18n).into_string()));
    let stale = super::FabWritten {
        stale: vec!["b-npth.drl".into()],
        ..clean
    };
    let (line, is_warning) = super::written_message(&stale, &i18n);
    assert!(is_warning);
    assert!(line.contains("b-npth.drl"), "{line}");
}
