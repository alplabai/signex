# ERC report

File > Export > ERC Report writes the results of the newest ERC run. It never
runs ERC itself; the rows are the ERC panel's rows, in the same order. The
format follows the extension picked in the save dialog:

| Extension | Form |
|---|---|
| `.txt` (or none) | Plain text, one line per finding. |
| `.csv` | RFC 4180 CSV with a UTF-8 BOM. |
| `.json` | Machine-readable, schema below. |
| `.html`, `.htm` | One self-contained page: inline CSS, no scripts, no external assets. |
| `.pdf` | The same content as the HTML, set as text on A4 pages with the embedded fonts. |

JSON, HTML and PDF are built from one model (`signex_output::erc_report`):
a summary with counts, the findings grouped by severity (Error, Warning, Info)
and then by sheet, and every stored waiver with its reason and author. All
user text in the HTML form (net names, designators, waiver reasons) is
HTML-escaped.

## JSON schema (`schema_version` 1)

`schema_version` changes only when a field is removed, renamed or changes
meaning; new fields may be added within a version, so readers should ignore
keys they do not know.

```json
{
  "schema_version": 1,
  "generator": "signex",
  "rule_set": { "profile": "Signex Default", "sha256": "<hex>" },
  "summary": { "errors": 1, "warnings": 2, "info": 0, "total": 3, "waived": 1 },
  "groups": [
    {
      "severity": "Error",
      "sheets": [
        {
          "sheet": "power.snxsch",
          "sheet_path": "/path/to/project/power.snxsch",
          "violations": [
            { "rule": "Power port short", "message": "...", "x_mm": 25.4, "y_mm": 12.7 }
          ]
        }
      ]
    }
  ],
  "waivers": [
    {
      "kind": "per_finding",
      "rule_id": "builtin::unused_pin",
      "subject": "Pin on TP1 is not connected",
      "reason": "Test point, left open on purpose",
      "author": "A. Designer",
      "date": "2026-10-29",
      "status": "active",
      "waived_count": 1
    }
  ]
}
```

| Field | Type | Meaning |
|---|---|---|
| `schema_version` | integer | `1`. |
| `generator` | string | Always `"signex"`. |
| `rule_set` | object or `null` | The rule set the run used: `profile` (string or `null`) and `sha256` of the applied rules. |
| `summary.errors` / `warnings` / `info` | integer | Reported findings per severity. |
| `summary.total` | integer | All reported findings. |
| `summary.waived` | integer | Findings hidden by waivers; they are not in `groups`. |
| `groups[]` | array | One entry per severity that has findings, `Error` first. |
| `groups[].severity` | string | `"Error"`, `"Warning"` or `"Info"`. |
| `groups[].sheets[]` | array | Sheets in first-seen order. |
| `sheet` / `sheet_path` | string | File name and full path of the sheet. |
| `violations[].rule` | string | Rule name as the panel shows it. |
| `violations[].message` | string | The finding's text. |
| `violations[].x_mm` / `y_mm` | number | Location on the sheet, in mm. |
| `waivers[].kind` | string | `"per_finding"` (#1285) or `"per_net"` (#1581). |
| `waivers[].rule_id` | string | Stable rule id, `builtin::...` or `user::...`. |
| `waivers[].subject` | string | The waived finding's message, or `net <name>` for a per-net waiver. |
| `waivers[].reason` / `author` / `date` | string | Empty for a per-net waiver, which records none. |
| `waivers[].status` | string | `"active"`, `"lapsed"` (an involved object changed, so the finding is reported again), `"unmatched"` or `"incomplete"` (no reason or author; ignored). |
| `waivers[].waived_count` | integer | Findings it hid in this run. |

For CI, a non-zero `summary.errors` is the usual failure condition.
