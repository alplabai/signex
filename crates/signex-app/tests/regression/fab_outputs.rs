//! #2388: File ▸ Fabrication Outputs, driven through `Signex::update` with
//! the messages the menu row, the palette and the dialog send. Nothing is
//! written until Generate; Generate writes every file into the folder, and
//! the same board gives the same bytes. #2489: the IPC-2581 file, the ODB++
//! job, the assembly drawings and the STEP model through the same dialog.

use signex_app::app::{
    CommandPaletteMsg, FabOutput, FabOutputsMsg, FabProblem, FileMsg, LoadedProject, Message,
    Signex,
};
use signex_app::diagnostics;
use signex_app::keymap::AppCommandId;
use signex_app::menu_bar::MenuMessage;
use signex_output::fab::{FabError, FabNote};
use signex_types::format::SnxPcb;
use signex_types::pcb::{Footprint, Pad, PadShape, PadType, PcbBoard, Point, Zone};
use signex_types::project::{FabOutputKinds, parse_project};
use uuid::Uuid;

fn pad(n: u128, x: f64) -> Pad {
    Pad {
        uuid: Uuid::from_u128(n),
        number: n.to_string(),
        pad_type: PadType::Smd,
        shape: PadShape::Rect,
        position: Point::new(x, 0.0),
        size: Point::new(1.0, 1.2),
        drill: None,
        layers: vec![
            "TopCopper".into(),
            "TopSolderMask".into(),
            "TopPaste".into(),
        ],
        net: None,
        roundrect_ratio: 0.0,
        trapezoid_delta: Default::default(),
        custom_outline: Vec::new(),
        rotation: 0.0,
        net_locked: false,
        layer_sizes: None,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn board(with_outline: bool) -> PcbBoard {
    let outline = [(0.0, 0.0), (20.0, 0.0), (20.0, 10.0), (0.0, 10.0)];
    PcbBoard {
        sketch: None,
        stackup: None,
        options: Default::default(),
        uuid: Uuid::nil(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: if with_outline {
            outline.iter().map(|&(x, y)| Point::new(x, y)).collect()
        } else {
            Vec::new()
        },
        layers: Vec::new(),
        setup: None,
        nets: Vec::new(),
        footprints: vec![Footprint {
            uuid: Uuid::from_u128(0x10),
            reference: "R1".into(),
            value: "10k".into(),
            footprint_id: "R_0603".into(),
            position: Point::new(5.0, 5.0),
            rotation: 0.0,
            layer: "TopCopper".into(),
            locked: false,
            schematic_link: None,
            library_ref: None,
            component: Default::default(),
            pads: vec![pad(1, -1.0), pad(2, 1.0)],
            graphics: Vec::new(),
            properties: Vec::new(),
            unknown: Default::default(),
            unknown_columns: Default::default(),
        }],
        segments: Vec::new(),
        vias: Vec::new(),
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        free: Default::default(),
        unknown: Default::default(),
    }
}

struct Fixture {
    app: Signex,
    tmp: tempfile::TempDir,
}

fn open_board(with_outline: bool) -> Fixture {
    open(board(with_outline))
}

fn open(board: PcbBoard) -> Fixture {
    let _ = diagnostics::init_logging();
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("board.snxpcb");
    let text = SnxPcb::new(board).write_string().unwrap();
    std::fs::write(&path, text).unwrap();
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    send(&mut app, Message::File(FileMsg::Opened(Some(path))));
    Fixture { app, tmp }
}

fn send(app: &mut Signex, message: Message) {
    let task = app.update(message);
    crate::drive::drive(app, task);
}

fn fab(msg: FabOutputsMsg) -> Message {
    Message::from(msg)
}

fn menu_row(app: &mut Signex) {
    let id = AppCommandId::new("fabrication_outputs").expect("a catalog id");
    send(app, Message::Menu(MenuMessage::Command(id)));
}

#[test]
fn the_menu_row_opens_the_dialog_and_writes_nothing() {
    let mut f = open_board(true);
    menu_row(&mut f.app);
    let state = f.app.ui_state.fab_outputs.as_ref().expect("the dialog");
    assert_eq!(state.name, "board");
    let folder = std::path::PathBuf::from(&state.folder);
    // Beside the board, however the opener spelled its path.
    assert_eq!(
        folder.parent().map(|dir| dir.join("board.snxpcb")).as_ref(),
        Some(&state.board)
    );
    assert!(folder.ends_with("fabrication"));
    assert!(!folder.exists(), "nothing is written before Generate");
    send(&mut f.app, fab(FabOutputsMsg::CancelRequested));
    assert!(f.app.ui_state.fab_outputs.is_none());
    assert!(!folder.exists(), "Cancel writes nothing");
}

#[test]
fn the_palette_opens_the_dialog() {
    let mut f = open_board(true);
    send(&mut f.app, Message::CommandPalette(CommandPaletteMsg::Open));
    send(
        &mut f.app,
        Message::CommandPalette(CommandPaletteMsg::QueryChanged(
            "Fabrication Outputs".into(),
        )),
    );
    send(
        &mut f.app,
        Message::CommandPalette(CommandPaletteMsg::ExecuteSelected),
    );
    assert!(f.app.ui_state.fab_outputs.is_some());
}

#[test]
fn generate_writes_every_file_into_the_chosen_folder_the_same_each_time() {
    let mut f = open_board(true);
    menu_row(&mut f.app);
    let out = f.tmp.path().join("out");
    send(
        &mut f.app,
        fab(FabOutputsMsg::FolderPicked(Some(out.clone()))),
    );
    send(&mut f.app, fab(FabOutputsMsg::NameEdited("rev-a".into())));
    send(&mut f.app, fab(FabOutputsMsg::GenerateRequested));
    let state = f
        .app
        .ui_state
        .fab_outputs
        .as_ref()
        .expect("the dialog stays");
    let written = match &state.outcome {
        Some(Ok(written)) => written.clone(),
        other => panic!("written, not {other:?}"),
    };
    assert_eq!(written.folder, out);
    assert!(
        written
            .files
            .contains(&"rev-a-copper-l1-top.gbr".to_string())
    );
    assert!(written.files.contains(&"rev-a-job.gbrjob".to_string()));
    for name in &written.files {
        assert!(out.join(name).is_file(), "{name} is on disk");
    }
    let copper = std::fs::read(out.join("rev-a-copper-l1-top.gbr")).unwrap();
    assert!(copper.starts_with(b"%TF.GenerationSoftware,Alp Lab,Signex*%\n"));

    send(&mut f.app, fab(FabOutputsMsg::GenerateRequested));
    let again = std::fs::read(out.join("rev-a-copper-l1-top.gbr")).unwrap();
    assert_eq!(copper, again, "the same board gives the same bytes");
}

#[test]
fn a_kind_left_out_is_not_written() {
    let mut f = open_board(true);
    menu_row(&mut f.app);
    let out = f.tmp.path().join("out");
    send(
        &mut f.app,
        fab(FabOutputsMsg::FolderEdited(out.display().to_string())),
    );
    send(
        &mut f.app,
        fab(FabOutputsMsg::OutputToggled(FabOutput::Gerber)),
    );
    send(&mut f.app, fab(FabOutputsMsg::GenerateRequested));
    assert!(out.join("board-centroid.csv").is_file());
    assert!(!out.join("board-copper-l1-top.gbr").exists());
}

#[test]
fn a_board_without_an_outline_says_why_and_writes_nothing() {
    let mut f = open_board(false);
    menu_row(&mut f.app);
    let out = f.tmp.path().join("out");
    send(
        &mut f.app,
        fab(FabOutputsMsg::FolderPicked(Some(out.clone()))),
    );
    send(&mut f.app, fab(FabOutputsMsg::GenerateRequested));
    let state = f
        .app
        .ui_state
        .fab_outputs
        .as_ref()
        .expect("the dialog stays");
    assert_eq!(
        state.outcome,
        Some(Err(FabProblem::Refused(FabError::NoOutline)))
    );
    assert!(!out.exists(), "nothing is written");
}

#[test]
fn with_no_board_in_view_the_command_does_not_open_the_dialog() {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    menu_row(&mut app);
    assert!(app.ui_state.fab_outputs.is_none());
}

/// A top copper pour on `net` named `net_name` with the corners `outline`.
fn top_pour(net: u32, net_name: &str, outline: &[(f64, f64)]) -> Zone {
    Zone {
        uuid: Uuid::from_u128(0x80),
        net,
        net_name: net_name.into(),
        layer: "TopCopper".into(),
        outline: outline.iter().map(|&(x, y)| Point::new(x, y)).collect(),
        holes: Vec::new(),
        priority: 0,
        fill_type: signex_types::zone_pour::POUR_FILL.into(),
        thermal_relief: false,
        thermal_gap: 0.0,
        thermal_width: 0.0,
        clearance: 0.0,
        min_thickness: 0.0,
        min_island_area: 0.0,
        remove_dead_islands: false,
        shelved: false,
        unknown: Default::default(),
    }
}

#[test]
fn a_board_with_an_unfilled_copper_zone_warns_in_messages_after_generate() {
    let mut with_zone = board(true);
    // A pour wholly off the 20 x 10 mm board fills nothing, so the images
    // hold no copper for it.
    with_zone
        .zones
        .push(top_pour(0, "", &[(25.0, 1.0), (30.0, 1.0), (30.0, 5.0)]));
    let mut f = open(with_zone);
    menu_row(&mut f.app);
    let out = f.tmp.path().join("zoned-out");
    send(
        &mut f.app,
        fab(FabOutputsMsg::FolderPicked(Some(out.clone()))),
    );
    let before = diagnostics::newest_entry_id();
    send(&mut f.app, fab(FabOutputsMsg::GenerateRequested));
    let state = f.app.ui_state.fab_outputs.as_ref().expect("the dialog");
    assert_eq!(
        state.notes(),
        [FabNote::ZonesNotFilled { count: 1 }].as_slice()
    );
    let folder = out.display().to_string();
    let logged = diagnostics::entries_since(before).entries;
    let line = logged
        .iter()
        .find(|e| e.message.contains(&folder))
        .unwrap_or_else(|| panic!("a Messages line for {folder}: {logged:?}"));
    assert_eq!(line.level, diagnostics::DiagnosticLevel::Warning);
    assert!(line.message.contains("copper zone"), "{}", line.message);
}

/// #2451 review: Generate fills the board's pours, so a pour inside the
/// board is a region in the top copper image and no note says it was not
/// filled.
#[test]
fn a_pour_inside_the_board_is_a_region_in_the_top_copper_image() {
    let mut with_zone = board(true);
    with_zone.zones.push(top_pour(
        1,
        "GND",
        &[(1.0, 1.0), (19.0, 1.0), (19.0, 9.0), (1.0, 9.0)],
    ));
    let mut f = open(with_zone);
    menu_row(&mut f.app);
    let out = f.tmp.path().join("poured-out");
    send(
        &mut f.app,
        fab(FabOutputsMsg::FolderPicked(Some(out.clone()))),
    );
    send(&mut f.app, fab(FabOutputsMsg::GenerateRequested));
    let state = f.app.ui_state.fab_outputs.as_ref().expect("the dialog");
    assert!(state.notes().is_empty(), "{:?}", state.notes());
    let copper = std::fs::read_to_string(out.join("board-copper-l1-top.gbr")).unwrap();
    assert!(
        copper.contains("%TO.N,GND*%\nG36*"),
        "the pour's fill: {copper}"
    );
}

/// Open the dialog on a board with an outline, aim it at `out` under the
/// fixture's folder and leave only `kinds` ticked.
fn dialog_with_only(kinds: &[FabOutput], out: &str) -> (Fixture, std::path::PathBuf) {
    let mut f = open_board(true);
    menu_row(&mut f.app);
    let out = f.tmp.path().join(out);
    send(
        &mut f.app,
        fab(FabOutputsMsg::FolderPicked(Some(out.clone()))),
    );
    for kind in FabOutput::ALL {
        let state = f.app.ui_state.fab_outputs.as_ref().expect("the dialog");
        if kind.is_on(&state.kinds) != kinds.contains(&kind) {
            send(&mut f.app, fab(FabOutputsMsg::OutputToggled(kind)));
        }
    }
    (f, out)
}

/// The files the last Generate wrote, as the dialog lists them.
fn written_files(app: &Signex) -> Vec<String> {
    let state = app.ui_state.fab_outputs.as_ref().expect("the dialog stays");
    match &state.outcome {
        Some(Ok(written)) => written.files.clone(),
        other => panic!("written, not {other:?}"),
    }
}

/// #2489: each new kind of file is ticked in the dialog, nothing reaches
/// the disk before Generate, and Generate writes it.
#[test]
fn the_board_documents_and_whole_job_files_are_written_by_generate() {
    let (mut f, out) = dialog_with_only(
        &[
            FabOutput::Ipc2581,
            FabOutput::Odb,
            FabOutput::AssemblyPdf,
            FabOutput::AssemblySvg,
            FabOutput::Step,
        ],
        "documents",
    );
    assert!(!out.exists(), "nothing is written before Generate");
    send(&mut f.app, fab(FabOutputsMsg::GenerateRequested));
    let files = written_files(&f.app);
    for name in [
        "board-ipc2581.xml",
        "board-odb.tar",
        "board-assembly.pdf",
        "board-assembly-top.svg",
        "board-assembly-bottom.svg",
        "board-board.step",
    ] {
        assert!(files.contains(&name.to_string()), "{name} in {files:?}");
        assert!(out.join(name).is_file(), "{name} is on disk");
    }
    assert!(
        !out.join("board-copper-l1-top.gbr").exists(),
        "Gerber unticked"
    );

    let ipc = std::fs::read_to_string(out.join("board-ipc2581.xml")).unwrap();
    assert!(ipc.starts_with("<?xml"), "{ipc}");
    assert!(ipc.contains("IPC-2581"));
    assert!(ipc.contains("R1"), "the part is in the file");
    let odb = std::fs::read(out.join("board-odb.tar")).unwrap();
    assert!(
        odb.windows(b"matrix/matrix".len())
            .any(|w| w == b"matrix/matrix"),
        "the archive holds the matrix"
    );
    let pdf = std::fs::read(out.join("board-assembly.pdf")).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
    for side in ["top", "bottom"] {
        let svg = std::fs::read_to_string(out.join(format!("board-assembly-{side}.svg"))).unwrap();
        assert!(svg.contains("<svg "), "{side}: {svg}");
    }
    let step = std::fs::read_to_string(out.join("board-board.step")).unwrap();
    assert!(step.starts_with("ISO-10303-21;"));
    assert!(
        step.contains("=MANIFOLD_SOLID_BREP('Board'"),
        "the board body"
    );
}

#[test]
fn the_odb_folder_box_writes_the_job_as_its_folder_tree() {
    let (mut f, out) = dialog_with_only(&[FabOutput::Odb], "odb-folder");
    send(&mut f.app, fab(FabOutputsMsg::OdbFolderToggled));
    send(&mut f.app, fab(FabOutputsMsg::GenerateRequested));
    let files = written_files(&f.app);
    assert!(
        files.contains(&"board-odb/board/matrix/matrix".to_string()),
        "{files:?}"
    );
    assert!(
        out.join("board-odb")
            .join("board")
            .join("matrix")
            .join("matrix")
            .is_file()
    );
    assert!(!out.join("board-odb.tar").exists(), "no archive");
}

/// A STEP model alone is no fabrication set: no job file is written.
#[test]
fn the_step_model_alone_writes_only_the_model() {
    let (mut f, out) = dialog_with_only(&[FabOutput::Step], "step-only");
    send(&mut f.app, fab(FabOutputsMsg::GenerateRequested));
    assert_eq!(written_files(&f.app), ["board-board.step"]);
    assert!(!out.join("board-job.gbrjob").exists());
}

/// The checks every file shares hold with documents alone: a board without
/// an outline writes nothing.
#[test]
fn documents_alone_of_a_board_without_an_outline_write_nothing() {
    let mut f = open_board(false);
    menu_row(&mut f.app);
    let out = f.tmp.path().join("out");
    send(
        &mut f.app,
        fab(FabOutputsMsg::FolderPicked(Some(out.clone()))),
    );
    for kind in FabOutput::ALL {
        let state = f.app.ui_state.fab_outputs.as_ref().expect("the dialog");
        if kind.is_on(&state.kinds) != (kind == FabOutput::AssemblySvg) {
            send(&mut f.app, fab(FabOutputsMsg::OutputToggled(kind)));
        }
    }
    send(&mut f.app, fab(FabOutputsMsg::GenerateRequested));
    let state = f.app.ui_state.fab_outputs.as_ref().expect("the dialog");
    assert_eq!(
        state.outcome,
        Some(Err(FabProblem::Refused(FabError::NoOutline)))
    );
    assert!(!out.exists(), "nothing is written");
}

/// A fresh app with the saved project at `prj` loaded and its board
/// `Oscar.snxpcb` open in view.
fn load_project_and_board(prj: &std::path::Path) -> Signex {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: prj.to_path_buf(),
        data: parse_project(prj).unwrap(),
        pending_libraries: Default::default(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    app.document_state.active_project = Some(id);
    let board_path = prj.with_file_name("Oscar.snxpcb");
    if !board_path.exists() {
        let text = SnxPcb::new(board(true)).write_string().unwrap();
        std::fs::write(&board_path, text).unwrap();
    }
    send(&mut app, Message::File(FileMsg::Opened(Some(board_path))));
    app
}

/// #2489: the ticks of a board its project names are a project edit: the
/// project is dirty, nothing is written until Save, Save writes them in the
/// `.snxprj`, and the next dialog on that project opens with them.
#[test]
fn the_projects_ticks_are_saved_with_it_and_open_the_next_dialog() {
    let f = crate::project_tab_support::fixture();
    let mut app = load_project_and_board(&f.prj);
    let prj_dirty = |app: &Signex| {
        let path = &app.document_state.projects[0].path;
        app.document_state.is_document_dirty(path)
    };
    assert!(!prj_dirty(&app));
    menu_row(&mut app);
    send(&mut app, fab(FabOutputsMsg::OutputToggled(FabOutput::Step)));
    send(&mut app, fab(FabOutputsMsg::OdbFolderToggled));
    send(&mut app, fab(FabOutputsMsg::NameEdited("rev-b".into())));
    let stored = app.document_state.projects[0]
        .data
        .fab_outputs
        .expect("the project holds the ticks");
    assert!(stored.step && stored.odb_folder && stored.gerber);
    assert!(prj_dirty(&app), "a project edit");
    assert_eq!(
        parse_project(&f.prj).unwrap().fab_outputs,
        None,
        "nothing is written before Save"
    );
    send(&mut app, fab(FabOutputsMsg::CancelRequested));
    crate::project_tab_support::open_tab(&mut app);
    crate::project_tab_support::save(&mut app);
    assert!(!prj_dirty(&app), "saved");
    assert_eq!(parse_project(&f.prj).unwrap().fab_outputs, Some(stored));

    let mut next = load_project_and_board(&f.prj);
    menu_row(&mut next);
    let state = next.ui_state.fab_outputs.as_ref().expect("the dialog");
    assert_eq!(state.kinds, stored, "the project's ticks");
    assert_eq!(state.name, "Oscar", "the name is the board's own");
}

/// A board no project names keeps its ticks in the dialog only: the next
/// opening starts from the defaults.
#[test]
fn a_loose_boards_ticks_are_not_kept() {
    let (mut f, _out) = dialog_with_only(&[FabOutput::Step], "loose");
    send(&mut f.app, fab(FabOutputsMsg::CancelRequested));
    menu_row(&mut f.app);
    let state = f.app.ui_state.fab_outputs.as_ref().expect("the dialog");
    assert_eq!(state.kinds, FabOutputKinds::default());
}
