//! #2410 — design reuse blocks through `Signex::update`: a routed channel
//! selected on the board and saved from the board menu, its file written to
//! the project's `reuse-blocks` folder, then placed on the other channel
//! around its anchor as one undo step; and the headless path an agent and the
//! CLI take, which reads the project's blocks first when they are not read.

use std::path::{Path, PathBuf};

use pcb_geom::ClickMode;
use signex_types::pcb::{PcbBoard, SymbolLink};
use signex_types::schematic::InstancePath;
use uuid::Uuid;

use signex_i18n::RenderTranslatable;

use super::ReuseBlockMsg;
use crate::app::command::{CommandArgs, CommandOutcome, translator};
use crate::app::handlers::menu::export::settle;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{ContextAction, ContextMenuMsg, Message, Signex};
use crate::keymap::AppCommandId;
use crate::menu_bar::MenuMessage;
use crate::pcb_canvas::PcbCanvasMsg;

const CH_A: Uuid = Uuid::from_u128(0xca);
const CH_B: Uuid = Uuid::from_u128(0xcb);
const UNIT_U: Uuid = Uuid::from_u128(0x100);
const UNIT_R: Uuid = Uuid::from_u128(0x200);
const U2: Uuid = Uuid::from_u128(0x21);
const R2: Uuid = Uuid::from_u128(0x22);

/// Channel A (U1 at (10, 10), R1 at (14, 10)) routed with a track on A1
/// between their pads and a GND via; channel B (U2, R2) placed apart and not
/// routed. Each part links to its channel's sheet placement with the units
/// the two placements share.
fn board() -> PcbBoard {
    let pad = |number: &str, x: f64, net: (u32, &str)| {
        serde_json::json!({
            "uuid": Uuid::now_v7(), "number": number, "pad_type": "smd", "shape": "rect",
            "position": {"x": x, "y": 0.0}, "size": {"x": 0.6, "y": 0.6}, "drill": null,
            "layers": ["TopCopper"], "net": {"number": net.0, "name": net.1}
        })
    };
    let part = |uuid: u128, reference: &str, at: (f64, f64), nets: [(u32, &str); 2]| {
        serde_json::json!({
            "uuid": Uuid::from_u128(uuid), "reference": reference, "value": "x",
            "footprint_id": "P2", "position": {"x": at.0, "y": at.1}, "rotation": 0.0,
            "layer": "TopCopper", "pads": [pad("1", -1.0, nets[0]), pad("2", 1.0, nets[1])]
        })
    };
    let mut board: PcbBoard = serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "nets": [
            {"number": 1, "name": "GND"}, {"number": 2, "name": "A1"},
            {"number": 3, "name": "OUT1"}, {"number": 4, "name": "A2"},
            {"number": 5, "name": "OUT2"}
        ],
        "footprints": [
            part(0x11, "U1", (10.0, 10.0), [(2, "A1"), (1, "GND")]),
            part(0x12, "R1", (14.0, 10.0), [(2, "A1"), (3, "OUT1")]),
            part(0x21, "U2", (50.0, 50.0), [(4, "A2"), (1, "GND")]),
            part(0x22, "R2", (70.0, 70.0), [(4, "A2"), (5, "OUT2")])
        ],
        "segments": [{
            "uuid": Uuid::from_u128(0x31), "start": {"x": 11.0, "y": 10.0},
            "end": {"x": 13.0, "y": 10.0}, "width": 0.25, "layer": "TopCopper", "net": 2
        }],
        "vias": [{
            "uuid": Uuid::from_u128(0x32), "position": {"x": 12.0, "y": 12.0},
            "diameter": 0.6, "drill": 0.3, "layers": ["TopCopper", "BottomCopper"], "net": 1
        }]
    }))
    .unwrap();
    for fp in &mut board.footprints {
        let (sheet, unit) = match fp.reference.as_str() {
            "U1" => (CH_A, UNIT_U),
            "R1" => (CH_A, UNIT_R),
            "U2" => (CH_B, UNIT_U),
            _ => (CH_B, UNIT_R),
        };
        fp.schematic_link = Some(SymbolLink::new(
            InstancePath::from_chain(vec![sheet]),
            [unit],
        ));
    }
    board
}

