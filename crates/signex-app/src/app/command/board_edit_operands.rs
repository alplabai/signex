//! The board edits an agent or the CLI runs by naming the board (#2401,
//! #2407): `set_layer_stack`, `lock_board_items`, `unlock_board_items`,
//! `set_via_span`, `set_layer_sizes` and `set_pad_shape`. Each acts on the board `document`
//! names, never on the active tab, runs the same step as its GUI surface (the
//! Layer Stack manager, the board menu's Lock rows, the Via and Pad panels)
//! and answers with what the board then holds. One undo step each.

use std::path::PathBuf;

use iced::Task;
use serde_json::json;
use signex_engine::pcb::primitive_edit::{PadEdit, ViaEdit};
use signex_i18n::ids;
use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{PadShape, PcbBoard, Point};
use signex_types::pcb_layer::CopperLayer;
use signex_types::pcb_stack::{LayerStack, StackLayer, parse_layer_sizes};
use uuid::Uuid;

use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::document::DocumentKey;
use crate::app::handlers::pcb_item_edits::track_or_via;
use crate::app::{Message, Signex};
use crate::panels::pcb_primitive_properties::parse_number;

type Ran = (CommandOutcome, Task<Message>);

fn refused(reason: impl Into<String>) -> Ran {
    (CommandOutcome::disabled(reason), Task::none())
}

fn answered(value: serde_json::Value) -> Ran {
    (CommandOutcome::Answered(value), Task::none())
}

/// The path of the open board `args` names.
fn named_board(app: &Signex, args: &CommandArgs, what: &str) -> Result<PathBuf, String> {
    let Some(OperandValue::Document(target)) = args.operand("document") else {
        return Err(format!("{what} needs the operand `document`"));
    };
    let entry = app.resolve_document_target(target)?;
    match entry.key {
        Some(DocumentKey::Pcb(path)) if app.document_state.board_engine(&path).is_some() => {
            Ok(path)
        }
        Some(DocumentKey::Pcb(_)) | None => {
            Err(format!("{} is not open; open it first", entry.name))
        }
        Some(_) => Err(format!(
            "{what} acts on a board, and {} is a {}",
            entry.name,
            entry.kind.as_str()
        )),
    }
}

fn text<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.as_str()),
        _ => None,
    }
}

/// `ids`: uuids, comma separated.
fn uuids(args: &CommandArgs) -> Result<Vec<Uuid>, String> {
    let list = text(args, "ids").ok_or("the operand `ids` is needed")?;
    list.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| Uuid::parse_str(s).map_err(|_| format!("`{s}` is not a uuid")))
        .collect()
}

fn board<'a>(app: &'a Signex, path: &std::path::Path) -> Option<&'a PcbBoard> {
    app.document_state.pcb_board(path)
}

/// `set_layer_stack document=.. stack=<json>`: the whole stack, as the
/// manager stores it.
pub(super) fn set_layer_stack(app: &mut Signex, args: &CommandArgs) -> Ran {
    let path = match named_board(app, args, "`set_layer_stack`") {
        Ok(path) => path,
        Err(reason) => return refused(reason),
    };
    let Some(raw) = text(args, "stack") else {
        return refused("`set_layer_stack` needs the operand `stack`");
    };
    let stack = serde_json::from_str::<LayerStack>(raw).or_else(|_| {
        serde_json::from_str::<Vec<StackLayer>>(raw).map(|layers| LayerStack { layers })
    });
    let stack = match stack {
        Ok(stack) => stack,
        Err(error) => return refused(format!("`stack` is not a layer stack: {error}")),
    };
    match app.apply_layer_stack(&path, stack) {
        Ok(stored) => answered(json!({
            "document": path,
            "total_thickness_mm": stored.total_thickness_mm(),
            "copper_layers": stored.copper().map(|l| l.layer.clone()).collect::<Vec<_>>(),
            "stack": stored,
        })),
        Err(why) => refused(why),
    }
}

pub(super) fn lock_board_items(app: &mut Signex, args: &CommandArgs) -> Ran {
    lock(app, args, true)
}

pub(super) fn unlock_board_items(app: &mut Signex, args: &CommandArgs) -> Ran {
    lock(app, args, false)
}

