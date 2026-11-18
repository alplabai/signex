//! `add_board_graphic` and `set_board_graphic` (#2424): the Graphics tools
//! and the drawing's Properties fields as an agent or the CLI runs them, on
//! the board `document` names. Points and X/Y values are read from the board
//! origin with Y up, as the status bar and Properties show them. Each edit is
//! one board undo step through the same engine commands a click or a field
//! runs, and answers with the drawing as JSON.

use iced::Task;
use serde_json::{Value, json};
use signex_engine::pcb::graphic as gfx;
use signex_engine::pcb::primitive_edit::{GraphicEdit, graphic_edit_command};
use signex_i18n::{ids, t};
use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{BoardGraphic, PcbBoard, Point};
use signex_types::pcb_graphic::{BoardGraphicKind, takes_board_graphics, text_from_field};
use uuid::Uuid;

use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::pcb_tools::BoardPlaceTool;
use crate::app::state::pcb_edit::CheckedStep;
use crate::app::{Message, Signex};
use crate::panels::pcb_primitive_properties::parse_number;

type Ran = (CommandOutcome, Task<Message>);

/// The fields `set_board_graphic` takes, in the order its error lists them.
const FIELDS: [&str; 11] = [
    "layer",
    "width",
    "start_x",
    "start_y",
    "end_x",
    "end_y",
    "center_x",
    "center_y",
    "radius",
    "text",
    "text_height",
];

fn refused(reason: impl Into<String>) -> Ran {
    (CommandOutcome::disabled(reason), Task::none())
}

fn text<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.as_str()),
        _ => None,
    }
}

fn point(args: &CommandArgs, name: &str) -> Option<Point> {
    match args.operand(name) {
        Some(OperandValue::Point(x, y)) => Some(Point::new(*x, *y)),
        _ => None,
    }
}

/// The drawing as an answer carries it.
fn answer(path: &std::path::Path, graphic: &BoardGraphic, revision: u64) -> Value {
    json!({
        "document": path.display().to_string(),
        "id": graphic.uuid.to_string(),
        "kind": graphic.kind().map(BoardGraphicKind::id),
        "layer": graphic.layer,
        "revision": revision,
        "graphic": serde_json::to_value(graphic).unwrap_or(Value::Null),
    })
}

impl Signex {
    /// The board `document` names, when it is open and can be edited.
    pub(super) fn editable_named_board(
        &self,
        args: &CommandArgs,
    ) -> Result<std::path::PathBuf, String> {
        let path = self.named_output_board(args)?;
        let engine = self
            .document_state
            .board_engine(&path)
            .ok_or("the board is not open")?;
        if engine.board().unknown.newer_minor_source.is_some() {
            return Err(t!(self.ui_state.i18n, ids::PCB_PRIM_READ_ONLY).into_string());
        }
        Ok(path)
    }

    /// Run `command` on the board at `path` as one undo step labelled
    /// `label`, then answer with the drawing `id` as it is.
    fn run_graphic_command(
        &mut self,
        path: &std::path::Path,
        command: signex_engine::pcb::BoardCommand,
        label: &str,
        id: Uuid,
    ) -> Result<Value, String> {
        let revision = self
            .document_state
            .board_engine(path)
            .map(|engine| engine.revision())
            .ok_or("the board is not open")?;
        let step = CheckedStep {
            label,
            expected_revision: revision,
        };
        self.execute_board_commands(path, vec![command], Some(step))
            .map_err(|error| error.to_string())?;
        let engine = self
            .document_state
            .board_engine(path)
            .ok_or("the board is not open")?;
        let graphic = gfx::graphic_of(engine.board(), id).ok_or("the drawing is gone")?;
        Ok(answer(path, graphic, engine.revision()))
    }
}

/// The drawing the operands of `add_board_graphic` make on `board`.
fn new_graphic(app: &Signex, board: &PcbBoard, args: &CommandArgs) -> Result<BoardGraphic, String> {
    let kind = text(args, "kind")
        .and_then(BoardGraphicKind::from_id)
        .ok_or_else(|| {
            let kinds: Vec<&str> = BoardGraphicKind::ALL.iter().map(|k| k.id()).collect();
            format!("`kind` is one of {}", kinds.join(", "))
        })?;
    let path = app.named_output_board(args)?;
    let layer = match text(args, "layer") {
        Some(token) => parse_pcb_layer(token.trim())
            .ok_or_else(|| format!("`{token}` is not a board layer name"))?,
        None => app.pcb_layer_view(&path).active(),
    };
    if !takes_board_graphics(layer) {
        return Err(
            "a drawing goes on a non-copper layer other than a keep-out; name one with `layer`"
                .to_string(),
        );
    }
    let origin = board.options.origin;
    let need = |name: &str| {
        point(args, name)
            .map(|p| origin.from_user(p))
            .ok_or_else(|| format!("a {} needs the operand `{name}`", kind.id()))
    };
    let defaults = &app.interaction_state.pcb_tool_defaults;
    let style = defaults.graphic_style(layer);
    let id = Uuid::now_v7();
    let drawing = match kind {
        BoardGraphicKind::Line => gfx::new_line(id, need("start")?, need("end")?, style),
        BoardGraphicKind::Arc => gfx::new_arc(id, need("at")?, need("start")?, need("end")?, style),
        BoardGraphicKind::Circle => gfx::new_circle(id, need("at")?, need("end")?, style),
        BoardGraphicKind::Rectangle => gfx::new_rectangle(id, need("start")?, need("end")?, style),
        BoardGraphicKind::TextFrame => {
            let body = text(args, "text").map_or_else(
                || t!(app.ui_state.i18n, ids::PCB_TOOL_TEXT_FRAME_TEXT).into_string(),
                text_from_field,
            );
            let (a, b) = (need("start")?, need("end")?);
            gfx::new_text_frame(id, a, b, style, body, defaults.frame_text_height())
        }
    };
    drawing.ok_or_else(|| format!("the points make no {}", kind.id()))
}

