//! Per-finding DRC waivers (#2307), stored as `ProjectRules::drc_waivers`
//! in the shape of ERC's. A waiver names a finding by its rule id and the
//! ids of the items it involves, and records a fingerprint of their
//! geometry: moving, resizing or re-netting one of them lapses it, an edit
//! anywhere else does not. See `docs/architecture/pcb-drc.md`, "Waivers".

use std::collections::{BTreeMap, BTreeSet};

use pcb_geom::{ItemRef, Shape, footprint_body, via_layers};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
// Every number, angles and ratios too, quantised as the file stores a length.
use signex_types::format::{mm_as_stored_nm as nm, pcb_layer_token};
use signex_types::pcb::{Footprint, PcbBoard, Point};
use signex_types::project::DrcWaiver;
use uuid::Uuid;

use crate::Violation;
use crate::findings::{item_id, rule_of};

/// The identity and geometry fingerprint a finding would be waived by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaiverKey {
    pub rule_id: String,
    /// Item ids (a pad as `<footprint>/<pad>`), `keepout:<uuid>` and, for a
    /// written rule, `kind:<builtin id>`; sorted.
    pub objects: Vec<String>,
    pub fingerprint: String,
}

/// `v`'s key on `board`. `None` when it names no item and no keepout (an
/// unconnected link with no pad at either end), or names one `board` lacks.
pub fn waiver_key(v: &Violation, board: &PcbBoard) -> Option<WaiverKey> {
    let mut objects: Vec<String> = v.items.iter().map(item_id).collect();
    objects.extend(v.keepout.map(|k| format!("keepout:{k}")));
    if objects.is_empty() {
        return None;
    }
    // One written rule can set several limits; the kind tells them apart.
    if v.rule.is_some() {
        objects.push(format!("kind:{}", v.kind.rule_id()));
    }
    objects.sort();
    let rule_id = rule_of(v).to_string();
    let mut hasher = Sha256::new();
    hasher.update(rule_id.as_bytes());
    for item in &v.items {
        hasher.update(b"\n");
        hasher.update(item_geometry(board, item)?.as_bytes());
    }
    if let Some(keepout) = v.keepout {
        hasher.update(b"\n");
        hasher.update(keepout_geometry(board, keepout)?.as_bytes());
    }
    Some(WaiverKey {
        rule_id,
        objects,
        fingerprint: format!("{:x}", hasher.finalize()),
    })
}

/// What one run made of the stored waivers.
#[derive(Debug, Default)]
pub struct WaiverSplit<'a> {
    /// Findings no waiver hides.
    pub kept: Vec<&'a Violation>,
    /// Findings a waiver hides, with the index of that waiver in the list.
    pub waived: Vec<(&'a Violation, usize)>,
    /// Indices of waivers whose finding is back because an involved item
    /// changed (same rule and items, different fingerprint).
    pub lapsed: BTreeSet<usize>,
}

/// The keys of one run's `violations` on `board`, in order. A finding whose
/// identity (rule and ids) another finding of the run shares gets none: one
/// waiver must never hide several findings (stacked pads give two links the
/// same end pads).
pub fn waiver_keys(violations: &[&Violation], board: &PcbBoard) -> Vec<Option<WaiverKey>> {
    let keys: Vec<Option<WaiverKey>> = violations.iter().map(|v| waiver_key(v, board)).collect();
    let mut seen: BTreeMap<(&str, &[String]), usize> = BTreeMap::new();
    for key in keys.iter().flatten() {
        *seen.entry((&key.rule_id, &key.objects)).or_default() += 1;
    }
    let shared: BTreeSet<(String, Vec<String>)> = seen
        .into_iter()
        .filter(|&(_, n)| n > 1)
        .map(|((rule, objects), _)| (rule.to_string(), objects.to_vec()))
        .collect();
    keys.into_iter()
        .map(|key| key.filter(|k| !shared.contains(&(k.rule_id.clone(), k.objects.clone()))))
        .collect()
}

/// Split `violations` on `board` by `waivers`. An incomplete waiver (no
/// reason or author) never hides anything. A waiver whose identity names
/// several findings of the run hides none of them and is lapsed.
pub fn split_waived<'a>(
    violations: impl IntoIterator<Item = &'a Violation>,
    board: &PcbBoard,
    waivers: &[DrcWaiver],
) -> WaiverSplit<'a> {
    let violations: Vec<&Violation> = violations.into_iter().collect();
    let mut split = WaiverSplit::default();
    if !waivers.iter().any(DrcWaiver::is_complete) {
        split.kept = violations;
        return split;
    }
    let keys = waiver_keys(&violations, board);
    for (v, key) in violations.into_iter().zip(keys) {
        let identity = match &key {
            Some(k) => Some((k.rule_id.clone(), k.objects.clone())),
            None => waiver_key(v, board).map(|k| (k.rule_id, k.objects)),
        };
        let same = identity.and_then(|(rule, objects)| {
            waivers
                .iter()
                .enumerate()
                .find(|(_, w)| w.is_complete() && w.same_identity(&rule, &objects))
        });
        match (same, key) {
            (Some((i, w)), Some(k)) if w.fingerprint == k.fingerprint => {
                split.waived.push((v, i));
            }
            (Some((i, _)), _) => {
                split.lapsed.insert(i);
                split.kept.push(v);
            }
            (None, _) => split.kept.push(v),
        }
    }
    split
}

