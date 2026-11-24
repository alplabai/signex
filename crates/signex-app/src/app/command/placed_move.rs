//! The operand move of a placed object (#2552): `set_object_position` with
//! its wire handling as arguments, never read from `UiState` (ADR-0006).
//! It runs the `MoveSelection` a drag commits, through the drag's gateway, so
//! the rubber band and its guard are the drag's; the answer names the wires
//! that changed and every contact the guard could not keep.

use std::collections::HashMap;

use serde_json::{Value, json};
use signex_engine::EngineNotice;
use signex_types::corner::DragWiringMode;
use signex_types::schematic::{Point, SelectedItem, Wire};

use super::placed_operands::PlacedRefusal;
use super::{CommandArgs, OperandValue};
use crate::app::Signex;
use crate::app::mutation_gateway::MutationOutcome as Applied;

/// The `keep_wires` operand; left out, wires follow.
pub(super) fn keep_wires(args: &CommandArgs) -> bool {
    !matches!(args.operand("keep_wires"), Some(OperandValue::Bool(false)))
}

/// The `wiring` operand; left out, `auto`.
pub(super) fn wiring(args: &CommandArgs) -> Result<DragWiringMode, String> {
    let Some(value) = args.operand("wiring") else {
        return Ok(DragWiringMode::Auto);
    };
    let OperandValue::Text(word) = value else {
        return Err(WIRING_WORDS.to_string());
    };
    match word.trim() {
        "auto" => Ok(DragWiringMode::Auto),
        "horizontal_first" => Ok(DragWiringMode::HorizontalFirst),
        "vertical_first" => Ok(DragWiringMode::VerticalFirst),
        "straight" => Ok(DragWiringMode::Straight),
        other => Err(format!("{WIRING_WORDS}, got `{other}`")),
    }
}

const WIRING_WORDS: &str = "`wiring` is auto, horizontal_first, vertical_first or straight";

impl Signex {
    /// Put `item`'s position at `at` (stored mm), its wires following when
    /// `keep_wires`, bent by `wiring`. The answer adds `moved_wires` and
    /// `removed_wires` (wire ids) and `fallbacks`, the contacts kept as a
    /// plain move with the Messages line naming them.
    pub(crate) fn move_placed(
        &mut self,
        item: SelectedItem,
        at: Point,
        keep_wires: bool,
        wiring: DragWiringMode,
    ) -> Result<Value, PlacedRefusal> {
        let engine = self
            .document_state
            .edit_engine()
            .ok_or_else(|| PlacedRefusal::new("no schematic is open"))?;
        let before = engine.document().wires.clone();
        let command =
            signex_engine::move_to_command(engine.document(), &item, at, keep_wires, wiring)
                .ok_or_else(|| PlacedRefusal::new("this object has no position to set"))?;
        if let Some(reason) = engine.lock_refusal(&command) {
            return Err(PlacedRefusal::new(reason));
        }
        let (outcome, reason, notices) = self.apply_engine_command_noting(command);
        if outcome == Applied::Failed {
            return Err(PlacedRefusal {
                reason: reason.unwrap_or_else(|| "the engine refused the edit".into()),
                logged: true,
            });
        }
        let mut answer = self.placed_answer(item)?;
        let after = (self.document_state.edit_engine())
            .map(|engine| engine.document().wires.clone())
            .unwrap_or_default();
        let (moved, removed) = wire_changes(&before, &after);
        answer["moved_wires"] = json!(moved);
        answer["removed_wires"] = json!(removed);
        answer["fallbacks"] = notices.iter().map(fallback).collect();
        Ok(answer)
    }
}

/// The ids of wires that are new or have other ends, and of wires gone.
fn wire_changes(before: &[Wire], after: &[Wire]) -> (Vec<String>, Vec<String>) {
    let ends = |w: &Wire| (w.start, w.end);
    let was: HashMap<_, _> = before.iter().map(|w| (w.uuid, ends(w))).collect();
    let moved = (after.iter())
        .filter(|w| was.get(&w.uuid) != Some(&ends(w)))
        .map(|w| w.uuid.to_string())
        .collect();
    let removed = (before.iter())
        .filter(|w| !after.iter().any(|a| a.uuid == w.uuid))
        .map(|w| w.uuid.to_string())
        .collect();
    (moved, removed)
}

/// One fallback of the answer: its kind, what it names and its Messages line.
fn fallback(notice: &EngineNotice) -> Value {
    let message = crate::app::engine_notices::notice_line(&signex_i18n::I18n::en_us(), notice);
    let (kind, names): (&str, Vec<String>) = match notice {
        EngineNotice::PinsNotKept { pins } => ("pins_not_kept", pins.clone()),
        EngineNotice::DraggedWireLeft { held } => ("dragged_wire_left", held.clone()),
        EngineNotice::HeldPinsNotJoined { pins } => ("held_pins_not_joined", pins.clone()),
        EngineNotice::PinDotRefused { pins } => (
            "pin_dot_refused",
            pins.iter().map(|p| p.pin.clone()).collect(),
        ),
        EngineNotice::GripEndLeft { at } => {
            return json!({ "kind": "grip_end_left", "at": [at.x, at.y], "message": message });
        }
    };
    json!({ "kind": kind, "names": names, "message": message })
}
