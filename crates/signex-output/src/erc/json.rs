//! JSON emitter (#1287): the [`ErcReport`] model, pretty-printed. Schema:
//! `docs/ERC_REPORT.md`.

use super::ErcReportError;
use super::report::ErcReport;

pub(super) fn emit(report: &ErcReport) -> Result<Vec<u8>, ErcReportError> {
    let mut bytes =
        serde_json::to_vec_pretty(report).map_err(|e| ErcReportError::Json(e.to_string()))?;
    bytes.push(b'\n');
    Ok(bytes)
}