/// A waiver as a waiver object of a check report, as ERC writes one.
pub fn waiver_json(waiver: &DrcWaiver) -> Value {
    json!({
        "kind": "finding",
        "rule_id": waiver.rule_id,
        "objects": waiver.objects,
        "fingerprint": waiver.fingerprint,
        "reason": waiver.reason,
        "author": waiver.author,
        "date": waiver.date,
        "message": waiver.message,
    })
}

/// The geometry a rule sees of `item`: layer, net, position and size, every
/// length in stored nm (a raw `f64` changes across a save + load).
fn item_geometry(board: &PcbBoard, item: &ItemRef) -> Option<String> {
    match *item {
        ItemRef::Track(id) => {
            let s = board.segments.iter().find(|s| s.uuid == id)?;
            Some(format!(
                "track {id} {} net={} {} {} w={}",
                s.layer,
                net_name(board, s.net),
                point(s.start)?,
                point(s.end)?,
                nm(s.width)?
            ))
        }
        ItemRef::Via(id) => {
            let v = board.vias.iter().find(|v| v.uuid == id)?;
            // The resolved span: a through via reaches every layer whatever it lists.
            Some(format!(
                "via {id} span={:x} net={} {} d={} drill={}",
                via_layers(v),
                net_name(board, v.net),
                point(v.position)?,
                nm(v.diameter)?,
                nm(v.drill)?
            ))
        }
        ItemRef::Pad { footprint, pad } => {
            let fp = board.footprints.iter().find(|f| f.uuid == footprint)?;
            let p = fp.pads.iter().find(|p| p.uuid == pad)?;
            Some(format!(
                "pad {} {pad} {} {} {} {} size={} rot={} rr={} drill={} {} net={}",
                placement(fp)?,
                serde_json::to_string(&p.pad_type).ok()?,
                serde_json::to_string(&p.shape).ok()?,
                point(p.position)?,
                p.layers.join(","),
                point(p.size)?,
                nm(p.rotation)?,
                nm(p.roundrect_ratio)?,
                p.drill.as_ref().map_or(Some(0), |d| nm(d.diameter))?,
                p.number,
                p.net.as_ref().map_or("", |n| n.name.as_str()),
            ))
        }
        ItemRef::Footprint(id) => {
            let fp = board.footprints.iter().find(|f| f.uuid == id)?;
            Some(format!(
                "footprint {} {} body={}",
                placement(fp)?,
                fp.footprint_id,
                body(fp)?
            ))
        }
    }
}

/// The body a `no_components` keepout tests (`footprint_body`): its centre
/// and half sizes in nm; the turn is the placement's.
fn body(fp: &Footprint) -> Option<String> {
    Some(match footprint_body(fp).ok()? {
        Shape::Rect {
            center,
            half_w,
            half_h,
            ..
        } => format!("({},{}) {half_w}x{half_h}", center.x, center.y),
        Shape::Disc { center, radius } => format!("({},{}) r{radius}", center.x, center.y),
        // `footprint_body` makes no other shape.
        other => format!("{other:?}"),
    })
}

/// Where a footprint sits: its id, position, turn and side.
fn placement(fp: &Footprint) -> Option<String> {
    Some(format!(
        "{} {} rot={} {}",
        fp.uuid,
        point(fp.position)?,
        nm(fp.rotation)?,
        fp.layer
    ))
}

fn keepout_geometry(board: &PcbBoard, id: Uuid) -> Option<String> {
    let k = board
        .sketch
        .as_ref()?
        .keepouts
        .iter()
        .find(|k| k.uuid == id)?;
    let boundary: Option<Vec<String>> = k.boundary.iter().map(|p| point(*p)).collect();
    let layers: Vec<String> = k.layers.iter().map(|l| pcb_layer_token(*l)).collect();
    Some(format!(
        "keepout {id} {} {} {}",
        boundary?.join(" "),
        layers.join(","),
        serde_json::to_string(&k.forbids).ok()?
    ))
}

/// A net by name, so a renumbering of the board's nets keeps the waiver.
fn net_name(board: &PcbBoard, net: u32) -> String {
    board
        .nets
        .iter()
        .find(|n| n.number == net)
        .map_or_else(|| format!("#{net}"), |n| n.name.clone())
}

fn point(p: Point) -> Option<String> {
    Some(format!("({},{})", nm(p.x)?, nm(p.y)?))
}

#[cfg(test)]
mod tests;
