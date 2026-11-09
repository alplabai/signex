//! The guard's comparison on the nets a move reaches, not on every net.
//!
//! [`Guard::check`](super::guard::Guard::check) compares the nets of a
//! candidate sheet with the nets the user's move asks for (see `guard.rs`).
//! Over every net that costs the number of nets of the sheet on every ghost
//! step. For a drag preview the pre-move sheet's connectivity is known
//! ([`NetBase`]), and the nets of the plain-moved sheet and of a candidate
//! come as a [`NetDelta`] against it: `nets = (base nets - removed) + added`,
//! where `added` is the handful of nets the move reached.
//!
//! # Why the local comparison equals the full one
//!
//! Write `B` for the base nets, `P = (B - rp) + ap` for the plain-moved
//! sheet's nets, `G = (B - rg) + ag` for the candidate's (each a delta, see
//! `signex_net::NetDelta`). The full comparison builds
//!
//! * `want = F(P)` - the intent: `P` with every moved pin taken out, then put
//!   back on the net the plain move defines for it (`intent` in `guard.rs`);
//! * `got = project(G)` - the candidate's nets without the pins whose plain
//!   connection is undefined (`extra`);
//!
//! and asks whether the two sets are equal; if not, `blame` reads the
//! symmetric difference.
//!
//! 1. `F` changes only nets that hold a moved pin or sit at a moved pin's old
//!    tip. A moved pin's old tip is a queried point, so it is dirty and its
//!    clusters are rebuilt: the pin is in `ap` (or on no net at all), and the
//!    net at the old tip is in `ap` too. A pin found on a base net the plain
//!    delta left standing is not a case this comparison covers
//!    ([`LocalGuard::new`] returns `None`, so the full guard runs). Every other
//!    net of `P` is a base net `F` leaves alone. The kept-power-port join
//!    renames and merges nets by value across the sheet, which no delta
//!    bounds, so a move with a power port or a child-sheet pin among its tips
//!    is not compared locally either.
//! 2. So `want = C + (rg - rp) + F(ap)` and `got = C + (rp - rg) + ag`, where
//!    `C = B - rp - rg` are the base nets neither delta touched, and `project`
//!    leaves `C` alone (it removes only moved pins, and a net of `C` holds
//!    none). `rg - rp` is a far net the candidate touched and left as it was:
//!    the plain move still has it, so the intent keeps it.
//! 3. `C` is the same set on both sides and shares no element with the rest:
//!    the rest holds only nets that contain a pin, label or child-sheet pin,
//!    each of which sits in one net of any derive (pins are unique across the
//!    base nets, [`NetBase::delta_ready`]; labels and sheet pins have one id
//!    each). So `want = got` exactly when the local sets are equal, and
//!    `want` and `got` differ by the same nets as the local sets do, so
//!    `blame` blames the same tips.
//!
//! The local sets are built from `added`, the few base nets named above and
//! the exclusions, so a step costs the nets the move reaches; the diff of the
//! candidate against the base is the one scan of the sheet left, and it
//! allocates nothing per element.

use std::collections::{BTreeSet, HashSet};
use std::sync::Arc;

use signex_net::{NetBase, NetDelta, NetSignature, uf::Key};
use signex_types::schematic::SchematicSheet;
use uuid::Uuid;

use super::guard::{Exclusions, Guard, Placement, Projected, place_moved_pins};
use super::{PinIdent, PinTip, Policy};

/// The plain-moved sheet seen from the moved pins, as a delta.
pub(super) struct LocalGuard {
    base: Arc<NetBase>,
    plain: NetDelta,
    /// Per tip: the net its pin is on in the plain-moved sheet, as a position
    /// in `plain.added`.
    home: Vec<Option<usize>>,
    /// Per tip: nothing stays at the old tip to carry the connection.
    carried: Vec<bool>,
}

