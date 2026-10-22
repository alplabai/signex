//! CSV emitter — fields are escaped by the BOM's shared RFC 4180 helper.

use super::{COLUMNS, ErcReportError, ErcReportRow, coord};
use crate::bom::csv::write_csv_row;

pub(super) fn emit(rows: &[&ErcReportRow], labels: &[String]) -> Result<Vec<u8>, ErcReportError> {
    let mut out = Vec::new();
    // UTF-8 BOM so spreadsheets detect the encoding; CRLF line endings.
    out.extend_from_slice(b"\xef\xbb\xbf");
    let to_err = |e: crate::bom::BomError| ErcReportError::Csv(e.to_string());
    write_csv_row(&mut out, &COLUMNS).map_err(to_err)?;
    out.extend_from_slice(b"\r\n");
    for (r, label) in rows.iter().zip(labels) {
        let fields = [
            label.clone(),
            r.severity.label().to_string(),
            r.rule.clone(),
            r.message.clone(),
            coord(r.x),
            coord(r.y),
        ];
        write_csv_row(&mut out, &fields).map_err(to_err)?;
        out.extend_from_slice(b"\r\n");
    }
    Ok(out)
}
