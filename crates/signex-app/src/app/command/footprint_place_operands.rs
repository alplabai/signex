//! #2936: board footprints moved, turned and flipped by id on the board
//! `document` names, from the command line or a running Signex. Points are
//! read in the board's own frame (from the board origin, Y up), as
//! Properties and `measure_board` show them.
//!
//! Each command builds its edit with the engine function its GUI surface
//! uses, and runs it as one board undo step: a position moves as a drag
//! moves the footprint (the track ends on its pads follow, bent at the
//! session's corner rule); a rotation and a side are what the Component
//! view's Rotation and Layer rows set. A locked footprint refuses the whole
//! edit with the reason, and nothing changes. The answer is each footprint
//! as it is after the edit.

use std::collections::BTreeSet;
use std::path::Path;

use iced::Task;
use serde_json::{Value, json};
use signex_engine::pcb::BoardCommand;
use signex_engine::pcb::component::{self, LocationField};
use signex_engine::pcb::footprint_place::{self, FootprintPlace};
use signex_engine::pcb::placement;
use signex_i18n::{MessageId, ids, t};
use signex_types::format::mm_as_stored_nm;
use signex_types::numeric::parse_finite_f64;
use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_layer::BoardSide;
use uuid::Uuid;

use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::state::pcb_edit::CheckedStep;
use crate::app::{Message, Signex};

type Ran = (CommandOutcome, Task<Message>);

fn refused(reason: impl Into<String>) -> Ran {
    (CommandOutcome::disabled(reason), Task::none())
}

fn text<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.as_str()),
        _ => None,
    }
}

/// The side as the operand and the answer write it.
pub(super) fn side_id(side: BoardSide) -> &'static str {
    match side {
        BoardSide::Top => "top",
        BoardSide::Bottom => "bottom",
    }
}

fn side_of_id(text: &str) -> Option<BoardSide> {
    [BoardSide::Top, BoardSide::Bottom]
        .into_iter()
        .find(|side| side_id(*side) == text.trim())
}

/// One footprint as the answer carries it, in the board's own frame.
pub(super) fn footprint_answer(board: &PcbBoard, id: Uuid) -> Value {
    let Some(fp) = board.footprints.iter().find(|fp| fp.uuid == id) else {
        return Value::Null;
    };
    let at = board.options.origin.to_user(fp.position);
    json!({
        "id": fp.uuid,
        "designator": fp.reference,
        "position": {"x": at.x, "y": at.y},
        "rotation": fp.rotation,
        "side": side_id(component::side_of(fp)),
    })
}

/// The footprints the edit names: `ids`, `id` (a uuid) or `designator`,
/// exactly one of them.
pub(super) fn named_footprints(
    app: &Signex,
    path: &Path,
    args: &CommandArgs,
) -> Result<Vec<Uuid>, String> {
    let given = ["ids", "id", "designator"]
        .into_iter()
        .filter(|name| args.operand(name).is_some())
        .count();
    if given != 1 {
        return Err("name the footprints once: `id`, `designator` or `ids`".into());
    }
    if let Some(list) = args.targets("ids") {
        return Ok(list.to_vec());
    }
    if let Some(id) = text(args, "id") {
        let id = Uuid::parse_str(id.trim()).map_err(|_| format!("`id` {id} is not a uuid"))?;
        return Ok(vec![id]);
    }
    let typed = text(args, "designator").unwrap_or_default();
    // `-QAC1`, as the designator is shown, names the stored `QAC1` (#1416).
    let designator = app.document_state.stored_reference_for(path, typed);
    let board = app
        .document_state
        .pcb_board(path)
        .ok_or("the board is not open")?;
    placement::by_designator(board, designator).map(|id| vec![id])
}

/// The board point `x,y` of the board's frame stands for, in the model's.
fn model_point(board: &PcbBoard, x: f64, y: f64) -> Point {
    board.options.origin.from_user(Point::new(x, y))
}

/// `points=x,y;x,y`: one point per footprint of `ids`, in the board frame.
fn parse_points(textual: &str) -> Result<Vec<(f64, f64)>, String> {
    let number = |part: &str| {
        parse_finite_f64(part)
            .ok()
            .filter(|mm| mm_as_stored_nm(*mm).is_some())
    };
    textual
        .split(';')
        .map(|pair| {
            let (x, y) = pair
                .split_once(',')
                .ok_or_else(|| format!("`{pair}` in `points` is not x,y"))?;
            number(x)
                .zip(number(y))
                .ok_or_else(|| format!("`{pair}` in `points` is not x,y in mm"))
        })
        .collect()
}