impl LocalGuard {
    /// `None` when the comparison cannot be local: the base cannot answer a
    /// delta, a tip is a power port or a child-sheet pin, or the plain-moved
    /// sheet is not one the base can judge.
    pub(super) fn new(
        base: &Arc<NetBase>,
        plain_sheet: &SchematicSheet,
        tips: &[PinTip],
    ) -> Option<Self> {
        if !base.delta_ready() || tips.iter().any(|t| t.is_power() || t.is_sheet_pin()) {
            return None;
        }
        let at: Vec<(Key, bool)> = tips.iter().map(|t| (t.old_key(), false)).collect();
        let plain = base.derive_delta(plain_sheet, &at)?;
        #[cfg(test)]
        if super::guard::self_check::enabled() {
            let (full, found) = signex_net::net_signatures_at(plain_sheet, &at);
            assert_eq!(base.nets_with(&plain), full, "the plain delta differs");
            for ((id, in_added), (full_id, in_full)) in plain.found.iter().zip(&found) {
                assert_eq!(id, full_id, "the net id at an old tip differs");
                assert_eq!(
                    in_added.map(|n| &plain.added[n]),
                    in_full.map(|n| &full[n]),
                    "the net at an old tip differs"
                );
            }
        }
        // A moved pin's old tip is a queried point, so it is dirty and the
        // pin's symbol is rebuilt there: the pin is in `plain.added` or on no
        // net. A pin that is on a base net the delta left standing is not a
        // case this comparison covers.
        let mut home = Vec::with_capacity(tips.len());
        for tip in tips {
            let PinIdent::Terminal(symbol, pin) = tip.ident() else {
                return None;
            };
            let key = (*symbol, pin.clone());
            let at = plain.added.iter().position(|n| n.terminals.contains(&key));
            let standing = (base.net_of_pin(*symbol, pin))
                .is_some_and(|b| plain.removed.binary_search(&b).is_err());
            if at.is_none() && standing {
                return None;
            }
            home.push(at);
        }
        let carried = plain.occupied.iter().map(|on| !on).collect();
        #[cfg(test)]
        super::guard::tally::guard_built_local();
        Some(Self {
            base: base.clone(),
            plain,
            home,
            carried,
        })
    }

    /// Whether the pre-move nets had this pin on a net.
    pub(super) fn was_connected(&self, symbol: Uuid, pin: &str) -> bool {
        self.base.net_of_pin(symbol, pin).is_some()
    }

    /// The outcome of [`Guard::check`] for `candidate`; `None` when the
    /// candidate is not one the base can judge, so the caller compares over
    /// every net.
    pub(super) fn check(
        &self,
        guard: &Guard,
        candidate: &SchematicSheet,
        tips: &[PinTip],
        policy: &Policy,
        edited: &[Key],
        skip: &Exclusions,
    ) -> Option<(bool, Option<Vec<Key>>)> {
        let got_delta = self.base.derive_delta(candidate, &[])?;
        #[cfg(test)]
        if super::guard::self_check::enabled() {
            assert_eq!(
                self.base.nets_with(&got_delta),
                signex_net::net_signatures(candidate),
                "the candidate delta differs"
            );
        }
        let (want, extra) = self.intent(guard, tips, skip);
        let base_nets = self.base.nets();
        let mut want: Projected = want.into_iter().collect();
        // Base nets the candidate lost that the plain move still has: a
        // far net the candidate touched and left as it was.
        for b in &got_delta.removed {
            if self.plain.removed.binary_search(b).is_err() {
                want.insert(base_nets[*b].clone());
            }
        }
        let mut got: Projected = BTreeSet::new();
        let project = |net: &NetSignature| net.without(&extra.terminals, &extra.sheet_pins);
        got.extend(got_delta.added.iter().filter_map(project));
        // Base nets the plain move changed that the candidate kept.
        let kept = |b: &&usize| got_delta.removed.binary_search(b).is_err();
        got.extend(
            (self.plain.removed.iter())
                .filter(kept)
                .filter_map(|b| project(&base_nets[*b])),
        );
        if want == got {
            return Some((true, None));
        }
        Some((false, Guard::blame(tips, policy, edited, &want, &got)))
    }

    /// The nets the user's move asks for among the nets the move reaches
    /// (`Guard::intent`): those nets with the moved pins put where the plain
    /// move defines, and the exclusions.
    fn intent(
        &self,
        guard: &Guard,
        tips: &[PinTip],
        skip: &Exclusions,
    ) -> (Vec<NetSignature>, Exclusions) {
        let mut nets = self.plain.added.clone();
        // Take every moved pin out of every net; the intent puts them back.
        let moved: HashSet<(Uuid, &str)> = tips
            .iter()
            .filter_map(|tip| match tip.ident() {
                PinIdent::Terminal(s, p) => Some((*s, p.as_str())),
                PinIdent::SheetPin(_) => None,
            })
            .collect();
        for net in &mut nets {
            net.terminals
                .retain(|(s, p)| !moved.contains(&(*s, p.as_str())));
        }
        debug_assert_eq!(tips.len(), self.home.len());
        let (extra, lone) = place_moved_pins(
            tips,
            skip,
            &mut nets,
            &Placement {
                home: &self.home,
                carried: &self.carried,
                at_old: &self.plain.found,
            },
            |tip| guard.was_connected(tip),
        );
        let want = (nets.into_iter().chain(lone.into_values()))
            .filter(|n| !(n.terminals.is_empty() && n.labels.is_empty() && n.sheet_pins.is_empty()))
            .collect();
        (want, extra)
    }
}