fn app_with_board(dir: &Path) -> (Signex, PathBuf) {
    app_on(dir, board())
}

/// An app with `pcb` open as the project's board.
fn app_on(dir: &Path, pcb: PcbBoard) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    app.ui_state.first_run_tour_open = false;
    app.document_state.projects[0].data.pcb_file = Some("board.snxpcb".into());
    let path = dir.join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), pcb);
    (app, path)
}

fn click(app: &mut Signex, x: f64, y: f64, mode: ClickMode) {
    let _ = app.update(Message::PcbCanvas(PcbCanvasMsg::Clicked {
        at: signex_types::pcb::Point::new(x, y),
        tolerance_mm: 0.05,
        mode,
    }));
}

fn at(app: &Signex, board: &Path, id: Uuid) -> (f64, f64) {
    let pcb = app.document_state.pcb_board(board).expect("board");
    let fp = pcb
        .footprints
        .iter()
        .find(|fp| fp.uuid == id)
        .expect("footprint");
    (fp.position.x, fp.position.y)
}

fn segments(app: &Signex, board: &Path) -> usize {
    app.document_state
        .pcb_board(board)
        .expect("board")
        .segments
        .len()
}

/// Channel A picked on the board (a pad of each part, the track and the
/// via) and saved from the board menu; the file write settled.
fn save_channel_a(app: &mut Signex) {
    click(app, 9.0, 10.0, ClickMode::Replace);
    click(app, 15.0, 10.0, ClickMode::Add);
    click(app, 12.0, 10.0, ClickMode::Add);
    click(app, 12.0, 12.0, ClickMode::Add);
    let task = app.update(Message::Menu(MenuMessage::command(
        "save_board_reuse_block",
    )));
    settle(app, task);
}

/// A right-click on the board, its task (the project's block read) settled.
fn right_click(app: &mut Signex) {
    let task = app.update(Message::ContextMenu(ContextMenuMsg::Show(5.0, 5.0)));
    settle(app, task);
    assert!(
        app.interaction_state.context_menu.is_some(),
        "the menu opened"
    );
}

fn headless(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let command = AppCommandId::new(id).unwrap();
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    app.run_headless_command_in_test(&command, args).0
}

#[test]
fn a_channel_saved_from_the_menu_is_written_and_placed_on_the_other_channel_in_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board) = app_with_board(dir.path());

    save_channel_a(&mut app);
    let file = dir.path().join("reuse-blocks").join("R1-U1.snxpcb");
    assert!(
        file.is_file(),
        "the block file is written: {}",
        file.display()
    );
    assert_eq!(app.reuse_block_names_in_window(None), ["R1-U1"]);

    click(&mut app, 49.0, 50.0, ClickMode::Replace);
    right_click(&mut app);
    let place = ReuseBlockMsg::PlaceRequested {
        name: "R1-U1".to_string(),
    };
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::Action(
        ContextAction::ReuseBlock(place),
    )));
    assert!(
        app.interaction_state.context_menu.is_none(),
        "the row closes the menu"
    );
    assert_eq!(at(&app, &board, U2), (50.0, 50.0), "the anchor stays");
    assert_eq!(at(&app, &board, R2), (54.0, 50.0), "R2 lands as R1 sat");
    assert_eq!(segments(&app, &board), 2, "the track came with the block");

    let _ = app.update(Message::Menu(MenuMessage::command("undo")));
    assert_eq!(at(&app, &board, R2), (70.0, 70.0), "one Undo takes it back");
    assert_eq!(segments(&app, &board), 1);
}

