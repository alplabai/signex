//! Mockup row 207: cross probing works across the panes of the document
//! area. A part selected on the focused sheet outlines its footprint in the
//! board pane, and a footprint selected on the focused board outlines its
//! symbol in the sheet pane; with Cross Select Mode off nothing is outlined.

use pcb_geom::ItemRef;
use signex_types::schematic::{SelectedItem, SelectedKind};

use crate::app::handlers::pane_views_tests::{FOOTPRINT, SheetAndBoard, sheet_and_board};
use crate::app::{CrossProbeMsg, Message, UiMsg, WindowMsg};

fn outlined(app: &crate::app::Signex, board: &std::path::Path) -> bool {
    let highlight = app.interaction_state.tile_panes.board[board].highlight();
    !highlight.selected.is_empty() || !highlight.bodies.is_empty()
}

#[test]
fn a_part_selected_on_the_sheet_is_outlined_on_the_board_pane() {
    let SheetAndBoard {
        mut app,
        board,
        symbol,
        ..
    } = sheet_and_board();
    let _ = app.update(Message::Window(WindowMsg::TilePanePressed(0)));
    assert!(!outlined(&app, &board), "nothing selected yet");

    app.interaction_state.canvas.selected = vec![SelectedItem::new(symbol, SelectedKind::Symbol)];
    let _ = app.update(Message::Ui(UiMsg::Noop));
    assert!(outlined(&app, &board), "the footprint is outlined");

    app.interaction_state.canvas.selected.clear();
    let _ = app.update(Message::Ui(UiMsg::Noop));
    assert!(!outlined(&app, &board), "the outline follows the selection");
}

#[test]
fn a_footprint_selected_on_the_board_is_outlined_on_the_sheet_pane() {
    let SheetAndBoard {
        mut app,
        sheet,
        symbol,
        ..
    } = sheet_and_board();
    app.select_pcb_items(&[ItemRef::Footprint(FOOTPRINT)]);
    let _ = app.update(Message::Ui(UiMsg::Noop));
    let pane = &app.interaction_state.tile_panes.schematic[&sheet];
    assert_eq!(
        pane.selected,
        [SelectedItem::new(symbol, SelectedKind::Symbol)]
    );
}

#[test]
fn with_cross_select_off_the_panes_are_not_outlined() {
    let SheetAndBoard {
        mut app,
        board,
        symbol,
        ..
    } = sheet_and_board();
    let _ = app.update(Message::CrossProbe(CrossProbeMsg::CrossSelectModeToggled));
    let _ = app.update(Message::Window(WindowMsg::TilePanePressed(0)));
    app.interaction_state.canvas.selected = vec![SelectedItem::new(symbol, SelectedKind::Symbol)];
    let _ = app.update(Message::Ui(UiMsg::Noop));
    assert!(!outlined(&app, &board));
}
