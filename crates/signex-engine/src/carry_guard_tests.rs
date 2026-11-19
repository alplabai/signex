//! The guarded keep-wires pass held to its invariants (`exec::guarded`): the
//! written rule worked out apart from the guard, the all-plain identity, and
//! the property cases that once failed, pinned.

use std::collections::{BTreeMap, BTreeSet};

use signex_net::{NetSignature, net_signatures, net_signatures_at, point_is_occupied, pt_key};
use signex_types::schematic::{SchematicSheet, SelectedItem, SelectedKind};
use uuid::Uuid;

use crate::exec::guarded::{Record, all_plain, take_record};
use crate::rubber_band_tests::{Lcg, RunCounts, case_at, property_case, random_case};
use crate::{Command, Engine, PinIdent, PinTip};

fn on(net: &NetSignature, ident: &PinIdent) -> bool {
    match ident {
        PinIdent::Terminal(s, p) => net.terminals.contains(&(*s, p.clone())),
        PinIdent::SheetPin(u) => net.sheet_pins.contains(u),
    }
}

fn take_out(net: &mut NetSignature, ident: &PinIdent) {
    match ident {
        PinIdent::Terminal(s, p) => {
            net.terminals.remove(&(*s, p.clone()));
        }
        PinIdent::SheetPin(u) => {
            net.sheet_pins.remove(u);
        }
    }
}

fn put_in(net: &mut NetSignature, ident: &PinIdent) {
    match ident {
        PinIdent::Terminal(s, p) => {
            net.terminals.insert((*s, p.clone()));
        }
        PinIdent::SheetPin(u) => {
            net.sheet_pins.insert(*u);
        }
    }
}

fn empty() -> NetSignature {
    NetSignature {
        terminals: BTreeSet::new(),
        labels: BTreeSet::new(),
        sheet_pins: BTreeSet::new(),
        name: String::new(),
    }
}

fn root(parent: &[usize], mut at: usize) -> usize {
    while parent[at] != at {
        at = parent[at];
    }
    at
}

