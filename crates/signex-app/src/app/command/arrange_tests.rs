//! Arrange commands through the registry (#1583): they act on the drawings
//! in the selection only, and are disabled without one.

use std::path::PathBuf;

use super::CommandArgs;
use super::*;
use crate::keymap::AppCommandId;
use signex_types::schematic::{
    Point, SchDrawing, SchematicSheet, SelectedItem, SelectedKind, Wire,
};
use uuid::Uuid;

fn line(n: u128) -> SchDrawing {
    SchDrawing::Line {
        uuid: Uuid::from_u128(n),
        start: Point::new(n as f64, 0.0),
        end: Point::new(n as f64, 5.0),
        width: 0.25,
        stroke_color: None,
    }
}

fn wire_id() -> Uuid {
    Uuid::from_u128(900)
}

/// An app whose active sheet holds drawings 1..=4 and one wire.
fn app_with_drawings() -> Signex {
    let (mut app, _task) = Signex::new();
    let path = PathBuf::from("arrange.snxsch");
    let sheet = SchematicSheet {
        drawings: (1..=4).map(line).collect(),
        wires: vec![Wire {
            uuid: wire_id(),
            start: Point::new(0.0, 0.0),
            end: Point::new(5.0, 0.0),
            stroke_width: 0.25,
            ..Default::default()
        }],
        ..SchematicSheet::default()
    };
    app.document_state
        .engines
        .insert(path.clone(), signex_engine::Engine::new(sheet).unwrap());
    app.document_state.active_path = Some(path.clone());
    app.document_state.tabs.push(TabInfo {
        title: "doc".into(),
        path,
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app
}

fn select(app: &mut Signex, items: Vec<SelectedItem>) {
    app.interaction_state.active_canvas_mut().selected = items;
}

fn drawing(n: u128) -> SelectedItem {
    SelectedItem::new(Uuid::from_u128(n), SelectedKind::Drawing)
}

fn order(app: &Signex) -> Vec<u128> {
    app.document_state
        .active_engine()
        .unwrap()
        .document()
        .drawings
        .iter()
        .map(|d| d.uuid().as_u128())
        .collect()
}

fn run(app: &mut Signex, command: &str) {
    let id = AppCommandId::new(command).unwrap();
    let _ = app.dispatch_command(&id, CommandArgs::none(), None);
}

fn enabled(app: &Signex, command: &str) -> bool {
    let id = AppCommandId::new(command).unwrap();
    is_enabled(&id, &app.command_context(None))
}

const ARRANGE: [&str; 4] = [
    "bring_drawings_to_front",
    "send_drawings_to_back",
    "bring_drawings_forward",
    "send_drawings_backward",
];

#[test]
fn the_commands_are_disabled_without_a_drawing_in_the_selection() {
    let mut app = app_with_drawings();
    for command in ARRANGE {
        assert!(!enabled(&app, command), "{command}: no selection");
    }

    select(
        &mut app,
        vec![SelectedItem::new(wire_id(), SelectedKind::Wire)],
    );
    for command in ARRANGE {
        assert!(!enabled(&app, command), "{command}: only a wire selected");
    }

    select(&mut app, vec![drawing(2)]);
    for command in ARRANGE {
        assert!(enabled(&app, command), "{command}: a drawing is selected");
    }
}

#[test]
fn bring_to_front_reorders_the_selected_drawings_and_undo_restores() {
    let mut app = app_with_drawings();
    select(&mut app, vec![drawing(3), drawing(1)]);

    run(&mut app, "bring_drawings_to_front");
    assert_eq!(order(&app), [2, 4, 1, 3], "list order is kept among them");

    app.document_state
        .active_engine_mut()
        .unwrap()
        .undo()
        .unwrap();
    assert_eq!(order(&app), [1, 2, 3, 4]);
}

/// A mixed selection moves its drawings and leaves the circuit alone.
#[test]
fn a_mixed_selection_ignores_the_circuit_items() {
    let mut app = app_with_drawings();
    select(
        &mut app,
        vec![SelectedItem::new(wire_id(), SelectedKind::Wire), drawing(2)],
    );
    let wires_before = format!(
        "{:?}",
        app.document_state.active_engine().unwrap().document().wires
    );

    run(&mut app, "send_drawings_to_back");

    assert_eq!(order(&app), [2, 1, 3, 4]);
    let wires_after = format!(
        "{:?}",
        app.document_state.active_engine().unwrap().document().wires
    );
    assert_eq!(wires_before, wires_after);
}

#[test]
fn forward_and_backward_step_one_place() {
    let mut app = app_with_drawings();
    select(&mut app, vec![drawing(2)]);

    run(&mut app, "bring_drawings_forward");
    assert_eq!(order(&app), [1, 3, 2, 4]);
    run(&mut app, "send_drawings_backward");
    assert_eq!(order(&app), [1, 2, 3, 4]);
}

#[test]
fn a_disabled_command_leaves_the_document_alone() {
    let mut app = app_with_drawings();
    select(
        &mut app,
        vec![SelectedItem::new(wire_id(), SelectedKind::Wire)],
    );

    run(&mut app, "bring_drawings_to_front");

    assert_eq!(order(&app), [1, 2, 3, 4]);
}

/// The Properties panel's Placement row sets one drawing above the circuit
/// through the engine, and undo takes it back.
#[test]
fn the_placement_row_sets_a_drawing_above_and_undo_restores() {
    use crate::app::contracts::DrawingFieldEdit;
    use signex_types::schematic::DrawingPlacement;

    let mut app = app_with_drawings();
    let placement = |app: &Signex| {
        app.document_state
            .active_engine()
            .unwrap()
            .document()
            .drawing_placement(Uuid::from_u128(2))
    };

    let _ = app.dispatch_edit_message(EditMsg::UpdateDrawingField(
        Uuid::from_u128(2),
        DrawingFieldEdit::Placement(DrawingPlacement::Above),
    ));
    assert_eq!(placement(&app), DrawingPlacement::Above);
    // The canvas draws from its render cache, which must see it too.
    let cached = app
        .interaction_state
        .active_canvas()
        .active_snapshot()
        .expect("the active sheet has a render snapshot")
        .drawing_placement(Uuid::from_u128(2));
    assert_eq!(cached, DrawingPlacement::Above);

    app.document_state
        .active_engine_mut()
        .unwrap()
        .undo()
        .unwrap();
    assert_eq!(placement(&app), DrawingPlacement::Behind);
}
