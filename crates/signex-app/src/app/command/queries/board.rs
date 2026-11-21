//! The board queries (#2403): `measure_board`, `query_board_nets` and
//! `find_similar_items`. Each reads the one board it names, through the same
//! domain functions as the Measure tool, the Net Inspector and the Find
//! Similar dialog, answers with data and changes nothing. Shapes:
//! `docs/CLI.md`, "Board queries".

use pcb_geom::{ItemRef, NetReport, Span};
use serde_json::{Value, json};
use signex_engine::SimilarProperty;
use signex_engine::pcb::similar;
use signex_types::pcb::{PcbBoard, Point};

use super::{Answer, text_operand};
use crate::app::Signex;
use crate::app::command::{CommandArgs, OperandValue};
use crate::app::document::DocumentKey;

/// The item of `board` whose uuid is `text`: a footprint, pad, track, via
/// or drawn area.
fn item_by_id(board: &PcbBoard, text: &str) -> Result<ItemRef, String> {
    let id = uuid::Uuid::parse_str(text.trim())
        .map_err(|_| format!("`{}` is not an id (a hyphenated uuid)", text.trim()))?;
    pcb_geom::item_by_uuid(board, id).ok_or_else(|| format!("nothing on the board has the id {id}"))
}

/// The uuid an agent acts on for `item`: a pad's own, not its footprint's.
fn item_id(item: ItemRef) -> String {
    match item {
        ItemRef::Pad { pad: id, .. }
        | ItemRef::Footprint(id)
        | ItemRef::Track(id)
        | ItemRef::Via(id)
        | ItemRef::Zone { zone: id, .. }
        | ItemRef::Text(id)
        | ItemRef::Arc(id)
        | ItemRef::FreePad(id)
        | ItemRef::Dimension(id)
        | ItemRef::Graphic { graphic: id, .. }
        | ItemRef::Room(id) => id.to_string(),
        ItemRef::FootprintText { footprint, field } => format!("{footprint}/{}", field.id()),
    }
}

fn point_operand(args: &CommandArgs, name: &str) -> Option<Point> {
    match args.operand(name) {
        Some(OperandValue::Point(x, y)) => Some(Point::new(*x, *y)),
        _ => None,
    }
}

/// A span as the agent reads it: `from` and `to` from the board origin, Y
/// up (#2384), like every other board readout.
fn span_json(kind: &str, document: &str, board: &PcbBoard, span: &Span) -> Value {
    let origin = board.options.origin;
    let (from, to) = (origin.to_user(span.from), origin.to_user(span.to));
    json!({
        "document": document,
        "kind": kind,
        "distance_mm": span.distance_mm,
        "dx_mm": span.dx_mm,
        "dy_mm": span.dy_mm,
        "from": [from.x, from.y],
        "to": [to.x, to.y],
    })
}

/// One `query_board_nets` row.
fn net_json(report: &NetReport) -> Value {
    json!({
        "net": report.net,
        "name": report.name,
        "routed_length_mm": report.routed_length_mm,
        "tracks": report.tracks,
        "vias": report.vias,
        "pads": report.pads,
        "unrouted": report.unrouted,
        "routed": report.is_routed(),
    })
}

impl Signex {
    /// The loaded board `document` names, and its name.
    fn named_board(&self, args: &CommandArgs) -> Result<(&PcbBoard, String), String> {
        let Some(OperandValue::Document(target)) = args.operand("document") else {
            return Err("a board query needs the operand `document`".into());
        };
        let entry = self.resolve_document_target(target)?;
        let Some(DocumentKey::Pcb(path)) = &entry.key else {
            return Err(format!("{} is not an open board", entry.name));
        };
        let board = self
            .document_state
            .pcb_board(path)
            .ok_or_else(|| format!("{} is not loaded", entry.name))?;
        Ok((board, entry.name.clone()))
    }

    /// `measure_board document= start= end=`: point to point; `ids=a,b`:
    /// edge to edge between two items.
    pub(super) fn measure_board(&self, args: &CommandArgs) -> Answer {
        let (board, name) = self.named_board(args)?;
        if let Some(ids) = text_operand(args, "ids") {
            let items = ids
                .split(',')
                .map(|text| item_by_id(board, text))
                .collect::<Result<Vec<_>, _>>()?;
            let [a, b] = items.as_slice() else {
                return Err("`ids` names two items, comma separated".into());
            };
            let span = pcb_geom::edge_gap(board, *a, *b)
                .ok_or("an item has no geometry the board can measure")?;
            return Ok(span_json("edge_to_edge", &name, board, &span));
        }
        let (Some(start), Some(end)) = (point_operand(args, "start"), point_operand(args, "end"))
        else {
            return Err("`measure_board` needs `start` and `end`, or `ids`".into());
        };
        // `start` and `end` count from the board origin, Y up, as typed
        // board points do elsewhere.
        let origin = board.options.origin;
        let span = Span::between(origin.from_user(start), origin.from_user(end));
        Ok(span_json("point_to_point", &name, board, &span))
    }

    /// `query_board_nets document= [net=]`: every net of the board, or the
    /// one named, with its routed length, members and routed state.
    pub(super) fn query_board_nets(&self, args: &CommandArgs) -> Answer {
        let (board, name) = self.named_board(args)?;
        let wanted = match text_operand(args, "net").map(str::trim) {
            Some(net) => Some(
                pcb_geom::net_named(board, net)
                    .ok_or_else(|| format!("{name} has no net named {net}"))?,
            ),
            None => None,
        };
        let nets: Vec<Value> = pcb_geom::net_reports(board)
            .iter()
            .filter(|r| wanted.is_none_or(|w| r.net == w))
            .map(net_json)
            .collect();
        Ok(json!({ "document": name, "count": nets.len(), "nets": nets }))
    }

    /// `find_similar_items document= id= [fields=]`: the objects of the board
    /// that agree with the item `id` on each property `fields` lists (`kind`
    /// when left out), as Find Similar Objects selects them.
    pub(super) fn find_similar_items(&self, args: &CommandArgs) -> Answer {
        let (board, name) = self.named_board(args)?;
        let seed = item_by_id(board, text_operand(args, "id").unwrap_or_default())?;
        let values = similar::properties_of(board, seed);
        let mut criteria = Vec::new();
        for key in text_operand(args, "fields").unwrap_or("kind").split(',') {
            let property = SimilarProperty::from_key(key)
                .ok_or_else(|| format!("`{}` is not a property", key.trim()))?;
            let value = values
                .iter()
                .find(|v| v.property == property)
                .ok_or_else(|| format!("the seed has no `{}`", property.key()))?;
            criteria.push(value.clone());
        }
        let items = similar::find_similar(board, &criteria);
        let criteria_json: Vec<Value> = criteria
            .iter()
            .map(|c| json!({ "property": c.property.key(), "value": c.value }))
            .collect();
        Ok(json!({
            "document": name,
            "seed": similar::describe(board, seed),
            "criteria": criteria_json,
            "count": items.len(),
            "ids": items.into_iter().map(item_id).collect::<Vec<_>>(),
        }))
    }
}
