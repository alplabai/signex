//! The joins a guarded move asks for beyond the plain move, and the two open
//! questions of how they compose, each answered in one place.
//!
//! The intent is the plain move's nets with every kept contact restored: a
//! moved pin on the net its old tip sits on (`guard::place_moved_pins`), and a
//! point a dragged wire carries joining the net at its old place to the net
//! the wire landed on. Both answers below are provisional, pending the
//! maintainer (docs/DECISIONS.md, 2026-11-19); each is the conservative
//! choice, which never merges two nets the move did not already join.

use std::collections::BTreeMap;

use signex_net::{NetSignature, pt_key, uf::Key};
use signex_types::schematic::Point;

/// Spec gap (a), provisional: a pin that keep-wires holds on its old net and
/// whose new tip lands inside a wire of another net joins only its old net.
/// No pin dot is made into the other net while the pin is held; the user is
/// told. `true` would join both nets.
pub(super) const KEPT_PIN_JOINS_LANDING_NET: bool = false;

/// Spec gap (b), provisional: a pin left as a plain move (warned) ends on
/// exactly the net the plain move gives it; a carried wire's kept contact may
/// not join that net to another. `true` would let it.
pub(super) const WARNED_PIN_MAY_JOIN_CARRIED_NET: bool = false;

/// The plain move's pin dots a guarded round keeps: all of them, except
/// (spec gap (a)) one at the new tip of a pin held on its old net.
#[must_use]
pub fn pin_dots_to_keep(plain_dots: &[Point], held_tips: &[Point]) -> Vec<Point> {
    if KEPT_PIN_JOINS_LANDING_NET {
        return plain_dots.to_vec();
    }
    (plain_dots.iter())
        .filter(|dot| !held_tips.iter().any(|tip| pt_key(tip) == pt_key(dot)))
        .copied()
        .collect()
}

/// A net of the intent: one of the plain move's nets, or a net known only by
/// its id at a queried point (nothing on it names it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Slot {
    Net(usize),
    Lone(Key),
}

impl Slot {
    /// The slot of a queried point's answer: its net's index when named.
    pub(super) fn at((root, index): (Key, Option<usize>)) -> Self {
        index.map_or(Self::Lone(root), Self::Net)
    }
}

/// The intent's nets: `nets` and the `lone` ones with each pair of `joins`
/// made one net. A join is left out when (spec gap (b)) either side holds a
/// warned pin (`warned`). When both sides carry different names the joined
/// net is named as the netlist names it (`name_of`, from its labels and
/// ports); a join it cannot name is left out, so the candidate that makes it
/// differs and the contact goes down the ladder. Emptied nets stay in the
/// list; the caller drops every net with nothing to identify it.
pub(super) fn join_nets(
    nets: Vec<NetSignature>,
    lone: BTreeMap<Key, NetSignature>,
    joins: &[(Slot, Slot)],
    warned: impl Fn(&NetSignature) -> bool,
    name_of: impl Fn(&NetSignature) -> Option<String>,
) -> Vec<NetSignature> {
    let named = nets.len();
    let mut all = nets;
    let mut lone_at: BTreeMap<Key, usize> = BTreeMap::new();
    for (root, net) in lone {
        lone_at.insert(root, all.len());
        all.push(net);
    }
    let mut parent: Vec<usize> = (0..all.len()).collect();
    for (a, b) in joins {
        let mut index = |slot: Slot| match slot {
            Slot::Net(n) if n < named => Some(n),
            Slot::Net(_) => None,
            Slot::Lone(root) => Some(*lone_at.entry(root).or_insert_with(|| {
                let next = all.len();
                all.push(super::guard::empty_net());
                parent.push(next);
                next
            })),
        };
        let (Some(a), Some(b)) = (index(*a), index(*b)) else {
            continue;
        };
        let (a, b) = (find(&mut parent, a), find(&mut parent, b));
        if a == b {
            continue;
        }
        if !WARNED_PIN_MAY_JOIN_CARRIED_NET && (warned(&all[a]) || warned(&all[b])) {
            #[cfg(test)]
            super::guard::reasons::note("(b): a carried point's join holds a warned pin");
            continue;
        }
        let mut joined = all[a].clone();
        joined.terminals.extend(all[b].terminals.iter().cloned());
        joined.labels.extend(all[b].labels.iter().copied());
        joined.sheet_pins.extend(all[b].sheet_pins.iter().copied());
        let (x, y) = (&all[a].name, &all[b].name);
        joined.name = if x.is_empty() || x == y {
            y.clone()
        } else if y.is_empty() {
            x.clone()
        } else {
            let Some(name) = name_of(&joined) else {
                #[cfg(test)]
                super::guard::reasons::note("a join of two named nets no label names");
                continue;
            };
            name
        };
        all[a] = joined;
        all[b] = super::guard::empty_net();
        parent[b] = a;
    }
    all
}

fn find(parent: &mut [usize], mut at: usize) -> usize {
    while parent[at] != at {
        parent[at] = parent[parent[at]];
        at = parent[at];
    }
    at
}
