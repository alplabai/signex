import type { SchematicData } from "@/types";

interface BomRow {
  designators: string[];
  value: string;
  footprint: string;
  library: string;
  quantity: number;
  fields: Record<string, string>;
}

function collectBomRows(data: SchematicData, groupBy: string[] = ["Value", "Footprint"]): BomRow[] {
  const groups = new Map<string, BomRow>();

  for (const sym of data.symbols) {
    if (sym.is_power || sym.reference.endsWith("?")) continue;

    const key = groupBy
      .map((g) => {
        if (g === "Value") return sym.value;
        if (g === "Footprint") return sym.footprint;
        if (g === "Library") return sym.lib_id;
        return sym.fields[g] || "";
      })
      .join("|");

    if (!groups.has(key)) {
      groups.set(key, {
        designators: [],
        value: sym.value,
        footprint: sym.footprint,
        library: sym.lib_id,
        quantity: 0,
        fields: { ...sym.fields },
      });
    }
    const row = groups.get(key)!;
    row.designators.push(sym.reference);
    row.quantity++;
  }

  // Sort by first designator (natural sort)
  const rows = Array.from(groups.values());
  rows.sort((a, b) => naturalSort(a.designators[0] || "", b.designators[0] || ""));
  for (const row of rows) row.designators.sort(naturalSort);
  return rows;
}

function naturalSort(a: string, b: string): number {
  const ap = a.replace(/[0-9]+$/, ""), bp = b.replace(/[0-9]+$/, "");
  if (ap !== bp) return ap.localeCompare(bp);
  const an = parseInt(a.replace(/^[A-Z]+/i, ""), 10) || 0;
  const bn = parseInt(b.replace(/^[A-Z]+/i, ""), 10) || 0;
  return an - bn;
}

function escapeHtml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

/**
 * Generate BOM as HTML table
 */
export function generateBomHtml(data: SchematicData, columns: string[] = ["Designator", "Value", "Footprint", "Library", "Quantity"]): string {
  const rows = collectBomRows(data);
  const title = data.title_block?.title || "Untitled";

  let html = `<!DOCTYPE html><html><head><meta charset="UTF-8"><title>BOM - ${escapeHtml(title)}</title>
<style>
body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; margin: 20px; background: #1a1b2e; color: #cdd6f4; }
h1 { font-size: 18px; color: #89b4fa; }
table { border-collapse: collapse; width: 100%; margin-top: 12px; }
th { background: #313244; color: #a6adc8; text-align: left; padding: 8px 12px; font-size: 11px; text-transform: uppercase; letter-spacing: 0.05em; border-bottom: 2px solid #45475a; }
td { padding: 6px 12px; font-size: 12px; border-bottom: 1px solid #313244; font-family: 'SF Mono', 'Fira Code', monospace; }
tr:hover { background: #313244; }
.summary { color: #a6adc8; font-size: 12px; margin-top: 8px; }
</style></head><body>
<h1>Bill of Materials — ${escapeHtml(title)}</h1>
<p class="summary">Generated ${new Date().toLocaleString()} | ${rows.length} unique parts, ${rows.reduce((s, r) => s + r.quantity, 0)} total components</p>
<table><thead><tr>`;

  for (const col of columns) html += `<th>${escapeHtml(col)}</th>`;
  html += `</tr></thead><tbody>`;

  for (const row of rows) {
    html += `<tr>`;
    for (const col of columns) {
      let val = "";
      if (col === "Designator") val = row.designators.join(", ");
      else if (col === "Quantity") val = String(row.quantity);
      else if (col === "Value") val = row.value;
      else if (col === "Footprint") val = row.footprint;
      else if (col === "Library") val = row.library;
      else val = row.fields[col] || "";
      html += `<td>${escapeHtml(val)}</td>`;
    }
    html += `</tr>`;
  }

  html += `</tbody></table></body></html>`;
  return html;
}

/**
 * Generate BOM as Excel-compatible XML spreadsheet (.xls)
 * Uses SpreadsheetML format which Excel can open natively
 */
export function generateBomExcel(data: SchematicData, columns: string[] = ["Designator", "Value", "Footprint", "Library", "Quantity"]): string {
  const rows = collectBomRows(data);

  let xml = `<?xml version="1.0" encoding="UTF-8"?>
<?mso-application progid="Excel.Sheet"?>
<Workbook xmlns="urn:schemas-microsoft-com:office:spreadsheet"
 xmlns:ss="urn:schemas-microsoft-com:office:spreadsheet">
 <Styles>
  <Style ss:ID="Header"><Font ss:Bold="1" ss:Size="11"/><Interior ss:Color="#313244" ss:Pattern="Solid"/></Style>
  <Style ss:ID="Data"><Font ss:Size="10"/></Style>
 </Styles>
 <Worksheet ss:Name="BOM">
  <Table>`;

  // Header row
  xml += `\n   <Row ss:StyleID="Header">`;
  for (const col of columns) xml += `<Cell><Data ss:Type="String">${escapeHtml(col)}</Data></Cell>`;
  xml += `</Row>`;

  // Data rows
  for (const row of rows) {
    xml += `\n   <Row ss:StyleID="Data">`;
    for (const col of columns) {
      let val = "";
      const isNum = col === "Quantity";
      if (col === "Designator") val = row.designators.join(", ");
      else if (col === "Quantity") val = String(row.quantity);
      else if (col === "Value") val = row.value;
      else if (col === "Footprint") val = row.footprint;
      else if (col === "Library") val = row.library;
      else val = row.fields[col] || "";
      xml += `<Cell><Data ss:Type="${isNum ? "Number" : "String"}">${escapeHtml(val)}</Data></Cell>`;
    }
    xml += `</Row>`;
  }

  xml += `\n  </Table>\n </Worksheet>\n</Workbook>`;
  return xml;
}
