//! The copper checks. Candidate pairs come from the board index, one
//! bounding-box query per item, so the cost follows what is near each item,
//! not the square of the item count.

use std::collections::{HashMap, HashSet};

use signex_types::pcb::PcbBoard;
use signex_types::pcb_free::free_pad_frame;

use pcb_geom::{
    BoardIndex, CopperItem, IndexedItem, ItemRef, PointNm, collect_copper, no_net_copper,
    pad_pieces, ratsnest, via_pieces,
};

use super::{DrcReport, DrcRules, Violation, ViolationKind};

/// Check `board` against `rules`.
pub fn run(board: &PcbBoard, rules: &DrcRules) -> DrcReport {
    let items = per_layer_sizes(board, collect_copper(board));
    let no_net = per_layer_sizes(board, no_net_copper(board));
    let index = BoardIndex::build(board);
    let mut violations = copper_pairs(&items, &no_net, &index, rules);
    violations.extend(unconnected(board, &items));
    let keepouts = super::keepout::check(board);
    violations.extend(keepouts.violations);
    violations.extend(super::limits::violations(board, rules));
    violations.extend(super::holes::violations(board, rules));
    violations.extend(super::edge::violations(board, rules));
    violations.extend(super::silk::violations(board, rules));
    violations.extend(super::mask::violations(board, rules));
    violations.extend(super::courtyard::violations(board, rules));
    let (outside_rooms, unmeasured) = super::rooms::violations(board);
    violations.extend(outside_rooms);
    violations.sort_unstable();
    let mut skipped = index.rejected().to_vec();
    skipped.extend(keepouts.bodies);
    // A body the keepout check could not convert either is listed once.
    let unlisted: Vec<_> = unmeasured
        .into_iter()
        .filter(|r| !skipped.iter().any(|s| s.item == r.item))
        .collect();
    skipped.extend(unlisted);
    DrcReport {
        rules: rules.clone(),
        violations,
        skipped,
        skipped_keepouts: keepouts.keepouts,
    }
}

/// A pad or via with per-layer sizes is checked with its copper on each
/// layer (#2401): one item per group of layers that share a size. The
/// ratsnest keeps one item per pad and via, so only the check splits them.
fn per_layer_sizes(board: &PcbBoard, items: Vec<CopperItem>) -> Vec<CopperItem> {
    let free_frame = free_pad_frame();
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let pieces = match item.item {
            ItemRef::Via(id) => board
                .vias
                .iter()
                .find(|v| v.uuid == id && v.layer_sizes.is_some())
                .and_then(|v| via_pieces(v).ok()),
            ItemRef::Pad { footprint, pad } => board
                .footprints
                .iter()
                .find(|f| f.uuid == footprint)
                .and_then(|f| Some((f, f.pads.iter().find(|p| p.uuid == pad)?)))
                .filter(|(_, p)| p.layer_sizes.is_some())
                .and_then(|(f, p)| pad_pieces(f, p).ok()),
            ItemRef::FreePad(id) => board
                .free
                .pads
                .iter()
                .find(|p| p.uuid == id && p.layer_sizes.is_some())
                .and_then(|p| pad_pieces(&free_frame, p).ok()),
            _ => None,
        };
        match pieces {
            Some(pieces) => out.extend(pieces.into_iter().map(|(layers, shape)| CopperItem {
                item: item.item,
                net: item.net,
                layers,
                shape,
                pad_center: item.pad_center,
            })),
            None => out.push(item),
        }
    }
    out
}

/// Shorts and clearance violations between copper of different nets:
/// between the netted `items`, and between each piece of `no_net` copper
/// (pads, tracks, arcs and vias of net 0, drawings and strings on copper;
/// #2883, #2515) and the netted items. Two pieces of no net are never a
/// pair.
fn copper_pairs(
    items: &[CopperItem],
    no_net: &[CopperItem],
    index: &BoardIndex,
    rules: &DrcRules,
) -> Vec<Violation> {
    // Every copy of an identity, in list order: an arc's chords share one
    // (#2373), and the index holds each of them.
    let mut slots: HashMap<ItemRef, Vec<usize>> = HashMap::new();
    for (i, c) in items.iter().enumerate() {
        slots.entry(c.item).or_default().push(i);
    }
    let mut seen: HashSet<(usize, usize)> = HashSet::new();
    let mut found = Findings::default();
    for (i, a) in items.iter().enumerate() {
        let near = a.shape.aabb().inflate(rules.reach_of(a.net));
        for hit in index.bounds_meeting(&near) {
            let Some(j) = slot_of(&slots, items, hit).filter(|&j| j != i) else {
                continue;
            };
            // A pair is looked at once, from whichever side finds it first.
            if !seen.insert((i.min(j), i.max(j))) {
                continue;
            }
            found.keep(check_pair(a, &items[j], rules));
        }
    }
    // Each piece of no net looks for the netted copper as far as any net's
    // rule reaches, and takes every per-layer copy of what it finds; index
    // hits on copper of no net have no slot and are passed over.
    let reach = items
        .iter()
        .map(|c| c.net)
        .chain([0])
        .collect::<HashSet<u32>>()
        .into_iter()
        .map(|net| rules.reach_of(net))
        .max()
        .unwrap_or(0);
    for a in no_net {
        let near = a.shape.aabb().inflate(reach);
        for hit in index.bounds_meeting(&near) {
            for j in copies_of(&slots, items, hit) {
                found.keep(check_pair(a, &items[j], rules));
            }
        }
    }
    found.found
}

