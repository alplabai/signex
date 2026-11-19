//! `set_footprint_text` (#2368): a component's designator or comment placed
//! on its own, as a drag on the board moves it, by an agent or the CLI on the
//! board `document` names. X and Y are read from the board origin with Y up,
//! as Properties shows them. One board undo step through the same engine
//! command a drag runs; the answer is the text as it is.

use iced::Task;
use pcb_geom::BoardTextField;
use serde_json::{Value, json};
use signex_engine::pcb::footprint_text_edit::FootprintTextEdit;
use signex_engine::pcb::placement;
use signex_types::pcb::{PcbBoard, Point};
use signex_types::schematic::HAlign;
use uuid::Uuid;

use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::{Message, Signex};
use crate::panels::pcb_primitive_properties::parse_number;

type Ran = (CommandOutcome, Task<Message>);

/// The fields `set_footprint_text` takes, in the order its error lists them.
const FIELDS: [&str; 5] = ["x", "y", "rotation", "height", "justify"];

fn refused(reason: impl Into<String>) -> Ran {
    (CommandOutcome::disabled(reason), Task::none())
}

fn text<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.as_str()),
        _ => None,
    }
}

fn justify_id(justify: HAlign) -> &'static str {
    match justify {
        HAlign::Left => "left",
        HAlign::Center => "center",
        HAlign::Right => "right",
    }
}

fn justify_of(value: &str) -> Option<HAlign> {
    [HAlign::Left, HAlign::Center, HAlign::Right]
        .into_iter()
        .find(|j| justify_id(*j) == value.trim())
}

/// The edit `field=value` stands for on `text` of `footprint`; X and Y are
/// read from `board`'s origin, the other axis kept.
fn text_field(
    board: &PcbBoard,
    footprint: Uuid,
    which: BoardTextField,
    field: &str,
    value: &str,
) -> Result<FootprintTextEdit, String> {
    let origin = board.options.origin;
    let number =
        || parse_number(value).ok_or_else(|| format!("`{value}` is not a number for `{field}`"));
    let at = || {
        board
            .footprints
            .iter()
            .find(|fp| fp.uuid == footprint)
            .and_then(|fp| pcb_geom::footprint_text(fp, which)?.ok())
            .map(|text| text.placement.anchor.to_mm())
            .ok_or_else(|| format!("the component has no {} text", which.id()))
    };
    Ok(match field.trim() {
        "x" => FootprintTextEdit::At(Point::new(origin.board_x(number()?), at()?.y)),
        "y" => FootprintTextEdit::At(Point::new(at()?.x, origin.board_y(number()?))),
        "rotation" => FootprintTextEdit::Rotation(number()?),
        "height" => FootprintTextEdit::Height(number()?),
        "justify" => FootprintTextEdit::Justify(
            justify_of(value)
                .ok_or_else(|| "`justify` is `left`, `center` or `right`".to_string())?,
        ),
        other => {
            return Err(format!(
                "`{other}` is not a text field (one of {})",
                FIELDS.join(", ")
            ));
        }
    })
}

/// The text as an answer carries it.
fn answer(board: &PcbBoard, footprint: Uuid, which: BoardTextField) -> Value {
    let text = board
        .footprints
        .iter()
        .find(|fp| fp.uuid == footprint)
        .and_then(|fp| pcb_geom::footprint_text(fp, which)?.ok());
    let Some(text) = text else {
        return Value::Null;
    };
    let at = board.options.origin.to_user(text.placement.anchor.to_mm());
    json!({
        "text": which.id(),
        "value": text.content,
        "x": at.x,
        "y": at.y,
        "rotation": text.placement.rotation_deg,
        "height": text.size_mm,
        "justify": justify_id(text.justify),
        "mirrored": text.placement.mirrored,
        "layer": signex_types::format::pcb_layer_token(text.layer),
    })
}

/// `set_footprint_text document=<board> designator=<ref>
/// text=designator|comment field=<field> value=<v>`: one field of one text,
/// one undo step; nothing when it already reads so.
pub(super) fn set_footprint_text(app: &mut Signex, args: &CommandArgs) -> Ran {
    let path = match app.editable_named_board(args) {
        Ok(path) => path,
        Err(reason) => return refused(reason),
    };
    let Some(which) = text(args, "text").and_then(|t| BoardTextField::of_id(t.trim())) else {
        return refused("`text` is `designator` or `comment`");
    };
    let (Some(designator), Some(field), Some(value)) = (
        text(args, "designator"),
        text(args, "field"),
        text(args, "value"),
    ) else {
        return refused("needs the operands `designator`, `field` and `value`");
    };
    // `-QAC1`, as the designator is shown, names the stored `QAC1` (#1416).
    let designator = app.document_state.stored_reference_for(&path, designator);
    let Some(board) = app.document_state.pcb_board(&path) else {
        return refused("the board is not open");
    };
    let built = placement::by_designator(board, designator)
        .and_then(|fp| Ok((fp, text_field(board, fp, which, field, value)?)));
    let (footprint, edit) = match built {
        Ok(built) => built,
        Err(reason) => return refused(reason),
    };
    match app.edit_footprint_text(&path, footprint, which, edit) {
        Ok(changed) => {
            let Some(board) = app.document_state.pcb_board(&path) else {
                return refused("the board is not open");
            };
            let mut answered = answer(board, footprint, which);
            answered["document"] = json!(path.display().to_string());
            answered["designator"] = json!(designator.trim());
            answered["changed"] = json!(changed);
            (CommandOutcome::Answered(answered), Task::none())
        }
        Err(reason) => refused(reason),
    }
}