/// The nets a guarded move may commit, by the written rule and nothing of
/// the guard's code: the plain move's nets; a moved pin that was on a net and
/// is not warned goes on the net at its old tip when something stays there,
/// else where the plain move put it; a warned pin, or one on no net before,
/// stays where the plain move put it; a kept carried point joins the net at
/// its old place to the net its wire landed on, unless either holds a warned
/// pin, named as the netlist names the join (decisions 2026-11-19). Power
/// ports name nets across the sheet, which this check does not restate: it
/// passes a move of one, returning `false` (counted by the property run).
pub(crate) fn check_allowed(
    before: &SchematicSheet,
    plain: &SchematicSheet,
    after: &SchematicSheet,
    tips: &[PinTip],
    record: &Record,
) -> Result<bool, String> {
    if tips.iter().any(PinTip::is_power) {
        return Ok(false);
    }
    // A pin riding on a carried wire is no contact of its own: it is where
    // the plain move put it, like any member of the plain nets.
    let tips: Vec<&PinTip> = (tips.iter())
        .filter(|tip| record.tips.contains(tip.ident()))
        .collect();
    let was = net_signatures(before);
    let connected = |tip: &PinTip| {
        was.iter().any(|net| on(net, tip.ident()))
            && (!tip.is_sheet_pin() || point_is_occupied(before, tip.old_key()))
    };
    let mut keys: Vec<(signex_net::uf::Key, bool)> = tips
        .iter()
        .map(|tip| (tip.old_key(), tip.is_sheet_pin()))
        .collect();
    for (old, new) in &record.wires {
        keys.push((pt_key(old), false));
        keys.push((pt_key(new), true));
    }
    let (nets, found) = net_signatures_at(plain, &keys);
    let mut bags = nets.clone();
    for bag in &mut bags {
        for tip in &tips {
            take_out(bag, tip.ident());
        }
    }
    let mut lone: BTreeMap<signex_net::uf::Key, usize> = BTreeMap::new();
    // A net known only by its id at a queried point gets a bag of its own.
    let mut slot = |bags: &mut Vec<NetSignature>, at: (signex_net::uf::Key, Option<usize>)| {
        at.1.unwrap_or_else(|| {
            *lone.entry(at.0).or_insert_with(|| {
                bags.push(empty());
                bags.len() - 1
            })
        })
    };
    let mut excluded: Vec<&PinIdent> = Vec::new();
    let mut warned: Vec<&PinIdent> = Vec::new();
    for (i, &tip) in tips.iter().enumerate() {
        let home = nets.iter().position(|net| on(net, tip.ident()));
        let is_plain = record.plain.contains(&tip.old_key());
        let stays = point_is_occupied(plain, tip.old_key());
        if is_plain && connected(tip) {
            warned.push(tip.ident());
        }
        let target = if is_plain || !connected(tip) || !stays {
            home
        } else {
            Some(slot(&mut bags, found[i]))
        };
        match target {
            Some(n) => put_in(&mut bags[n], tip.ident()),
            None if connected(tip) && !is_plain => excluded.push(tip.ident()),
            None => {}
        }
    }
    // The component pins of the plain sheet that are no contact: power ports
    // name nets and are left out.
    let symbols: Vec<SelectedItem> = (plain.symbols.iter())
        .filter(|symbol| !symbol.is_power)
        .map(|symbol| SelectedItem::new(symbol.uuid, SelectedKind::Symbol))
        .collect();
    let stationary: Vec<(signex_net::uf::Key, (Uuid, String))> =
        (crate::moved_pin_tips(plain, &symbols).into_iter())
            .filter(|pin| !tips.iter().any(|tip| tip.ident() == pin.ident()))
            .filter_map(|pin| match pin.ident() {
                PinIdent::Terminal(s, p) => Some((pin.old_key(), (*s, p.clone()))),
                PinIdent::SheetPin(_) => None,
            })
            .collect();
    let mut parent: Vec<usize> = (0..bags.len()).collect();
    for (j, (old, _)) in record.wires.iter().enumerate() {
        let key = pt_key(old);
        if record.plain.contains(&key) || record.bailed.contains(&key) {
            continue;
        }
        let at = tips.len() + 2 * j;
        let (a, b) = (slot(&mut bags, found[at]), slot(&mut bags, found[at + 1]));
        while parent.len() < bags.len() {
            parent.push(parent.len());
        }
        let (a, b) = (root(&parent, a), root(&parent, b));
        // A pin alone at the old place is on no net, yet the point joined it.
        if found[at].1.is_none() {
            let alone = (stationary.iter()).filter(|(k, _)| *k == pt_key(old));
            bags[a].terminals.extend(alone.map(|(_, pin)| pin.clone()));
        }
        if a == b {
            continue;
        }
        let holds_warned = |net: &NetSignature| warned.iter().any(|ident| on(net, ident));
        if holds_warned(&bags[a]) || holds_warned(&bags[b]) {
            continue;
        }
        let (x, y) = (bags[a].name.clone(), bags[b].name.clone());
        let gone = std::mem::replace(&mut bags[b], empty());
        let mut joined = bags[a].clone();
        joined.terminals.extend(gone.terminals.iter().cloned());
        joined.labels.extend(gone.labels.iter().copied());
        joined.sheet_pins.extend(gone.sheet_pins.iter().copied());
        joined.name = if x.is_empty() || x == y {
            y
        } else if y.is_empty() {
            x
        } else {
            match netlist_name(plain, &joined) {
                Some(name) => name,
                None => {
                    bags[b] = gone;
                    continue;
                }
            }
        };
        bags[a] = joined;
        parent[b] = a;
    }
    let project = |net: &NetSignature| {
        let mut net = net.clone();
        for ident in &excluded {
            take_out(&mut net, ident);
        }
        let identified =
            !(net.terminals.is_empty() && net.labels.is_empty() && net.sheet_pins.is_empty());
        // A lone pin with nothing at its tip is on no net of a netlist.
        let lone_pin = net.terminals.len() == 1
            && net.labels.is_empty()
            && net.sheet_pins.is_empty()
            && net.name.is_empty();
        (identified && !lone_pin).then_some(net)
    };
    let want: BTreeSet<NetSignature> = bags.iter().filter_map(&project).collect();
    let got: BTreeSet<NetSignature> = net_signatures(after).iter().filter_map(&project).collect();
    if want == got {
        return Ok(true);
    }
    let missing: Vec<_> = want.difference(&got).collect();
    let extra: Vec<_> = got.difference(&want).collect();
    Err(format!(
        "NOT ALLOWED: the rule asks for {missing:?}, the move made {extra:?}"
    ))
}

/// The cases the property run once collected as failures (silent splits, a
/// part short, warned pins off their plain net), held to every check of
/// `property_case` for good.
#[test]
fn the_collected_property_cases_hold() {
    const CASES: [(u64, usize); 15] = [
        (0x5EED_0963, 1083),
        (0xC0FFEE, 17),
        (0xC0FFEE, 259),
        (0xC0FFEE, 780),
        (0xC0FFEE, 1014),
        (0xC0FFEE, 1568),
        (0xC0FFEE, 1803),
        (0xBADC_AB1E, 262),
        (0xBADC_AB1E, 464),
        (0xBADC_AB1E, 1133),
        (0xBADC_AB1E, 1234),
        (0xBADC_AB1E, 1427),
        (0x1234_5678, 474),
        (0x1234_5678, 927),
        (0x1234_5678, 1906),
    ];
    let mut counts = RunCounts::default();
    for (seed, index) in CASES {
        let case = case_at(seed, index).expect("the generator still draws this case");
        property_case(seed, index, case, &mut counts);
    }
}

