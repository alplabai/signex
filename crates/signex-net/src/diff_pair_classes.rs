//! Differential pairs from the schematic's directives (#1282): a
//! Differential Pair directive (or a Net Parameters directive naming a
//! diff-pair class) on one half of a pair names the pair and, when set, its
//! diff-pair class. The partner is found by name: with the class's
//! `pair_by_suffix` (and with no class) by `_P`/`_N`, `DP`/`DN` or `+`/`-`;
//! with it off by `_P`/`_N` only. Design note:
//! `docs/SCHEMATIC_DIRECTIVES.md`, "Differential pairs".

use signex_types::diff_pair_class::DiffPairClass;
use signex_types::net::Netlist;
use signex_types::schematic::{SchDirective, SchematicSheet, diff_pair_halves};
use uuid::Uuid;

/// One Differential Pair directive resolved against the netlist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffPair {
    /// The directive that marks the pair.
    pub directive: Uuid,
    /// The pair's name: the shared base of its nets (`USB_D`).
    pub name: String,
    /// The positive net (`USB_D_P`, `USB_DP`, `CLK+`), when the sheet has one.
    pub positive: Option<String>,
    /// The negative net, when the sheet has one.
    pub negative: Option<String>,
    /// The diff-pair class the directive names, trimmed.
    pub class: Option<String>,
}

impl DiffPair {
    /// Whether both halves were found.
    pub fn is_complete(&self) -> bool {
        self.positive.is_some() && self.negative.is_some()
    }
}

/// Whether a pair of class `class` is matched by every suffix style: with
/// no class, an unknown class, or a class that pairs by suffix.
pub fn pairs_by_every_suffix(class: Option<&str>, classes: &[DiffPairClass]) -> bool {
    class
        .and_then(|name| classes.iter().find(|c| c.name == name))
        .is_none_or(|c| c.pair_by_suffix)
}

/// Every Differential Pair directive of `sheet`, and every Net Parameters
/// directive naming a diff-pair class, that sits on a wire whose net (in
/// `netlist`, built from `sheet`) carries a pair suffix, with both halves
/// looked up by name. A directive on no wire, or on a net without a suffix,
/// pairs nothing and is left out.
pub fn diff_pairs_with_classes(
    sheet: &SchematicSheet,
    netlist: &Netlist,
    classes: &[DiffPairClass],
) -> Vec<DiffPair> {
    let mut out = Vec::new();
    for directive in &sheet.directives {
        let position = match directive {
            SchDirective::DiffPair { position, .. } => position,
            SchDirective::NetParameters { position, .. }
                if directive.diff_pair_class().is_some() =>
            {
                position
            }
            _ => continue,
        };
        let Some(net) = crate::directives::net_at(sheet, netlist, position) else {
            continue;
        };
        let class = directive.diff_pair_class();
        let every = pairs_by_every_suffix(class, classes);
        let Some(halves) = diff_pair_halves(&net.name, every) else {
            continue;
        };
        let named = |want: String| netlist.nets.iter().any(|n| n.name == want).then_some(want);
        out.push(DiffPair {
            directive: directive.uuid(),
            name: halves.name,
            positive: named(halves.positive),
            negative: named(halves.negative),
            class: class.map(str::to_string),
        });
    }
    out
}

/// One pair of the project as the Differential Pair Classes dialog lists
/// it: the pair and the sheet its directive is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassMember {
    pub pair: DiffPair,
    /// The file name of the sheet the directive sits on.
    pub sheet: String,
}

/// Every classed pair of `sheets` (file name, sheet), sheet by sheet in the
/// order given. Each sheet's netlist is built here.
pub fn class_members(
    sheets: &[(String, &SchematicSheet)],
    classes: &[DiffPairClass],
) -> Vec<ClassMember> {
    let mut out = Vec::new();
    for (name, sheet) in sheets {
        if !sheet
            .directives
            .iter()
            .any(|d| d.diff_pair_class().is_some())
        {
            continue;
        }
        let netlist = crate::build_netlist(sheet);
        for pair in diff_pairs_with_classes(sheet, &netlist, classes) {
            if pair.class.is_some() {
                out.push(ClassMember {
                    pair,
                    sheet: name.clone(),
                });
            }
        }
    }
    out
}

/// How many of `members` belong to class `name`.
pub fn pair_count(members: &[ClassMember], name: &str) -> usize {
    members
        .iter()
        .filter(|m| m.pair.class.as_deref() == Some(name))
        .count()
}
