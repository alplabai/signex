//! RFC 4180 CSV emitter for BOM export.
//!
//! Emits with UTF-8 BOM prefix and CRLF line endings for Windows compatibility.
//!
//! Spreadsheet formula injection (OWASP "CSV Injection"): a cell whose first
//! character is `=`, `+`, `-`, `@`, TAB or CR is run as a formula by Excel and
//! friends, and component fields are user data. [`write_csv_row`] prefixes
//! such a cell with a single quote (`'`, which spreadsheets hide) and quotes
//! it, on the header row too. A leading `+`/`-` stays untouched when what
//! follows is a plain number with an optional alphabetic unit (`-5V`,
//! `+3.3V`, `-12`), because that cannot form a formula and the quote would
//! corrupt legitimate rail names.

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
                BomColumn::Name => row.name.clone(),
                BomColumn::Designator | BomColumn::Reference => row.references.join(", "),
                BomColumn::LibRef => row.lib_ref.clone(),
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

/// Whether a spreadsheet would read `s` as a formula (see the module docs).
fn is_formula_lead(s: &str) -> bool {
    match s.chars().next() {
        Some('=' | '@' | '\t' | '\r') => true,
        Some('+' | '-') => !is_signed_number_with_unit(s),
        _ => false,
    }
}

/// `[+-]digits[.digits]` followed only by letters / `%` (`-5V`, `+3.3V`).
fn is_signed_number_with_unit(s: &str) -> bool {
    let body = s[1..].trim_start_matches(|c: char| c.is_ascii_digit() || c == '.');
    let digits = s.len() - 1 - body.len();
    digits > 0 && body.chars().all(|c| c.is_alphabetic() || c == '%')
}

/// Write a single CSV row with proper quoting and escaping per RFC 4180.
pub(crate) fn write_csv_row(
    output: &mut Vec<u8>,
    fields: &[impl AsRef<str>],
) -> Result<(), BomError> {
    for (i, field) in fields.iter().enumerate() {
        let raw = field.as_ref();
        let neutralised = is_formula_lead(raw);
        let prefixed;
        let s = if neutralised {
            prefixed = format!("'{raw}");
            prefixed.as_str()
        } else {
            raw
        };

        // Quote if field contains comma, quote, or newline, or was neutralised
        if neutralised || s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r')
        {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn row(fields: &[&str]) -> String {
        let mut out = Vec::new();
        write_csv_row(&mut out, fields).unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn formula_leading_cells_are_quoted_and_neutralised() {
        assert_eq!(
            row(&[
                "=HYPERLINK(\"http://x\")",
                "@SUM(A1)",
                "+1+1",
                "-1+cmd|' /c calc'!A0"
            ]),
            "\"'=HYPERLINK(\"\"http://x\"\")\",\"'@SUM(A1)\",\"'+1+1\",\"'-1+cmd|' /c calc'!A0\""
        );
        assert_eq!(row(&["\tx"]), "\"'\tx\"");
        assert_eq!(row(&["\rx"]), "\"'\rx\"");
    }

    #[test]
    fn legitimate_values_stay_readable() {
        assert_eq!(
            row(&["-5V", "+3.3V", "-12", "10k", "a=b"]),
            "-5V,+3.3V,-12,10k,a=b"
        );
        // A sign alone, or a sign then an operator, is not a number.
        assert_eq!(row(&["-", "-V", "-5+2"]), "\"'-\",\"'-V\",\"'-5+2\"");
    }

    #[test]
    fn header_row_is_neutralised_too() {
        let table = BomTable::default();
        let out = emit(&table, &[BomColumn::Custom("=evil()".into())]).unwrap();
        assert!(String::from_utf8(out).unwrap().contains("\"'=evil()\""));
    }
}
