//! Differential pairs read off the schematic's net names: the halves whose
//! partner net is missing (#2951, the ERC rule `diff_pair_partner_missing`)
//! and the list of every pair a project defines (#2952). A pair is defined by
//! a Differential Pair directive (or a Net Parameters directive naming a
//! diff-pair class, `diff_pair_classes.rs`), or by name alone: two nets
//! `X_P` / `X_N`, `XDP` / `XDN` or `X+` / `X-` on one sheet. Design note:
//! `docs/SCHEMATIC_DIRECTIVES.md`, "Differential pairs".

use std::collections::HashSet;

use signex_types::diff_pair_class::DiffPairClass;
use signex_types::net::Netlist;
use signex_types::schematic::{
    DIFF_PAIR_SUFFIX_PAIRS, DiffPairHalves, Point, SchDirective, SchematicSheet, diff_pair_halves,
};
use uuid::Uuid;

/// One half of a differential pair whose partner net is not on the sheet.
#[derive(Debug, Clone, PartialEq)]
pub struct LonePairMember {
    /// The half that is on the sheet.
    pub net: String,
    /// The partner's name, which no net of the sheet has.
    pub partner: String,
    /// The pair's name: the shared base of its nets (`USB_D`).
    pub pair: String,
    /// The directive that marks the pair, and where it sits; `None` when
    /// the net's name alone makes it a member.
    pub directive: Option<(Uuid, Point)>,
    /// The first label that names the net, and where it is.
    pub label: Option<(Uuid, Point)>,
}

/// The pair `name` is a positive half of, by name: `USB_D_P`, `USB_DP` or
/// `CLK+`. A negative half alone (`RESET_N`, `PWRDN`) names an active-low
/// signal far more often than a pair, so it is a member only when a
/// directive says so.
fn positive_half(name: &str) -> Option<DiffPairHalves> {
    diff_pair_halves(name, true).filter(|halves| halves.positive == name)
}

/// Whether anything on `sheet` can make a pair: a pair directive, or a label
/// or power port whose text carries a positive suffix. The netlist is built
/// only when one does.
fn may_have_pairs(sheet: &SchematicSheet) -> bool {
    let positive = |text: &str| {
        DIFF_PAIR_SUFFIX_PAIRS
            .iter()
            .any(|(pos, _, _)| text.len() > pos.len() && text.ends_with(pos))
    };
    sheet
        .directives
        .iter()
        .any(|d| matches!(d, SchDirective::DiffPair { .. }) || d.diff_pair_class().is_some())
        || sheet.labels.iter().any(|l| positive(&l.text))
}

/// The first label of `sheet` whose text is `net`.
fn naming_label(sheet: &SchematicSheet, net: &str) -> Option<(Uuid, Point)> {
    sheet
        .labels
        .iter()
        .find(|l| l.text == net)
        .map(|l| (l.uuid, l.position))
}

/// Every half of a differential pair on `sheet` whose partner net is not on
/// it: the present half of an incomplete directive pair (either half), and
/// every net named as a positive half (`_P`, `DP`, `+`) with no negative
/// partner. A net named by a power port or a Power label is never a member
/// by name. Each net is reported once, the directive's finding first.
pub fn lone_pair_members(sheet: &SchematicSheet) -> Vec<LonePairMember> {
    if !may_have_pairs(sheet) {
        return Vec::new();
    }
    let netlist = crate::build_netlist(sheet);
    let mut out: Vec<LonePairMember> = Vec::new();
    for pair in crate::diff_pairs(sheet, &netlist) {
        let present = match (pair.positive, pair.negative) {
            (Some(net), None) | (None, Some(net)) => net,
            _ => continue,
        };
        let Some(halves) = diff_pair_halves(&present, true) else {
            continue;
        };
        if out.iter().any(|m| m.net == present) {
            continue;
        }
        let partner = if halves.positive == present {
            halves.negative
        } else {
            halves.positive
        };
        let directive = sheet
            .directives
            .iter()
            .find(|d| d.uuid() == pair.directive)
            .map(|d| (d.uuid(), d.anchor()));
        let label = naming_label(sheet, &present);
        out.push(LonePairMember {
            net: present,
            partner,
            pair: pair.name,
            directive,
            label,
        });
    }
    let names: HashSet<&str> = netlist.nets.iter().map(|n| n.name.as_str()).collect();
    let powers = crate::directives::power_names(sheet);
    for net in &netlist.nets {
        let Some(halves) = positive_half(&net.name) else {
            continue;
        };
        let skipped = out.iter().any(|m| m.net == net.name)
            || names.contains(halves.negative.as_str())
            || powers.contains(net.name.as_str());
        if skipped {
            continue;
        }
        out.push(LonePairMember {
            net: net.name.clone(),
            partner: halves.negative,
            pair: halves.name,
            directive: None,
            label: naming_label(sheet, &net.name),
        });
    }
    out
}