#[test]
fn an_agent_places_a_block_of_an_earlier_session_once_the_blocks_are_read() {
    let dir = tempfile::tempdir().unwrap();
    {
        let (mut first, _) = app_with_board(dir.path());
        save_channel_a(&mut first);
    }
    let (mut app, board) = app_with_board(dir.path());
    let document = format!("document={}", board.display());

    let outcome = headless(
        &mut app,
        "place_board_reuse_block",
        &[&document, "name=R1-U1", "designator=U2", "degrees=90"],
    );

    let CommandOutcome::Answered(answer) = outcome else {
        panic!("not answered: {outcome:?}");
    };
    assert_eq!(
        answer["footprints"],
        serde_json::json!([["U1", "U2"], ["R1", "R2"]])
    );
    assert_eq!(answer["tracks"], 1);
    assert_eq!(answer["vias"], 1);
    assert_eq!(
        at(&app, &board, R2),
        (50.0, 54.0),
        "turned a quarter about U2"
    );
}

#[test]
fn an_agent_saves_a_block_by_designators_and_a_bad_name_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board) = app_with_board(dir.path());
    let document = format!("document={}", board.display());

    let outcome = headless(
        &mut app,
        "save_board_reuse_block",
        &[&document, "name=amp", "ids=U1,R1"],
    );
    let CommandOutcome::Answered(answer) = outcome else {
        panic!("not answered: {outcome:?}");
    };
    assert_eq!(answer["name"], "amp");
    assert_eq!(answer["footprints"], 2);
    assert_eq!(answer["tracks"], 0, "only what `ids` names is saved");

    let outcome = headless(
        &mut app,
        "save_board_reuse_block",
        &[&document, "name=../amp", "ids=U1"],
    );
    assert!(
        matches!(&outcome, CommandOutcome::Disabled { reason } if reason.render_en_us().contains("cannot name a block")),
        "{outcome:?}"
    );
}

#[test]
fn place_without_operands_says_what_it_needs() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with_board(dir.path());
    let command = AppCommandId::new("place_board_reuse_block").unwrap();
    let (outcome, _) = app.dispatch_command_reporting(&command, CommandArgs::default(), None);
    assert!(
        matches!(&outcome, CommandOutcome::Disabled { reason } if reason.render_en_us().contains("designator")),
        "{outcome:?}"
    );
}

/// The first right-click on a board reads the project's block folder off
/// the update thread, so a block saved in an earlier session gets its Place
/// row.
#[test]
fn a_right_click_lists_the_blocks_of_an_earlier_session() {
    let dir = tempfile::tempdir().unwrap();
    {
        let (mut first, _) = app_with_board(dir.path());
        save_channel_a(&mut first);
    }
    let (mut app, _) = app_with_board(dir.path());
    assert!(
        app.reuse_block_names_in_window(None).is_empty(),
        "not read yet"
    );

    right_click(&mut app);

    assert_eq!(app.reuse_block_names_in_window(None), ["R1-U1"]);
}

/// A case-blind file system keeps one file for `amp` and `AMP`: the second
/// name is refused instead of overwriting the first block's file.
#[test]
fn a_name_that_differs_from_a_block_only_in_case_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board) = app_with_board(dir.path());
    let document = format!("document={}", board.display());
    let outcome = headless(
        &mut app,
        "save_board_reuse_block",
        &[&document, "name=amp", "ids=U1"],
    );
    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{outcome:?}"
    );

    let outcome = headless(
        &mut app,
        "save_board_reuse_block",
        &[&document, "name=AMP", "ids=R1"],
    );

    assert!(
        matches!(&outcome, CommandOutcome::Disabled { reason } if reason.render_en_us().contains("named amp")),
        "{outcome:?}"
    );
    assert_eq!(app.reuse_block_names_in_window(None), ["amp"]);
}

