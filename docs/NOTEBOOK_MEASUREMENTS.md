# Design Notebook measurements, timelines and attachments

Mockup row 199. A notebook records bench readings against the parts, pins
and nets of the design, shows the readings of one target over time, and
holds the scope captures, thermal images and simulation plots they were
taken with. The reading rules live in the UI-free crate `signex-notebook`
(`measurement`, `attachment`, `scope`, `record`, `report`); the commands are
`crates/signex-app/src/app/notebook_measurements/`. The file format is
`.snxnbk` (`docs/FORMAT_VERSIONS.md`); nothing here adds a header key.

## A reading

```text
#measurement(
  target: net("+3V3"),
  quantity: "ripple",
  value: 18.0, unit: "mV",
  limit: (max: 30),
  conditions: "VIN 12.0 V, 2.0 A, 25 °C",
  instrument: "LAB-0042",
  operator: "j.berg",
  timestamp: "2026-11-24T15:12",
  attach: "scope/ripple-2A-revB.csv",
)
```

| Field | Meaning |
|---|---|
| `id` | A `MEAS-` id (`MEAS-009`). |
| `target` | One reference or an array of them: `net(..)`, `component(..)`, `pin(..)`. |
| `quantity` | What was measured. The table's Quantity column; without it the target names. |
| `value`, `unit` | The reading, a number as written (`3.310` keeps its zero). |
| `limit` | `(max: 30)`, `(min: 90)` or `(min: 2.9, max: 3.4)`. |
| `nominal`, `tolerance` | The other way to give a limit: `nominal: 3.3, tolerance: 2%`, or an absolute tolerance in the reading's unit, `tolerance: 0.05`. |
| `computed` | The calculated value the timeline draws beside the readings. |
| `conditions`, `instrument`, `operator` | Printed under the table. |
| `timestamp` | `YYYY-MM-DD`, optionally `THH:MM`; `date` is read the same. |
| `rev`, `board` | The design revision and board serial, listed on the timeline. |
| `attach` | The attachment it was taken with, a path inside `notebook/attachments/`. |
| `supersedes` | The `MEAS-` id of the reading it corrects. |
| `requirements` | The requirement ids it shows were met. |
| `taken-with` | The part values it was taken with: `("L1": "3.3 µH")`. |

A field that cannot be read (a computed `value`, a `limit` that is not a
dictionary of numbers, a malformed date or id) is reported with its line,
and the reading is still listed without it.

**A block of readings.** A `#measurement` with a body is a block: each
`#measurement` in its body is one row of its table and takes the block's
id, targets, instrument, conditions, operator, time, revision, board,
requirements, unit and stored part values when it does not set its own. A
block with a `caption:` is a numbered table. A block with its own `value:`
is its own first row.

**Saved readings are read-only.** A correction is a new reading with
`supersedes:`; `measurement::changed_saved_readings` lists the saved
readings an edit would change, found again by file, id and row (moving a
reading is no change).

## The verdict

The bounds are the limit's; the reference value is the nominal, the one
limit given, or the middle of a range.

- Outside a bound: **Fail**.
- Inside, but closer to a bound than 1 % of the reference value: **Warn**
  ("Warn = inside tolerance, under 1 % from the limit", mockup row 60).
- Otherwise **Pass**.

The Error column is `(value − reference) / |reference|`, in percent to two
significant digits with its sign (`+0.30 %`, `−40 %`); there is none
against a reference of 0. The printed table and the commands give the same
results: the package's `_check` and `measurement::check` follow this rule,
and `tests_measure_print` holds the printed Error column to the crate's.

## The timeline

The timeline of a net, a part or a pin lists its readings newest first,
undated last, with revision, board, conditions and operator, against the
newest reading's limit and computed value. A part's timeline holds the
readings of its pins too. It can show every quantity or one. A failing
reading is a fail; a reading whose stored part values have changed, or
whose attachment's have, is stale. The list exports as CSV
(`measurement::CSV_HEADER`).

## Attachments

```text
#attachment("scope/ripple-2A-revB.csv",
  caption: [Output ripple on +3V3 at 2 A],
  target: (net("+3V3"), component("C15")),
  taken-with: ("L1": "4.7 µH", "C15": "22 µF"),
  taken: "2026-11-24T15:12",
  channels: ((name: "CH1", label: "+3V3", coupling: "AC", scale: "5 mV", bandwidth: "20 MHz"),),
  time-scale: "1 µs",
  cursors: (y1: "9 mV", y2: "-9 mV", x1: "1 µs", x2: "3 µs"),
  trigger: (time: 0),
)
```

