//! Plain-text emitter.

use super::{ErcReportRow, coord};

/// One report row is one line: line breaks inside a field are written as the
/// two characters `\n` / `\r` so a multi-line message cannot split the row.
fn flatten(s: &str) -> String {
    s.replace('\r', "\\r").replace('\n', "\\n")
}

pub(super) fn emit(rows: &[&ErcReportRow], labels: &[String], grouped: bool) -> Vec<u8> {
    let mut out = String::new();
    out.push_str(&format!("ERC report: {} violation(s)\n", rows.len()));
    let mut current: Option<&str> = None;
    for (r, label) in rows.iter().zip(labels) {
        let label = flatten(label);
        let (rule, message) = (flatten(&r.rule), flatten(&r.message));
        if grouped {
            if current != Some(r.sheet_key.as_str()) {
                let n = rows.iter().filter(|x| x.sheet_key == r.sheet_key).count();
                out.push_str(&format!("\n== {label} ({n}) ==\n"));
                current = Some(&r.sheet_key);
            }
            out.push_str(&format!(
                "[{}] {rule}: {message} @ ({}, {})\n",
                r.severity.label(),
                coord(r.x),
                coord(r.y)
            ));
        } else {
            out.push_str(&format!(
                "{label} [{}] {rule}: {message} @ ({}, {})\n",
                r.severity.label(),
                coord(r.x),
                coord(r.y)
            ));
        }
    }
    out.into_bytes()
}