/// #2451: an agent's save before anything read the project's blocks reads
/// them first, so `r1-u1` is checked against `R1-U1` of an earlier session
/// and refused, and the earlier block's file is left as it was.
#[test]
fn an_agent_save_before_the_blocks_are_read_cannot_overwrite_a_block_named_in_another_case() {
    let dir = tempfile::tempdir().unwrap();
    {
        let (mut first, _) = app_with_board(dir.path());
        save_channel_a(&mut first);
    }
    let file = dir.path().join("reuse-blocks").join("R1-U1.snxpcb");
    let before = std::fs::read(&file).expect("the earlier block's file");
    let (mut app, board) = app_with_board(dir.path());
    let document = format!("document={}", board.display());

    let outcome = headless(
        &mut app,
        "save_board_reuse_block",
        &[&document, "name=r1-u1", "ids=U2"],
    );

    assert!(
        matches!(&outcome, CommandOutcome::Disabled { reason } if reason.render_en_us().contains("named R1-U1")),
        "{outcome:?}"
    );
    assert_eq!(
        std::fs::read(&file).unwrap(),
        before,
        "the file is untouched"
    );
    // Compare the listed names: on a case-insensitive file system (Windows,
    // macOS) `exists("r1-u1.snxpcb")` is true for `R1-U1.snxpcb` itself.
    let names: Vec<String> = std::fs::read_dir(dir.path().join("reuse-blocks"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        names.iter().any(|name| name == "R1-U1.snxpcb")
            && !names.iter().any(|name| name == "r1-u1.snxpcb"),
        "no second file beside it: {names:?}"
    );
}

/// #2451: two saves of one block whose writes finish out of order: the
/// second write waits for the first, so the file holds the second save.
#[test]
fn a_second_save_of_a_block_is_written_after_the_first() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board) = app_with_board(dir.path());
    right_click(&mut app);
    let document = format!("document={}", board.display());
    let save = |app: &mut Signex, ids: &str| {
        let command = AppCommandId::new("save_board_reuse_block").unwrap();
        let operands = [document.as_str(), "name=amp", ids];
        let args = CommandArgs::parse_operands(&command, &operands).expect("operands parse");
        let (outcome, task) = app.run_headless_command_in_test(&command, args);
        assert!(
            matches!(outcome, CommandOutcome::Answered(_)),
            "{outcome:?}"
        );
        task
    };
    let first = save(&mut app, "ids=U1");
    let second = save(&mut app, "ids=U1,R1");
    let file = dir.path().join("reuse-blocks").join("amp.snxpcb");

    // The second write lands first: it only waits.
    settle(&mut app, second);
    assert!(!file.exists(), "nothing written before the first write");
    settle(&mut app, first);

    let text = std::fs::read_to_string(&file).expect("the block file");
    let written = signex_types::format::SnxPcb::parse(&text).unwrap().board;
    assert_eq!(written.footprints.len(), 2, "the second save is the file");
}

/// What Messages said, since `mark`, about the block file `file` with
/// `word` in it.
fn said_about(mark: Option<u64>, file: &Path, word: &str) -> usize {
    let file = file.display().to_string();
    crate::diagnostics::entries_since(mark)
        .entries
        .iter()
        .filter(|entry| entry.message.contains(&file) && entry.message.contains(word))
        .count()
}

/// #2451 review: a block write holds a project folder rename off until it
/// lands, and the save that waited behind it is dropped, not written, when
/// the folder is being renamed by then. The old folder is not made again
/// and Messages does not say the second save was saved.
#[test]
fn a_waiting_block_write_is_dropped_when_the_project_folder_moves() {
    let root = tempfile::tempdir().unwrap();
    let old = root.path().join("Foo");
    std::fs::create_dir(&old).unwrap();
    let (mut app, board) = app_with_board(&old);
    right_click(&mut app);
    let document = format!("document={}", board.display());
    let save = |app: &mut Signex, ids: &str| {
        let command = AppCommandId::new("save_board_reuse_block").unwrap();
        let operands = [document.as_str(), "name=amp", ids];
        let args = CommandArgs::parse_operands(&command, &operands).expect("operands parse");
        let (outcome, task) = app.run_headless_command_in_test(&command, args);
        assert!(
            matches!(outcome, CommandOutcome::Answered(_)),
            "{outcome:?}"
        );
        task
    };
    let first = save(&mut app, "ids=U1");
    let second = save(&mut app, "ids=U1,R1");
    let file = old.join("reuse-blocks").join("amp.snxpcb");
    assert_eq!(
        app.document_state.project_writes,
        std::slice::from_ref(&file),
        "a folder rename waits for the running write"
    );
    settle(&mut app, second);

    // The first write lands on disk; the project folder moves before the
    // app takes its landing in.
    let landed = crate::task_driver::TaskDriver::new()
        .outputs(first)
        .expect("the first write ran");
    assert!(file.is_file(), "the first save is written before the move");
    let _ = crate::diagnostics::logged_count("");
    let mark = crate::diagnostics::newest_entry_id();
    std::fs::rename(&old, root.path().join("Bar")).unwrap();
    let project = app.document_state.projects[0].id;
    app.document_state.project_folder_renames.insert(project);
    for message in landed {
        let task = app.update(message);
        settle(&mut app, task);
    }

    assert!(!old.exists(), "the old project folder is not made again");
    assert_eq!(
        said_about(mark, &file, "saved to"),
        1,
        "the first save only"
    );
    assert_eq!(
        said_about(mark, &file, "could not be written"),
        1,
        "the second save says it was not written"
    );
    assert!(app.document_state.project_writes.is_empty());
    assert!(!app.document_state.reuse_blocks.is_writing(&file));
}