/// `lock_board_items` / `unlock_board_items document=.. ids=..`.
fn lock(app: &mut Signex, args: &CommandArgs, locked: bool) -> Ran {
    let path = match named_board(app, args, "the lock") {
        Ok(path) => path,
        Err(reason) => return refused(reason),
    };
    let targets = match uuids(args) {
        Ok(targets) => targets,
        Err(reason) => return refused(reason),
    };
    let Some(b) = board(app, &path) else {
        return refused("the board is no longer open");
    };
    let mut items = Vec::with_capacity(targets.len());
    for id in &targets {
        match track_or_via(b, *id) {
            Some(item) => items.push(item),
            None => return refused(format!("{id} is not a track or via on the board")),
        }
    }
    match app.set_items_locked(&path, &items, locked) {
        Ok(()) => answered(json!({ "document": path, "locked": locked, "ids": targets })),
        Err(why) => refused(why),
    }
}

/// `set_via_span document=.. ids=.. layers=TopCopper,Inner2Copper`.
pub(super) fn set_via_span(app: &mut Signex, args: &CommandArgs) -> Ran {
    let path = match named_board(app, args, "`set_via_span`") {
        Ok(path) => path,
        Err(reason) => return refused(reason),
    };
    let targets = match uuids(args) {
        Ok(targets) => targets,
        Err(reason) => return refused(reason),
    };
    let layers: Option<Vec<CopperLayer>> = text(args, "layers")
        .unwrap_or_default()
        .split(',')
        .map(|name| parse_pcb_layer(name.trim()).and_then(|l| l.copper()))
        .collect();
    let Some([a, b]) = layers
        .as_deref()
        .and_then(|l| <[CopperLayer; 2]>::try_from(l).ok())
    else {
        return refused("`layers` names two copper layers, as TopCopper,Inner2Copper");
    };
    if let Err(why) = app.edit_vias(&path, &targets, ViaEdit::Span(a, b), ids::PCB_PRIM_VIA_NAME) {
        return refused(why);
    }
    let vias: Vec<serde_json::Value> = board(app, &path)
        .map(|b| {
            b.vias
                .iter()
                .filter(|v| targets.contains(&v.uuid))
                .map(|v| json!({ "id": v.uuid, "layers": v.layers, "via_type": v.via_type }))
                .collect()
        })
        .unwrap_or_default();
    answered(json!({ "document": path, "vias": vias }))
}

/// `set_layer_sizes document=.. ids=.. sizes=..`: vias (a diameter per
/// layer) or pads (`<x>x<y>` per layer), all of one kind.
pub(super) fn set_layer_sizes(app: &mut Signex, args: &CommandArgs) -> Ran {
    let path = match named_board(app, args, "`set_layer_sizes`") {
        Ok(path) => path,
        Err(reason) => return refused(reason),
    };
    let targets = match uuids(args) {
        Ok(targets) => targets,
        Err(reason) => return refused(reason),
    };
    let sizes = text(args, "sizes").unwrap_or_default();
    let Some(b) = board(app, &path) else {
        return refused("the board is no longer open");
    };
    let vias = targets
        .iter()
        .all(|id| b.vias.iter().any(|v| v.uuid == *id));
    let pads: Option<Vec<(Uuid, Uuid)>> = targets
        .iter()
        .map(|id| {
            b.footprints
                .iter()
                .find(|fp| fp.pads.iter().any(|p| p.uuid == *id))
                .map(|fp| (fp.uuid, *id))
        })
        .collect();
    let done = if vias {
        parse_layer_sizes(sizes, parse_number).and_then(|s| {
            app.edit_vias(&path, &targets, ViaEdit::Sizes(s), ids::PCB_PRIM_VIA_STACK)
        })
    } else if let Some(pads) = pads {
        parse_layer_sizes(sizes, pad_size)
            .and_then(|s| app.edit_pads(&path, &pads, &PadEdit::Sizes(s), ids::PCB_PRIM_PAD_STACK))
    } else {
        Err("`ids` names vias, or pads, of the board; not both and nothing else".to_string())
    };
    if let Err(why) = done {
        return refused(why);
    }
    let held = board(app, &path).map(|b| {
        let via_sizes = b
            .vias
            .iter()
            .filter(|v| targets.contains(&v.uuid))
            .map(|v| json!({ "id": v.uuid, "layer_sizes": v.layer_sizes }));
        let pad_sizes = b
            .footprints
            .iter()
            .flat_map(|fp| fp.pads.iter())
            .filter(|p| targets.contains(&p.uuid))
            .map(|p| json!({ "id": p.uuid, "layer_sizes": p.layer_sizes }));
        via_sizes.chain(pad_sizes).collect::<Vec<_>>()
    });
    answered(json!({ "document": path, "items": held.unwrap_or_default() }))
}

