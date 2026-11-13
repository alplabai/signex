//! DRC findings in the shared check shape (`signex_types::check`, #2199,
//! decisions 6817821301 and 6818011610): the rule id (`builtin::<kind>` or a
//! written rule's `user::<name>`), its severity from the resolved rule
//! model, DRC's own facts in `evidence`, and a key that names the same
//! finding in every run. See `docs/CLI.md`, "Checks on demand".

use std::collections::BTreeMap;

use pcb_geom::{ItemRef, NM_PER_MM, Rejected, copper_layers};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use signex_types::check::{CheckReport, Finding};
use signex_types::format::pcb_layer_token;
use signex_types::pcb_layer::PcbLayer;

use crate::{DrcReport, DrcRules, Violation, ViolationKind};

/// The id a report names its check by.
pub const DRC_CHECK: &str = "drc";

/// Hex digits of the key: 128 bits of the hash, as ERC's.
const KEY_HEX_DIGITS: usize = 32;

/// `report`, a run over the board file `board`, as a check report.
/// `net_names` names the board's nets by number. The warnings are
/// `warnings`, then what the rule text and the setup said, then one per item
/// the run could not check. `rule_set` gets the clearance the run used.
/// DRC has no waivers, so `waived` and `lapsed` are empty; a finding of a
/// rule switched off is not listed.
pub fn check_report(
    report: &DrcReport,
    net_names: &BTreeMap<u32, String>,
    board: &str,
    warnings: Vec<String>,
    rule_set: Map<String, Value>,
) -> CheckReport {
    let findings = report
        .violations
        .iter()
        .filter_map(|v| finding(report, v, net_names, board))
        .collect();
    let warnings = warnings
        .into_iter()
        .chain(rule_warnings(&report.rules))
        .chain(report.skipped.iter().map(skipped_warning))
        .collect();
    let mut rule_set = rule_set;
    rule_set.insert("clearance_nm".into(), json!(report.rules.clearance));
    rule_set.insert("from_default".into(), json!(report.rules.from_default));
    CheckReport::new(DRC_CHECK, findings, Vec::new(), Vec::new(), warnings)
        .with_rule_set(Value::Object(rule_set))
}

/// `v` as a finding; `None` when its rule is switched off.
fn finding(
    report: &DrcReport,
    v: &Violation,
    net_names: &BTreeMap<u32, String>,
    board: &str,
) -> Option<Finding> {
    let severity = report.severity_of(v)?;
    let (a, b) = (net_name(net_names, v.nets.0), net_name(net_names, v.nets.1));
    let message = format!("{}, {a} / {b}", describe(v));
    Some(Finding {
        rule: rule_of(v).to_string(),
        severity,
        key: key(v, board, &message),
        message,
        evidence: evidence(&report.rules, v, &a, &b),
        items: v.items.iter().map(item_id).collect(),
        sheet: board.to_string(),
    })
}

fn rule_of(v: &Violation) -> &str {
    v.rule.as_deref().unwrap_or(v.kind.rule_id())
}

/// A net by its name, as Run DRC's Messages lines name it when it has none.
fn net_name(net_names: &BTreeMap<u32, String>, net: u32) -> String {
    net_names
        .get(&net)
        .cloned()
        .unwrap_or_else(|| format!("net {net}"))
}

/// What broke, in the words of Run DRC's Messages lines.
fn describe(v: &Violation) -> String {
    match v.kind {
        ViolationKind::Clearance => format!(
            "clearance {} mm, needs {} mm",
            mm(v.measured_nm),
            mm(v.required_nm)
        ),
        ViolationKind::Short => "short".to_string(),
        ViolationKind::Unconnected => {
            format!("link of {} mm not routed", mm(v.measured_nm))
        }
        ViolationKind::Keepout => "inside a keepout that forbids it".to_string(),
        ViolationKind::TrackWidth => size_text("track width", v),
        ViolationKind::ViaDiameter => size_text("via diameter", v),
        ViolationKind::ViaDrill => size_text("via drill", v),
    }
}

fn size_text(what: &str, v: &Violation) -> String {
    let bound = if v.measured_nm < v.required_nm {
        "at least"
    } else {
        "at most"
    };
    format!(
        "{what} {} mm, needs {bound} {} mm",
        mm(v.measured_nm),
        mm(v.required_nm)
    )
}

/// `nm` in mm, divided so a round value reads round (`0.2`, not the
/// `0.19999999999999998` a multiplication by 1e-6 gives).
fn mm(nm: i64) -> f64 {
    nm as f64 / NM_PER_MM as f64
}