/// #2451 review: a block folder that could not be listed is listed again
/// on the next save, so one failed read does not refuse every save until
/// Signex restarts.
#[test]
fn a_failed_block_folder_read_is_retried_on_the_next_save() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board) = app_with_board(dir.path());
    let not_a_folder = dir.path().join("reuse-blocks");
    std::fs::write(&not_a_folder, b"x").unwrap();
    let document = format!("document={}", board.display());
    let outcome = headless(
        &mut app,
        "save_board_reuse_block",
        &[&document, "name=amp", "ids=U1"],
    );
    assert!(
        matches!(&outcome, CommandOutcome::Disabled { reason } if reason.render_en_us().contains("could not be read")),
        "{outcome:?}"
    );

    std::fs::remove_file(&not_a_folder).unwrap();
    let outcome = headless(
        &mut app,
        "save_board_reuse_block",
        &[&document, "name=amp", "ids=U1"],
    );

    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{outcome:?}"
    );
}

/// #2451 review: a board menu that opens after a failed folder listing
/// lists the folder again.
#[test]
fn a_failed_block_folder_read_is_retried_on_the_next_menu_open() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with_board(dir.path());
    let not_a_folder = dir.path().join("reuse-blocks");
    std::fs::write(&not_a_folder, b"x").unwrap();
    right_click(&mut app);
    let blocks = &app.document_state.reuse_blocks;
    assert!(blocks.read_failure(dir.path()).is_some(), "the read failed");
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::Close));

    std::fs::remove_file(&not_a_folder).unwrap();
    right_click(&mut app);

    let blocks = &app.document_state.reuse_blocks;
    assert_eq!(blocks.read_failure(dir.path()), None);
}

/// The menu save of channel A writes the block file the operand handler
/// writes for the same board, name and items (#2326 slice 6 translator).
#[test]
fn a_menu_save_writes_the_block_the_operand_handler_writes() {
    // One board for both: its pads take fresh uuids.
    let pcb = board();
    let menu_dir = tempfile::tempdir().unwrap();
    let (mut menu, _) = app_on(menu_dir.path(), pcb.clone());
    save_channel_a(&mut menu);
    assert!(translator::ran("save_board_reuse_block"));
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board) = app_on(dir.path(), pcb);
    let document = format!("document={}", board.display());
    let ids: Vec<String> = [0x11u128, 0x12, 0x31, 0x32]
        .map(|n| Uuid::from_u128(n).to_string())
        .to_vec();
    let ids = format!("ids={}", ids.join(","));
    let outcome = headless(
        &mut app,
        "save_board_reuse_block",
        &[&document, "name=R1-U1", &ids],
    );
    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{outcome:?}"
    );
    let file = |dir: &Path| {
        std::fs::read_to_string(dir.join("reuse-blocks").join("R1-U1.snxpcb")).expect("block")
    };
    assert_eq!(file(menu_dir.path()), file(dir.path()));
}

