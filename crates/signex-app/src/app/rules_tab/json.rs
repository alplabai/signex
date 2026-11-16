//! The JSON the `rules_*` commands answer with. Shapes are in
//! `docs/DRC_RULES.md`, "Rules manager".

use serde_json::{Map, Value, json};
use signex_drc::{Effective, FlaggedRule, RuleFlag, Source};
use signex_drc_dsl::table::{RuleRow, scope_text};
use signex_drc_dsl::{Diagnostic, Level};
use signex_types::net_class::{NetClass, first_matching_class};
use signex_types::pcb::PcbBoard;
use signex_types::violation::Severity;

use super::RulesDomain;

/// `nm` as mm, without float noise.
pub(crate) fn mm(nm: i64) -> f64 {
    (pcb_geom::nm_to_mm(nm) * 1e6).round() / 1e6
}

pub(crate) const fn severity_str(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    }
}

fn diagnostic_json(d: &Diagnostic, src: &str) -> Value {
    let (line, column) = d.line_col(src);
    let level = match d.level {
        Level::Error => "error",
        Level::Warning => "warning",
    };
    json!({
        "level": level,
        "message": d.message,
        "line": line,
        "column": column,
    })
}

/// One rule as `rules_list` and `rules_get` answer it.
pub(crate) fn row_json(row: &RuleRow, src: &str) -> Value {
    let limits: Map<String, Value> = row
        .limits
        .iter()
        .map(|l| (l.field.id().to_string(), json!(l.value)))
        .collect();
    json!({
        "id": row.id(),
        "name": row.name,
        "enabled": row.enabled,
        "applied": row.applied,
        "kind": row.kind.map(|k| k.keyword()),
        "domain": row.kind.map(|k| RulesDomain::of(k).as_str()),
        "scope": scope_text(&row.scope),
        "layer": row.layer,
        "pair": row.pair.as_ref().map(|[a, b]| format!("{a}-{b}")),
        "limits": limits,
        "severity": severity_str(row.severity),
        "message": row.message,
        "text_only": row.text_only,
        "problems": row.problems.iter().map(|d| diagnostic_json(d, src)).collect::<Vec<_>>(),
        "text": row.text(src),
    })
}

/// One flag on a rule.
pub(crate) fn flag_json(flagged: &FlaggedRule) -> Value {
    let mut out = json!({ "rule": flagged.rule, "flag": flagged.flag.as_str() });
    let extra = match &flagged.flag {
        RuleFlag::Shadowed { by } => json!({ "by": by }),
        RuleFlag::Conflict { with } => json!({ "with": with }),
        RuleFlag::LowersClass {
            class,
            from_nm,
            to_nm,
        } => json!({ "class": class, "from_mm": mm(*from_nm), "to_mm": mm(*to_nm) }),
        RuleFlag::NoNets => json!({}),
    };
    if let (Some(out), Value::Object(extra)) = (out.as_object_mut(), extra) {
        out.extend(extra);
    }
    out
}

/// A net's governing value and the chain behind it.
pub(crate) fn effective_json(
    effective: &Effective,
    board: &PcbBoard,
    classes: &[NetClass],
) -> Value {
    let net_name = board
        .nets
        .iter()
        .find(|n| n.number == effective.net)
        .map(|n| n.name.clone())
        .unwrap_or_else(|| format!("#{}", effective.net));
    let source = match &effective.source {
        Source::Rule(name) => json!({ "kind": "rule", "rule": name }),
        Source::NetClass => json!({
            "kind": "net_class",
            "class": first_matching_class(classes, &net_name).map(|c| c.name.clone()),
        }),
        Source::BoardSetup => json!({ "kind": "board_setup" }),
        Source::Unlimited => json!({ "kind": "unlimited" }),
    };
    let chain: Vec<Value> = effective
        .chain
        .iter()
        .map(|c| {
            json!({
                "rule": c.name,
                "value_mm": mm(c.value_nm),
                "specificity": [c.specificity.0, c.specificity.1],
                "verdict": c.verdict.as_str(),
            })
        })
        .collect();
    json!({
        "net": net_name,
        "kind": effective.kind.as_str(),
        "layer": effective.layer.and_then(layer_name),
        "value_mm": effective.value_nm.map(mm),
        "source": source,
        "without_rules_mm": effective.implicit_nm.map(mm),
        "chain": chain,
    })
}

/// The copper layer at `bit`, as the board file names it.
pub(crate) fn layer_name(bit: u32) -> Option<String> {
    pcb_geom::copper_layers(1u32.checked_shl(bit)?)
        .first()
        .map(|&layer| {
            signex_types::format::pcb_layer_token(signex_types::pcb_layer::PcbLayer::from(layer))
        })
}
