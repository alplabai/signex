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

use std::cell::OnceCell;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::Arc;

use signex_net::{
    NetBase, NetSignature, best_net_name_of, name_carrying_ports, net_signatures,
    net_signatures_at, pt_key, uf::Key,
};
use signex_types::schematic::{LabelType, SchematicSheet, SelectedItem, SelectedKind};
use uuid::Uuid;

use super::guard_local::LocalGuard;
use super::intent::{self, Slot};
use super::{PinIdent, PinTip, WirePoint};

/// A queried point's answer: its net's opaque id, and its index when named.
pub(super) type Found = (Key, Option<usize>);

/// Per old-point routing overrides chosen by the guard, for moved pins and
/// for the points a dragged wire carries alike.
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

pub(super) type Projected = BTreeSet<NetSignature>;
pub(super) type Terminals = BTreeSet<(Uuid, String)>;

/// The pre-move nets: computed once and shared, so a drag preview does not
/// redo them for every cursor step.
pub type BeforeNets = Arc<Vec<NetSignature>>;

/// Derive the pre-move nets of `sheet`.
#[must_use]
pub fn before_nets(sheet: &SchematicSheet) -> BeforeNets {
    Arc::new(net_signatures(sheet))
}

type Derived = (Vec<NetSignature>, Vec<(Key, Option<usize>)>);

/// [`net_signatures_at`] of `sheet`, found from `base` when there is one. The
/// tests hold every such answer to the full derive.
fn derive_at(base: Option<&NetBase>, sheet: &SchematicSheet, at: &[(Key, bool)]) -> Derived {
    let Some(base) = base else {
        return net_signatures_at(sheet, at);
    };
    let found = base.derive_at(sheet, at);
    #[cfg(test)]
    if self_check::enabled() {
        debug_assert!(
            base.derives_incrementally(sheet),
            "the base fell back to the full derive"
        );
        assert_eq!(
            found,
            net_signatures_at(sheet, at),
            "the incremental derive differs from the full derive"
        );
    }
    found
}

/// Pins and power ports left out of (or standing in for) a comparison.
pub(super) struct Exclusions {
    pub(super) terminals: Terminals,
    pub(super) sheet_pins: BTreeSet<Uuid>,
    /// Power ports that are not rubber-banded: they took their name-merges
    /// with them, so both sheets are compared without them.
    pub(super) power_out: HashSet<Uuid>,
}

/// The plain-moved sheet seen from the moved pins and the carried points.
struct PlainView {
    nets: Vec<NetSignature>,
    /// Per tip: the net its old tip sits on (opaque id, index when named).
    at_old: Vec<Found>,
    /// Per tip: the net the pin itself is on in the plain-moved sheet.
    home: Vec<Option<usize>>,
    /// Per tip: nothing stays at the old tip to carry the connection.
    carried: Vec<bool>,
    /// Per carried point: the net at its old place and the net its wire
    /// landed on.
    wire_at: Vec<(Found, Found)>,
}

/// The points to query on the plain-moved sheet: every tip's old place, then
/// each carried point's old place and the place on its wire it moved to.
pub(super) fn query_keys(tips: &[PinTip], wires: &[WirePoint]) -> Vec<(Key, bool)> {
    let mut keys: Vec<(Key, bool)> = tips
        .iter()
        .map(|t| (t.old_key(), t.is_sheet_pin()))
        .collect();
    keys.extend((wires.iter()).flat_map(|w| [(w.old_key(), false), (w.new_key(), true)]));
    keys
}

/// The answers of [`query_keys`] past the tips, paired per carried point.
pub(super) fn wire_answers(found: &[Found], tips: usize) -> Vec<(Found, Found)> {
    (found[tips..].chunks(2))
        .filter(|pair| pair.len() == 2)
        .map(|pair| (pair[0], pair[1]))
        .collect()
}

impl PlainView {
    fn of(
        sheet: &SchematicSheet,
        tips: &[PinTip],
        wires: &[WirePoint],
        base: Option<&NetBase>,
    ) -> Self {
        let keys = query_keys(tips, wires);
        let (nets, found) = derive_at(base, sheet, &keys);
        let wire_at = wire_answers(&found, tips.len());
        let at_old = found[..tips.len()].to_vec();
        let home = tips
            .iter()
            .map(|tip| nets.iter().position(|net| holds(net, tip.ident())))
            .collect();
        let carried = keys[..tips.len()]
            .iter()
            .map(|(k, _)| !signex_net::point_is_occupied(sheet, *k))
            .collect();
        Self {
            nets,
            at_old,
            home,
            carried,
            wire_at,
        }
    }
}