/// A pad size typed as `<x>x<y>` in mm.
fn pad_size(text: &str) -> Option<Point> {
    let (x, y) = text.split_once('x')?;
    Some(Point::new(parse_number(x)?, parse_number(y)?))
}

/// `set_pad_shape document=.. ids=.. shape=.. [delta=<x>,<y>] [corners=<x>,<y>;..]`
/// (#2242): footprint pads take the shape with the parameters it reads, in
/// pad-local mm. A trapezoid needs `delta`, a custom pad `corners`; the
/// board's rule (`signex_types::pcb_pad_shape`) refuses ones that make no
/// copper.
pub(super) fn set_pad_shape(app: &mut Signex, args: &CommandArgs) -> Ran {
    let path = match named_board(app, args, "`set_pad_shape`") {
        Ok(path) => path,
        Err(reason) => return refused(reason),
    };
    let targets = match uuids(args) {
        Ok(targets) => targets,
        Err(reason) => return refused(reason),
    };
    let edit = match pad_geometry(args) {
        Ok(edit) => edit,
        Err(reason) => return refused(reason),
    };
    let Some(b) = board(app, &path) else {
        return refused("the board is no longer open");
    };
    let pads: Option<Vec<(Uuid, Uuid)>> = targets
        .iter()
        .map(|id| {
            b.footprints
                .iter()
                .find(|fp| fp.pads.iter().any(|p| p.uuid == *id))
                .map(|fp| (fp.uuid, *id))
        })
        .collect();
    let Some(pads) = pads.filter(|p| !p.is_empty()) else {
        return refused("`ids` names pads of the board's footprints and nothing else");
    };
    if let Err(why) = app.edit_pads(&path, &pads, &edit, ids::PCB_PRIM_SHAPE) {
        return refused(why);
    }
    let held = board(app, &path).map(|b| {
        b.footprints
            .iter()
            .flat_map(|fp| fp.pads.iter())
            .filter(|p| targets.contains(&p.uuid))
            .map(|p| {
                json!({
                    "id": p.uuid,
                    "shape": p.shape,
                    "trapezoid_delta": p.trapezoid_delta,
                    "custom_outline": p.custom_outline,
                })
            })
            .collect::<Vec<_>>()
    });
    answered(json!({ "document": path, "pads": held.unwrap_or_default() }))
}

/// The `shape`, `delta` and `corners` operands as one pad edit.
fn pad_geometry(args: &CommandArgs) -> Result<PadEdit, String> {
    let shape = match text(args, "shape").map(str::trim) {
        Some("circle") => PadShape::Circle,
        Some("rect") => PadShape::Rect,
        Some("oval") => PadShape::Oval,
        Some("roundrect") => PadShape::RoundRect,
        Some("trapezoid") => PadShape::Trapezoid,
        Some("custom") => PadShape::Custom,
        Some(other) => {
            return Err(format!(
                "`{other}` is not a pad shape (circle, rect, oval, roundrect, trapezoid, custom)"
            ));
        }
        None => return Err("`set_pad_shape` needs the operand `shape`".to_string()),
    };
    let trapezoid_delta = match text(args, "delta") {
        None => Point::ZERO,
        Some(raw) => {
            mm_point(raw).ok_or_else(|| format!("`delta` must be <x>,<y> in mm, not `{raw}`"))?
        }
    };
    let custom_outline = match text(args, "corners") {
        None => Vec::new(),
        Some(raw) => raw
            .split(';')
            .map(mm_point)
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| format!("`corners` must be <x>,<y>;<x>,<y>;.. in mm, not `{raw}`"))?,
    };
    Ok(PadEdit::Geometry {
        shape,
        trapezoid_delta,
        custom_outline,
    })
}

/// A point typed as `<x>,<y>` in mm.
fn mm_point(text: &str) -> Option<Point> {
    let (x, y) = text.split_once(',')?;
    Some(Point::new(parse_number(x)?, parse_number(y)?))
}
