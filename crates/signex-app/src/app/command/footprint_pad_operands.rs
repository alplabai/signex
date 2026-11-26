//! `footprint_move_pads` and `footprint_align_pads` (#3013): the edits Move
//! Selection By and Align Selection make on the selection, made on the pads
//! a caller names by number on the footprint `document` names. Each sends
//! the footprint editor the one message its edit runs on (`pad_moves`), so
//! it is one undo step of that footprint, as the dialog's is. The selection
//! is not touched. The answer is the named pads after the edit.

use std::path::{Path, PathBuf};

use iced::Task;
use serde_json::{Value, json};

use super::document_operands::named;
use super::operand_handlers::refused;
use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::{DocumentKey, FootprintDocument, Message, Signex};
use crate::library::editor::footprint::{AlignOp, pads_needed};
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};

type Ran = (CommandOutcome, Task<Message>);

/// The path of the open footprint `document` names.
fn named_footprint(app: &Signex, args: &CommandArgs) -> Result<PathBuf, String> {
    let entry = named(app, args)?;
    match entry.key {
        Some(DocumentKey::Footprint(path)) => Ok(path),
        Some(_) => Err(format!(
            "the command acts on a footprint, and {} is a {}",
            entry.name,
            entry.kind.as_str()
        )),
        None => Err(format!(
            "{} is referenced by a project but not loaded; open it first",
            entry.name
        )),
    }
}

/// The indices of the pads `text` names by number, comma separated, in the
/// order given and each once. A number no pad has, or that more than one
/// pad has, is refused: the caller could not tell which pad moves.
fn pad_indices(editor: &FootprintDocument, text: &str) -> Result<Vec<usize>, String> {
    let mut indices = Vec::new();
    for number in text.split(',').map(str::trim) {
        if number.is_empty() {
            return Err("`pads` is pad numbers, comma separated".to_string());
        }
        let found: Vec<usize> = editor
            .state
            .pads
            .iter()
            .enumerate()
            .filter(|(_, pad)| pad.number == number)
            .map(|(i, _)| i)
            .collect();
        match found.as_slice() {
            [] => return Err(format!("the footprint has no pad `{number}`")),
            [i] if !indices.contains(i) => indices.push(*i),
            [_] => {}
            more => {
                return Err(format!(
                    "{} pads are numbered `{number}`; give them their own numbers to name one",
                    more.len()
                ));
            }
        }
    }
    Ok(indices)
}

/// The open footprint and the pads `args` names on it.
fn footprint_and_pads(app: &Signex, args: &CommandArgs) -> Result<(PathBuf, Vec<usize>), String> {
    let path = named_footprint(app, args)?;
    let editor = app
        .document_state
        .footprint_editors
        .get(&path)
        .ok_or("the footprint is not open")?;
    let Some(OperandValue::Text(text)) = args.operand("pads") else {
        return Err("the operand `pads` is needed".to_string());
    };
    let indices = pad_indices(editor, text)?;
    Ok((path, indices))
}

fn centres(app: &Signex, path: &Path, indices: &[usize]) -> Vec<(f64, f64)> {
    app.document_state
        .footprint_editors
        .get(path)
        .map(|editor| {
            indices
                .iter()
                .filter_map(|&i| editor.state.pads.get(i).map(|pad| pad.position_mm))
                .collect()
        })
        .unwrap_or_default()
}

/// The answer: the named pads as they are after the edit, and whether any
/// of them moved.
fn answer(app: &Signex, path: &Path, indices: &[usize], before: &[(f64, f64)]) -> Value {
    let pads: Vec<Value> = app
        .document_state
        .footprint_editors
        .get(path)
        .map(|editor| {
            indices
                .iter()
                .filter_map(|&i| editor.state.pads.get(i))
                .map(|pad| {
                    let (x, y) = pad.position_mm;
                    json!({"number": pad.number, "x": x, "y": y})
                })
                .collect()
        })
        .unwrap_or_default();
    let changed = centres(app, path, indices) != before;
    json!({
        "document": path.display().to_string(),
        "pads": pads,
        "changed": changed,
    })
}