/// `add_board_graphic document=<board> kind=<kind> [at=x,y] [start=x,y]
/// [end=x,y] [layer=..] [text=..]`: one drawing, one undo step.
pub(super) fn add_board_graphic(app: &mut Signex, args: &CommandArgs) -> Ran {
    let path = match app.editable_named_board(args) {
        Ok(path) => path,
        Err(reason) => return refused(reason),
    };
    let Some(board) = app.document_state.pcb_board(&path) else {
        return refused("the board is not open");
    };
    let drawing = match new_graphic(app, board, args) {
        Ok(drawing) => drawing,
        Err(reason) => return refused(reason),
    };
    let id = drawing.uuid;
    let label = drawing
        .kind()
        .map(|kind| t!(app.ui_state.i18n, BoardPlaceTool::for_graphic(kind).name()).into_string())
        .unwrap_or_default();
    match app.run_graphic_command(&path, gfx::place_graphic(drawing), &label, id) {
        Ok(answer) => (CommandOutcome::Answered(answer), Task::none()),
        Err(reason) => refused(reason),
    }
}

/// The edit `field=value` stands for, X and Y read from `board`'s origin.
fn graphic_field(board: &PcbBoard, field: &str, value: &str) -> Result<GraphicEdit, String> {
    let origin = board.options.origin;
    let number =
        || parse_number(value).ok_or_else(|| format!("`{value}` is not a number for `{field}`"));
    Ok(match field.trim() {
        "layer" => GraphicEdit::Layer(
            parse_pcb_layer(value.trim())
                .ok_or_else(|| format!("`{value}` is not a board layer name"))?,
        ),
        "width" => GraphicEdit::Width(number()?),
        "start_x" => GraphicEdit::StartX(origin.board_x(number()?)),
        "start_y" => GraphicEdit::StartY(origin.board_y(number()?)),
        "end_x" => GraphicEdit::EndX(origin.board_x(number()?)),
        "end_y" => GraphicEdit::EndY(origin.board_y(number()?)),
        "center_x" => GraphicEdit::CenterX(origin.board_x(number()?)),
        "center_y" => GraphicEdit::CenterY(origin.board_y(number()?)),
        "radius" => GraphicEdit::Radius(number()?),
        "text" => GraphicEdit::Text(text_from_field(value)),
        "text_height" => GraphicEdit::TextHeight(number()?),
        other => {
            return Err(format!(
                "`{other}` is not a drawing field (one of {})",
                FIELDS.join(", ")
            ));
        }
    })
}

/// `set_board_graphic document=<board> id=<uuid> field=<field> value=<v>`:
/// one field of one drawing, one undo step; nothing when it already reads so.
pub(super) fn set_board_graphic(app: &mut Signex, args: &CommandArgs) -> Ran {
    let path = match app.editable_named_board(args) {
        Ok(path) => path,
        Err(reason) => return refused(reason),
    };
    let Some(id) = text(args, "id").and_then(|id| Uuid::parse_str(id.trim()).ok()) else {
        return refused("`id` is a drawing's uuid");
    };
    let (Some(field), Some(value)) = (text(args, "field"), text(args, "value")) else {
        return refused("needs the operands `field` and `value`");
    };
    let Some(board) = app.document_state.pcb_board(&path) else {
        return refused("the board is not open");
    };
    let Some(graphic) = gfx::graphic_of(board, id) else {
        return refused(format!("no drawing {id} on the board"));
    };
    let edit = match graphic_field(board, field, value) {
        Ok(edit) => edit,
        Err(reason) => return refused(reason),
    };
    let command = match graphic_edit_command(board, &[id], &edit) {
        Ok(Some(command)) => command,
        Ok(None) => {
            let revision = app
                .document_state
                .board_engine(&path)
                .map_or(0, |engine| engine.revision());
            let mut unchanged = answer(&path, graphic, revision);
            unchanged["changed"] = json!(false);
            return (CommandOutcome::Answered(unchanged), Task::none());
        }
        Err(refusal) => return refused(refusal.to_string()),
    };
    let label = t!(app.ui_state.i18n, field_label(field)).into_string();
    match app.run_graphic_command(&path, command, &label, id) {
        Ok(mut answer) => {
            answer["changed"] = json!(true);
            (CommandOutcome::Answered(answer), Task::none())
        }
        Err(reason) => refused(reason),
    }
}

/// The catalog name of a drawing field, for the undo label.
fn field_label(field: &str) -> signex_i18n::MessageId {
    match field.trim() {
        "layer" => ids::PCB_PRIM_LAYER,
        "start_x" | "start_y" => ids::PCB_PRIM_START,
        "end_x" | "end_y" => ids::PCB_PRIM_END,
        "center_x" | "center_y" => ids::PCB_PRIM_GRAPHIC_CENTER,
        "radius" => ids::PCB_PRIM_GRAPHIC_RADIUS,
        "text" => ids::PCB_PRIM_GRAPHIC_TEXT,
        "text_height" => ids::PCB_PRIM_GRAPHIC_TEXT_HEIGHT,
        _ => ids::PCB_PRIM_WIDTH,
    }
}
