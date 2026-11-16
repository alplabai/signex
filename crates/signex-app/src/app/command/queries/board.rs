//! The board queries (#2403): `measure_board`, `query_board_nets` and
//! `find_similar_items`. Each reads the one board it names, through the same
//! domain functions as the Measure tool, the Net Inspector and the Find
//! Similar dialog, answers with data and changes nothing. Shapes:
//! `docs/CLI.md`, "Board queries".

use pcb_geom::{ItemRef, NetReport, PickKind, Span};
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
    let found = board
        .footprints
        .iter()
        .find_map(|fp| {
            if fp.uuid == id {
                return Some(ItemRef::Footprint(id));
            }
            fp.pads.iter().find(|p| p.uuid == id).map(|_| ItemRef::Pad {
                footprint: fp.uuid,
                pad: id,
            })
        })
        .or_else(|| {
            board
                .segments
                .iter()
                .any(|s| s.uuid == id)
                .then_some(ItemRef::Track(id))
        })
        .or_else(|| {
            board
                .vias
                .iter()
                .any(|v| v.uuid == id)
                .then_some(ItemRef::Via(id))
        })
        .or_else(|| {
            let zone = board.zones.iter().find(|z| z.uuid == id)?;
            Some(ItemRef::Zone {
                zone: id,
                kind: PickKind::of_zone(zone)?,
            })
        });
    found.ok_or_else(|| format!("nothing on the board has the id {id}"))
}

/// The uuid an agent acts on for `item`: a pad's own, not its footprint's.
fn item_id(item: ItemRef) -> String {
    match item {
        ItemRef::Pad { pad: id, .. }
        | ItemRef::Footprint(id)
        | ItemRef::Track(id)
        | ItemRef::Via(id)
        | ItemRef::Zone { zone: id, .. } => id.to_string(),
    }
}

fn point_operand(args: &CommandArgs, name: &str) -> Option<Point> {
    match args.operand(name) {
        Some(OperandValue::Point(x, y)) => Some(Point::new(*x, *y)),
        _ => None,
    }
}

fn span_json(kind: &str, document: &str, span: &Span) -> Value {
    json!({
        "document": document,
        "kind": kind,
        "distance_mm": span.distance_mm,
        "dx_mm": span.dx_mm,
        "dy_mm": span.dy_mm,
        "from": [span.from.x, span.from.y],
        "to": [span.to.x, span.to.y],
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
            return Ok(span_json("edge_to_edge", &name, &span));
        }
        let (Some(start), Some(end)) = (point_operand(args, "start"), point_operand(args, "end"))
        else {
            return Err("`measure_board` needs `start` and `end`, or `ids`".into());
        };
        Ok(span_json(
            "point_to_point",
            &name,
            &Span::between(start, end),
        ))
    }

    /// `query_board_nets document= [net=]`: every net of the board, or the
    /// one named, with its routed length, members and routed state.
    pub(super) fn query_board_nets(&self, args: &CommandArgs) -> Answer {
        let (board, name) = self.named_board(args)?;
        let wanted = text_operand(args, "net").map(|n| n.trim().to_lowercase());
        let nets: Vec<Value> = pcb_geom::net_reports(board)
            .iter()
            .filter(|r| wanted.as_ref().is_none_or(|w| r.name.to_lowercase() == *w))
            .map(net_json)
            .collect();
        if let Some(net) = wanted.filter(|_| nets.is_empty()) {
            return Err(format!("{name} has no net named {net}"));
        }
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
