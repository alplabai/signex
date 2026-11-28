//! Mockup row 207: a sheet and a board side by side, each pane keeping its
//! own view as the focus moves between them. The fixture also serves the
//! cross probe across panes (`cross_probe::panes_tests`).

use std::path::PathBuf;

use signex_types::pcb::{Footprint, Pad, PadShape, PadType, Point, SymbolLink};
use signex_types::schematic::InstancePath;
use uuid::Uuid;

use crate::app::handlers::document_panes_tests::{app, path};
use crate::app::{FileMsg, Message, Signex, WindowMsg};
use crate::document_panes::DropEdge;

/// The footprint of the board, linked to the shown sheet's first symbol.
pub(super) const FOOTPRINT: Uuid = Uuid::from_u128(0x1307_0001);

/// A sheet and a board side by side.
pub(super) struct SheetAndBoard {
    pub app: Signex,
    pub sheet: PathBuf,
    pub board: PathBuf,
    /// The sheet's symbol the footprint stands for.
    pub symbol: Uuid,
}

fn footprint(symbol: Uuid) -> Footprint {
    let pad = Pad {
        uuid: Uuid::from_u128(0x1307_0002),
        number: "1".into(),
        pad_type: PadType::Smd,
        shape: PadShape::Rect,
        position: Point::new(0.0, 0.0),
        size: Point::new(1.0, 1.0),
        drill: None,
        layers: vec!["TopCopper".into()],
        net: None,
        roundrect_ratio: 0.0,
        trapezoid_delta: Default::default(),
        custom_outline: Vec::new(),
        rotation: 0.0,
        layer_sizes: None,
        net_locked: false,
        thermal: Default::default(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    };
    Footprint {
        uuid: FOOTPRINT,
        reference: "R1".into(),
        value: String::new(),
        footprint_id: String::new(),
        position: Point::new(10.0, 10.0),
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        pads: vec![pad],
        graphics: Vec::new(),
        properties: Vec::new(),
        schematic_link: Some(SymbolLink::new(InstancePath::root(), [symbol])),
        library_ref: None,
        component: Default::default(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

/// The root sheet (`power`) on the left and a board on the right, the board
/// focused.
pub(super) fn sheet_and_board() -> SheetAndBoard {
    let (mut app, _) = app();
    let sheet = path("power.snxsch");
    let symbol = app.document_state.engines[&sheet].document().symbols[0].uuid;
    let mut board = crate::app::handlers::document_files::save_async_tests::empty_board();
    board.footprints.push(footprint(symbol));
    let board_path = path("buck.snxpcb");
    let _ = app.update(Message::File(FileMsg::PcbOpenFinished {
        disk: None,
        path: board_path.clone(),
        title: "buck.snxpcb".to_string(),
        result: Ok(Box::new(board)),
    }));
    let board_tab = app
        .document_state
        .tabs
        .iter()
        .position(|tab| tab.path == board_path)
        .expect("the board's tab");
    // The sheet stays in the left pane: it is the active tab when the board
    // moves right.
    app.document_state.active_tab = 0;
    app.sync_active_tab();
    app.split_document_pane(board_tab, DropEdge::Right, false)
        .expect("split");
    assert!(app.has_active_pcb(), "the board is focused");
    SheetAndBoard {
        app,
        sheet,
        board: board_path,
        symbol,
    }
}

/// The board pane keeps its pan and zoom while the sheet has the focus, and
/// gives them back when the focus returns.
#[test]
fn the_board_pane_keeps_its_view_while_the_sheet_is_focused() {
    let SheetAndBoard { mut app, board, .. } = sheet_and_board();
    let view = (5.0, 6.0, 20.0);
    app.interaction_state.pcb_canvas.set_pane_camera(view);
    let _ = app.update(Message::Window(WindowMsg::TilePanePressed(0)));
    assert!(!app.has_active_pcb(), "the sheet is focused");
    let kept = app.interaction_state.tile_panes.board[&board].live_camera();
    assert_eq!(kept, view);
    let _ = app.update(Message::Window(WindowMsg::TilePanePressed(1)));
    assert_eq!(app.interaction_state.pcb_canvas.live_camera(), view);
}
