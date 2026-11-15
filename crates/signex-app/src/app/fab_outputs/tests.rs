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
fn a_refused_board_creates_no_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let folder = tmp.path().join("out");
    let result = write_fab_outputs(&board(&[]), &FabOptions::all("b"), &folder);
    assert_eq!(result, Err(FabProblem::Refused(FabError::NoOutline)));
    assert!(!folder.exists());
}