/// The joins the kept carried points ask for: per live point, the net at its
/// old place with the net its wire landed on.
pub(super) fn wire_joins(
    wires: &[WirePoint],
    at: &[(Found, Found)],
    policy: &Policy,
) -> Vec<(Slot, Slot)> {
    (wires.iter().zip(at))
        .filter(|(w, _)| !policy.plain.contains(&w.old_key()))
        .map(|(_, (old, new))| (Slot::at(*old), Slot::at(*new)))
        .collect()
}

/// The pins left as a plain move that were on a net: the warned ones.
pub(super) fn warned_idents<'t>(
    tips: &'t [PinTip],
    policy: &Policy,
    was_connected: impl Fn(&PinTip) -> bool,
) -> Vec<&'t PinIdent> {
    tips.iter()
        .filter(|tip| policy.plain.contains(&tip.old_key()) && was_connected(tip))
        .map(PinTip::ident)
        .collect()
}

/// Per carried point, the component pins at its old place in `sheet` (power
/// ports, which name nets, and the moved pins in `tips` left out). A pin with
/// nothing else at its tip is on no net, yet it is what the point joined.
pub(super) fn pins_at(
    sheet: &SchematicSheet,
    wires: &[WirePoint],
    tips: &[PinTip],
) -> Vec<Terminals> {
    if wires.is_empty() {
        return Vec::new();
    }
    let olds: HashSet<Key> = wires.iter().map(WirePoint::old_key).collect();
    let symbols: Vec<SelectedItem> = (sheet.symbols.iter())
        .filter(|symbol| !symbol.is_power)
        .map(|symbol| SelectedItem::new(symbol.uuid, SelectedKind::Symbol))
        .collect();
    let moved: Vec<&PinIdent> = tips.iter().map(PinTip::ident).collect();
    let mut at: Vec<(Key, (Uuid, String))> = Vec::new();
    for tip in super::moved_pin_tips(sheet, &symbols) {
        if !olds.contains(&tip.old_key()) || moved.contains(&tip.ident()) {
            continue;
        }
        if let PinIdent::Terminal(symbol, pin) = tip.ident() {
            at.push((tip.old_key(), (*symbol, pin.clone())));
        }
    }
    (wires.iter())
        .map(|w| {
            (at.iter())
                .filter(|(k, _)| *k == w.old_key())
                .map(|(_, pin)| pin.clone())
                .collect()
        })
        .collect()
}

/// Put the pins at a kept point's old place (`pins_at`) on the net known only
/// by its id there, so the point's join holds them as it joins that net.
pub(super) fn seed_lone(
    lone: &mut BTreeMap<Key, NetSignature>,
    wires: &[WirePoint],
    at: &[(Found, Found)],
    pins: &[Terminals],
    policy: &Policy,
) {
    for ((w, (old, _)), pins) in wires.iter().zip(at).zip(pins) {
        if old.1.is_some() || pins.is_empty() || policy.plain.contains(&w.old_key()) {
            continue;
        }
        let net = lone.entry(old.0).or_insert_with(empty_net);
        net.terminals.extend(pins.iter().cloned());
    }
}

/// Whether `net` holds one of `idents`.
pub(super) fn holds_any(net: &NetSignature, idents: &[&PinIdent]) -> bool {
    idents.iter().any(|ident| holds(net, ident))
}

fn holds(net: &NetSignature, ident: &PinIdent) -> bool {
    match ident {
        PinIdent::Terminal(symbol, pin) => net.terminals.contains(&(*symbol, pin.clone())),
        PinIdent::SheetPin(uuid) => net.sheet_pins.contains(uuid),
    }
}