/// What a sheet holds that a move can change, ids of fresh dots aside.
/// Wires (id, geometry) and dots (place, minted) of a sheet.
type Shape = (BTreeSet<(Uuid, String)>, BTreeSet<(String, bool)>);

fn shape(sheet: &SchematicSheet) -> Shape {
    let wires = (sheet.wires.iter())
        .map(|w| {
            (
                w.uuid,
                format!("{:?}-{:?}", pt_key(&w.start), pt_key(&w.end)),
            )
        })
        .collect();
    let dots = (sheet.junctions.iter())
        .map(|j| (format!("{:?}", pt_key(&j.position)), j.minted))
        .collect();
    (wires, dots)
}

/// Invariant I1: with every contact plain from the first round, the guarded
/// pass commits exactly the plain move, on every property case.
#[test]
fn every_contact_plain_commits_the_plain_move() {
    let mut checked = 0;
    for seed in [0x5EED_0963, 0xC0FFEE, 0xBADC_AB1E, 0x1234_5678] {
        let mut rng = Lcg(seed);
        for index in 0..500 {
            let Some(case) = random_case(&mut rng) else {
                continue;
            };
            let run = |keep_wires: bool| {
                let mut engine = Engine::new(case.sheet.clone()).expect("engine");
                let command = Command::MoveEach {
                    moves: case.moves.clone(),
                    keep_wires,
                };
                engine.execute(command).expect("move");
                engine
            };
            let plain = run(false);
            let kept = {
                let _plain = all_plain();
                run(true)
            };
            let _ = take_record();
            assert_eq!(
                shape(kept.document()),
                shape(plain.document()),
                "seed {seed:#x} case {index}: all-plain differs from the plain move"
            );
            checked += 1;
        }
    }
    assert!(checked > 1000, "only {checked} cases");
}

/// The name the netlist gives a join of two named nets: the best of its
/// labels with the values of the name-carrying ports on it; `None` with no
/// label.
fn netlist_name(sheet: &SchematicSheet, net: &NetSignature) -> Option<String> {
    let labels: Vec<_> = (sheet.labels.iter())
        .filter(|l| net.labels.contains(&l.uuid))
        .collect();
    if labels.is_empty() {
        return None;
    }
    let carriers = signex_net::name_carrying_ports(sheet);
    let values: Vec<&str> = (carriers.iter())
        .filter(|(id, _)| net.terminals.iter().any(|(s, _)| s == id))
        .map(|(_, value)| *value)
        .collect();
    let named = labels.iter().map(|l| (l.label_type, l.text.as_str()));
    signex_net::best_net_name_of(named, &values).map(|(_, name)| name)
}

/// A wire the pass lays over a pin end must not leave it touching bare (no
/// dot, so no connection, #107) where the plain move leaves it free: the
/// round that would is taken down the ladder. A probe of the rule the
/// property run holds on every case.
#[test]
fn a_dragged_segment_never_leaves_a_bare_touch_on_a_pin_end() {
    use crate::rubber_band_tests::{add_wire, lib_pin, lib_symbol, place};
    use signex_types::schematic::Point;
    let mut sheet = crate::test_support::test_sheet();
    sheet
        .lib_symbols
        .insert("TP".to_string(), lib_symbol("TP", vec![lib_pin("1", 0.0)]));
    // A free pin end just below the joint the stretched neighbour passes.
    place(&mut sheet, "TP1", "TP", Point::new(40.0, 45.0));
    let segment = add_wire(&mut sheet, (40.0, 40.0), (60.0, 40.0));
    add_wire(&mut sheet, (40.0, 40.0), (40.0, 20.0));
    let run = |keep_wires: bool| {
        let mut engine = Engine::new(sheet.clone()).expect("engine");
        let item = SelectedItem::new(segment, SelectedKind::Wire);
        let command = Command::MoveEach {
            moves: vec![(item, 0.0, 10.0)],
            keep_wires,
        };
        engine.execute(command).expect("move");
        engine
    };
    let plain = crate::transform::bare_tap_keys(run(false).document());
    let kept = crate::transform::bare_tap_keys(run(true).document());
    assert!(
        kept.is_subset(&plain),
        "new bare touches: {kept:?} over {plain:?}"
    );
}
