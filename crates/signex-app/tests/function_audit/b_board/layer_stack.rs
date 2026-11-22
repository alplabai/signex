//! Row m105: the Layer Stack manager. A typed cell keeps a draft until
//! Enter; Enter, a Type pick, Add, Remove and Move are one undo step each.

use signex_app::panels::{LayerStackFacts, LayerStackMsg, PanelMsg, StackField};
use signex_types::pcb_stack::StackLayerKind;

use super::support::{Bench, four_layers, open};

fn facts(b: &Bench) -> LayerStackFacts {
    b.app
        .document_state
        .panel_ctx
        .layer_stack
        .clone()
        .expect("the manager shows the board")
}

fn stack(b: &mut Bench, msg: LayerStackMsg) {
    b.panel(PanelMsg::LayerStack(msg));
}

fn row_of(b: &Bench, kind: StackLayerKind) -> usize {
    facts(b)
        .stack
        .layers
        .iter()
        .position(|layer| layer.kind == kind)
        .expect("a row of that kind")
}

fn enter(b: &mut Bench, row: usize, field: StackField, text: &str) {
    let board = b.path.clone();
    stack(
        b,
        LayerStackMsg::FieldTyped {
            board: board.clone(),
            row,
            field,
            text: text.to_string(),
        },
    );
    stack(b, LayerStackMsg::FieldSubmitted { board, row, field });
}

fn select(b: &mut Bench, row: usize) {
    let board = b.path.clone();
    stack(b, LayerStackMsg::RowSelected { board, row });
}

#[test]
fn the_layer_stack_command_opens_its_document_tab() {
    let mut b = open(four_layers());
    b.view_only(|b| b.command("board_layer_stack"));
    // Layer Stack is a document tab (DECISIONS 2026-11-18).
    assert!(b.app.document_state.tabs.iter().any(|t| t.kind.is_layer_stack()));
}

#[test]
fn a_typed_cell_changes_nothing_until_enter() {
    let mut b = open(four_layers());
    let row = row_of(&b, StackLayerKind::Dielectric);
    let board = b.path.clone();
    b.refused(|b| {
        stack(
            b,
            LayerStackMsg::FieldTyped {
                board,
                row,
                field: StackField::Thickness,
                text: "0.3".to_string(),
            },
        );
    });
    assert_eq!(
        facts(&b)
            .drafts
            .get(&(row, StackField::Thickness))
            .map(String::as_str),
        Some("0.3")
    );
}

#[test]
fn a_thickness_edit_is_one_undo_step_and_undo_shows_the_old_stack() {
    let mut b = open(four_layers());
    let row = row_of(&b, StackLayerKind::Dielectric);
    let was = facts(&b).stack.layers[row].thickness_mm;
    let edited = b.one_step(|b| {
        enter(b, row, StackField::Thickness, "0.3");
        assert_eq!(facts(b).stack.layers[row].thickness_mm, 0.3, "the table");
        assert!(facts(b).stored, "the first edit saves it with the board");
    });
    assert_eq!(
        edited.stackup.expect("a stored stack").layers[row].thickness_mm,
        0.3
    );
    assert_eq!(facts(&b).stack.layers[row].thickness_mm, was);
    assert!(!facts(&b).stored, "undo goes back to the derived stack");
}

#[test]
fn a_row_name_and_material_edit_are_one_undo_step_each() {
    let mut b = open(four_layers());
    let row = row_of(&b, StackLayerKind::Dielectric);
    let edited = b.one_step(|b| enter(b, row, StackField::Name, "Core"));
    assert_eq!(edited.stackup.expect("stored").layers[row].name, "Core");
    let edited = b.one_step(|b| enter(b, row, StackField::Material, "FR-4"));
    assert_eq!(edited.stackup.expect("stored").layers[row].material, "FR-4");
}

#[test]
fn a_thickness_that_is_negative_or_not_a_number_is_refused() {
    let mut b = open(four_layers());
    let row = row_of(&b, StackLayerKind::Dielectric);
    for bad in ["-0.1", "abc"] {
        b.refused(|b| enter(b, row, StackField::Thickness, bad));
    }
}

#[test]
fn a_copper_row_turns_into_a_plane_in_one_undo_step() {
    let mut b = open(four_layers());
    let row = row_of(&b, StackLayerKind::Signal);
    let board = b.path.clone();
    let edited = b.one_step(|b| {
        stack(
            b,
            LayerStackMsg::KindPicked {
                board,
                row,
                kind: StackLayerKind::Plane,
            },
        );
    });
    assert_eq!(
        edited.stackup.expect("stored").layers[row].kind,
        StackLayerKind::Plane
    );
}

#[test]
fn a_dielectric_row_cannot_become_copper() {
    let mut b = open(four_layers());
    let row = row_of(&b, StackLayerKind::Dielectric);
    let board = b.path.clone();
    b.refused(|b| {
        stack(
            b,
            LayerStackMsg::KindPicked {
                board,
                row,
                kind: StackLayerKind::Signal,
            },
        );
    });
}

#[test]
fn adding_a_dielectric_below_the_selected_row_is_one_undo_step() {
    let mut b = open(four_layers());
    let rows = facts(&b).stack.layers.len();
    let row = row_of(&b, StackLayerKind::Dielectric);
    select(&mut b, row);
    let board = b.path.clone();
    let edited = b.one_step(|b| {
        stack(
            b,
            LayerStackMsg::LayerAdded {
                board,
                kind: StackLayerKind::Dielectric,
            },
        );
    });
    assert_eq!(edited.stackup.expect("stored").layers.len(), rows + 1);
}

#[test]
fn a_layer_of_a_kind_that_cannot_be_added_is_refused() {
    let mut b = open(four_layers());
    let board = b.path.clone();
    b.refused(|b| {
        stack(
            b,
            LayerStackMsg::LayerAdded {
                board,
                kind: StackLayerKind::Overlay,
            },
        );
    });
}

#[test]
fn removing_the_selected_row_is_one_undo_step() {
    let mut b = open(four_layers());
    let rows = facts(&b).stack.layers.len();
    let row = row_of(&b, StackLayerKind::Dielectric);
    select(&mut b, row);
    let board = b.path.clone();
    let edited = b.one_step(|b| stack(b, LayerStackMsg::RowRemoved { board }));
    assert_eq!(edited.stackup.expect("stored").layers.len(), rows - 1);
}

#[test]
fn moving_the_top_row_up_is_refused() {
    let mut b = open(four_layers());
    select(&mut b, 0);
    let board = b.path.clone();
    b.refused(|b| stack(b, LayerStackMsg::RowMoved { board, up: true }));
}

#[test]
fn moving_a_row_down_is_one_undo_step_and_the_selection_follows() {
    let mut b = open(four_layers());
    select(&mut b, 0);
    let first = facts(&b).stack.layers[0].kind;
    let board = b.path.clone();
    let edited = b.one_step(|b| {
        stack(b, LayerStackMsg::RowMoved { board, up: false });
        assert_eq!(facts(b).selected, Some(1));
    });
    assert_eq!(edited.stackup.expect("stored").layers[1].kind, first);
}