/// How a listed pair was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairFoundBy {
    /// A Differential Pair directive, or a Net Parameters directive naming a
    /// diff-pair class.
    Directive(Uuid),
    /// Two nets named as the halves of one pair, with no directive.
    Suffix,
}

/// One differential pair of a project, as the pair list shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedPair {
    /// The pair's name: the shared base of its nets (`USB_D`).
    pub name: String,
    pub positive: Option<String>,
    pub negative: Option<String>,
    /// The diff-pair class a directive names; `None` for a pair found by
    /// name or a directive naming none.
    pub class: Option<String>,
    pub found_by: PairFoundBy,
    /// The file name of the sheet it was found on.
    pub sheet: String,
}

/// Every differential pair of `sheets` (file name, sheet), sheet by sheet in
/// the order given: each sheet's directive pairs in sheet order (an
/// incomplete one included, with its missing half `None`), then its pairs
/// found by name, by positive net. A pair found by name is listed once in the
/// project, on the first sheet that has both nets, and not at all when a
/// directive lists the same two nets.
pub fn listed_pairs(
    sheets: &[(String, &SchematicSheet)],
    classes: &[DiffPairClass],
) -> Vec<ListedPair> {
    let mut directed: Vec<ListedPair> = Vec::new();
    let mut by_name: Vec<ListedPair> = Vec::new();
    for (file, sheet) in sheets {
        if !may_have_pairs(sheet) {
            continue;
        }
        let netlist = crate::build_netlist(sheet);
        for pair in crate::diff_pairs_with_classes(sheet, &netlist, classes) {
            directed.push(ListedPair {
                name: pair.name,
                positive: pair.positive,
                negative: pair.negative,
                class: pair.class,
                found_by: PairFoundBy::Directive(pair.directive),
                sheet: file.clone(),
            });
        }
        by_name.extend(named_pairs(&netlist, file));
    }
    let same_nets =
        |a: &ListedPair, b: &ListedPair| a.positive == b.positive && a.negative == b.negative;
    let mut out = directed;
    for pair in by_name {
        if !out.iter().any(|seen| same_nets(seen, &pair)) {
            out.push(pair);
        }
    }
    // By sheet; the sort is stable, so on each sheet the directive pairs
    // stay ahead of the pairs found by name.
    out.sort_by_key(|p| sheets.iter().position(|(file, _)| *file == p.sheet));
    out
}

/// The pairs of `netlist` found by name: a positive net whose negative
/// partner is on the sheet too, by positive net name.
fn named_pairs(netlist: &Netlist, file: &str) -> Vec<ListedPair> {
    let names: HashSet<&str> = netlist.nets.iter().map(|n| n.name.as_str()).collect();
    let mut out: Vec<ListedPair> = netlist
        .nets
        .iter()
        .filter_map(|net| positive_half(&net.name))
        .filter(|halves| names.contains(halves.negative.as_str()))
        .map(|halves| ListedPair {
            name: halves.name,
            positive: Some(halves.positive),
            negative: Some(halves.negative),
            class: None,
            found_by: PairFoundBy::Suffix,
            sheet: file.to_string(),
        })
        .collect();
    out.sort_by(|a, b| a.positive.cmp(&b.positive));
    out.dedup();
    out
}

#[cfg(test)]
#[path = "diff_pair_partners_tests.rs"]
mod tests;
