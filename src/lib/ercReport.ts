import type { ErcViolation } from "./erc";

/**
 * Generate a self-contained HTML report for ERC violations.
 */
export function generateErcHtmlReport(violations: ErcViolation[], projectName: string): string {
  const now = new Date();
  const timestamp = now.toLocaleString();
  const errors = violations.filter(v => v.severity === "error");
  const warnings = violations.filter(v => v.severity === "warning");

  const rows = violations.map((v, i) => {
    const severityColor = v.severity === "error" ? "#f38ba8" : "#fab387";
    const severityLabel = v.severity === "error" ? "Error" : "Warning";
    const position = v.position ? `(${v.position.x.toFixed(2)}, ${v.position.y.toFixed(2)})` : "&mdash;";
    const typeLabel = v.type.replace(/_/g, " ");

    return `<tr>
      <td>${i + 1}</td>
      <td style="color: ${severityColor}; font-weight: 600;">${severityLabel}</td>
      <td>${typeLabel}</td>
      <td>${escapeHtml(v.message)}</td>
      <td>${position}</td>
    </tr>`;
  }).join("\n");

  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>ERC Report — ${escapeHtml(projectName)}</title>
  <style>
    * { margin: 0; padding: 0; box-sizing: border-box; }
    body {
      background: #1a1b2e;
      color: #cdd6f4;
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
      font-size: 14px;
      padding: 32px;
    }
    h1 {
      font-size: 22px;
      font-weight: 700;
      margin-bottom: 4px;
      color: #cdd6f4;
    }
    .subtitle {
      font-size: 12px;
      color: #6c7086;
      margin-bottom: 24px;
    }
    .summary {
      display: flex;
      gap: 16px;
      margin-bottom: 24px;
    }
    .summary-card {
      padding: 12px 20px;
      border-radius: 8px;
      background: #24253a;
      min-width: 120px;
    }
    .summary-card .label {
      font-size: 11px;
      text-transform: uppercase;
      letter-spacing: 0.05em;
      color: #6c7086;
      margin-bottom: 4px;
    }
    .summary-card .count {
      font-size: 24px;
      font-weight: 700;
    }
    .count-total { color: #cdd6f4; }
    .count-error { color: #f38ba8; }
    .count-warning { color: #fab387; }
    table {
      width: 100%;
      border-collapse: collapse;
      background: #24253a;
      border-radius: 8px;
      overflow: hidden;
    }
    thead th {
      text-align: left;
      padding: 10px 14px;
      font-size: 11px;
      text-transform: uppercase;
      letter-spacing: 0.05em;
      color: #6c7086;
      background: #1e1f33;
      border-bottom: 1px solid #313244;
    }
    tbody td {
      padding: 8px 14px;
      border-bottom: 1px solid #313244;
      font-size: 13px;
      vertical-align: top;
    }
    tbody tr:last-child td {
      border-bottom: none;
    }
    tbody tr:hover {
      background: #2a2b40;
    }
    .empty {
      text-align: center;
      padding: 32px;
      color: #6c7086;
    }
  </style>
</head>
<body>
  <h1>ERC Report — ${escapeHtml(projectName)}</h1>
  <div class="subtitle">Generated ${escapeHtml(timestamp)}</div>

  <div class="summary">
    <div class="summary-card">
      <div class="label">Total</div>
      <div class="count count-total">${violations.length}</div>
    </div>
    <div class="summary-card">
      <div class="label">Errors</div>
      <div class="count count-error">${errors.length}</div>
    </div>
    <div class="summary-card">
      <div class="label">Warnings</div>
      <div class="count count-warning">${warnings.length}</div>
    </div>
  </div>

  ${violations.length === 0
    ? '<div class="empty">No violations found.</div>'
    : `<table>
    <thead>
      <tr>
        <th>#</th>
        <th>Severity</th>
        <th>Type</th>
        <th>Message</th>
        <th>Position</th>
      </tr>
    </thead>
    <tbody>
      ${rows}
    </tbody>
  </table>`}
</body>
</html>`;
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}