/// One finding per pair of items, the worst of their parts' findings.
#[derive(Default)]
struct Findings {
    found: Vec<Violation>,
    by_items: HashMap<Vec<ItemRef>, usize>,
}

impl Findings {
    fn keep(&mut self, v: Option<Violation>) {
        let Some(v) = v else {
            return;
        };
        match self.by_items.get(&v.items) {
            Some(&k) if is_worse(&v, &self.found[k]) => self.found[k] = v,
            Some(_) => {}
            None => {
                self.by_items.insert(v.items.clone(), self.found.len());
                self.found.push(v);
            }
        }
    }
}

/// Every copper list entry the index entry `hit` stands for: for an item
/// held as several parts, the part of its shape; else every per-layer copy.
fn copies_of(
    slots: &HashMap<ItemRef, Vec<usize>>,
    items: &[CopperItem],
    hit: &IndexedItem,
) -> Vec<usize> {
    let Some(copies) = slots.get(&hit.item) else {
        return Vec::new();
    };
    if hit.item.is_multi_part() {
        copies
            .iter()
            .copied()
            .filter(|&j| items[j].shape == hit.shape)
            .collect()
    } else {
        copies.clone()
    }
}

/// The copper list entry the index entry `hit` is: the first copy of its
/// identity, or, for an item held as several parts, the part of its shape.
fn slot_of(
    slots: &HashMap<ItemRef, Vec<usize>>,
    items: &[CopperItem],
    hit: &IndexedItem,
) -> Option<usize> {
    let copies = slots.get(&hit.item)?;
    if hit.item.is_multi_part() {
        copies
            .iter()
            .copied()
            .find(|&j| items[j].shape == hit.shape)
    } else {
        copies.first().copied()
    }
}

/// Whether `new` says more than `old` about one pair of items: a short over
/// a clearance finding, else the smaller gap.
fn is_worse(new: &Violation, old: &Violation) -> bool {
    let short = |v: &Violation| matches!(v.kind, ViolationKind::Short);
    match (short(new), short(old)) {
        (true, false) => true,
        (false, true) => false,
        _ => new.measured_nm < old.measured_nm,
    }
}

fn check_pair(a: &CopperItem, b: &CopperItem, rules: &DrcRules) -> Option<Violation> {
    let shared = a.layers & b.layers;
    if a.net == b.net || shared == 0 {
        return None;
    }
    let (required, rule) = rules.required_between(a, b, shared);
    let ((first, second), nets) = if a.item <= b.item {
        ((a, b), (a.net, b.net))
    } else {
        ((b, a), (b.net, a.net))
    };
    let items = vec![first.item, second.item];
    if a.shape.overlaps(&b.shape) {
        return Some(Violation {
            kind: ViolationKind::Short,
            items,
            nets,
            layers: shared,
            at: shared_point(a, b),
            measured_nm: 0,
            required_nm: required,
            keepout: None,
            rule: None,
        });
    }
    let gap = a.shape.separation(&b.shape);
    gap.is_under(required).then(|| Violation {
        kind: ViolationKind::Clearance,
        items,
        nets,
        layers: shared,
        at: gap.midpoint(),
        // Rounding down can only land under the rule here, never on it.
        measured_nm: gap.distance_nm().min(required - 1),
        required_nm: required,
        keepout: None,
        rule,
    })
}

/// A point on both coppers where they meet: the first of a few candidates
/// (the centre of where the bounding boxes meet, then each shape's own
/// centre) that lies on both. A diagonal track across a pad corner can have
/// the first fall on neither; with none on both, the first is used.
fn shared_point(a: &CopperItem, b: &CopperItem) -> PointNm {
    let (ra, rb) = (a.shape.aabb(), b.shape.aabb());
    let lo = PointNm::new(ra.min.x.max(rb.min.x), ra.min.y.max(rb.min.y));
    let hi = PointNm::new(ra.max.x.min(rb.max.x), ra.max.y.min(rb.max.y));
    let meet = mid(lo, hi);
    [meet, mid(ra.min, ra.max), mid(rb.min, rb.max)]
        .into_iter()
        .find(|&p| a.shape.contains(p, 0) && b.shape.contains(p, 0))
        .unwrap_or(meet)
}

fn mid(a: PointNm, b: PointNm) -> PointNm {
    PointNm::new(a.x + (b.x - a.x) / 2, a.y + (b.y - a.y) / 2)
}

/// One violation per link the ratsnest still draws.
fn unconnected(board: &PcbBoard, items: &[CopperItem]) -> Vec<Violation> {
    let mut pads: HashMap<(u32, PointNm), ItemRef> = HashMap::new();
    for pad in items.iter().filter(|c| c.pad_center.is_some()) {
        if let Some(center) = pad.pad_center {
            pads.entry((pad.net, center))
                .and_modify(|held| *held = (*held).min(pad.item))
                .or_insert(pad.item);
        }
    }
    ratsnest(board)
        .into_iter()
        .map(|line| {
            let mut items: Vec<ItemRef> = [line.from, line.to]
                .iter()
                .filter_map(|&end| pads.get(&(line.net, end)).copied())
                .collect();
            items.sort_unstable();
            items.dedup();
            let length = (line.from.distance_sq(line.to) as f64).sqrt().floor() as i64;
            Violation {
                kind: ViolationKind::Unconnected,
                items,
                nets: (line.net, line.net),
                layers: 0,
                at: PointNm::new(
                    line.from.x + (line.to.x - line.from.x) / 2,
                    line.from.y + (line.to.y - line.from.y) / 2,
                ),
                measured_nm: length,
                required_nm: 0,
                keepout: None,
                rule: None,
            }
        })
        .collect()
}
