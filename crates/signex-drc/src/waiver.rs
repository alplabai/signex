//! DRC waivers (#2399, #2307), kept in the board's options. A waiver names
//! its finding by [`waiver_key`] (the rule, the kind of a written rule, the
//! items and the keepout, not the board path) and records a [`fingerprint`]
//! of their geometry: a waiver whose objects changed lapses. The model and
//! its inputs: `docs/architecture/pcb-drc.md`, "Waivers".

use pcb_geom::{ItemRef, is_courtyard, via_layers};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use signex_types::check::finding_waiver_json;
// Every number, angles and ratios too, quantised as the file stores a length.
use signex_types::format::{mm_as_stored_nm, parse_pcb_layer};
use signex_types::pcb::{Footprint, FpGraphic, Pad, PadShape, PcbBoard};
use signex_types::pcb_layer::PcbLayer;
use signex_types::pcb_options::DrcWaiver;
use uuid::Uuid;

use crate::Violation;
use crate::findings::{item_id, item_label, rule_of};

/// Hex digits of a key or fingerprint: 128 bits of the hash.
const KEY_HEX_DIGITS: usize = 32;

/// The rule id `v` is filed under: a written rule's `user::<name>`, else
/// its kind's `builtin::<kind>`.
pub fn rule_id(v: &Violation) -> &str {
    rule_of(v)
}

/// The waiver key of `v`. A finding that names no object (an unconnected
/// link with no pad at either end) is named by where it is instead.
pub fn waiver_key(v: &Violation) -> String {
    let mut hasher = Sha256::new();
    hasher.update(rule_of(v).as_bytes());
    if v.rule.is_some() {
        hasher.update(b"\nkind:");
        hasher.update(v.kind.rule_id().as_bytes());
    }
    for item in &v.items {
        hasher.update(b"\nitem:");
        hasher.update(item_label(item).as_bytes());
    }
    if let Some(keepout) = v.keepout {
        hasher.update(b"\nkeepout:");
        hasher.update(keepout.to_string().as_bytes());
    }
    if v.items.is_empty() && v.keepout.is_none() {
        hasher.update(format!("\nat:{},{}", v.at.x, v.at.y).as_bytes());
    }
    short_hex(hasher)
}

/// The geometry of `v`'s items and keepout on `board`: lengths, angles and
/// ratios in stored nm, nets by name, a footprint by its local shapes and
/// its placement (never a value that went through sin/cos).
pub fn fingerprint(v: &Violation, board: &PcbBoard) -> String {
    let mut text = String::new();
    for item in &v.items {
        canonical(&item_geometry(board, item), &mut text);
        text.push('\n');
    }
    if let Some(keepout) = v.keepout {
        canonical(&keepout_geometry(board, keepout), &mut text);
    }
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    short_hex(hasher)
}

/// What the stored waivers make of one finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaiverState<'a> {
    /// Hidden by this waiver.
    Waived(&'a DrcWaiver),
    /// Named by this waiver, whose objects changed since: the finding is back.
    Lapsed(&'a DrcWaiver),
    /// No waiver names it.
    Open,
}

/// The state of `v` on `board` under `waivers`. Of the waivers with `v`'s
/// key, one whose fingerprint matches wins; else the last is lapsed. A
/// waiver without a fingerprint (made before #2307) never matches.
pub fn waiver_state<'a>(
    waivers: &'a [DrcWaiver],
    v: &Violation,
    board: &PcbBoard,
) -> WaiverState<'a> {
    let key = waiver_key(v);
    let same: Vec<&DrcWaiver> = waivers.iter().filter(|w| w.key == key).collect();
    let Some(&last) = same.last() else {
        return WaiverState::Open;
    };
    let now = fingerprint(v, board);
    match same.iter().find(|w| w.fingerprint == now) {
        Some(&w) => WaiverState::Waived(w),
        None => WaiverState::Lapsed(last),
    }
}

/// A waiver as a waiver object of a check report, in the shape every check
/// shares, with its `key`.
pub fn waiver_json(waiver: &DrcWaiver) -> Value {
    let mut extra = Map::new();
    extra.insert("key".into(), json!(waiver.key));
    finding_waiver_json(&waiver.rule, &waiver.fingerprint, &waiver.note, extra)
}

fn short_hex(hasher: Sha256) -> String {
    let mut hex = format!("{:x}", hasher.finalize());
    hex.truncate(KEY_HEX_DIGITS);
    hex
}

