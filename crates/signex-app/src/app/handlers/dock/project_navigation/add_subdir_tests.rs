//! Add Existing registers a sheet/PCB picked from a project subdirectory by
//! its path below the project, not by bare file name (#1367).

use super::add_io;
use super::add_tests::two_projects;
use crate::app::state::scope::CaseFold;
use crate::app::{Message, ProjectMsg};

#[test]
fn add_existing_registers_sheet_and_pcb_from_a_subdirectory_with_their_relative_path() {
    let (mut app, d1, _d2) = two_projects();
    let dir = d1.path().to_path_buf();
    let target = app.document_state.projects[0].id;
    std::fs::create_dir_all(dir.join("sub")).expect("subdir");
    let sheet = dir.join("sub").join("Power.snxsch");
    let pcb = dir.join("sub").join("Board.snxpcb");
    std::fs::write(&sheet, b"x").expect("sheet");
    std::fs::write(&pcb, b"x").expect("pcb");

    let results = [sheet, pcb]
        .into_iter()
        .map(|p| add_io::run_existing(add_io::plan_existing(&dir, p, CaseFold::default())))
        .collect();
    let _ = app.update(Message::Project(ProjectMsg::AddExistingFilesCopied {
        project_id: target,
        project_dir: dir.clone(),
        results: Ok(results),
    }));

    let data = &app.document_state.projects[0].data;
    assert_eq!(data.sheets[0].filename, "sub/Power.snxsch");
    assert_eq!(data.pcb_file.as_deref(), Some("sub/Board.snxpcb"));
}
