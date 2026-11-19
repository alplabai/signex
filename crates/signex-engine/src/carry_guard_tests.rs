//! The guarded keep-wires pass held to its invariants (`exec::guarded`): the
//! written rule worked out apart from the guard, the all-plain identity, and
//! the property cases that once failed, pinned.

use std::collections::{BTreeMap, BTreeSet};

use signex_net::{NetSignature, net_signatures, net_signatures_at, point_is_occupied, pt_key};
use signex_types::schematic::SchematicSheet;
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
/// pin or both carry different names (decisions 2026-11-19). Power ports name
/// nets across the sheet, which this check does not restate: it passes them.
pub(crate) fn check_allowed(
    before: &SchematicSheet,
    plain: &SchematicSheet,
    after: &SchematicSheet,
    tips: &[PinTip],
    record: &Record,
) -> Result<(), String> {
    if tips.iter().any(PinTip::is_power) {
        return Ok(());
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
    let mut parent: Vec<usize> = (0..bags.len()).collect();
    for (j, (old, _)) in record.wires.iter().enumerate() {
        if record.plain.contains(&pt_key(old)) {
            continue;
        }
        let at = tips.len() + 2 * j;
        let (a, b) = (slot(&mut bags, found[at]), slot(&mut bags, found[at + 1]));
        while parent.len() < bags.len() {
            parent.push(parent.len());
        }
        let (a, b) = (root(&parent, a), root(&parent, b));
        if a == b {
            continue;
        }
        let holds_warned = |net: &NetSignature| warned.iter().any(|ident| on(net, ident));
        if holds_warned(&bags[a]) || holds_warned(&bags[b]) {
            continue;
        }
        let (x, y) = (bags[a].name.clone(), bags[b].name.clone());
        if !x.is_empty() && !y.is_empty() && x != y {
            continue;
        }
        let gone = std::mem::replace(&mut bags[b], empty());
        bags[a].terminals.extend(gone.terminals);
        bags[a].labels.extend(gone.labels);
        bags[a].sheet_pins.extend(gone.sheet_pins);
        if bags[a].name.is_empty() {
            bags[a].name = gone.name;
        }
        parent[b] = a;
    }
    let project = |net: &NetSignature| {
        let mut net = net.clone();
        for ident in &excluded {
            take_out(&mut net, ident);
        }
        let identified =
            !(net.terminals.is_empty() && net.labels.is_empty() && net.sheet_pins.is_empty());
        identified.then_some(net)
    };
    let want: BTreeSet<NetSignature> = bags.iter().filter_map(&project).collect();
    let got: BTreeSet<NetSignature> = net_signatures(after).iter().filter_map(&project).collect();
    if want == got {
        return Ok(());
    }
    let missing: Vec<_> = want.difference(&got).collect();
    let extra: Vec<_> = got.difference(&want).collect();
    Err(format!("NOT ALLOWED: the rule asks for {missing:?}, the move made {extra:?}"))
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
fn shape(sheet: &SchematicSheet) -> (BTreeSet<(Uuid, String)>, BTreeSet<(String, bool)>) {
    let wires = (sheet.wires.iter())
        .map(|w| (w.uuid, format!("{:?}-{:?}", pt_key(&w.start), pt_key(&w.end))))
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
