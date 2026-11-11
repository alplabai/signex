//! ERC findings in the shared check shape (`signex_types::check`, #2199):
//! the rule, severity and message every check has, ERC's own facts in
//! `evidence`, and a key that names the same finding in every run. The key is
//! made from the finding's waiver identity (`waivers::violation`), so moving
//! an object keeps it and a change a waiver would lapse on does not. See
//! `docs/CLI.md`, "Checks on demand".

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use signex_types::check::{Finding, Severity as CheckSeverity};
use signex_types::project::ErcWaiver;

use crate::{LocationSpace, Severity, Violation, waivers};

/// Hex digits of the key: 128 bits of the hash.
const KEY_HEX_DIGITS: usize = 32;

/// `v`, filed under the sheet named `sheet`, as a check finding. `v` carries
/// its waiver key when it names an object (`waivers::violation::stamp_keys`).
pub fn finding(v: &Violation, sheet: &str) -> Finding {
    Finding {
        rule: v.rule_id.to_string(),
        severity: severity(v.severity),
        message: v.message.clone(),
        evidence: evidence(v),
        items: waivers::violation::involved_items(v)
            .iter()
            .map(|item| item.uuid.to_string())
            .collect(),
        sheet: sheet.to_string(),
        key: key(v, sheet),
    }
}

/// The net waiver `(rule id, net name)` as a waiver object of a report.
pub fn net_waiver(rule_id: &str, net: &str) -> Value {
    json!({"kind": "net", "rule_id": rule_id, "net": net})
}

/// A per-finding waiver as a waiver object of a report.
pub fn finding_waiver(waiver: &ErcWaiver) -> Value {
    json!({
        "kind": "finding",
        "rule_id": waiver.rule_id,
        "objects": waiver.objects,
        "fingerprint": waiver.fingerprint,
        "reason": waiver.reason,
        "author": waiver.author,
        "date": waiver.date,
        "message": waiver.message,
    })
}

/// A run never reports a rule that is off (`apply_overrides` drops it).
fn severity(severity: Severity) -> CheckSeverity {
    match severity {
        Severity::Error => CheckSeverity::Error,
        Severity::Warning => CheckSeverity::Warning,
        Severity::Info | Severity::Off => CheckSeverity::Info,
    }
}

/// The sheet, the rule and the objects of the finding's waiver identity; a
/// finding that names no object is keyed by its message instead.
fn key(v: &Violation, sheet: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(sheet.as_bytes());
    hasher.update(b"\n");
    hasher.update(v.rule_id.to_string().as_bytes());
    match &v.waiver_key {
        Some(waiver) => {
            for object in &waiver.objects {
                hasher.update(b"\nobject:");
                hasher.update(object.as_bytes());
            }
        }
        None => {
            hasher.update(b"\nmessage:");
            hasher.update(v.message.as_bytes());
        }
    }
    let mut hex = format!("{:x}", hasher.finalize());
    hex.truncate(KEY_HEX_DIGITS);
    hex
}

/// What ERC knows beyond the shared fields.
fn evidence(v: &Violation) -> Value {
    let mut out = Map::new();
    out.insert("rule_name".into(), json!(v.display_name()));
    let space = match v.location_space {
        LocationSpace::ThisSheet => "sheet",
        LocationSpace::Unplaced => "unplaced",
        LocationSpace::ChildSheet => "child_sheet",
        LocationSpace::Net => "net",
    };
    out.insert("space".into(), json!(space));
    if v.location_space != LocationSpace::Unplaced {
        out.insert("at".into(), json!([v.location.x, v.location.y]));
    }
    if let Some(child) = &v.child_sheet {
        out.insert("child_sheet".into(), json!(child));
    }
    if let Some(net) = &v.net {
        let pins: Vec<String> = net
            .pins
            .iter()
            .map(|p| format!("{}.{}", p.reference.trim(), p.number))
            .collect();
        out.insert("net".into(), json!({"name": net.name, "pins": pins}));
    }
    if let Some(pin) = &v.anchor_pin {
        out.insert("pin".into(), json!(pin));
    }
    if let Some(waiver) = &v.waiver_key {
        out.insert(
            "waiver_key".into(),
            json!({
                "rule_id": waiver.rule_id,
                "objects": waiver.objects,
                "fingerprint": waiver.fingerprint,
            }),
        );
    }
    Value::Object(out)
}

#[cfg(test)]
mod tests;
