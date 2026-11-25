# Design Notebook annotations and the Annotations panel

A project's Design Notebook lives in its `notebook/` folder as `.snxnbk`
files: the notebook and the chapter files it includes. A notebook file is
Typst source with an optional TOML header. Inside it, **annotation blocks**
record notes, calculations, measurements, issues and decisions next to the
design, each a design review record with a typed id (mockup row 198). The
**Annotations** panel (mockup row 165, View ▸ Annotations) lists every
annotation block of every notebook file of the project, and the
`list_annotations` command answers with them. The notebook tab that edits
these files is not built yet; until it is, the files are written with any
text editor.

The reading rules live in the UI-free crate `signex-notebook`
(`crates/signex-notebook`); the panel is `panels/annotations/` and its scan
and actions are `app/handlers/annotations.rs`.

## The file

```text
+++
# TOML front matter: the reference ids the notebook tab keeps.
+++
= Bring-up notes

#issue(id: "ISS-004", title: [U3 runs warm], status: "open",
  owner: "j.berg", date: "2026-11-17", target: component("U3"))[
  The case reaches 71 °C at 300 mA from #net("VBUS").
  Check the copper under #component("U3").
]
```

- **Front matter.** When the first line is `+++`, everything up to the next
  line that is `+++` is TOML front matter. It holds the reference ids (the
  part UUID beside its designator, decision of 2026-11-18) and belongs to the
  notebook tab; the panel skips it. A front matter with no closing `+++` is
  reported, and the whole file is then read as Typst.
- **No sidecar files.** A plain `.typ` file is not read; only `.snxnbk`
  files in `notebook/` and its subfolders are (a chapter in
  `notebook/chapters/` is read too; a link to a folder is not followed).

## The annotation blocks

One Typst function per kind (mockup row 198, decided as drawn):

```text
#note(id: "NOTE-012", title: [..], ...)[ body ]
#calculation(id: "CALC-2.1", ...)[ body ]
#measurement(id: "MEAS-010", ...)
#issue(id: "ISS-004", ...)[ body ]
#decision(id: "DEC-002", ...)
```

| Argument | Meaning |
|---|---|
| `id` | The typed id: the kind's prefix (`NOTE`, `CALC`, `MEAS`, `ISS`, `DEC`), a dash and a number (`MEAS-009`) or dotted numbers within a section (`CALC-2.1`). Reviews and the command line cite the block by it. |
| `title` | The row's title, a string or content (`[Output voltage]`). Without it, the first text line of the body, cut at its first `:`, at most 60 characters. |
| `status` | One of the kind's statuses (below). |
| `date` | The day it was written (an Issue's raised date, a Decision's decided date), `YYYY-MM-DD`. Undated blocks sort last. |
| `target` | The parts, pins and nets the block concerns: one reference or an array, `target: (component("L1"), pin("U3", "3"))`. Listed first among its references. |

The other arguments are the record fields of the kind ("Review records"
below). The body is optional: it follows the closing `)` directly, or is
the one positional content argument. A block inside another block belongs
to the outer one's text.

The earlier one-function form is still read and still prints:

```text
#annotation(kind: "<kind>", status: "<status>", title: "<title>", date: "YYYY-MM-DD")[ body ]
```

Its `kind` is required, every value is a quoted string, the body is
required, and it has no id and no record fields.

### Statuses

| Kind | Statuses (token: shown as) |
|---|---|
| Note | none |
| Calculation | `current`: Up to date, `stale`: Out of date |
| Measurement | `planned`: Planned, `done`: Done |
| Issue | `open`: Open, `resolved`: Resolved |
| Decision | `proposed`: Proposed, `accepted`: Accepted, `superseded`: Superseded |

### References

The body names the design with three Typst calls:

- `#component("U3")`: a part, by designator. `#component("R1").value` and
  `#component("R3").param("Vref")` read its parameters and still name R1 and
  R3.
- `#net("VBUS")`: a net, by name.
- `#pin("U1", "3")`: a pin of a part, read as `U1.3`. One dotted name,
  `#pin("U1.3")`, names the same pin (cut at its first dot); the editor
  writes the two-argument form.

Each reference is listed once per block, first mention first. A reference
whose arguments are not the quoted names it takes is reported and left out.

## Review records

Each block prints like a design review record (mockup row 198): the kind
label and the id, the title, a status badge, a field grid, the body and a
dashed footer line. The kind colour is in the left rule and the label
only, so a page of blocks reads in print; a block never splits across a
page break. The fields each kind takes (`signex_notebook::Record`):

