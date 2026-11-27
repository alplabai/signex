//! The soak's connectivity and short checks (#2967). An action changes the
//! open links (ratsnest lines) of the nets it touched and of no other net;
//! a landed route only joins copper of its own net, so its net never ends
//! up with more open links; and the copper a route lays never shorts
//! another net. The design rule check is the one definition of a short: a
//! cheap overlap test only decides whether to ask it.

use std::collections::{BTreeMap, BTreeSet};

use pcb_geom::{ItemRef, RatLine, collect_copper};
use signex_drc::{DrcRules, ViolationKind};
use signex_engine::pcb::route::RouteDraft;
use signex_types::pcb::PcbBoard;

/// The nets an action may have changed the links of; `None` when it may
/// have changed any (a footprint move, an undo, a redo).
pub type Touched = Option<BTreeSet<u32>>;

/// The ratsnest lines of each net.
fn by_net(lines: &[RatLine]) -> BTreeMap<u32, Vec<RatLine>> {
    let mut nets: BTreeMap<u32, Vec<RatLine>> = BTreeMap::new();
    for line in lines {
        nets.entry(line.net).or_default().push(*line);
    }
    nets
}

fn open_links(nets: &BTreeMap<u32, Vec<RatLine>>, net: u32) -> usize {
    nets.get(&net).map_or(0, Vec::len)
}

/// The nets whose links an edit elsewhere can change: those with a zone,
/// since a pour's fill is cut back from every other net's copper.
pub fn poured_nets(board: &PcbBoard) -> BTreeSet<u32> {
    board.zones.iter().map(|z| z.net).collect()
}

/// Every net outside `touched` and `poured` has the same ratsnest lines
/// `after` as `before`.
pub fn untouched_links_kept(
    before: &[RatLine],
    after: &[RatLine],
    touched: &BTreeSet<u32>,
    poured: &BTreeSet<u32>,
) -> Result<(), String> {
    let (was, now) = (by_net(before), by_net(after));
    let nets: BTreeSet<u32> = was.keys().chain(now.keys()).copied().collect();
    let changed = nets
        .into_iter()
        .filter(|net| !touched.contains(net) && !poured.contains(net))
        .find(|net| was.get(net) != now.get(net));
    match changed {
        None => Ok(()),
        Some(net) => Err(format!(
            "net {net} was not touched, but its open links changed: {} before, {} after",
            open_links(&was, net),
            open_links(&now, net)
        )),
    }
}

/// A landed route on `net` leaves it at most as many open links as it had.
pub fn route_net_not_worse(before: &[RatLine], after: &[RatLine], net: u32) -> Result<(), String> {
    let (was, now) = (by_net(before), by_net(after));
    let (was, now) = (open_links(&was, net), open_links(&now, net));
    if now <= was {
        Ok(())
    } else {
        Err(format!(
            "a route on net {net} left it more open: {was} open links before, {now} after"
        ))
    }
}

/// The net a landed `draft` may only have joined: its tracks' (or vias')
/// net, unless it pushed a track of that net or the net is poured, where
/// its links may move for other reasons.
pub fn joined_net(draft: &RouteDraft, poured: &BTreeSet<u32>) -> Option<u32> {
    let net = (draft.tracks.first().map(|t| t.net)).or(draft.vias.first().map(|v| v.net))?;
    let pushed_own = draft.shoved.iter().any(|s| s.net == net);
    (net != 0 && !pushed_own && !poured.contains(&net)).then_some(net)
}

/// The nets a landed `draft` touched: its own copper's and the pushed
/// tracks'.
pub fn draft_nets(draft: &RouteDraft) -> BTreeSet<u32> {
    (draft.tracks.iter().map(|t| t.net))
        .chain(draft.vias.iter().map(|v| v.net))
        .chain(draft.shoved.iter().map(|s| s.net))
        .collect()
}

/// No new track or via of `draft`, landed on `board`, shorts copper of
/// another net. A route's new copper that overlaps another net's on a
/// shared layer is only a candidate; the design rule check confirms it.
pub fn route_shorts(board: &PcbBoard, draft: &RouteDraft, rules: &DrcRules) -> Result<(), String> {
    let new: BTreeSet<ItemRef> = (draft.tracks.iter().map(|t| ItemRef::Track(t.uuid)))
        .chain(draft.vias.iter().map(|v| ItemRef::Via(v.uuid)))
        .collect();
    let copper = collect_copper(board);
    let (ours, others): (Vec<_>, Vec<_>) = copper.iter().partition(|c| new.contains(&c.item));
    let candidate = ours.iter().any(|a| {
        others.iter().any(|b| {
            a.net != b.net
                && a.layers & b.layers != 0
                && a.shape.aabb().intersects(&b.shape.aabb())
                && a.shape.overlaps(&b.shape)
        })
    });
    if !candidate {
        return Ok(());
    }
    let shorts: Vec<String> = signex_drc::run(board, rules)
        .violations
        .iter()
        .filter(|v| v.kind == ViolationKind::Short)
        .filter(|v| v.items.iter().any(|item| new.contains(item)))
        .map(|v| format!("{:?} (nets {} and {})", v.items, v.nets.0, v.nets.1))
        .collect();
    if shorts.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "a route's new copper shorts another net: {shorts:?}"
        ))
    }
}