The file is a path inside `notebook/attachments/` (a path that climbs out
of it is reported and skipped). It prints as a numbered figure with a
source line naming the file, when it was taken and the part values it was
taken with. A CSV is drawn as a plot; a picture (`png`, `jpg`, `svg`, ...)
is shown as is; a missing or unreadable file prints a note in its place,
never a compile error.

**Copied into the project.** A file dropped on the page, picked from
Figure or named on the command line is copied into
`notebook/attachments/<folder>/` (`scope`, `images`, `sim` or `files` by
its kind, or a folder asked for), under a plain name numbered while it is
taken (`ripple-2.csv`). The inserted call stores the current value of
every part its targets name (`attachment::attachment_call`).

**Stale.** An attachment, or a reading, is stale once a part it stored a
value for has another value in the schematic, or is gone. `4.7 µH`,
`4.7uH` and `4.7 μH` are the same value. While the design is not read
nothing is stale. The figure prints a Stale mark and a note naming the
change ("Stale: taken with L1 = 3.3 µH; L1 is now 4.7 µH."). Mark still
valid stores the current values in the call (`attachment::mark_still_valid`).

## Scope captures

A scope CSV is read as: optional lines of instrument metadata, a line of
column names (`Time (s),CH1 (V)`), an optional line of units (`s,V`,
`Second,Volt`), then one line per sample, the time first. Commas,
semicolons or tabs separate the cells; with semicolons a decimal comma is
read. Blank lines and lines starting with `#` are skipped. A ragged line or
a cell that is not a number is an error with its line.

The plot is 10 × 8 divisions. A scale not given is the 1-2-5 step that
fits the samples (an AC channel is drawn around its mean, filling six
divisions; a DC channel around zero). The legend names the channel, its
label, coupling, scale and bandwidth limit (`CH1 +3V3 AC · 5 mV/div · 20
MHz BW`); the readout gives ΔY (peak to peak), ΔX and the frequency from
the cursors; the trigger is marked when it is on screen (at time 0 when
none is given). A trace of more than 800 points keeps the lowest and
highest sample of each run, so a spike is never dropped.

### Printing

The plot is an SVG of lines only, drawn as vectors in the PDF; its labels
are laid out by the package in the page's fonts. A Typst package reads
only inside its own root, so Signex serves the package what it prints
(`signex-notebook` `derived`): `derived/attachments.json` (every
attachment's kind, presence, stale changes, image path or plot), each
attachment image, and each plot. The PDF is always white, so the plot and
the table use fixed print colours, not the app theme. Two exports of the
same notebook are the same bytes.

## Commands

Each names the project with `document=<the project file, or any document
of it>`, reads `notebook/design.snxnbk`, its chapters and the attachments
they name off the UI thread, and is answered once that lands. Every answer
also carries `notebook` (the notebook file) and `load_problems` (a chapter
that could not be read). A project with no readable notebook file is
refused with the reason.

| Command | Operands | Answer |
|---|---|---|
| `notebook_measurements` | `document` | `readings` (each reading with `quantity_label`, `target_text`, `tolerance_text`, `measured_text`, `error_text`, `error`, `verdict`, `changes`, `superseded_by`), `summary` (readings, pass, warn, fail, unchecked, stale readings, attachments, stale and missing attachments), `problems` |
| `notebook_timeline` | `document`, one of `net`, `part`, `pin` (`U3.3`), `quantity` | `timeline` (target, quantity, unit, limit, computed, `entries` newest first, readings, fails, stale), `quantities` of the target, `csv` |
| `notebook_attachments` | `document` | `attachments` (each with `present`, `changes`, `plot`, `error`, `readings`), `summary`, `problems` |
| `export_notebook_timeline` | as `notebook_timeline`, and `path` | writes the CSV; `path`, `readings` |
| `mark_notebook_attachment_valid` | `document`, `file` (an attachment) or `id` (a reading) | writes the notebook file; `file`, `line` |
| `attach_notebook_file` | `document`, `path`, `kind` (the folder), `text` (the caption), `net`, `part`, `pin` | copies the file and adds its figure at the end of the notebook file; `file`, `copied_to`, `call` |

The reads are queries (the read capability, on the agent channel too).
`export_notebook_timeline` needs the export capability;
`mark_notebook_attachment_valid` and `attach_notebook_file` write the
project and need the save capability. On the command line a path operand
of a command that writes is confined to the allowed roots, so a file
attached from outside the project needs `--allow-path`.

## Not built yet

- The timeline panel and the attachment card in the notebook's margin
  lane, and the toolbar's Measurement (Ctrl+Shift+M) and Figure buttons,
  wait for the Notebook tab; their views are
  `notebook_measurements::views`.
- Holding saved readings read-only in the editor waits for the Notebook
  tab's save; `changed_saved_readings` is what it asks.