pub(super) fn insert_ident(net: &mut NetSignature, ident: &PinIdent) {
    match ident {
        PinIdent::Terminal(symbol, pin) => {
            net.terminals.insert((*symbol, pin.clone()));
        }
        PinIdent::SheetPin(uuid) => {
            net.sheet_pins.insert(*uuid);
        }
    }
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
pub struct Guard<'a> {
    before_sheet: &'a SchematicSheet,
    plain_sheet: &'a SchematicSheet,
    before: BeforeNets,
    /// Constant across rounds: computed once, when the comparison over every
    /// net first needs it.
    plain_view: OnceCell<PlainView>,
    /// Pins the pre-move netlist saw; a moved pin that was on no net is the
    /// user's to place and is left out of the comparison.
    seen_terminals: OnceCell<Terminals>,
    seen_sheet_pins: OnceCell<BTreeSet<Uuid>>,
    /// The plain move changed nothing but symbol and child-sheet positions.
    only_symbols_moved: OnceCell<bool>,
    /// The pre-move sheet's connectivity, to derive the moved sheets from
    /// instead of from scratch (a drag preview).
    base: Option<Arc<NetBase>>,
    /// The comparison done on the nets the move reaches, when it can be
    /// (`guard_local.rs`); `None` runs the comparison over every net.
    local: Option<LocalGuard>,
    /// The points the dragged wires carry, each kept unless the policy has
    /// its old key plain.
    wires: Vec<WirePoint>,
    /// Per carried point, the component pins at its old place (`pins_at`).
    wire_pins: Vec<Terminals>,
}

impl<'a> Guard<'a> {
    #[must_use]
    pub fn new(
        before_sheet: &'a SchematicSheet,
        plain_sheet: &'a SchematicSheet,
        tips: &[PinTip],
        before: Option<BeforeNets>,
    ) -> Self {
        Self::with_base(before_sheet, plain_sheet, tips, before, None)
    }

    /// [`Self::new`] deriving every moved sheet's nets from `base`, the
    /// connectivity of `before_sheet` (the answer is the same, found faster).
    #[must_use]
    pub fn with_base(
        before_sheet: &'a SchematicSheet,
        plain_sheet: &'a SchematicSheet,
        tips: &[PinTip],
        before: Option<BeforeNets>,
        base: Option<Arc<NetBase>>,
    ) -> Self {
        Self::for_move(before_sheet, plain_sheet, tips, &[], before, base)
    }

    /// [`Self::with_base`] for a move that also drags wires: `wires` are the
    /// points they carry, which the intent keeps joined like the moved pins.
    #[must_use]
    pub fn for_move(
        before_sheet: &'a SchematicSheet,
        plain_sheet: &'a SchematicSheet,
        tips: &[PinTip],
        wires: &[WirePoint],
        before: Option<BeforeNets>,
        base: Option<Arc<NetBase>>,
    ) -> Self {
        let before = before.unwrap_or_else(|| before_nets(before_sheet));
        let wire_pins = pins_at(plain_sheet, wires, tips);
        let local = (base.as_ref())
            .and_then(|base| LocalGuard::new(base, plain_sheet, tips, wires, &wire_pins));
        Self {
            before_sheet,
            plain_sheet,
            before,
            plain_view: OnceCell::new(),
            seen_terminals: OnceCell::new(),
            seen_sheet_pins: OnceCell::new(),
            only_symbols_moved: OnceCell::new(),
            base,
            local,
            wires: wires.to_vec(),
            wire_pins,
        }
    }

