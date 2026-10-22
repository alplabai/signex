//! Plain-text emitter.

use super::{ErcReportRow, coord};

pub(super) fn emit(rows: &[&ErcReportRow], grouped: bool) -> Vec<u8> {
    let mut out = String::new();
    out.push_str(&format!("ERC report: {} violation(s)\n", rows.len()));
    let mut current: Option<&str> = None;
    for r in rows {
        if grouped {
            if current != Some(r.sheet_name.as_str()) {
                let n = rows.iter().filter(|x| x.sheet_name == r.sheet_name).count();
                out.push_str(&format!("\n== {} ({n}) ==\n", r.sheet_name));
                current = Some(&r.sheet_name);
            }
            out.push_str(&format!(
                "[{}] {}: {} @ ({}, {})\n",
                r.severity.label(),
                r.rule,
                r.message,
                coord(r.x),
                coord(r.y)
            ));
        } else {
            out.push_str(&format!(
                "{} [{}] {}: {} @ ({}, {})\n",
                r.sheet_name,
                r.severity.label(),
                r.rule,
                r.message,
                coord(r.x),
                coord(r.y)
            ));
        }
    }
    out.into_bytes()
}