/// A menu save of a selection with no footprint, track or via in it says
/// what it always said, not the `ids` refusal an agent gets.
#[test]
fn a_menu_save_of_a_selection_with_nothing_a_block_holds_is_refused() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    app.ui_state.first_run_tour_open = false;
    app.document_state.projects[0].data.pcb_file = Some("board.snxpcb".into());
    let path = dir.path().join("board.snxpcb");
    let mut pcb = board();
    pcb.zones = serde_json::from_value(serde_json::json!([{
        "uuid": Uuid::from_u128(0x41), "net": 1, "net_name": "GND", "layer": "TopCopper",
        "outline": [{"x": 0.0, "y": 0.0}, {"x": 5.0, "y": 0.0}, {"x": 5.0, "y": 5.0}],
        "fill_type": "solid"
    }]))
    .unwrap();
    let kind = pcb_geom::PickKind::of_zone(&pcb.zones[0]).expect("a pickable area");
    app.open_pcb_tab(path, "board".into(), pcb);
    let zone = Uuid::from_u128(0x41);
    app.select_pcb_items(&[pcb_geom::ItemRef::Zone { zone, kind }]);
    let mark = crate::diagnostics::newest_entry_id();
    let task = app.update(Message::Menu(MenuMessage::command(
        "save_board_reuse_block",
    )));
    settle(&mut app, task);
    let said = crate::diagnostics::entries_since(mark).entries;
    assert!(
        said.iter()
            .any(|e| e.message.contains("a block needs at least one footprint")),
        "{said:?}"
    );
    assert!(
        !said.iter().any(|e| e.message.contains("`ids`")),
        "{said:?}"
    );
    assert!(
        !dir.path().join("reuse-blocks").exists(),
        "nothing is saved"
    );
}

/// The board menu's Place row for `name` on the component at (x, y).
fn place_from_menu(app: &mut Signex, name: &str, x: f64, y: f64) {
    click(app, x, y, ClickMode::Replace);
    right_click(app);
    let place = ReuseBlockMsg::PlaceRequested {
        name: name.to_string(),
    };
    let task = app.update(Message::ContextMenu(ContextMenuMsg::Action(
        ContextAction::ReuseBlock(place),
    )));
    settle(app, task);
}

/// Where each footprint sits and how many tracks and vias: a placed block
/// takes fresh uuids, so two placements compare by these.
fn layout(app: &Signex, board: &Path) -> Vec<String> {
    let pcb = app.document_state.pcb_board(board).expect("board");
    let mut lines: Vec<String> = pcb
        .footprints
        .iter()
        .map(|fp| {
            let (x, y) = (fp.position.x, fp.position.y);
            format!("{} {x} {y} {}", fp.reference, fp.rotation)
        })
        .collect();
    lines.push(format!(
        "{} tracks {} vias",
        pcb.segments.len(),
        pcb.vias.len()
    ));
    lines
}

/// The menu Place on U2 lays the block as the operand handler does with
/// `designator=U2`, through the handler (#2326 slice 6 translator).
#[test]
fn a_menu_place_lays_the_block_as_the_operand_handler_does() {
    let pcb = board();
    let menu_dir = tempfile::tempdir().unwrap();
    let (mut menu, menu_board) = app_on(menu_dir.path(), pcb.clone());
    save_channel_a(&mut menu);
    place_from_menu(&mut menu, "R1-U1", 49.0, 50.0);
    assert!(translator::ran("place_board_reuse_block"));
    assert_eq!(at(&menu, &menu_board, R2), (54.0, 50.0), "it placed");

    let dir = tempfile::tempdir().unwrap();
    let (mut app, board) = app_on(dir.path(), pcb);
    save_channel_a(&mut app);
    let document = format!("document={}", board.display());
    let outcome = headless(
        &mut app,
        "place_board_reuse_block",
        &[&document, "name=R1-U1", "designator=U2"],
    );
    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{outcome:?}"
    );
    assert_eq!(layout(&menu, &menu_board), layout(&app, &board));
}

