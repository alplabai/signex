//! Self-contained HTML emitter for BOM export.
//!
//! Produces a standalone HTML file with inline CSS suitable for printing.

use super::{BomColumn, BomError, BomTable};

/// Emit a BOM table as self-contained HTML with inline CSS.
pub fn emit(table: &BomTable, columns: &[BomColumn]) -> Result<Vec<u8>, BomError> {
    let mut html = String::new();

    // HTML header with inline CSS
    html.push_str("<!DOCTYPE html>\n");
    html.push_str("<html>\n");
    html.push_str("<head>\n");
    html.push_str("  <meta charset=\"UTF-8\">\n");
    html.push_str("  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
    html.push_str("  <title>Bill of Materials</title>\n");
    html.push_str("  <style>\n");

    // Inline CSS — print-friendly
    html.push_str("    body {\n");
    html.push_str("      font-family: Arial, sans-serif;\n");
    html.push_str("      margin: 20px;\n");
    html.push_str("      background-color: #fff;\n");
    html.push_str("    }\n");
    html.push_str("    h1, h2, p {\n");
    html.push_str("      margin: 0 0 10px 0;\n");
    html.push_str("    }\n");
    html.push_str("    .bom-header {\n");
    html.push_str("      margin-bottom: 20px;\n");
    html.push_str("      border-bottom: 2px solid #333;\n");
    html.push_str("      padding-bottom: 10px;\n");
    html.push_str("    }\n");
    html.push_str("    .bom-header h1 {\n");
    html.push_str("      margin: 0;\n");
    html.push_str("    }\n");
    html.push_str("    .bom-meta {\n");
    html.push_str("      font-size: 12px;\n");
    html.push_str("      color: #666;\n");
    html.push_str("    }\n");
    html.push_str("    table {\n");
    html.push_str("      width: 100%;\n");
    html.push_str("      border-collapse: collapse;\n");
    html.push_str("      margin-top: 20px;\n");
    html.push_str("    }\n");
    html.push_str("    th, td {\n");
    html.push_str("      border: 1px solid #ccc;\n");
    html.push_str("      padding: 8px;\n");
    html.push_str("      text-align: left;\n");
    html.push_str("    }\n");
    html.push_str("    th {\n");
    html.push_str("      background-color: #f0f0f0;\n");
    html.push_str("      font-weight: bold;\n");
    html.push_str("    }\n");
    html.push_str("    tr:nth-child(even) td {\n");
    html.push_str("      background-color: #f9f9f9;\n");
    html.push_str("    }\n");
    html.push_str("    @media print {\n");
    html.push_str("      body { margin: 0; }\n");
    html.push_str("      table { page-break-inside: avoid; }\n");
    html.push_str("    }\n");

    html.push_str("  </style>\n");
    html.push_str("</head>\n");
    html.push_str("<body>\n");

    // Header with project metadata
    html.push_str("  <div class=\"bom-header\">\n");
    html.push_str(&format!(
        "    <h1>{}</h1>\n",
        escape_html(&table.metadata.title)
    ));
    html.push_str("    <div class=\"bom-meta\">\n");
    if !table.metadata.revision.is_empty() {
        html.push_str(&format!(
            "      <p>Revision: {}</p>\n",
            escape_html(&table.metadata.revision)
        ));
    }
    if !table.metadata.date.is_empty() {
        html.push_str(&format!(
            "      <p>Date: {}</p>\n",
            escape_html(&table.metadata.date)
        ));
    }
    html.push_str(&format!(
        "      <p>Total parts: {}</p>\n",
        table.rows.iter().map(|r| r.qty).sum::<u32>()
    ));
    html.push_str("    </div>\n");
    html.push_str("  </div>\n");

    // Table with data
    html.push_str("  <table>\n");
    html.push_str("    <thead>\n");
    html.push_str("      <tr>\n");

    for col in columns {
        html.push_str(&format!("        <th>{}</th>\n", col.header()));
    }

    html.push_str("      </tr>\n");
    html.push_str("    </thead>\n");
    html.push_str("    <tbody>\n");

    for row in &table.rows {
        html.push_str("      <tr>\n");

        for col in columns {
            let value = match col {
                BomColumn::Reference => row.references.join(", "),
                BomColumn::Qty => row.qty.to_string(),
                BomColumn::Value => row.value.clone(),
                BomColumn::Footprint => row.footprint.clone(),
                BomColumn::Description => row.description.clone(),
                BomColumn::Custom(name) => row.custom.get(name).cloned().unwrap_or_default(),
            };

            html.push_str(&format!("        <td>{}</td>\n", escape_html(&value)));
        }

        html.push_str("      </tr>\n");
    }

    html.push_str("    </tbody>\n");
    html.push_str("  </table>\n");

    html.push_str("</body>\n");
    html.push_str("</html>\n");

    Ok(html.into_bytes())
}

/// Escape HTML special characters.
fn escape_html(s: &str) -> String {
    s.chars()
        .map(|ch| match ch {
            '<' => "&lt;".to_string(),
            '>' => "&gt;".to_string(),
            '"' => "&quot;".to_string(),
            '&' => "&amp;".to_string(),
            _ => ch.to_string(),
        })
        .collect()
}
