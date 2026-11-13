//! What a run says about its written rules before it checks (#2307): a
//! rule whose scope matches no net, and a written clearance that lowers
//! what a net class's nets would otherwise need. A looser rule is applied
//! and said, never silently (decision 6837552967 §1).

use std::collections::BTreeSet;

use signex_types::net_class::{NetClass, first_matching_class};
use signex_types::pcb::PcbBoard;

use pcb_geom::{layer_mask, nm_to_mm};

use crate::DrcRules;
use crate::user_rules::{self, CopperObject, LimitKind, NetScope, ScopedRule};

/// `(warnings, infos)` for `rules` whose net, class or pattern scope covers
/// no net: a class that does not exist is a warning (likely a typo); a net
/// or pattern a board lacks, or a class without nets here, is info (a
/// shared profile may name nets a board does not have).
pub(crate) fn empty_scopes(
    rules: &[ScopedRule],
    classes: &[NetClass],
) -> (Vec<String>, Vec<String>) {
    let (mut warnings, mut infos) = (Vec::new(), Vec::new());
    for scoped in rules.iter().filter(|s| s.covers_no_net()) {
        let name = &scoped.rule.name;
        let nothing = "the rule checks nothing";
        match &scoped.rule.nets {
            NetScope::Class(class) if !classes.iter().any(|c| &c.name == class) => warnings.push(
                format!("DRC rule '{name}': there is no net class '{class}'; {nothing}"),
            ),
            NetScope::Class(class) => infos.push(format!(
                "DRC rule '{name}': net class '{class}' has no net on this board; {nothing}"
            )),
            NetScope::Net(net) => infos.push(format!(
                "DRC rule '{name}': no net of this board is '{net}'; {nothing}"
            )),
            NetScope::Pattern(glob) => infos.push(format!(
                "DRC rule '{name}': no net of this board matches '{glob}'; {nothing}"
            )),
            NetScope::Board => {}
        }
    }
    (warnings, infos)
}

/// One line per written rule and net class where the rule, winning for a
/// net of the class on one of the board's copper layers or for an object
/// pair the rules name, asks less than the net would need without it (the
/// larger of the setup and class values).
pub(crate) fn lowered_classes(
    rules: &DrcRules,
    board: &PcbBoard,
    classes: &[NetClass],
) -> Vec<String> {
    if !rules.sets_written_clearance() {
        return Vec::new();
    }
    let mut pairs: Vec<[Option<CopperObject>; 2]> = vec![[None, None]];
    pairs.extend(
        rules
            .user
            .iter()
            .filter_map(|s| s.rule.pair.map(|[a, b]| [Some(a), Some(b)])),
    );
    pairs.sort_unstable();
    pairs.dedup();
    let bits = layer_kinds(&rules.user, board_copper(board));
    let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
    let mut lines = Vec::new();
    for net in &board.nets {
        if !rules.net_clearance.contains_key(&net.number) {
            continue;
        }
        let Some(class) = first_matching_class(classes, &net.name) else {
            continue;
        };
        let without = rules.required(net.number, net.number);
        for &bit in &bits {
            for &objects in &pairs {
                let picked = user_rules::pick(
                    &rules.user,
                    LimitKind::MinClearance,
                    net.number,
                    1 << bit,
                    objects,
                );
                let Some((nm, scoped)) = picked.filter(|(nm, _)| *nm < without) else {
                    continue;
                };
                let name = &scoped.rule.name;
                if seen.insert((name.clone(), class.name.clone())) {
                    lines.push(format!(
                        "DRC rule '{name}' lowers net class '{}' clearance {} \u{2192} {} mm",
                        class.name,
                        mm(without),
                        mm(nm)
                    ));
                }
            }
        }
    }
    lines
}

/// `nm` in mm without float noise or trailing zeros (`0.55`, `1`).
fn mm(nm: i64) -> String {
    let text = format!("{:.6}", nm_to_mm(nm));
    match text.trim_end_matches('0').trim_end_matches('.') {
        "" | "-" => "0".to_string(),
        trimmed => trimmed.to_string(),
    }
}

/// The board's copper layers as a bit mask; a board that lists none is
/// read as two layers, top and bottom.
fn board_copper(board: &PcbBoard) -> u32 {
    match layer_mask(board.layers.iter().map(|l| l.name.as_str())) {
        0 => 1 | 1 << 31,
        mask => mask,
    }
}

/// One layer bit of `copper` per distinct set of rules whose layer
/// condition covers it: one bit in all when no rule has a layer condition.
fn layer_kinds(rules: &[ScopedRule], copper: u32) -> Vec<u32> {
    let mut seen: BTreeSet<Vec<bool>> = BTreeSet::new();
    (0..32)
        .filter(|bit| copper & (1 << bit) != 0)
        .filter(|&bit| {
            let covered = rules
                .iter()
                .map(|s| s.rule.layers.is_none_or(|mask| mask & (1 << bit) != 0))
                .collect();
            seen.insert(covered)
        })
        .collect()
}
