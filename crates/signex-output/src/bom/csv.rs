//! RFC 4180 CSV emitter for BOM export.
//!
//! Emits with UTF-8 BOM prefix and CRLF line endings for Windows compatibility.

use super::{BomColumn, BomError, BomTable};

/// Emit a BOM table as RFC 4180 CSV with UTF-8 BOM.
pub fn emit(table: &BomTable, columns: &[BomColumn]) -> Result<Vec<u8>, BomError> {
    let mut output = Vec::new();

    // Prepend UTF-8 BOM so Excel auto-detects encoding
    output.extend_from_slice(b"\xef\xbb\xbf");

    // Write header row
    let headers: Vec<&str> = columns.iter().map(|col| col.header()).collect();
    write_csv_row(&mut output, &headers)?;
    output.extend_from_slice(b"\r\n");

    // Write data rows
    for row in &table.rows {
        let mut fields: Vec<String> = Vec::new();

        for col in columns {
            let value = match col {
                BomColumn::Reference => row.references.join(", "),
                BomColumn::Qty => row.qty.to_string(),
                BomColumn::Value => row.value.clone(),
                BomColumn::Footprint => row.footprint.clone(),
                BomColumn::Description => row.description.clone(),
                BomColumn::Custom(name) => row.custom.get(name).cloned().unwrap_or_default(),
            };
            fields.push(value);
        }

        write_csv_row(&mut output, &fields)?;
        output.extend_from_slice(b"\r\n");
    }

    Ok(output)
}

/// Write a single CSV row with proper quoting and escaping per RFC 4180.
fn write_csv_row(output: &mut Vec<u8>, fields: &[impl AsRef<str>]) -> Result<(), BomError> {
    for (i, field) in fields.iter().enumerate() {
        let s = field.as_ref();

        // Quote if field contains comma, quote, or newline
        if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
            output.push(b'"');
            for ch in s.chars() {
                if ch == '"' {
                    output.extend_from_slice(b"\"\"");
                } else {
                    output.extend_from_slice(ch.to_string().as_bytes());
                }
            }
            output.push(b'"');
        } else {
            output.extend_from_slice(s.as_bytes());
        }

        // Add comma separator, except after last field
        if i < fields.len() - 1 {
            output.push(b',');
        }
    }

    Ok(())
}
