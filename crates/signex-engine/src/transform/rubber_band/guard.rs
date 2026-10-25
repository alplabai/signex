//! The authoritative check behind the rubber-band heuristics.
//!
//! The geometric rules keep wires tidy; this guard keeps nets honest. After a
//! move has been rubber-banded and junctions reconciled, the candidate sheet's
//! nets are compared with the pre-move sheet's - terminals, labels, child-sheet
//! pins and the net name, via [`signex_net::net_signatures`] - and any
//! difference is a bug in the route, not a feature: the offending tips are
//! escalated (normal -> forced new path -> plain move) and the pass is run
//! again. A plain tip is projected away on both sides of the comparison,
//! because for it the user keeps the old behaviour and gets a warning.

use std::collections::{BTreeSet, HashSet};

use signex_net::{NetSignature, net_signatures, net_signatures_at, uf::Key};
use signex_types::schematic::SchematicSheet;
use uuid::Uuid;

use super::{PinIdent, PinTip};

/// Per old-tip routing overrides chosen by the guard.
#[derive(Debug, Clone, Default)]
pub struct Policy {
    /// Tips that skip the follow attempt and always get a new path.
    pub forced: HashSet<Key>,
    /// Tips that are not rubber-banded at all.
    pub plain: HashSet<Key>,
}

impl Policy {
    /// Move `key` one step down the ladder; false when it was already plain.
    pub fn escalate(&mut self, key: Key) -> bool {
        if self.plain.contains(&key) {
            return false;
        }
        if self.forced.contains(&key) {
            self.plain.insert(key);
        } else {
            self.forced.insert(key);
        }
        true
    }
}

/// The pre-move nets, to compare candidates against.
pub struct Guard {
    before_sheet: SchematicSheet,
    /// The sheet after the plain move (items moved, junctions reconciled,
    /// no rubber-banding): the user's own edits are already in it.
    plain_sheet: SchematicSheet,
    before: Vec<NetSignature>,
    /// Pins the pre-move netlist saw; a moved pin that was on no net is the
    /// user's to place and is left out of the comparison.
    seen_terminals: BTreeSet<(Uuid, String)>,
    seen_sheet_pins: BTreeSet<Uuid>,
}

type Projected = BTreeSet<NetSignature>;

impl Guard {
    #[must_use]
    pub fn new(before_sheet: &SchematicSheet, plain_sheet: &SchematicSheet) -> Self {
        let before = net_signatures(before_sheet);
        let seen_terminals = before
            .iter()
            .flat_map(|net| net.terminals.iter().cloned())
            .collect();
        let seen_sheet_pins = before
            .iter()
            .flat_map(|net| net.sheet_pins.iter().copied())
            .collect();
        Self {
            before_sheet: before_sheet.clone(),
            plain_sheet: plain_sheet.clone(),
            before,
            seen_terminals,
            seen_sheet_pins,
        }
    }

    /// Whether the pre-move netlist had `tip`'s pin on a net.
    #[must_use]
    pub fn was_connected(&self, tip: &PinTip) -> bool {
        match tip.ident() {
            PinIdent::Terminal(symbol, pin) => {
                self.seen_terminals.contains(&(*symbol, pin.clone()))
            }
            PinIdent::SheetPin(uuid) => self.seen_sheet_pins.contains(uuid),
        }
    }

    fn project(
        sigs: &[NetSignature],
        terminals: &BTreeSet<(Uuid, String)>,
        sheet_pins: &BTreeSet<Uuid>,
    ) -> Projected {
        sigs.iter()
            .filter_map(|net| net.without(terminals, sheet_pins))
            .collect()
    }

    /// The nets the user's move asks for: the plain-moved sheet's nets, with
    /// every moved pin that was on a net put back on the net its old tip
    /// still sits on there (a lone one when that net has nothing else to
    /// name it). Pins left out of the comparison stay out.
    fn intent(
        &self,
        tips: &[PinTip],
        skip_terminals: &BTreeSet<(Uuid, String)>,
        skip_pins: &BTreeSet<Uuid>,
        power_out: &HashSet<Uuid>,
        strip: &dyn Fn(&SchematicSheet) -> SchematicSheet,
    ) -> Projected {
        let kept: Vec<&PinTip> = tips
            .iter()
            .filter(|t| {
                self.was_connected(t)
                    && match t.ident() {
                        PinIdent::Terminal(s, p) => !skip_terminals.contains(&(*s, p.clone())),
                        PinIdent::SheetPin(u) => !skip_pins.contains(u),
                    }
            })
            .collect();
        let keys: Vec<Key> = kept.iter().map(|t| t.old_key()).collect();
        let stripped;
        let sheet = if power_out.is_empty() {
            &self.plain_sheet
        } else {
            stripped = strip(&self.plain_sheet);
            &stripped
        };
        let (mut nets, found) = net_signatures_at(sheet, &keys);
        let mut all_terminals = BTreeSet::new();
        let mut all_pins = BTreeSet::new();
        for tip in tips {
            match tip.ident() {
                PinIdent::Terminal(s, p) => {
                    all_terminals.insert((*s, p.clone()));
                }
                PinIdent::SheetPin(u) => {
                    all_pins.insert(*u);
                }
            }
        }
        for net in &mut nets {
            net.terminals.retain(|t| !all_terminals.contains(t));
            net.sheet_pins.retain(|u| !all_pins.contains(u));
        }
        let mut lone: std::collections::BTreeMap<Key, NetSignature> = Default::default();
        for (tip, (root, at)) in kept.iter().zip(found) {
            let net = match at {
                Some(index) => &mut nets[index],
                None => lone.entry(root).or_insert_with(|| NetSignature {
                    terminals: BTreeSet::new(),
                    labels: BTreeSet::new(),
                    sheet_pins: BTreeSet::new(),
                    name: String::new(),
                }),
            };
            match tip.ident() {
                PinIdent::Terminal(s, p) => {
                    net.terminals.insert((*s, p.clone()));
                }
                PinIdent::SheetPin(u) => {
                    net.sheet_pins.insert(*u);
                }
            }
        }
        nets.into_iter()
            .chain(lone.into_values())
            .filter(|n| !(n.terminals.is_empty() && n.labels.is_empty() && n.sheet_pins.is_empty()))
            .collect()
    }

