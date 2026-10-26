//! The authoritative check behind the rubber-band heuristics.
//!
//! The geometric rules keep wires tidy; this guard keeps nets honest. After a
//! move has been rubber-banded and junctions reconciled, the candidate sheet's
//! nets are compared with what the user's move asks for - terminals, labels,
//! child-sheet pins and the net name, via [`signex_net::net_signatures`] - and
//! any difference is a bug in the route, not a feature: the offending tips are
//! escalated (normal -> forced new path -> plain move) and the pass is run
//! again. A plain tip is projected away on both sides of the comparison,
//! because for it the user keeps the old behaviour and gets a warning. Each
//! round escalates at least one tip, so there are at most two rounds per tip.
//!
//! The reference is the *intent*, not the pre-move sheet: the plain-moved
//! sheet's nets (a co-selected label, dot or wire that leaves its wire is the
//! user's own edit), with every moved pin put on the net the plain move
//! defines for it - the net at its old tip when something stays there to
//! carry the connection, else the net it ends up on (a co-moved wire carries
//! it along).

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::Arc;

use signex_net::{
    NetSignature, is_power_flag, net_signatures, net_signatures_at, point_on_segment, pt_key,
    uf::Key,
};
use signex_types::schematic::{LabelType, SchematicSheet};
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

type Projected = BTreeSet<NetSignature>;
type Terminals = BTreeSet<(Uuid, String)>;

/// The pre-move nets: computed once and shared, so a drag preview does not
/// redo them for every cursor step.
pub type BeforeNets = Arc<Vec<NetSignature>>;

/// Derive the pre-move nets of `sheet`.
#[must_use]
pub fn before_nets(sheet: &SchematicSheet) -> BeforeNets {
    Arc::new(net_signatures(sheet))
}

/// Pins and power ports left out of (or standing in for) a comparison.
struct Exclusions {
    terminals: Terminals,
    sheet_pins: BTreeSet<Uuid>,
    /// Power ports that are not rubber-banded: they took their name-merges
    /// with them, so both sheets are compared without them.
    power_out: HashSet<Uuid>,
}

/// The plain-moved sheet seen from the moved pins.
struct PlainView {
    nets: Vec<NetSignature>,
    /// Per tip: the net its old tip sits on (opaque id, index when named).
    at_old: Vec<(Key, Option<usize>)>,
    /// Per tip: the net the pin itself is on in the plain-moved sheet.
    home: Vec<Option<usize>>,
    /// Per tip: nothing stays at the old tip to carry the connection.
    carried: Vec<bool>,
}

impl PlainView {
    fn of(sheet: &SchematicSheet, tips: &[PinTip]) -> Self {
        let keys: Vec<(Key, bool)> = tips
            .iter()
            .map(|t| (t.old_key(), t.is_sheet_pin()))
            .collect();
        let (nets, at_old) = net_signatures_at(sheet, &keys);
        let home = tips
            .iter()
            .map(|tip| nets.iter().position(|net| holds(net, tip.ident())))
            .collect();
        let carried = keys.iter().map(|(k, _)| !something_at(sheet, *k)).collect();
        Self {
            nets,
            at_old,
            home,
            carried,
        }
    }
}

fn holds(net: &NetSignature, ident: &PinIdent) -> bool {
    match ident {
        PinIdent::Terminal(symbol, pin) => net.terminals.contains(&(*symbol, pin.clone())),
        PinIdent::SheetPin(uuid) => net.sheet_pins.contains(uuid),
    }
}

fn insert_ident(net: &mut NetSignature, ident: &PinIdent) {
    match ident {
        PinIdent::Terminal(symbol, pin) => {
            net.terminals.insert((*symbol, pin.clone()));
        }
        PinIdent::SheetPin(uuid) => {
            net.sheet_pins.insert(*uuid);
        }
    }
}

/// Whether a wire end or interior, label, dot or no-connect sits on `key`.
fn something_at(sheet: &SchematicSheet, key: Key) -> bool {
    sheet.wires.iter().any(|w| {
        let (a, b) = (pt_key(&w.start), pt_key(&w.end));
        point_on_segment(key, a, b)
    }) || sheet.labels.iter().any(|l| pt_key(&l.position) == key)
        || sheet.junctions.iter().any(|j| pt_key(&j.position) == key)
        || sheet.no_connects.iter().any(|n| pt_key(&n.position) == key)
}

