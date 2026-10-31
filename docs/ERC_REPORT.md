# ERC report

File > Export > ERC Report writes the results of the newest ERC run. It never
runs ERC itself; the rows are the ERC panel's rows, in the same order, so a
finding whose part, wire or label has since been deleted is left out, as it is
in the panel. When the document changed after the run, every form says so
(a notice line in text, CSV, HTML and PDF; `"results_stale": true` in JSON,
absent otherwise) and the Messages panel gets a warning. The
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

`results_stale` is present, and `true`, only when the sheet changed after the run;
it is omitted otherwise.

```json
{
  "schema_version": 1,
  "generator": "signex",
  "results_stale": true,
  "rule_set": { "profile": "Signex Default", "sha256": "<hex>" },
  "summary": { "errors": 1, "warnings": 2, "info": 0, "total": 3, "waived": 1 },
  "groups": [
    {
      "severity": "Error",
      "sheets": [
        {
          "sheet": "power.snxsch",
          "sheet_path": "sheets/power.snxsch",
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
      "waived_count": 1,
      "severity": "Warning"
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
| `summary.waived` | integer | Findings hidden by the listed waivers; they are not in `groups`. |
| `groups[]` | array | One entry per severity that has findings, `Error` first. |
| `groups[].severity` | string | `"Error"`, `"Warning"` or `"Info"`. |
| `groups[].sheets[]` | array | Sheets in first-seen order. |
| `sheet` / `sheet_path` | string | File name of the sheet, and its path relative to the project folder (absolute only for a sheet outside it). |
| `violations[].rule` | string | Rule name as the panel shows it. |
| `violations[].message` | string | The finding's text. |
| `violations[].x_mm` / `y_mm` | number | Location on the sheet, in mm. |
| `waivers[].kind` | string | `"per_finding"` (#1285) or `"per_net"` (#1581). |
| `waivers[].rule_id` | string | Stable rule id, `builtin::...` or `user::...`. |
| `waivers[].subject` | string | The waived finding's message, or `net <name>` for a per-net waiver. |
| `waivers[].reason` / `author` / `date` | string | Empty for a per-net waiver, which records none. |
| `waivers[].status` | string | `"active"`, `"lapsed"` (an involved object changed, so the finding is reported again), `"unmatched"` or `"incomplete"` (no reason or author; ignored). |
| `waivers[].waived_count` | integer | Findings it hid in this run. |
| `waivers[].severity` | string or `null` | Severity of the finding it hid (`"Error"`, `"Warning"`, `"Info"`); `null` when unknown (per-net waivers, or nothing hidden). |

The app exports every finding, so no severity filter applies. A report made
with a minimum severity lists only findings and waivers at or above it, and
`summary.waived` counts only the waivers listed. A waiver whose severity is
`null` is always listed, so a filtered report never hides a per-net, lapsed
or unmatched waiver.

For CI, a non-zero `summary.errors` is the usual failure condition.

**`unmatched` does not mean fixed.** It means the stored waiver's identity
matched no finding in this run. That happens when the problem was fixed, but
also when the finding is still there under a different identity:

- a library pin was moved or renumbered (the pin anchor is the pin's number
  and its position in the symbol);
- a part placed several times (a reused sheet) was re-annotated, so its
  designator tag changed;
- a part joined or left an unnamed net (an unnamed net is identified by its
  pin set);
- the object the waiver names was deleted and re-created with a new UUID.

In those cases the finding itself is reported again (in `groups`), so a CI
gate on `summary` stays correct; treat `unmatched` waivers as stale entries to
review, and `lapsed` ones as findings to re-check.