/// Send `msg` to the footprint editor at `path`, as the dialog's button does.
fn send(app: &mut Signex, path: &Path, msg: FootprintEditorMsg) -> Task<Message> {
    app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.to_path_buf(),
        msg: PrimitiveEdit::Footprint(msg),
    }))
}

/// `footprint_move_pads document=<footprint> pads=<numbers> by=dx,dy`: move
/// the pads by (dx, dy) mm, as Move Selection By moves the selection.
pub(super) fn footprint_move_pads(app: &mut Signex, args: &CommandArgs) -> Ran {
    let (path, indices) = match footprint_and_pads(app, args) {
        Ok(found) => found,
        Err(reason) => return refused(reason),
    };
    let Some(OperandValue::Point(dx, dy)) = args.operand("by") else {
        return refused("`by` is the offset dx,dy in mm");
    };
    let (dx, dy) = (*dx, *dy);
    let before = centres(app, &path, &indices);
    let msg = FootprintEditorMsg::PadsMoveRequested {
        pads: indices.clone(),
        dx,
        dy,
    };
    let task = send(app, &path, msg);
    let answered = answer(app, &path, &indices, &before);
    (CommandOutcome::Answered(answered), task)
}

/// The op `value` names on the axis `axis`, `None` for `none` or no value.
fn axis_op(args: &CommandArgs, axis: &str) -> Result<Option<AlignOp>, String> {
    let Some(OperandValue::Text(value)) = args.operand(axis) else {
        return Ok(None);
    };
    let ops: &[(&str, Option<AlignOp>)] = if axis == "horizontal" {
        &[
            ("left", Some(AlignOp::Left)),
            ("center", Some(AlignOp::CenterH)),
            ("right", Some(AlignOp::Right)),
            ("space", Some(AlignOp::DistributeH)),
            ("none", None),
        ]
    } else {
        &[
            ("top", Some(AlignOp::Top)),
            ("center", Some(AlignOp::CenterV)),
            ("bottom", Some(AlignOp::Bottom)),
            ("space", Some(AlignOp::DistributeV)),
            ("none", None),
        ]
    };
    let value = value.trim();
    ops.iter()
        .find(|(name, _)| *name == value)
        .map(|(_, op)| *op)
        .ok_or_else(|| {
            let names: Vec<&str> = ops.iter().map(|(name, _)| *name).collect();
            format!("`{axis}` is one of {}", names.join(", "))
        })
}

/// `footprint_align_pads document=<footprint> pads=<numbers>
/// [horizontal=..] [vertical=..]`: align the pads on each axis, as Align
/// Selection aligns the selection, both axes in one undo step. An axis the
/// pads are too few for (two to align, three to space) is skipped and named
/// in `skipped`; with no axis left it is refused.
pub(super) fn footprint_align_pads(app: &mut Signex, args: &CommandArgs) -> Ran {
    let (path, indices) = match footprint_and_pads(app, args) {
        Ok(found) => found,
        Err(reason) => return refused(reason),
    };
    let (horizontal, vertical) = match (axis_op(args, "horizontal"), axis_op(args, "vertical")) {
        (Ok(h), Ok(v)) => (h, v),
        (Err(reason), _) | (_, Err(reason)) => return refused(reason),
    };
    if horizontal.is_none() && vertical.is_none() {
        return refused("give `horizontal` or `vertical` an alignment other than `none`");
    }
    let fits = |op: Option<AlignOp>| op.is_none_or(|op| indices.len() >= pads_needed(op));
    let skipped: Vec<&str> = [("horizontal", horizontal), ("vertical", vertical)]
        .into_iter()
        .filter(|(_, op)| op.is_some() && !fits(*op))
        .map(|(axis, _)| axis)
        .collect();
    if (horizontal.is_none() || !fits(horizontal)) && (vertical.is_none() || !fits(vertical)) {
        return refused(format!(
            "`pads` names {} pad(s); aligning needs 2 and spacing needs 3",
            indices.len()
        ));
    }
    let before = centres(app, &path, &indices);
    let msg = FootprintEditorMsg::PadsAlignRequested {
        pads: indices.clone(),
        horizontal,
        vertical,
    };
    let task = send(app, &path, msg);
    let mut answered = answer(app, &path, &indices, &before);
    answered["skipped"] = json!(skipped);
    (CommandOutcome::Answered(answered), task)
}