    /// `None` when `candidate` has exactly the pre-move nets; otherwise the
    /// old-tip keys to blame (every non-plain tip when the difference cannot
    /// be tied to a moved pin).
    #[must_use]
    pub fn check(
        &self,
        candidate: &SchematicSheet,
        tips: &[PinTip],
        policy: &Policy,
        edited: &[Key],
    ) -> Option<Vec<Key>> {
        let mut terminals = BTreeSet::new();
        let mut sheet_pins = BTreeSet::new();
        let unseen = |tip: &PinTip| match tip.ident() {
            PinIdent::Terminal(symbol, pin) => {
                !self.seen_terminals.contains(&(*symbol, pin.clone()))
            }
            PinIdent::SheetPin(uuid) => !self.seen_sheet_pins.contains(uuid),
        };
        for tip in tips
            .iter()
            .filter(|t| policy.plain.contains(&t.old_key()) || unseen(t))
        {
            match tip.ident() {
                PinIdent::Terminal(symbol, pin) => {
                    terminals.insert((*symbol, pin.clone()));
                }
                PinIdent::SheetPin(uuid) => {
                    sheet_pins.insert(*uuid);
                }
            }
        }
        // A power port that is not rubber-banded took its name-merges with it,
        // which no projection can undo: compare the sheets without it.
        let power_out: HashSet<Uuid> = tips
            .iter()
            .filter(|t| t.is_power() && (policy.plain.contains(&t.old_key()) || unseen(t)))
            .filter_map(|t| match t.ident() {
                PinIdent::Terminal(symbol, _) => Some(*symbol),
                PinIdent::SheetPin(_) => None,
            })
            .collect();
        let moved_power_stays = tips
            .iter()
            .any(|t| t.is_power() && self.was_connected(t) && !policy.plain.contains(&t.old_key()));
        let strip = |sheet: &SchematicSheet| {
            let mut copy = sheet.clone();
            copy.symbols.retain(|s| !power_out.contains(&s.uuid));
            copy
        };
        let got = {
            let sheet = if power_out.is_empty() {
                None
            } else {
                Some(strip(candidate))
            };
            Self::project(
                &net_signatures(sheet.as_ref().unwrap_or(candidate)),
                &terminals,
                &sheet_pins,
            )
        };
        let want = if moved_power_stays {
            // A kept power port names nets by value, which the intent
            // reference below cannot restate: compare with the pre-move nets
            // (a co-selected edit beside a power port is then also held
            // against the pass).
            let sigs = if power_out.is_empty() {
                self.before.clone()
            } else {
                net_signatures(&strip(&self.before_sheet))
            };
            Self::project(&sigs, &terminals, &sheet_pins)
        } else {
            self.intent(tips, &terminals, &sheet_pins, &power_out, &strip)
        };
        if want == got {
            return None;
        }

        let mut mentioned_terminals: BTreeSet<&(Uuid, String)> = BTreeSet::new();
        let mut mentioned_pins: BTreeSet<&Uuid> = BTreeSet::new();
        for net in want.symmetric_difference(&got) {
            mentioned_terminals.extend(net.terminals.iter());
            mentioned_pins.extend(net.sheet_pins.iter());
        }
        let live = || tips.iter().filter(|t| !policy.plain.contains(&t.old_key()));
        let mut blamed: Vec<Key> = live()
            .filter(|tip| match tip.ident() {
                PinIdent::Terminal(symbol, pin) => {
                    mentioned_terminals.contains(&(*symbol, pin.clone()))
                }
                PinIdent::SheetPin(uuid) => mentioned_pins.contains(uuid),
            })
            .map(PinTip::old_key)
            .collect();
        if blamed.is_empty() {
            // A difference no moved pin is in: only a tip whose handling
            // edited something can have caused it. None did: it is the user's
            // own edit and stays.
            blamed = live()
                .map(PinTip::old_key)
                .filter(|key| edited.contains(key))
                .collect();
            if blamed.is_empty() {
                return None;
            }
        }
        blamed.sort_unstable();
        blamed.dedup();
        Some(blamed)
    }
}