/// Whether wires, labels, dots and no-connects sit where they did.
fn same_wiring(a: &SchematicSheet, b: &SchematicSheet) -> bool {
    let wires = |s: &SchematicSheet| -> Vec<_> {
        s.wires
            .iter()
            .map(|w| (w.uuid, pt_key(&w.start), pt_key(&w.end)))
            .collect()
    };
    let at = |pts: Vec<(Uuid, &signex_types::schematic::Point)>| -> Vec<_> {
        pts.into_iter().map(|(u, p)| (u, pt_key(p))).collect()
    };
    wires(a) == wires(b)
        && at(a.labels.iter().map(|l| (l.uuid, &l.position)).collect())
            == at(b.labels.iter().map(|l| (l.uuid, &l.position)).collect())
        && at(a.junctions.iter().map(|l| (l.uuid, &l.position)).collect())
            == at(b.junctions.iter().map(|l| (l.uuid, &l.position)).collect())
        && at(a
            .no_connects
            .iter()
            .map(|l| (l.uuid, &l.position))
            .collect())
            == at(b
                .no_connects
                .iter()
                .map(|l| (l.uuid, &l.position))
                .collect())
}

/// The pre-move nets and the plain-moved reference, to compare candidates
/// against.
pub struct Guard {
    before_sheet: SchematicSheet,
    plain_sheet: SchematicSheet,
    before: BeforeNets,
    /// Constant across rounds: computed once.
    plain_view: PlainView,
    /// Pins the pre-move netlist saw; a moved pin that was on no net is the
    /// user's to place and is left out of the comparison.
    seen_terminals: Terminals,
    seen_sheet_pins: BTreeSet<Uuid>,
    /// The plain move changed nothing but symbol and child-sheet positions.
    only_symbols_moved: bool,
}

