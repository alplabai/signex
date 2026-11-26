//! `set_object_rotation` takes `keep_wires` as an operand, default `true`, and
//! never reads the person's "keep wires" preference (Hakan, 2026-11-25, #2326
//! comment 7167743982); the Properties Rotation row still passes it.

use serde_json::Value;
use signex_types::schematic::{SchematicSheet, SelectedItem, SelectedKind, Wire};
use uuid::Uuid;

use super::operands::OperandError;
use super::placed_move_tests::{app_with, doc, resistor_with, shape, wire};
use super::{CommandArgs, CommandOutcome};
use crate::app::Signex;
use crate::keymap::AppCommandId;

/// R1 at (50, 50) with a wire into pin 1 at (45, 50).
fn wired_resistor() -> (SchematicSheet, Uuid) {
    resistor_with(vec![wire((30.0, 50.0), (45.0, 50.0))])
}

fn rotation_args(r1: Uuid, extra: &[&str]) -> Result<CommandArgs, OperandError> {
    let command = AppCommandId::new("set_object_rotation").expect("a catalog id");
    let mut operands = vec![format!("id={r1}"), "rotation=90".to_string()];
    operands.extend(extra.iter().map(|s| (*s).to_string()));
    let refs: Vec<&str> = operands.iter().map(String::as_str).collect();
    CommandArgs::parse_operands(&command, &refs)
}

/// The agent's `set_object_rotation` of R1 to 90 degrees; its answer.
fn agent_turn(app: &mut Signex, r1: Uuid, extra: &[&str]) -> Value {
    let command = AppCommandId::new("set_object_rotation").expect("a catalog id");
    let args = rotation_args(r1, extra).expect("operands");
    match app.run_operands_settled(&command, &args).0 {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    }
}

/// 90 degrees picked in R1's Rotation row, as a person does.
fn pick_rotation(app: &mut Signex, r1: Uuid) {
    use crate::panels::{PanelMsg, PlacedEditMsg};
    let item = SelectedItem::new(r1, SelectedKind::Symbol);
    app.interaction_state.active_canvas_mut().selected = vec![item];
    app.update_selection_info();
    let msg = PlacedEditMsg::RotationPicked {
        item,
        degrees: 90.0,
    };
    let _ = app.update(crate::app::Message::Dock(crate::dock::DockMessage::Panel(
        PanelMsg::PlacedEdit(msg),
    )));
}

fn rotation_of(app: &Signex, r1: Uuid) -> f64 {
    let part = doc(app).symbols.iter().find(|s| s.uuid == r1);
    part.expect("R1").rotation
}

/// Left out, `keep_wires` is true: the wires follow even with the person's
/// preference off, as the Rotation row's turn with the preference on.
#[test]
fn a_rotation_keeps_wires_by_default_whatever_the_persons_preference() {
    let (sheet, r1) = wired_resistor();
    let (mut gui, _g) = app_with(sheet.clone());
    gui.ui_state.move_keeps_wires = true;
    pick_rotation(&mut gui, r1);
    let (mut agent, _a) = app_with(sheet.clone());
    agent.ui_state.move_keeps_wires = false;
    let answer = agent_turn(&mut agent, r1, &[]);

    assert_eq!(answer["rotation"], 90.0, "{answer}");
    assert_ne!(doc(&agent).wires, sheet.wires, "the wires followed");
    assert_eq!(shape(doc(&agent)), shape(doc(&gui)));
}

/// `keep_wires=false` is a plain turn: the wires stay byte-equal, even with
/// the person's preference on.
#[test]
fn keep_wires_false_turns_the_part_alone() {
    let (sheet, r1) = wired_resistor();
    let (mut agent, _a) = app_with(sheet.clone());
    agent.ui_state.move_keeps_wires = true;
    let answer = agent_turn(&mut agent, r1, &["keep_wires=false"]);

    assert_eq!(answer["rotation"], 90.0, "{answer}");
    assert_eq!(rotation_of(&agent, r1), 90.0);
    assert_eq!(doc(&agent).wires, sheet.wires);
}

/// The Rotation row in Properties still passes the person's preference: off,
/// the wires stay byte-equal; on, they follow.
#[test]
fn the_rotation_row_honours_the_persons_keep_wires() {
    let (sheet, r1) = wired_resistor();
    for keeps in [false, true] {
        let (mut app, _d) = app_with(sheet.clone());
        app.ui_state.move_keeps_wires = keeps;
        pick_rotation(&mut app, r1);

        assert_eq!(rotation_of(&app, r1), 90.0, "keeps {keeps}");
        let wires: &[Wire] = &doc(&app).wires;
        assert_eq!(wires == sheet.wires.as_slice(), !keeps, "keeps {keeps}");
    }
}

/// A `keep_wires` that is not `true` or `false` is refused by name.
#[test]
fn a_keep_wires_that_is_not_a_bool_is_refused() {
    let r1 = Uuid::now_v7();
    for junk in ["maybe", "1", "yes", ""] {
        let operand = format!("keep_wires={junk}");
        match rotation_args(r1, &[&operand]) {
            Err(OperandError::Invalid { operand, .. }) => assert_eq!(operand, "keep_wires"),
            other => panic!("`{junk}` not refused: {other:?}"),
        }
    }
}