/// The menu names the clicked anchor by uuid (#2326 slice 6), so the block
/// is placed around it where its designator is shared, or stored as "U2 ",
/// as before the translator.
#[test]
fn a_menu_place_on_a_component_whose_designator_is_shared_places_around_it() {
    let mut pcb = board();
    pcb.footprints[3].reference = "U2".into();
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board) = app_on(dir.path(), pcb);
    save_channel_a(&mut app);
    place_from_menu(&mut app, "R1-U1", 49.0, 50.0);
    assert!(translator::ran("place_board_reuse_block"));
    assert_eq!(at(&app, &board, R2), (54.0, 50.0), "placed around U2");
}

#[test]
fn a_menu_place_on_a_reference_with_spaces_places_around_it() {
    let mut pcb = board();
    pcb.footprints[2].reference = "U2 ".into();
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board) = app_on(dir.path(), pcb);
    save_channel_a(&mut app);
    place_from_menu(&mut app, "R1-U1", 49.0, 50.0);
    assert_eq!(at(&app, &board, R2), (54.0, 50.0), "placed around U2");
}

/// An agent names the anchor by uuid where its designator is shared.
#[test]
fn an_agent_places_a_block_around_the_component_its_uuid_names() {
    let mut pcb = board();
    pcb.footprints[3].reference = "U2".into();
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board) = app_on(dir.path(), pcb);
    save_channel_a(&mut app);
    let document = format!("document={}", board.display());
    let ids = format!("ids={U2}");
    let outcome = headless(
        &mut app,
        "place_board_reuse_block",
        &[&document, "name=R1-U1", &ids],
    );
    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{outcome:?}"
    );
    assert_eq!(at(&app, &board, R2), (54.0, 50.0));
}

/// The menu's Place row sent with U2 selected, its task settled.
fn place_on_u2(app: &mut Signex, name: &str) -> Vec<String> {
    click(app, 49.0, 50.0, ClickMode::Replace);
    let mark = crate::diagnostics::newest_entry_id();
    let place = ReuseBlockMsg::PlaceRequested {
        name: name.to_string(),
    };
    let task = app.update(Message::Pcb(crate::app::PcbMsg::ReuseBlock(place)));
    settle(app, task);
    crate::diagnostics::entries_since(mark)
        .entries
        .into_iter()
        .map(|e| e.message)
        .collect()
}

/// A menu Place is refused at once, as it always was, while the project's
/// blocks are not read: the menu reads nothing first, even with the block
/// on disk.
#[test]
fn a_menu_place_before_the_blocks_are_read_is_refused_without_reading_them() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().unwrap();
    let (mut earlier, _) = app_with_board(dir.path());
    save_channel_a(&mut earlier);
    let (mut app, board) = app_with_board(dir.path());

    let said = place_on_u2(&mut app, "R1-U1");

    assert!(
        said.iter()
            .any(|m| m.contains("the project has no reuse block named R1-U1")),
        "{said:?}"
    );
    assert!(app.document_state.reuse_blocks.needs_read(dir.path()));
    assert_eq!(at(&app, &board, R2), (70.0, 70.0), "nothing is placed");
}

/// After a failed read a menu Place is refused with why, again on the next
/// Place: the menu does not read the folder again (a save or a menu open
/// does).
#[test]
fn a_menu_place_after_a_failed_read_is_refused_without_reading_again() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with_board(dir.path());
    let not_a_folder = dir.path().join("reuse-blocks");
    std::fs::write(&not_a_folder, b"x").unwrap();
    right_click(&mut app);
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::Close));
    let unread = "the project's reuse blocks could not be read";

    let first = place_on_u2(&mut app, "amp");
    std::fs::remove_file(&not_a_folder).unwrap();
    let second = place_on_u2(&mut app, "amp");

    assert!(first.iter().any(|m| m.contains(unread)), "{first:?}");
    assert!(second.iter().any(|m| m.contains(unread)), "{second:?}");
    let blocks = &app.document_state.reuse_blocks;
    assert!(blocks.read_failure(dir.path()).is_some(), "not read again");
}