    /// The tips the intent keeps on the net at their old place (live, on a
    /// net before, something still at the old tip in the plain move) whose
    /// plain move lands them on another net, as indices into `tips`: the pins
    /// spec gap (a) is about (`intent`).
    #[must_use]
    pub fn held_tips(&self, tips: &[PinTip], policy: &Policy) -> Vec<usize> {
        let (carried, home, old): (&[bool], &[Option<usize>], Vec<Option<usize>>) =
            match &self.local {
                Some(local) => (local.carried(), local.home(), local.old_nets(tips.len())),
                None => {
                    let view = self.plain_view(tips);
                    let old = view.at_old.iter().map(|found| found.1).collect();
                    (view.carried.as_slice(), view.home.as_slice(), old)
                }
            };
        (tips.iter().enumerate())
            .filter(|(i, tip)| {
                let landing = home.get(*i).copied().flatten();
                !policy.plain.contains(&tip.old_key())
                    && self.was_connected(tip)
                    && !carried.get(*i).copied().unwrap_or(true)
                    && landing.is_some_and(|n| old.get(*i).copied().flatten() != Some(n))
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// This guard comparing over every net, as it does without a drag
    /// preview's base: the reference the local comparison is held to.
    #[cfg(test)]
    pub(crate) fn without_local(mut self) -> Self {
        self.local = None;
        self
    }

    fn plain_view(&self, tips: &[PinTip]) -> &PlainView {
        self.plain_view.get_or_init(|| {
            PlainView::of(self.plain_sheet, tips, &self.wires, self.base.as_deref())
        })
    }

    fn seen_terminals(&self) -> &Terminals {
        self.seen_terminals.get_or_init(|| {
            (self.before.iter())
                .flat_map(|net| net.terminals.iter().cloned())
                .collect()
        })
    }

    fn seen_sheet_pins(&self) -> &BTreeSet<Uuid> {
        self.seen_sheet_pins.get_or_init(|| {
            (self.before.iter())
                .flat_map(|net| net.sheet_pins.iter().copied())
                .collect()
        })
    }

    fn only_symbols_moved(&self) -> bool {
        *self
            .only_symbols_moved
            .get_or_init(|| same_wiring(self.before_sheet, self.plain_sheet))
    }

    /// The nets of `sheet`.
    fn nets_of(&self, sheet: &SchematicSheet) -> Vec<NetSignature> {
        derive_at(self.base.as_deref(), sheet, &[]).0
    }

    /// Whether the pre-move netlist had `tip`'s pin on a net.
    #[must_use]
    pub fn was_connected(&self, tip: &PinTip) -> bool {
        match tip.ident() {
            PinIdent::Terminal(symbol, pin) => match &self.local {
                Some(local) => local.was_connected(*symbol, pin),
                None => self.seen_terminals().contains(&(*symbol, pin.clone())),
            },
            // A child-sheet pin always has a net of its own; it is connected
            // only if something is at its tip.
            PinIdent::SheetPin(uuid) => {
                self.seen_sheet_pins().contains(uuid)
                    && signex_net::point_is_occupied(self.before_sheet, tip.old_key())
            }
        }
    }

    pub(super) fn project(sigs: &[NetSignature], skip: &Exclusions) -> Projected {
        sigs.iter()
            .filter_map(|net| net.without(&skip.terminals, &skip.sheet_pins))
            .collect()
    }

    pub(super) fn exclusions(&self, tips: &[PinTip], policy: &Policy) -> Exclusions {
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
    fn intent(
        &self,
        tips: &[PinTip],
        skip: &Exclusions,
        policy: &Policy,
    ) -> (Projected, Exclusions) {
        let stripped;
        let view = if skip.power_out.is_empty() {
            self.plain_view(tips)
        } else {
            stripped = PlainView::of(
                &Self::strip(self.plain_sheet, &skip.power_out),
                tips,
                &self.wires,
                self.base.as_deref(),
            );
            &stripped
        };
        let mut nets = view.nets.clone();
        Self::clear_moved(&mut nets, tips);
        let (extra, mut lone, mut joins) = place_moved_pins(
            tips,
            skip,
            &mut nets,
            &Placement {
                home: &view.home,
                carried: &view.carried,
                at_old: &view.at_old,
            },
            |tip| self.was_connected(tip),
        );
        joins.extend(wire_joins(&self.wires, &view.wire_at, policy));
        seed_lone(
            &mut lone,
            &self.wires,
            &view.wire_at,
            &self.wire_pins,
            policy,
        );
        let warned = warned_idents(tips, policy, |tip| self.was_connected(tip));
        let joined = intent::join_nets(
            nets,
            lone,
            &joins,
            |net| holds_any(net, &warned),
            |net| self.joined_name(net),
        );
        let mut want: Vec<NetSignature> = joined
            .into_iter()
            .filter(|n| !(n.terminals.is_empty() && n.labels.is_empty() && n.sheet_pins.is_empty()))
            .collect();
        self.join_kept_ports(&mut want, tips, skip);
        (want.into_iter().collect(), extra)
    }

    /// A kept power port names the net its pin lands on and, like every port
    /// of its value, joins the nets that value already names on the sheet
    /// (other name-carrying ports, `Global` / `Power` labels). The plain move
    /// left the port elsewhere, so the intent restates both here, with the
    /// netlist's own port filter and naming rule.
    fn join_kept_ports(&self, want: &mut Vec<NetSignature>, tips: &[PinTip], skip: &Exclusions) {
        let mut carriers: Option<Vec<(Uuid, &str)>> = None;
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
            let carriers = carriers.get_or_insert_with(|| name_carrying_ports(self.plain_sheet));
            let ports: HashSet<Uuid> = carriers
                .iter()
                .filter(|(_, v)| *v == value)
                .map(|(id, _)| *id)
                .collect();
            let labels = self.labels_joining(value);
            let joined: Vec<usize> = (0..want.len())
                .filter(|&j| {
                    j != at
                        && (want[j].terminals.iter().any(|(s, _)| ports.contains(s))
                            || want[j].labels.iter().any(|l| labels.contains(l)))
                })
                .collect();
            for &j in &joined {
                let net = std::mem::replace(&mut want[j], empty_net());
                want[at].terminals.extend(net.terminals);
                want[at].labels.extend(net.labels);
                want[at].sheet_pins.extend(net.sheet_pins);
            }
            want[at].name = self.net_name(&want[at], carriers, value);
            want.retain(|n| {
                !(n.terminals.is_empty() && n.labels.is_empty() && n.sheet_pins.is_empty())
            });
        }
    }

    /// The name the netlist gives `net` (a join of two named nets): the best
    /// of its labels, with the values of the name-carrying ports on it; `None`
    /// with no label, where the port order decides and the intent cannot.
    pub(super) fn joined_name(&self, net: &NetSignature) -> Option<String> {
        let labels: Vec<_> = (self.plain_sheet.labels.iter())
            .filter(|l| net.labels.contains(&l.uuid))
            .collect();
        if labels.is_empty() {
            return None;
        }
        let carriers = name_carrying_ports(self.plain_sheet);
        let values: Vec<&str> = (carriers.iter())
            .filter(|(id, _)| net.terminals.iter().any(|(s, _)| s == id))
            .map(|(_, value)| *value)
            .collect();
        let named = labels.iter().map(|l| (l.label_type, l.text.as_str()));
        best_net_name_of(named, &values).map(|(_, name)| name)
    }

    /// The `Global` / `Power` labels of the plain sheet with text `value`: the
    /// labels a power port of that value joins (`merge_power_ports`).
    fn labels_joining(&self, value: &str) -> HashSet<Uuid> {
        self.plain_sheet
            .labels
            .iter()
            .filter(|l| {
                l.text == value && matches!(l.label_type, LabelType::Global | LabelType::Power)
            })
            .map(|l| l.uuid)
            .collect()
    }

    /// The name `net_signatures` gives `net` once the kept port carrying
    /// `value` is on it: the best of its labels and the values of the
    /// name-carrying ports on it, or with no label the first such value.
    fn net_name(&self, net: &NetSignature, carriers: &[(Uuid, &str)], value: &str) -> String {
        let labels: Vec<_> = self
            .plain_sheet
            .labels
            .iter()
            .filter(|l| net.labels.contains(&l.uuid))
            .collect();
        // Every port of a value on the net joins it, wired or not.
        let mut on_net: HashSet<&str> = HashSet::from([value]);
        on_net.extend(
            carriers
                .iter()
                .filter(|(id, _)| net.terminals.iter().any(|(s, _)| s == id))
                .map(|(_, v)| *v),
        );
        on_net.extend(
            labels
                .iter()
                .filter(|l| matches!(l.label_type, LabelType::Global | LabelType::Power))
                .map(|l| l.text.as_str()),
        );
        let mut values: Vec<&str> = carriers
            .iter()
            .map(|(_, v)| *v)
            .filter(|v| on_net.contains(v))
            .collect();
        if !values.contains(&value) {
            values.push(value);
        }
        if labels.is_empty() {
            return values[0].to_string();
        }
        best_net_name_of(
            labels.iter().map(|l| (l.label_type, l.text.as_str())),
            &values,
        )
        .map_or_else(String::new, |(_, name)| name)
    }

    /// Whether [`Self::reference`] is the pre-move nets: a kept power port
    /// and nothing but symbols moved.
    fn before_is_reference(&self, tips: &[PinTip], policy: &Policy) -> bool {
        let power_stays = tips
            .iter()
            .any(|t| t.is_power() && self.was_connected(t) && !policy.plain.contains(&t.old_key()));
        power_stays && self.only_symbols_moved()
    }

    /// Provisional decision (b): a warned pin ends on exactly the net the plain
    /// move gives it. Neither reference holds that alone: the pre-move nets
    /// leave warned pins out, and the intent puts a kept pin on its old net
    /// even when that is a warned pin's plain net (a kept power port then
    /// renames and merges it). Where a warned pin's net in `candidate`
    /// differs from its net in the plain move (the moved pins left out of
    /// both), the plain one goes to `want` and the candidate's to `got`, so
    /// the comparison fails and blames what joined it. The local comparison
    /// adds the same, so both decide alike.
    pub(super) fn restate_warned(
        &self,
        candidate: &SchematicSheet,
        tips: &[PinTip],
        policy: &Policy,
        want: &mut Projected,
        got: &mut Projected,
    ) {
        let warned = warned_idents(tips, policy, |tip| self.was_connected(tip));
        if warned.is_empty() {
            return;
        }
        let mut terminals: Terminals = BTreeSet::new();
        let mut sheet_pins: BTreeSet<Uuid> = BTreeSet::new();
        for tip in tips {
            match tip.ident() {
                PinIdent::Terminal(s, p) => {
                    terminals.insert((*s, p.clone()));
                }
                PinIdent::SheetPin(u) => {
                    sheet_pins.insert(*u);
                }
            }
        }
        let plain = &self.plain_view(tips).nets;
        let now = self.nets_of(candidate);
        for ident in warned {
            let on = |nets: &[NetSignature]| {
                (nets.iter())
                    .find(|net| holds(net, ident))
                    .and_then(|net| net.without(&terminals, &sheet_pins))
            };
            let (was, is) = (on(plain), on(&now));
            if was != is {
                want.extend(was);
                got.extend(is);
            }
        }
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
        if !self.before_is_reference(tips, policy) {
            return self.intent(tips, skip, policy);
        }
        let sigs = if skip.power_out.is_empty() {
            self.before.to_vec()
        } else {
            self.nets_of(&Self::strip(self.before_sheet, &skip.power_out))
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
        #[cfg(test)]
        {
            let (equal, blamed, local_used) = self.verdict(candidate, tips, policy, edited);
            tally::checked(local_used, equal, &blamed);
            blamed
        }
        #[cfg(not(test))]
        {
            self.verdict(candidate, tips, policy, edited).1
        }
    }

    /// Whether the candidate has the nets the move asks for, who is blamed
    /// when not, and whether the comparison ran on the nets the move reaches.
    fn verdict(
        &self,
        candidate: &SchematicSheet,
        tips: &[PinTip],
        policy: &Policy,
        edited: &[Key],
    ) -> (bool, Option<Vec<Key>>, bool) {
        if let Some(local) = &self.local {
            let skip = self.exclusions(tips, policy);
            if let Some((equal, blamed)) = local.check(self, candidate, tips, policy, edited, &skip)
            {
                #[cfg(test)]
                if self_check::enabled() {
                    let all = self.check_all_nets(candidate, tips, policy, edited);
                    assert_eq!(
                        (equal, blamed.clone()),
                        all,
                        "the local comparison differs from the comparison over every net"
                    );
                }
                return (equal, blamed, true);
            }
            #[cfg(test)]
            tally::fell_back();
        }
        let (equal, blamed) = self.check_all_nets(candidate, tips, policy, edited);
        (equal, blamed, false)
    }

    /// [`Self::check`] over every net of the candidate.
    fn check_all_nets(
        &self,
        candidate: &SchematicSheet,
        tips: &[PinTip],
        policy: &Policy,
        edited: &[Key],
    ) -> (bool, Option<Vec<Key>>) {
        let skip = self.exclusions(tips, policy);
        let (mut want, extra) = self.reference(tips, &skip, policy);
        // Pins with an undefined plain connection stay out of both sides;
        // skipped pins are compared against where the plain move left them.
        let sigs = if skip.power_out.is_empty() {
            self.nets_of(candidate)
        } else {
            self.nets_of(&Self::strip(candidate, &skip.power_out))
        };
        let mut got = Self::project(&sigs, &extra);
        self.restate_warned(candidate, tips, policy, &mut want, &mut got);
        if want == got {
            return (true, None);
        }
        let view = self.plain_view(tips);
        let at = wire_nets(&self.wires, &view.wire_at, &view.nets, &self.wire_pins);
        let blamed = Self::blame(tips, &at, policy, edited, &want, &got);
        (false, blamed)
    }

    /// The old-point keys to blame for the difference between `want` and
    /// `got`: the live moved pins it names, and the live carried points one of
    /// whose plain-move nets (`wire_nets`) shares a member with it other than a
    /// moved pin. When it names none of them, the live ones whose handling
    /// edited something (`edited`); `None` when there are none either.
    pub(super) fn blame(
        tips: &[PinTip],
        wire_nets: &[(Key, Vec<NetSignature>)],
        policy: &Policy,
        edited: &[Key],
        want: &Projected,
        got: &Projected,
    ) -> Option<Vec<Key>> {
        let mut terminals: BTreeSet<&(Uuid, String)> = BTreeSet::new();
        let mut pins: BTreeSet<&Uuid> = BTreeSet::new();
        let mut labels: BTreeSet<&Uuid> = BTreeSet::new();
        for net in want.symmetric_difference(got) {
            terminals.extend(net.terminals.iter());
            pins.extend(net.sheet_pins.iter());
            labels.extend(net.labels.iter());
        }
        let live = || tips.iter().filter(|t| !policy.plain.contains(&t.old_key()));
        let mut blamed: Vec<Key> = live()
            .filter(|tip| match tip.ident() {
                PinIdent::Terminal(symbol, pin) => terminals.contains(&(*symbol, pin.clone())),
                PinIdent::SheetPin(uuid) => pins.contains(uuid),
            })
            .map(PinTip::old_key)
            .collect();
        // A live moved pin in the difference is the pin's to answer for, not
        // the wire's; a plain one is where the plain move put it, so a
        // difference on its net is the carried points'.
        let moved: Vec<&PinIdent> = live().map(PinTip::ident).collect();
        let named = |net: &NetSignature| {
            let terminal = (net.terminals.iter()).any(|(s, p)| {
                terminals.contains(&(*s, p.clone())) && !holds_any_terminal(&moved, *s, p.as_str())
            });
            let pin = (net.sheet_pins.iter())
                .any(|u| pins.contains(u) && !moved.contains(&&PinIdent::SheetPin(*u)));
            terminal || pin || net.labels.iter().any(|l| labels.contains(l))
        };
        let live_wires = || (wire_nets.iter()).filter(|(k, _)| !policy.plain.contains(k));
        blamed.extend(
            live_wires()
                .filter(|(_, nets)| nets.iter().any(&named))
                .map(|(k, _)| *k),
        );
        if blamed.is_empty() {
            blamed = live()
                .map(PinTip::old_key)
                .chain(live_wires().map(|(k, _)| *k))
                .filter(|key| edited.contains(key))
                .collect();
            // With the plain move as the reference every difference is the
            // pass's: one nothing explains takes every live contact a step
            // down, so the rounds end at the plain move at worst.
            if blamed.is_empty() {
                blamed = live()
                    .map(PinTip::old_key)
                    .chain(live_wires().map(|(k, _)| *k))
                    .collect();
            }
            if blamed.is_empty() {
                return None;
            }
        }
        blamed.sort_unstable();
        blamed.dedup();
        Some(blamed)
    }
}

/// Whether `idents` holds the component pin `(symbol, pin)`.
fn holds_any_terminal(idents: &[&PinIdent], symbol: Uuid, pin: &str) -> bool {
    idents
        .iter()
        .any(|ident| matches!(ident, PinIdent::Terminal(s, p) if *s == symbol && p == pin))
}

/// Per carried point, its old key and the plain move's named nets at its old
/// place and where its wire landed: what a difference must touch to blame it.
pub(super) fn wire_nets(
    wires: &[WirePoint],
    at: &[(Found, Found)],
    nets: &[NetSignature],
    pins: &[Terminals],
) -> Vec<(Key, Vec<NetSignature>)> {
    (wires.iter().zip(at).enumerate())
        .map(|(i, (w, (old, new)))| {
            let named = [old.1, new.1].into_iter().flatten();
            let mut held: Vec<NetSignature> = named.filter_map(|n| nets.get(n).cloned()).collect();
            // The pins alone at the old place, on no net of the plain move.
            if let Some(pins) = pins.get(i).filter(|pins| !pins.is_empty()) {
                let mut alone = empty_net();
                alone.terminals.clone_from(pins);
                held.push(alone);
            }
            (w.old_key(), held)
        })
        .collect()
}

/// Where each moved pin sits among the plain-moved sheet's nets.
pub(super) struct Placement<'a> {
    /// Per tip: the net its pin is on in the plain-moved sheet.
    pub(super) home: &'a [Option<usize>],
    /// Per tip: nothing stays at the old tip to carry the connection.
    pub(super) carried: &'a [bool],
    /// Per tip: the net its old tip sits on (opaque id, index when named).
    pub(super) at_old: &'a [(Key, Option<usize>)],
}

/// Put every moved pin back on the net the user's move asks for, in `nets`
/// (the plain-moved nets with the moved pins taken out); returns the pins
/// whose plain-move connection is undefined and so stay out of the comparison
/// on both sides, the nets that exist only at an old tip, and the joins a
/// held pin asks for besides (spec gap (a), `intent`). The one place the
/// placement rules live: the comparison over every net and the one over the
/// nets a move reaches both call it.
pub(super) fn place_moved_pins(
    tips: &[PinTip],
    skip: &Exclusions,
    nets: &mut [NetSignature],
    at: &Placement,
    was_connected: impl Fn(&PinTip) -> bool,
) -> (Exclusions, BTreeMap<Key, NetSignature>, Vec<(Slot, Slot)>) {
    let mut joins = Vec::new();
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
        if skipped || !was_connected(tip) {
            // A pin that is not rubber-banded (plain, or never on a net)
            // is expected exactly where the plain move left it, so a
            // wire the pass stretched or added under it shows up.
            if let Some(n) = at.home[i].filter(|_| !tip.is_power()) {
                insert_ident(&mut nets[n], tip.ident());
            }
            continue;
        }
        let target = if at.carried[i] {
            // The wire that carried this pin moved with it: the plain
            // move defines the net the pin lands on.
            match at.home[i] {
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
            if let Some(home) = at.home[i].filter(|_| intent::KEPT_PIN_JOINS_LANDING_NET) {
                joins.push((Slot::at(at.at_old[i]), Slot::Net(home)));
            }
            match at.at_old[i] {
                (_, Some(n)) => Some(&mut nets[n]),
                (root, None) => Some(lone.entry(root).or_insert_with(empty_net)),
            }
        };
        if let Some(net) = target {
            insert_ident(net, tip.ident());
        }
    }
    (extra, lone, joins)
}

pub(super) fn empty_net() -> NetSignature {
    NetSignature {
        terminals: BTreeSet::new(),
        labels: BTreeSet::new(),
        sheet_pins: BTreeSet::new(),
        name: String::new(),
    }
}

/// Test switch for the check in [`derive_at`]; on unless a timing bench turns
/// it off for its own thread.
#[cfg(test)]
pub(crate) mod self_check {
    use std::cell::Cell;

    thread_local! {
        static ON: Cell<bool> = const { Cell::new(true) };
    }

    pub(crate) fn enabled() -> bool {
        ON.with(Cell::get)
    }

    /// Turn the check off for this thread until the guard drops.
    pub(crate) fn off() -> Off {
        Off(ON.with(|flag| flag.replace(false)))
    }

    /// Restores the setting [`off`] found.
    pub(crate) struct Off(bool);

    impl Drop for Off {
        fn drop(&mut self) {
            ON.with(|flag| flag.set(self.0));
        }
    }
}

/// What the tests count on this thread: the guard's checks (how many ran on
/// the nets the move reaches, how many over every net, how many fell back to
/// every net, how many blamed a tip, what each decided), the follows the
/// journal rewound and the reconciles that read fewer wires than the sheet.
#[cfg(test)]
pub(crate) mod tally {
    use std::cell::RefCell;

    use signex_net::uf::Key;

    #[derive(Debug, Default, Clone, PartialEq, Eq)]
    pub(crate) struct Tally {
        pub(crate) guards_built_local: usize,
        pub(crate) local_checks: usize,
        pub(crate) full_checks: usize,
        pub(crate) blames: usize,
        /// Checks of a guard built local that fell back to every net.
        pub(crate) fallbacks: usize,
        /// Failed follows whose journal put back at least one wire end.
        pub(crate) rewinds: usize,
        /// Reconciles that indexed only the wires near the touched ones.
        pub(crate) reconciles_narrowed: usize,
        /// Each check's answer, in order.
        pub(crate) decisions: Vec<(bool, Option<Vec<Key>>)>,
    }

    thread_local! {
        static TALLY: RefCell<Tally> = RefCell::new(Tally::default());
    }

    pub(crate) fn guard_built_local() {
        TALLY.with(|t| t.borrow_mut().guards_built_local += 1);
    }

    pub(super) fn checked(local: bool, equal: bool, blamed: &Option<Vec<Key>>) {
        TALLY.with(|t| {
            let mut t = t.borrow_mut();
            if local {
                t.local_checks += 1;
            } else {
                t.full_checks += 1;
            }
            t.blames += usize::from(blamed.is_some());
            t.decisions.push((equal, blamed.clone()));
        });
    }

    pub(crate) fn fell_back() {
        TALLY.with(|t| t.borrow_mut().fallbacks += 1);
    }

    pub(crate) fn rewound() {
        TALLY.with(|t| t.borrow_mut().rewinds += 1);
    }

    pub(crate) fn reconcile_narrowed() {
        TALLY.with(|t| t.borrow_mut().reconciles_narrowed += 1);
    }

    /// The counts so far on this thread, starting again from zero.
    pub(crate) fn take() -> Tally {
        TALLY.with(|t| std::mem::take(&mut *t.borrow_mut()))
    }
}