/// The geometry a rule sees of `item`; a fixed marker for one `board` lacks.
fn item_geometry(board: &PcbBoard, item: &ItemRef) -> Value {
    let found = match *item {
        ItemRef::Track(id) => board.segments.iter().find(|s| s.uuid == id).map(|s| {
            json!({"track": id, "layer": s.layer, "net": net_name(board, s.net),
                "start": s.start, "end": s.end, "width": s.width})
        }),
        // The resolved span: a through via reaches every layer whatever it lists.
        ItemRef::Via(id) => board.vias.iter().find(|v| v.uuid == id).map(|v| {
            json!({"via": id, "span": via_layers(v), "net": net_name(board, v.net),
                "position": v.position, "diameter": v.diameter, "drill": v.drill,
                "layer_sizes": v.layer_sizes})
        }),
        ItemRef::Pad { footprint, pad } => board
            .footprints
            .iter()
            .find(|f| f.uuid == footprint)
            .and_then(|fp| Some((fp, fp.pads.iter().find(|p| p.uuid == pad)?)))
            .map(|(fp, p)| json!({"placement": placement(fp), "pad": pad_geometry(p)})),
        ItemRef::Footprint(id) => board.footprints.iter().find(|f| f.uuid == id).map(|fp| {
            let pads: Vec<Value> = fp.pads.iter().map(pad_geometry).collect();
            let graphics: Vec<&FpGraphic> = fp.graphics.iter().filter(|g| is_checked(g)).collect();
            json!({"placement": placement(fp), "footprint_id": fp.footprint_id,
                "pads": pads, "graphics": graphics})
        }),
        ItemRef::FreePad(id) => board
            .free
            .pads
            .iter()
            .find(|p| p.uuid == id)
            .map(pad_geometry),
        ItemRef::Zone { zone, .. } => board
            .zones
            .iter()
            .find(|z| z.uuid == zone)
            .map(|z| with_net(json!(z), board, z.net)),
        ItemRef::Arc(id) => board
            .free
            .arcs
            .iter()
            .find(|a| a.uuid == id)
            .map(|a| with_net(json!(a), board, a.net)),
        ItemRef::Text(id) => board.texts.iter().find(|t| t.uuid == id).map(|t| json!(t)),
        ItemRef::Dimension(id) => board
            .free
            .dimensions
            .iter()
            .find(|d| d.uuid == id)
            .map(|d| json!(d)),
        ItemRef::Graphic { graphic, .. } => board
            .graphics
            .iter()
            .find(|g| g.uuid == graphic)
            .map(|g| json!(g)),
        ItemRef::FootprintText { footprint, field } => board
            .footprints
            .iter()
            .find(|f| f.uuid == footprint)
            .and_then(|fp| {
                let text = fp.properties.iter().find(|p| p.key == field.key())?;
                Some(json!({"placement": placement(fp), "field": field.id(), "text": text}))
            }),
        // Where a room is and what it holds (#2457), not its name or colour.
        ItemRef::Room(id) => {
            board.rooms.iter().find(|r| r.uuid == id).map(
                |r| json!({"outline": r.outline, "margin_mm": r.margin_mm, "source": r.source}),
            )
        }
    };
    found.unwrap_or_else(|| json!(format!("missing:{}", item_id(item))))
}

/// A pad in its own frame (a footprint pad) or on the board (a free pad),
/// with its per-layer sizes (#2401); its net by name. The trapezoid delta
/// and custom outline (#2242) join only for the shape that reads them, so
/// the fingerprint of every other pad stays as it was.
fn pad_geometry(p: &Pad) -> Value {
    let mut geometry = json!({"pad": p.uuid, "type": p.pad_type, "shape": p.shape,
        "position": p.position, "size": p.size, "rotation": p.rotation,
        "roundrect_ratio": p.roundrect_ratio, "drill": p.drill, "layers": p.layers,
        "layer_sizes": p.layer_sizes, "net": p.net.as_ref().map(|n| n.name.as_str())});
    match p.shape {
        PadShape::Trapezoid => geometry["trapezoid_delta"] = json!(p.trapezoid_delta),
        PadShape::Custom => geometry["custom_outline"] = json!(p.custom_outline),
        _ => {}
    }
    geometry
}

/// Whether a footprint graphic is something DRC sees: copper or courtyard
/// (the body `footprint_body` takes), not silkscreen or fabrication.
fn is_checked(graphic: &FpGraphic) -> bool {
    is_courtyard(graphic) || matches!(parse_pcb_layer(&graphic.layer), Some(PcbLayer::Copper(_)))
}

/// Where a footprint sits: its id, position, turn and side.
fn placement(fp: &Footprint) -> Value {
    json!({"footprint": fp.uuid, "position": fp.position, "rotation": fp.rotation,
        "layer": fp.layer})
}

fn keepout_geometry(board: &PcbBoard, id: Uuid) -> Value {
    board
        .sketch
        .as_ref()
        .and_then(|s| s.keepouts.iter().find(|k| k.uuid == id))
        .map_or_else(|| json!(format!("missing:keepout:{id}")), |k| json!(k))
}

/// `item` with its `net` number replaced by the net's name.
fn with_net(item: Value, board: &PcbBoard, net: u32) -> Value {
    let mut item = item;
    item["net"] = json!(net_name(board, net));
    item
}

/// A net by name, so a renumbering of the board's nets keeps the waiver.
fn net_name(board: &PcbBoard, net: u32) -> String {
    board
        .nets
        .iter()
        .find(|n| n.number == net)
        .map_or_else(|| format!("#{net}"), |n| n.name.clone())
}

/// `value` as text that depends on nothing but its content: object keys
/// sorted, every float as stored nm (a raw `f64` can change across a save).
fn canonical(value: &Value, out: &mut String) {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for key in keys {
                out.push_str(&json!(key).to_string());
                out.push(':');
                canonical(&map[key], out);
                out.push(',');
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for item in items {
                canonical(item, out);
                out.push(',');
            }
            out.push(']');
        }
        Value::Number(n) if n.is_f64() => {
            let nm = n.as_f64().and_then(mm_as_stored_nm);
            out.push_str(&nm.map_or_else(|| "nan".to_string(), |nm| format!("{nm}nm")));
        }
        other => out.push_str(&other.to_string()),
    }
}

#[cfg(test)]
#[path = "waiver_tests.rs"]
mod tests;
