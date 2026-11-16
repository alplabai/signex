//! #2410: DXF import, once through `Signex::update` (the file dialog's
//! answer) and once through the operand handler an agent or the CLI calls.
//! Each import is one board undo step.

use std::path::{Path, PathBuf};

use signex_engine::pcb::dxf::DxfTarget;
use signex_types::pcb::PcbBoard;
use signex_types::pcb_layer::{PcbLayer, TechnicalLayer};
use uuid::Uuid;

use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::{DxfImportMsg, Message, PcbMsg, Signex};
use crate::keymap::AppCommandId;
use crate::task_driver::TaskDriver;

/// A rectangle 20 x 10 mm of four lines, as a DXF file in mm.
const RECT: &str = "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n\
0\nSECTION\n2\nENTITIES\n\
0\nLINE\n10\n0\n20\n0\n11\n20\n21\n0\n\
0\nLINE\n10\n20\n20\n0\n11\n20\n21\n10\n\
0\nLINE\n10\n20\n20\n10\n11\n0\n21\n10\n\
0\nLINE\n10\n0\n20\n10\n11\n0\n21\n0\n\
0\nENDSEC\n0\nEOF\n";

struct Fixture {
    app: Signex,
    board: PathBuf,
    dxf: PathBuf,
}

/// An app with an empty board open, and `text` written to a DXF file.
fn fixture(name: &str, text: &str) -> Fixture {
    let (mut app, _task) = Signex::new();
    let board: PcbBoard =
        serde_json::from_value(serde_json::json!({ "uuid": Uuid::nil() })).expect("a board");
    let dir = std::env::temp_dir().join(format!("signex-dxf-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a temp folder");
    let path = dir.join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board);
    let dxf = dir.join("drawing.dxf");
    std::fs::write(&dxf, text).expect("the drawing is written");
    Fixture {
        app,
        board: path,
        dxf,
    }
}

fn board(app: &Signex, path: &Path) -> PcbBoard {
    app.document_state
        .board_engine(path)
        .expect("the board is open")
        .board()
        .clone()
}

fn drive(app: &mut Signex, task: iced::Task<Message>) {
    TaskDriver::new()
        .settle(app, task)
        .expect("the task settles");
}

fn send(app: &mut Signex, message: Message) {
    let task = app.update(message);
    drive(app, task);
}

const SILK: PcbLayer = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);

#[test]
fn a_picked_file_lands_on_the_layer_as_one_undo_step() {
    let mut f = fixture("layer", RECT);
    send(
        &mut f.app,
        Message::Pcb(PcbMsg::DxfImport(DxfImportMsg::Picked {
            board: f.board.clone(),
            target: DxfTarget::Layer(SILK),
            path: Some(f.dxf.clone()),
        })),
    );
    let after = board(&f.app, &f.board);
    assert_eq!(after.graphics.len(), 4);
    assert!(after.graphics.iter().all(|g| g.layer == "TopSilkscreen"));
    assert!(f.app.step_board_history(&f.board, true), "one undo");
    assert!(board(&f.app, &f.board).graphics.is_empty());
    assert!(
        !f.app.step_board_history(&f.board, true),
        "nothing else to undo"
    );
}

#[test]
fn an_outline_import_replaces_the_outline_and_undo_puts_it_back() {
    let mut f = fixture("outline", RECT);
    let before = board(&f.app, &f.board).outline;
    send(
        &mut f.app,
        Message::Pcb(PcbMsg::DxfImport(DxfImportMsg::Picked {
            board: f.board.clone(),
            target: DxfTarget::Outline,
            path: Some(f.dxf.clone()),
        })),
    );
    assert_eq!(board(&f.app, &f.board).outline.len(), 4);
    assert!(f.app.step_board_history(&f.board, true));
    assert_eq!(board(&f.app, &f.board).outline, before);
}

#[test]
fn a_cancelled_dialog_or_an_unreadable_file_changes_nothing() {
    let mut f = fixture("refused", "not a drawing");
    let before = serde_json::to_string(&board(&f.app, &f.board)).unwrap();
    send(
        &mut f.app,
        Message::Pcb(PcbMsg::DxfImport(DxfImportMsg::Picked {
            board: f.board.clone(),
            target: DxfTarget::Outline,
            path: None,
        })),
    );
    let refused = f.app.import_dxf_text(
        &f.board.clone(),
        DxfTarget::Layer(SILK),
        &f.dxf.clone(),
        Ok("not a drawing".into()),
    );
    assert!(refused.is_err(), "{refused:?}");
    assert_eq!(
        serde_json::to_string(&board(&f.app, &f.board)).unwrap(),
        before
    );
}

fn id(name: &str) -> AppCommandId {
    AppCommandId::new(name).expect("a catalog id")
}

/// The agent path: `import_dxf path=.. layer=..` runs the same import as
/// the dialog's answer, and a layer that is not one is refused up front.
#[test]
fn the_operand_command_imports_onto_the_named_layer() {
    let mut f = fixture("operands", RECT);
    let path = format!("path={}", f.dxf.display());
    let args =
        CommandArgs::parse_operands(&id("import_dxf"), &[path.as_str(), "layer=TopSilkscreen"])
            .expect("operands parse");
    let (outcome, task) = f.app.run_operands(&id("import_dxf"), &args);
    assert_eq!(outcome, CommandOutcome::Ran);
    drive(&mut f.app, task);
    assert_eq!(board(&f.app, &f.board).graphics.len(), 4);

    let bad = CommandArgs::parse_operands(&id("import_dxf"), &[path.as_str(), "layer=Nowhere"])
        .expect("operands parse");
    let (outcome, _) = f.app.run_operands(&id("import_dxf"), &bad);
    assert!(
        matches!(outcome, CommandOutcome::Disabled { .. }),
        "{outcome:?}"
    );

    let outline = CommandArgs::parse_operands(&id("import_dxf_outline"), &[path.as_str()])
        .expect("operands parse");
    let (outcome, task) = f.app.run_operands(&id("import_dxf_outline"), &outline);
    assert_eq!(outcome, CommandOutcome::Ran);
    drive(&mut f.app, task);
    assert_eq!(board(&f.app, &f.board).outline.len(), 4);
}
