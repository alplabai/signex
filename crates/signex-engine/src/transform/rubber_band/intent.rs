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
/// warned pin (`warned`), or when both sides carry different names, which no
/// join here can settle: the candidate that makes it then differs and the
/// contact goes down the ladder. Emptied nets stay in the list; the caller
/// drops every net with nothing to identify it.
pub(super) fn join_nets(
    nets: Vec<NetSignature>,
    lone: BTreeMap<Key, NetSignature>,
    joins: &[(Slot, Slot)],
    warned: impl Fn(&NetSignature) -> bool,
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
            continue;
        }
        let names = (&all[a].name, &all[b].name);
        if !names.0.is_empty() && !names.1.is_empty() && names.0 != names.1 {
            continue;
        }
        let gone = std::mem::replace(&mut all[b], super::guard::empty_net());
        let into = &mut all[a];
        into.terminals.extend(gone.terminals);
        into.labels.extend(gone.labels);
        into.sheet_pins.extend(gone.sheet_pins);
        if into.name.is_empty() {
            into.name = gone.name;
        }
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