impl Guard {
    #[must_use]
    pub fn new(
        before_sheet: &SchematicSheet,
        plain_sheet: &SchematicSheet,
        tips: &[PinTip],
        before: Option<BeforeNets>,
    ) -> Self {
        let before = before.unwrap_or_else(|| before_nets(before_sheet));
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
            plain_view: PlainView::of(plain_sheet, tips),
            seen_terminals,
            seen_sheet_pins,
            only_symbols_moved: same_wiring(before_sheet, plain_sheet),
        }
    }

    /// Whether the pre-move netlist had `tip`'s pin on a net.
    #[must_use]
    pub fn was_connected(&self, tip: &PinTip) -> bool {
        match tip.ident() {
            PinIdent::Terminal(symbol, pin) => {
                self.seen_terminals.contains(&(*symbol, pin.clone()))
            }
            // A child-sheet pin always has a net of its own; it is connected
            // only if something is at its tip.
            PinIdent::SheetPin(uuid) => {
                self.seen_sheet_pins.contains(uuid)
                    && something_at(&self.before_sheet, tip.old_key())
            }
        }
    }

    fn project(sigs: &[NetSignature], skip: &Exclusions) -> Projected {
        sigs.iter()
            .filter_map(|net| net.without(&skip.terminals, &skip.sheet_pins))
            .collect()
    }

    fn exclusions(&self, tips: &[PinTip], policy: &Policy) -> Exclusions {
        let mut out = Exclusions {
            terminals: BTreeSet::new(),
            sheet_pins: BTreeSet::new(),
            power_out: HashSet::new(),
        };
        for tip in tips {
            if !(policy.plain.contains(&tip.old_key()) || !self.was_connected(tip)) {
                continue;
            }
            match tip.ident() {
                PinIdent::Terminal(symbol, pin) => {
                    out.terminals.insert((*symbol, pin.clone()));
                    if tip.is_power() {
                        out.power_out.insert(*symbol);
                    }
                }
                PinIdent::SheetPin(uuid) => {
                    out.sheet_pins.insert(*uuid);
                }
            }
        }
        out
    }

    fn strip(sheet: &SchematicSheet, power_out: &HashSet<Uuid>) -> SchematicSheet {
        let mut copy = sheet.clone();
        copy.symbols.retain(|s| !power_out.contains(&s.uuid));
        copy
    }

    /// Take every moved pin out of `nets`; the intent puts them back.
    fn clear_moved(nets: &mut [NetSignature], tips: &[PinTip]) {
        for net in nets {
            for tip in tips {
                match tip.ident() {
                    PinIdent::Terminal(s, p) => {
                        net.terminals.remove(&(*s, p.clone()));
                    }
                    PinIdent::SheetPin(u) => {
                        net.sheet_pins.remove(u);
                    }
                }
            }
        }
    }

    /// The nets the user's move asks for. Returns them with the extra pins
    /// whose plain-move connection is undefined and so stay out of the
    /// comparison on both sides.
    fn intent(&self, tips: &[PinTip], skip: &Exclusions) -> (Projected, Exclusions) {
        let stripped;
        let view = if skip.power_out.is_empty() {
            &self.plain_view
        } else {
            stripped = PlainView::of(&Self::strip(&self.plain_sheet, &skip.power_out), tips);
            &stripped
        };
        let mut nets = view.nets.clone();
        Self::clear_moved(&mut nets, tips);
        let mut extra = Exclusions {
            terminals: BTreeSet::new(),
            sheet_pins: BTreeSet::new(),
            power_out: HashSet::new(),
        };
        let mut lone: BTreeMap<Key, NetSignature> = BTreeMap::new();
        for (i, tip) in tips.iter().enumerate() {
            let skipped = match tip.ident() {
                PinIdent::Terminal(s, p) => skip.terminals.contains(&(*s, p.clone())),
                PinIdent::SheetPin(u) => skip.sheet_pins.contains(u),
            };
            if skipped || !self.was_connected(tip) {
                // A pin that is not rubber-banded (plain, or never on a net)
                // is expected exactly where the plain move left it, so a
                // wire the pass stretched or added under it shows up.
                if let Some(n) = view.home[i].filter(|_| !tip.is_power()) {
                    insert_ident(&mut nets[n], tip.ident());
                }
                continue;
            }
            let target = if view.carried[i] {
                // The wire that carried this pin moved with it: the plain
                // move defines the net the pin lands on.
                match view.home[i] {
                    Some(n) => Some(&mut nets[n]),
                    None => {
                        match tip.ident() {
                            PinIdent::Terminal(s, p) => {
                                extra.terminals.insert((*s, p.clone()));
                            }
                            PinIdent::SheetPin(u) => {
                                extra.sheet_pins.insert(*u);
                            }
                        }
                        None
                    }
                }
            } else {
                match view.at_old[i] {
                    (_, Some(n)) => Some(&mut nets[n]),
                    (root, None) => Some(lone.entry(root).or_insert_with(empty_net)),
                }
            };
            if let Some(net) = target {
                insert_ident(net, tip.ident());
            }
        }
        let mut want: Vec<NetSignature> = nets
            .into_iter()
            .chain(lone.into_values())
            .filter(|n| !(n.terminals.is_empty() && n.labels.is_empty() && n.sheet_pins.is_empty()))
            .collect();
        self.join_kept_ports(&mut want, tips, skip);
        (want.into_iter().collect(), extra)
    }

    /// A kept power port names the net its pin lands on and, like every port
    /// of its value, joins the nets that value already names on the sheet
    /// (other ports, `Global` / `Power` labels). The plain move left the port
    /// elsewhere, so the intent restates both here. When the joined nets
    /// carry different names the merged name is not derivable from
    /// signatures; the nets stay apart and the guard escalates as before.
    fn join_kept_ports(&self, want: &mut Vec<NetSignature>, tips: &[PinTip], skip: &Exclusions) {
        for tip in tips {
            let PinIdent::Terminal(symbol, pin) = tip.ident() else {
                continue;
            };
            let key = (*symbol, pin.clone());
            let value = tip.power_name();
            if !tip.is_power()
                || value.is_empty()
                || skip.terminals.contains(&key)
                || !self.was_connected(tip)
                || !self
                    .before
                    .iter()
                    .any(|n| n.terminals.contains(&key) && n.name == value)
            {
                continue;
            }
            let Some(at) = want.iter().position(|n| n.terminals.contains(&key)) else {
                continue;
            };
            let (ports, labels) = self.named_by(value);
            let joined: Vec<usize> = (0..want.len())
                .filter(|&j| {
                    j != at
                        && (want[j].terminals.iter().any(|(s, _)| ports.contains(s))
                            || want[j].labels.iter().any(|l| labels.contains(l)))
                })
                .collect();
            let names: BTreeSet<&str> = std::iter::once(at)
                .chain(joined.iter().copied())
                .map(|j| want[j].name.as_str())
                .filter(|name| !name.is_empty())
                .collect();
            if names.len() > 1 {
                continue;
            }
            let name = names.first().copied().unwrap_or(value).to_string();
            for &j in &joined {
                let net = std::mem::replace(&mut want[j], empty_net());
                want[at].terminals.extend(net.terminals);
                want[at].labels.extend(net.labels);
                want[at].sheet_pins.extend(net.sheet_pins);
            }
            want[at].name = name;
            want.retain(|n| {
                !(n.terminals.is_empty() && n.labels.is_empty() && n.sheet_pins.is_empty())
            });
        }
    }

    /// The power ports (not flags) and `Global` / `Power` labels of the plain
    /// sheet that join a net by `value` (`signex_net`'s `merge_power_ports`).
    fn named_by(&self, value: &str) -> (HashSet<Uuid>, HashSet<Uuid>) {
        let ports = self
            .plain_sheet
            .symbols
            .iter()
            .filter(|s| s.is_power && s.value == value && !is_power_flag(&s.lib_id))
            .map(|s| s.uuid)
            .collect();
        let labels = self
            .plain_sheet
            .labels
            .iter()
            .filter(|l| {
                l.text == value && matches!(l.label_type, LabelType::Global | LabelType::Power)
            })
            .map(|l| l.uuid)
            .collect();
        (ports, labels)
    }

    /// The reference to compare with: the user's intent. A kept power port
    /// also merges nets by name, which the intent cannot restate once the
    /// user's own co-selected edits (a wire, label or dot that left its place)
    /// have renamed nets; when only symbols moved the pre-move nets are exact.
    fn reference(
        &self,
        tips: &[PinTip],
        skip: &Exclusions,
        policy: &Policy,
    ) -> (Projected, Exclusions) {
        let power_stays = tips
            .iter()
            .any(|t| t.is_power() && self.was_connected(t) && !policy.plain.contains(&t.old_key()));
        if !(power_stays && self.only_symbols_moved) {
            return self.intent(tips, skip);
        }
        let sigs = if skip.power_out.is_empty() {
            self.before.to_vec()
        } else {
            net_signatures(&Self::strip(&self.before_sheet, &skip.power_out))
        };
        let same = Exclusions {
            terminals: skip.terminals.clone(),
            sheet_pins: skip.sheet_pins.clone(),
            power_out: HashSet::new(),
        };
        (Self::project(&sigs, skip), same)
    }

    /// `None` when `candidate` has exactly the nets the move asks for;
    /// otherwise the old-tip keys to blame. A difference that names no moved
    /// pin is blamed on the tips whose handling edited something (`edited`),
    /// and when there are none it is the user's own edit and stands.
    #[must_use]
    pub fn check(
        &self,
        candidate: &SchematicSheet,
        tips: &[PinTip],
        policy: &Policy,
        edited: &[Key],
    ) -> Option<Vec<Key>> {
        let skip = self.exclusions(tips, policy);
        let (want, extra) = self.reference(tips, &skip, policy);
        // Pins with an undefined plain connection stay out of both sides;
        // skipped pins are compared against where the plain move left them.
        let sigs = if skip.power_out.is_empty() {
            net_signatures(candidate)
        } else {
            net_signatures(&Self::strip(candidate, &skip.power_out))
        };
        let got = Self::project(&sigs, &extra);
        if want == got {
            return None;
        }
        Self::blame(tips, policy, edited, &want, &got)
    }

    fn blame(
        tips: &[PinTip],
        policy: &Policy,
        edited: &[Key],
        want: &Projected,
        got: &Projected,
    ) -> Option<Vec<Key>> {
        let mut terminals: BTreeSet<&(Uuid, String)> = BTreeSet::new();
        let mut pins: BTreeSet<&Uuid> = BTreeSet::new();
        for net in want.symmetric_difference(got) {
            terminals.extend(net.terminals.iter());
            pins.extend(net.sheet_pins.iter());
        }
        let live = || tips.iter().filter(|t| !policy.plain.contains(&t.old_key()));
        let mut blamed: Vec<Key> = live()
            .filter(|tip| match tip.ident() {
                PinIdent::Terminal(symbol, pin) => terminals.contains(&(*symbol, pin.clone())),
                PinIdent::SheetPin(uuid) => pins.contains(uuid),
            })
            .map(PinTip::old_key)
            .collect();
        if blamed.is_empty() {
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

fn empty_net() -> NetSignature {
    NetSignature {
        terminals: BTreeSet::new(),
        labels: BTreeSet::new(),
        sheet_pins: BTreeSet::new(),
        name: String::new(),
    }
}
