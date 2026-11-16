//! #2388: File ▸ Fabrication Outputs, driven through `Signex::update` with
//! the messages the menu row, the palette and the dialog send. Nothing is
//! written until Generate; Generate writes every file into the folder, and
//! the same board gives the same bytes.

use signex_app::app::{
    CommandPaletteMsg, FabOutput, FabOutputsMsg, FabProblem, FileMsg, Message, Signex,
};
use signex_app::diagnostics;
use signex_app::keymap::AppCommandId;
use signex_app::menu_bar::MenuMessage;
use signex_output::fab::{FabError, FabNote};
use signex_types::format::SnxPcb;
use signex_types::pcb::{Footprint, Pad, PadShape, PadType, PcbBoard, Point, Zone};
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
        rotation: 0.0,
        net_locked: false,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn board(with_outline: bool) -> PcbBoard {
    let outline = [(0.0, 0.0), (20.0, 0.0), (20.0, 10.0), (0.0, 10.0)];
    PcbBoard {
        sketch: None,
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
    assert_eq!(state.options.name, "board");
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

#[test]
fn a_board_with_an_unfilled_copper_zone_warns_in_messages_after_generate() {
    let mut with_zone = board(true);
    with_zone.zones.push(Zone {
        uuid: Uuid::from_u128(0x80),
        net: 0,
        net_name: String::new(),
        layer: "TopCopper".into(),
        outline: vec![
            Point::new(1.0, 1.0),
            Point::new(9.0, 1.0),
            Point::new(9.0, 9.0),
        ],
        holes: Vec::new(),
        priority: 0,
        fill_type: String::new(),
        thermal_relief: false,
        thermal_gap: 0.0,
        thermal_width: 0.0,
        clearance: 0.0,
        min_thickness: 0.0,
        unknown: Default::default(),
    });
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
