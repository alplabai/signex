//! Wire merge and break against the app's per-wire state (#1254): net colours
//! and the selection both name wires by uuid.

use signex_types::schematic::{Point, SchematicSheet, SelectedItem, SelectedKind, Wire};
use signex_types::theme::Color;

use super::super::super::*;

fn id(n: u128) -> uuid::Uuid {
    uuid::Uuid::from_u128(n)
}

/// One wire `(0, 25.4)-(5.08, 25.4)` on a grid-snapping sheet, the Wire tool
/// armed.
fn app_with_first_leg() -> Signex {
    let _ = crate::diagnostics::init_logging();
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        wires: vec![Wire {
            uuid: id(1),
            start: Point::new(0.0, 25.4),
            end: Point::new(5.08, 25.4),
            ..Default::default()
        }],
        ..Default::default()
    };
    let path = std::path::PathBuf::from("wire-merge.snxsch");
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.snap_enabled = true;
    app.ui_state.grid_size_mm = 1.27;
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet).expect("engine"),
    );
    app.document_state.tabs.push(TabInfo {
        title: "wire-merge".to_string(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app
}

fn click(app: &mut Signex, x: f64, y: f64) {
    let _ = app.update(Message::CanvasEvent(crate::canvas::CanvasEvent::Clicked {
        world_x: x,
        world_y: y,
    }));
}

fn wires(app: &Signex) -> Vec<Wire> {
    app.document_state
        .active_engine()
        .expect("engine")
        .document()
        .wires
        .clone()
}

const RED: Color = Color::new(255, 0, 0, 255);

#[test]
fn a_wire_drawn_on_in_another_colour_is_not_folded_into_a_coloured_one() {
    let mut app = app_with_first_leg();
    app.ui_state.wire_color_overrides.insert(id(1), RED);
    app.interaction_state.current_tool = Tool::Wire;

    click(&mut app, 5.08, 25.4);
    click(&mut app, 10.16, 25.4);

    assert_eq!(wires(&app).len(), 2, "the red wire keeps its own extent");
    assert_eq!(app.ui_state.wire_color_overrides.get(&id(1)), Some(&RED));
}

#[test]
fn a_wire_drawn_on_with_no_colour_still_folds_into_an_uncoloured_one() {
    let mut app = app_with_first_leg();
    app.interaction_state.current_tool = Tool::Wire;

    click(&mut app, 5.08, 25.4);
    click(&mut app, 10.16, 25.4);

    assert_eq!(wires(&app).len(), 1);
}

#[test]
fn both_halves_of_a_broken_wire_keep_its_net_colour() {
    let mut app = app_with_first_leg();
    app.ui_state.wire_color_overrides.insert(id(1), RED);
    app.interaction_state.current_tool = Tool::BreakWire;

    click(&mut app, 2.54, 25.4);

    let halves = wires(&app);
    assert_eq!(halves.len(), 2);
    for half in &halves {
        assert_eq!(
            app.ui_state.wire_color_overrides.get(&half.uuid),
            Some(&RED),
            "half {}",
            half.uuid
        );
    }
}

#[test]
fn a_merge_never_leaves_the_selection_naming_the_folded_wire() {
    let mut app = app_with_first_leg();
    let folded = Wire {
        uuid: id(2),
        start: Point::new(5.08, 25.4),
        end: Point::new(10.16, 25.4),
        ..Default::default()
    };
    app.interaction_state.active_canvas_mut().selected = vec![
        SelectedItem::new(id(1), SelectedKind::Wire),
        SelectedItem::new(id(2), SelectedKind::Wire),
    ];

    app.apply_engine_command(
        signex_engine::Command::PlaceWireSegment { wire: folded },
        true,
        true,
    );

    assert_eq!(wires(&app).len(), 1, "the wires folded");
    assert_eq!(
        app.interaction_state.active_canvas().selected,
        vec![SelectedItem::new(id(1), SelectedKind::Wire)]
    );
}

/// The single-command path hands the engine the net colours too: moving a red
/// wire into line with an uncoloured one must not fold them.
#[test]
fn moving_a_red_wire_into_line_with_an_uncoloured_one_does_not_fold_them() {
    let mut app = app_with_first_leg();
    let red_wire = Wire {
        uuid: id(2),
        start: Point::new(5.08, 30.48),
        end: Point::new(10.16, 30.48),
        ..Default::default()
    };
    app.apply_engine_command(
        signex_engine::Command::PlaceWireSegment { wire: red_wire },
        true,
        true,
    );
    app.ui_state.wire_color_overrides.insert(id(2), RED);
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(id(2), SelectedKind::Wire)];
    app.ui_state.move_selection.dx = "0".to_string();
    app.ui_state.move_selection.dy = "-5.08".to_string();

    let _ = app.update(Message::MoveSelection(MoveSelectionMsg::Apply));

    let after = wires(&app);
    assert_eq!(after.len(), 2, "different colours stay two wires");
    assert!((after[1].start.y - 25.4).abs() < 1e-9, "the wire did move");
}

#[test]
fn the_same_move_folds_when_neither_wire_is_coloured() {
    let mut app = app_with_first_leg();
    let wire = Wire {
        uuid: id(2),
        start: Point::new(5.08, 30.48),
        end: Point::new(10.16, 30.48),
        ..Default::default()
    };
    app.apply_engine_command(
        signex_engine::Command::PlaceWireSegment { wire },
        true,
        true,
    );
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(id(2), SelectedKind::Wire)];
    app.ui_state.move_selection.dx = "0".to_string();
    app.ui_state.move_selection.dy = "-5.08".to_string();

    let _ = app.update(Message::MoveSelection(MoveSelectionMsg::Apply));

    assert_eq!(wires(&app).len(), 1);
}