/// What DRC knows beyond the shared fields; `at` in mm, as ERC's.
fn evidence(rules: &DrcRules, v: &Violation, a: &str, b: &str) -> Value {
    let layers: Vec<String> = copper_layers(v.layers)
        .into_iter()
        .map(|layer| pcb_layer_token(PcbLayer::Copper(layer)))
        .collect();
    let mut out = Map::new();
    out.insert("nets".into(), json!([a, b]));
    out.insert("layers".into(), json!(layers));
    out.insert("at".into(), json!([mm(v.at.x), mm(v.at.y)]));
    out.insert("measured_nm".into(), json!(v.measured_nm));
    out.insert("required_nm".into(), json!(v.required_nm));
    if let Some(keepout) = v.keepout {
        out.insert("keepout".into(), json!(keepout.to_string()));
    }
    if let Some(source) = rule_source(rules, v) {
        out.insert("rule_source".into(), json!(source));
    }
    Value::Object(out)
}

/// Where a limit came from: a written rule, a net class, the board setup or
/// the default clearance; `None` for a kind that has no limit.
fn rule_source(rules: &DrcRules, v: &Violation) -> Option<&'static str> {
    if v.rule.is_some() {
        return Some("written");
    }
    if v.kind != ViolationKind::Clearance {
        return None;
    }
    Some(if v.required_nm > rules.clearance {
        "net_class"
    } else if rules.from_default {
        "default"
    } else {
        "board_setup"
    })
}

/// An item's id: its uuid, or `<footprint>/<pad>` for a pad (a pad's own
/// uuid repeats across placements of one footprint).
fn item_id(item: &ItemRef) -> String {
    match item {
        ItemRef::Pad { footprint, pad } => format!("{footprint}/{pad}"),
        ItemRef::Track(id) | ItemRef::Via(id) | ItemRef::Footprint(id) => id.to_string(),
    }
}

fn item_label(item: &ItemRef) -> String {
    let kind = match item {
        ItemRef::Pad { .. } => "pad",
        ItemRef::Track(_) => "track",
        ItemRef::Via(_) => "via",
        ItemRef::Footprint(_) => "footprint",
    };
    format!("{kind} {}", item_id(item))
}

/// The board file, the rule, the items and the keepout: not the nets,
/// layers, position or gap, so moving an item keeps the key. A finding that
/// names no object (an unconnected link with no pad at either end) is keyed
/// by its `message` instead, as ERC keys one.
fn key(v: &Violation, board: &str, message: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(board.as_bytes());
    hasher.update(b"\n");
    hasher.update(rule_of(v).as_bytes());
    for item in &v.items {
        hasher.update(b"\nitem:");
        hasher.update(item_label(item).as_bytes());
    }
    if let Some(keepout) = v.keepout {
        hasher.update(b"\nkeepout:");
        hasher.update(keepout.to_string().as_bytes());
    }
    if v.items.is_empty() && v.keepout.is_none() {
        hasher.update(b"\nmessage:");
        hasher.update(message.as_bytes());
    }
    let mut hex = format!("{:x}", hasher.finalize());
    hex.truncate(KEY_HEX_DIGITS);
    hex
}

/// What Run DRC says in Messages about the rules, in its words (numbers in
/// [`mm`]), in its order: unrecognised entries, the rule text's notes and
/// remarks, what was not applied, and a clearance taken from the default.
fn rule_warnings(rules: &DrcRules) -> Vec<String> {
    let mut out: Vec<String> = rules
        .severities
        .unrecognised
        .iter()
        .map(|entry| format!("DRC rules: ignoring unrecognised entry ({entry})"))
        .collect();
    out.extend(rules.notes.iter().cloned());
    out.extend(rules.remarks.iter().cloned());
    if rules.texts_not_applied > 0 {
        out.push("DRC rules: the rule text was not applied (syntax error)".to_string());
    }
    if rules.rules_not_applied > 0 {
        out.push(format!(
            "DRC rules: {} written rule(s) not applied",
            rules.rules_not_applied
        ));
    }
    if rules.from_default {
        let value = mm(rules.clearance);
        out.push(if rules.sets_written_clearance() {
            format!(
                "DRC: No clearance rule set on this board; {value} mm applies where no written rule does. Set it in Board Setup."
            )
        } else {
            format!(
                "DRC: No clearance rule set on this board; checked with {value} mm. Set it in Board Setup."
            )
        });
    }
    out
}

fn skipped_warning(rejected: &Rejected) -> String {
    format!(
        "DRC: {} could not be checked: {}",
        item_label(&rejected.item),
        rejected.error
    )
}