| Kind | Fields |
|---|---|
| Note | `author` |
| Calculation | `target: (3.30, 2%)` (the nominal and its tolerance, a percentage or an absolute value) or `limit: (min: .., max: ..)`; `result`, `unit`, `requirement`, `inputs` (the block ids it reads) |
| Measurement | `rows: ((net("VBUS"), 5.00, 5%, 4.78), ...)` (what, nominal, tolerance, measured; or what, `(max: ..)`, measured), or one row of `target`, `value` and `limit`; `unit`, `conditions`, `instrument`, `operator`, `requirement` |
| Issue | `owner`, `severity` (`minor`, `major` or `critical`), `due` (a revision), `from` (the block that raised it), `requirement`, `closes` (what closes it, the footer) |
| Decision | `owner`, `review`, `requirement`, `chosen`, `alternatives: ((option, why it was dropped), ...)`, `supersedes`, `approver` |

`requirement` is one id or an array of them. A field the kind does not
take, or a value it cannot use, is reported and ignored.

**Verdict.** A Measurement row, and a Calculation result with a target
band or limit, is Pass inside its limits, Warn inside them but under 1 %
of a limit from it, and Fail outside (mockup row 60b): VBUS 4.78 V against
5.00 V ±5 % is Warn. A Measurement's verdict is the worst of its rows; the
printed header shows it, the table each row's Target, Tol., Measured,
Error and Result. The printed rule is the package's `verdict`, kept equal
to `signex_notebook::verdict_of` by a test.

**Citations.** An Issue's `from`, a Decision's `supersedes` and a
Calculation's `inputs` name other blocks by id, in any file of the
notebook. Read the other way, a Measurement "opens" the Issues raised from
it (the printed footer "opens ISS-005"), and a Decision is superseded by
the one that names it.

**A failing Measurement opens an Issue** (mockup row 198, decided as
drawn). For a Measurement with an id and a Fail verdict that no Issue is
raised from, `signex_notebook::open_failing_measurements` writes an
`#issue` right after it: the next free `ISS-` id in the Measurement's own
section (`MEAS-3.2` opens `ISS-3.1`), `status: "open"`, today's date,
`from:` the Measurement and `target:` its failing rows. It does nothing
once the Issue is there. The notebook tab runs it when it is built.

**New blocks.** `signex_notebook::new_block_source` is what the
Calculation, Measurement and Block buttons insert: the next free id of the
kind (`next_block_id`), the kind's fields left blank and dated today.

## How a notebook is read

The scan walks the Typst source with just enough syntax to match brackets
correctly:

- Markup: `\` escapes the next character; `//` starts a line comment
  (except right after `http:` or `https:`); `/* */` comments nest;
  backticks enclose raw text; `$ ... $` is an equation; `[ ]` nest.
- After `#`, a name followed (with no space) by `( )` arguments, `[ ]`
  content and `.field` or `.method( )` steps is one call. Inside `( )` and
  `{ }` strings, comments, raw text and nested brackets are skipped, so a
  `]` in a string does not close a block.
- A block inside another block belongs to the outer one's text.

The **snippet** of a row is the block's body as plain text: comments,
markup markers (`*`, heading `=`, list `-` and `+`) and the equation `$`
are dropped, strings in an equation lose their quotes, and references read as
their names. It is cut at 200 characters.

### Problems

Nothing is passed over silently. Each problem becomes one line in the
Messages panel (source `SIGNEX-NOTEBOOK`) that starts with the file and the
line:

| Problem | What happens |
|---|---|
| Unknown or missing `kind` | The block is skipped. |
| A `kind` that is not a quoted string | The block is skipped. |
| No `[ ]` body right after the arguments of `#annotation` | The block is skipped. |
| An `id` that is not a typed id of the block's kind (`DEC-002` on a `#note`) | The block is listed without an id. |
| An `id` an earlier block of the file has | The block is listed with it. |
| A record field whose value the kind cannot use (a `severity` that is not minor, major or critical, a `from` that is not a block id) | The field is ignored. |
| A bracket, string, raw text, equation or comment that never closes inside a block | The block and the rest of the file are skipped. |
| The same outside any block | The rest of the file is not read. |
| A `status` the kind does not have | The block is listed without a status. |
| A `date` that is not `YYYY-MM-DD` | The block is listed undated. |
| An unknown argument, or a value that is not a quoted string | The argument is ignored. |
| A malformed reference | The reference is left out. |
| A file that cannot be read, or is not UTF-8 | The file has no rows. |

## What the panel reads

- **Scope:** the whole project of the active document (the drawn choice of
  the mock-up), every `.snxnbk` file in `<project>/notebook/` and its
  subfolders (the chapters a notebook includes, mockup row 195), sorted by
  path.
- **When:** the files are read off the UI thread when the panel is shown,
  when the project in view changes (a project is opened, or a switch to
  another project and back), and on the Refresh button. The project-change
  read runs with the panel closed too, since the schematic's annotation
  flags come from it, except for a project whose Close Project is still
  resolving its sheets. While a new read runs, the last list stays shown.
