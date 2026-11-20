//! Move Selection reads ΔX / ΔY in the display unit: 100 typed while the
//! unit is mil moves the part 2.54 mm, and one Undo takes it back.

use signex_app::app::{EditMsg, Message, MoveSelectionMsg};
use signex_types::coord::Unit;
use signex_types::schematic::{SelectedItem, SelectedKind};

use crate::dirty_paths_gateway::{app_with, symbol};

fn x_of(app: &signex_app::app::Signex) -> f64 {
    app.document_state
        .active_engine()
        .expect("engine")
        .document()
        .symbols[0]
        .position
        .x
}

#[test]
fn a_mil_offset_moves_the_part_in_mm_and_undoes_in_one_step() {
    let moved = uuid::Uuid::now_v7();
    let (mut app, _path) = app_with(vec![symbol(moved, "R1", 50.0)]);
    app.ui_state.unit = Unit::Mil;
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(moved, SelectedKind::Symbol)];
    let _ = app.update(Message::MoveSelection(MoveSelectionMsg::Open));
    let _ = app.update(Message::MoveSelection(MoveSelectionMsg::DxChanged(
        "100".to_string(),
    )));
    let _ = app.update(Message::MoveSelection(MoveSelectionMsg::DyChanged(
        "0".to_string(),
    )));
    let _ = app.update(Message::MoveSelection(MoveSelectionMsg::Apply));

    let x = x_of(&app);
    assert!((x - 52.54).abs() < 1e-9, "100 mil is 2.54 mm: got {x}");

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert!((x_of(&app) - 50.0).abs() < 1e-9, "one Undo restores it");
}