/// Where each named footprint goes, in the model's frame.
fn places(
    board: &PcbBoard,
    ids: &[Uuid],
    args: &CommandArgs,
) -> Result<Vec<FootprintPlace>, String> {
    let points = match (args.operand("at"), text(args, "points")) {
        (Some(OperandValue::Point(x, y)), None) if ids.len() == 1 => vec![(*x, *y)],
        (None, Some(points)) => parse_points(points)?,
        (Some(OperandValue::PinRef { .. }), _) => {
            return Err("`at` is a board point x,y, not a pin reference".into());
        }
        _ => {
            return Err(
                "give `at` for one footprint, or `points` with one x,y per id of `ids`".into(),
            );
        }
    };
    if points.len() != ids.len() {
        return Err(format!(
            "`points` has {} points for {} footprints",
            points.len(),
            ids.len()
        ));
    }
    Ok(ids
        .iter()
        .zip(points)
        .map(|(&footprint, (x, y))| FootprintPlace {
            footprint,
            to: model_point(board, x, y),
        })
        .collect())
}

impl Signex {
    /// Run `commands` on the board at `path` as one undo step labelled
    /// `label`. Whether the board changed.
    fn run_footprint_step(
        &mut self,
        path: &Path,
        label: MessageId,
        commands: Vec<BoardCommand>,
    ) -> Result<bool, String> {
        if commands.is_empty() {
            return Ok(false);
        }
        let revision = self
            .document_state
            .board_engine(path)
            .map(|engine| engine.revision())
            .ok_or("the board is not open")?;
        let label = t!(self.ui_state.i18n, label).into_string();
        let step = CheckedStep {
            label: &label,
            expected_revision: revision,
        };
        self.execute_board_commands(path, commands, Some(step))
            .map(|patches| !patches.is_empty())
            .map_err(|error| error.to_string())
    }
}

/// The edit the operands name, run: the board, the footprints and whether
/// it changed, or why not.
fn run(
    app: &mut Signex,
    args: &CommandArgs,
    label: MessageId,
    build: impl FnOnce(&Signex, &PcbBoard, &[Uuid]) -> Result<Vec<BoardCommand>, String>,
) -> Ran {
    let path = match app.editable_named_board(args) {
        Ok(path) => path,
        Err(reason) => return refused(reason),
    };
    let ids = match named_footprints(app, &path, args) {
        Ok(ids) => ids,
        Err(reason) => return refused(reason),
    };
    let Some(board) = app.document_state.pcb_board(&path) else {
        return refused("the board is not open");
    };
    let commands = match build(app, board, &ids) {
        Ok(commands) => commands,
        Err(reason) => return refused(reason),
    };
    let changed = match app.run_footprint_step(&path, label, commands) {
        Ok(changed) => changed,
        Err(reason) => return refused(reason),
    };
    let Some(board) = app.document_state.pcb_board(&path) else {
        return refused("the board is not open");
    };
    let mut answer = if args.operand("ids").is_some() {
        let footprints: Vec<Value> = ids.iter().map(|id| footprint_answer(board, *id)).collect();
        json!({ "footprints": footprints })
    } else {
        footprint_answer(board, ids[0])
    };
    answer["document"] = json!(path.display().to_string());
    answer["changed"] = json!(changed);
    (CommandOutcome::Answered(answer), Task::none())
}

fn id_set(ids: &[Uuid]) -> BTreeSet<Uuid> {
    ids.iter().copied().collect()
}

/// `set_footprint_position document=<board> id=<uuid> at=x,y` (or
/// `designator=`, or `ids=` with `points=x,y;..`): each footprint's origin
/// to its point, as a drag takes it there.
pub(super) fn set_footprint_position(app: &mut Signex, args: &CommandArgs) -> Ran {
    run(app, args, ids::PCB_COMPONENT_POSITION, |app, board, ids| {
        let corner = pcb_geom::stretch_rule(app.interaction_state.corner_rule);
        let places = places(board, ids, args)?;
        let mut new_ids = signex_types::object_id::UuidV7Ids;
        footprint_place::position_commands(board, &places, corner, &mut new_ids)
            .map_err(|refusal| refusal.to_string())
    })
}

/// `set_footprint_rotation document=<board> id=<uuid> rotation=<deg>`: the
/// rotation the Component view's Rotation row sets, each footprint turning
/// about its own origin.
pub(super) fn set_footprint_rotation(app: &mut Signex, args: &CommandArgs) -> Ran {
    let Some(OperandValue::Angle(degrees)) = args.operand("rotation").cloned() else {
        return refused("`set_footprint_rotation` needs the operand `rotation`");
    };
    run(app, args, ids::PCB_COMPONENT_ROTATION, |_, board, ids| {
        component::location_commands(board, &id_set(ids), LocationField::Rotation, degrees)
            .map_err(|refusal| refusal.to_string())
    })
}

/// `set_footprint_side document=<board> id=<uuid> side=top|bottom`: each
/// footprint flipped onto that side, as the Component view's Layer row does.
pub(super) fn set_footprint_side(app: &mut Signex, args: &CommandArgs) -> Ran {
    let Some(side) = text(args, "side").and_then(side_of_id) else {
        return refused("`side` is `top` or `bottom`");
    };
    run(app, args, ids::PCB_COMPONENT_LAYER, |_, board, ids| {
        component::side_commands(board, &id_set(ids), side).map_err(|refusal| refusal.to_string())
    })
}