- **Parts:** the same read assembles the project's sheets (open sheets with
  their unsaved edits, the others from disk) and lists every designator and
  the sheet holding it. A `#component` or `#pin` whose part is not in the
  design is drawn amber, with the tooltip "Part deleted from the design".
  When a sheet of the design cannot be read or is missing, no part is called
  gone.

## The panel

- **Tools:** a search field (title, typed id, snippet and reference names,
  any case),
  Refresh, one chip per kind with its count over all annotations, and the
  Part, Status and Date fields. Part lists the parts the annotations name;
  Status lists the statuses they carry; Date offers Any date, Last 7 days
  (today and the six days before) and Last 30 days. A field with a value
  shows it with an x that clears it. New annotation (+) ends the row of
  fields; it inserts a note annotation into the open notebook and is greyed
  without one (decision 2026-11-24). No notebook tab exists yet, so it is
  always greyed, with the tooltip "New annotation · open a notebook first".
- **List:** one header per file (its path and how many rows it shows), then
  its rows newest first, undated last. A row shows the kind's icon box, the
  typed id in mono and the title, the snippet, the reference chips, the
  status pill, the Pass / Warn / Fail pill of a Calculation or Measurement
  and the date (`MM-DD`).
- **A row click** expands it: its review record (the fields of its kind,
  such as Owner, Review, Chosen, Severity, Due, From, Instrument, Operator,
  a Measurement's "2 pass 1 warn", and the blocks citing it: Opens,
  Superseded by, Read by), its file and line, the block as written, and a
  Jump button to its first part. Hovering a row (or expanding it) shows the
  Jump to schematic button.
- **A part chip** opens the sheet that holds the part, centres and zooms the
  view on it and selects it. **A net chip** highlights the net on every sheet and goes
  to its first member, as the Net Inspector does; that needs a sheet of the
  project in view.
- **Footer:** "N of M annotations" and the number of notebook files.
- **No notebook:** with no `notebook/` folder, or no `.snxnbk` file in it,
  the panel shows "No notebook in this project" at the top.

Kind colours: Note the secondary ink, Calculation the info blue,
Measurement the success green, Issue the error red, Decision purple (Light
`#7A3EA8`, Dark `#D2A8F0`); `signex_widgets::theme_ext::annotation_kind_colour`.

## Annotation flags

A part that annotations name carries flags on the schematic canvas (drawn
on by default, the mock-up's choice):

- One flag per kind, at the part's top-right corner, side by side in kind
  order (Note, Calculation, Measurement, Issue, Decision). A flag is a pole
  with a notched pennant in the kind's colour; the pennant holds how many
  blocks of that kind name the part. A block naming the part twice
  (`#component("U1")` and `#pin("U1", "3")`) counts once. Nets and power
  ports carry no flag.
- A part is matched by the designator the canvas draws for it, so a sheet
  reused under several placements flags each placement's own parts.
- The flags come from the last read, which runs whether or not the
  Annotations panel is open, and show only on the sheets of the project that
  read was of. A notebook file edited outside Signex shows once the project
  is read again (the panel shown, a project switch, or Refresh). While a read of another project runs, and
  with no project in view, there are none.
- They are sized from the caption text and keep their size on screen at any
  zoom. They are drawn in the selection layer, so they are editor-only and
  never reach a print or an export.

## The `list_annotations` command

`list_annotations document=<project or one of its documents> [id=DEC-002]
[kind=decision] [part=U3]` reads the same files off the UI thread, with no
part lookup and no Messages line, and answers with JSON:

- `files`: per file (`notebook/chapters/power.snxnbk`) its `blocks` the
  operands keep, its `problems` by name (`{"line": 3, "text": ...,
  "problem": "bad_id", "kind": "note", "id": "DEC-9"}`) and its
  `read_error` (`{"io": ...}` or `"not_utf8"`);
- each block: `id`, `kind`, `title`, `status`, `date`, `line`,
  `references`, `snippet`, `record` (the fields above, a Measurement's
  `rows` with `limits`, `error_percent` and `verdict`) and `cited_by`
  (`[{"id": "ISS-005", "citation": "from"}]`);
- `links`: every citation (`by`, `to`, `citation`, `resolved`);
- `unopened_failures`: the failing Measurements no Issue is raised from;
- `count`, `notebook_found`, `unlisted` and `project`.

A bad `id` or `kind` is refused before anything is read.

## Not built yet

These parts of the mock-up wait for the notebook tab and are tracked as
their own issues: Open in notebook, New Notebook and Import .typ… in the
empty state, the New annotation action itself (the button is there, greyed),
the Issue a failing Measurement opens written into the open notebook,
calculation results computed from part parameters, and the flag hover card
("U3 · 3 annotations"). The View ▸ Annotation Flags switch of the mock-up is a
proposal and is not built. The header More button is left out until its
items are decided (decision 2026-11-24).
