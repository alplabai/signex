# Design Notebook annotations and the Annotations panel

A project's Design Notebook lives in its `notebook/` folder as `.snxnbk`
files. A notebook file is Typst source with an optional TOML header. Inside
it, **annotation blocks** record notes, calculations, measurements, issues
and decisions next to the design. The **Annotations** panel (mockup row 165, View ▸
Annotations) lists every annotation block of every notebook file of the
project. The notebook tab that edits these files is not built yet; until it
is, the files are written with any text editor.

The reading rules live in the UI-free crate `signex-notebook`
(`crates/signex-notebook`); the panel is `panels/annotations/` and its scan
and actions are `app/handlers/annotations.rs`.

## The file

```text
+++
# TOML front matter: the reference ids the notebook tab keeps.
+++
= Bring-up notes

#annotation(kind: "issue", status: "open", date: "2026-11-17")[
  U3 runs warm: case reaches 71 °C at 300 mA from #net("VBUS").
  Check the copper under #component("U3").
]
```

- **Front matter.** When the first line is `+++`, everything up to the next
  line that is `+++` is TOML front matter. It holds the reference ids (the
  part UUID beside its designator, decision of 2026-11-18) and belongs to the
  notebook tab; the panel skips it. A front matter with no closing `+++` is
  reported, and the whole file is then read as Typst.
- **No sidecar files.** A plain `.typ` file is not read; only `.snxnbk`
  files directly in `notebook/` are (not in subfolders).

## The annotation block

```text
#annotation(kind: "<kind>", status: "<status>", title: "<title>", date: "YYYY-MM-DD")[ body ]
```

| Argument | Required | Meaning |
|---|---|---|
| `kind` | yes | `note`, `calculation`, `measurement`, `issue` or `decision`. |
| `status` | no | One of the kind's statuses (below). |
| `title` | no | The row's title. Without it, the first text line of the body, cut at its first `:`, at most 60 characters. |
| `date` | no | The day it was written, `YYYY-MM-DD`. Undated blocks sort last. |

Every argument value is a quoted string. The body follows the closing `)`
directly, with no space, as Typst requires for a content block.

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
| No `[ ]` body right after the arguments | The block is skipped. |
| A bracket, string, raw text, equation or comment that never closes inside a block | The block and the rest of the file are skipped. |
| The same outside any block | The rest of the file is not read. |
| A `status` the kind does not have | The block is listed without a status. |
| A `date` that is not `YYYY-MM-DD` | The block is listed undated. |
| An unknown argument, or a value that is not a quoted string | The argument is ignored. |
| A malformed reference | The reference is left out. |
| A file that cannot be read, or is not UTF-8 | The file has no rows. |

## What the panel reads

- **Scope:** the whole project of the active document (the drawn choice of
  the mock-up), every `.snxnbk` file in `<project>/notebook/`, sorted by file
  name.
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

- **Tools:** a search field (title, snippet and reference names, any case),
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
  title, the snippet, the reference chips, the status pill and the date
  (`MM-DD`).
- **A row click** expands it: its file and line, the block as written, and
  a Jump button to its first part. Hovering a row (or expanding it) shows the
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

## Notebook badges

A part or net the notebook writes about carries a Notebook badge on the
schematic canvas (mockup row 197; the annotation flags of mockup row 165,
renamed). The badges are on by default; View ▸ Notebook Badges (B in the
Signex Standard profile) hides or shows them on every sheet.

- One badge per part, at its top-right corner, and one on each label of a
  net. A badge is a pole with a notched pennant that holds how many blocks
  name the part (or one of its pins) or the net. A block naming the part
  twice (`#component("U1")` and `#pin("U1", "3")`) counts once. Power
  ports carry none.
- The colour is the worst status of those blocks: red for a failing
  measurement, else amber for an open issue, else green when there are
  measurements and every one passes, else blue (notes and calculations
  only; a planned measurement keeps it blue). The blocks are read without a
  measurement result today, so no badge is red yet.
- Hovering a badge shows its card: the part or net with the block count,
  what the colour says, and one line per block (kind, title, status,
  date). Clicking a badge with the Select tool opens the notebook at its
  first block.
- A part is matched by the designator the canvas draws for it, so a sheet
  reused under several placements badges each placement's own parts.
- The badges come from the last read, which runs whether or not the
  Annotations panel is open, and show only on the sheets of the project that
  read was of. A notebook file edited outside Signex shows once the project
  is read again (the panel shown, a project switch, or Refresh). While a
  read of another project runs, and with no project in view, there are
  none.
- They are sized from the caption text and keep their size on screen at any
  zoom. They are drawn in the selection layer, so they are editor-only and
  never reach a print or an export.

**Properties ▸ Design notes** lists the blocks of the selected part's badge,
each with its kind, title, status and date, and Open in notebook; a row
opens the notebook at its block.

## References

The same read resolves every reference of every notebook file against the
project (`signex_notebook::resolve_references`), once every sheet of the
design reads; with a sheet missing no reference is checked.

- A part reference is found by the id the file's header stored for it, then
  by its name. A pin needs its part and the pin (by number, then by name).
  A net is found by name; it has no stored id.
- A reference that no longer reads as the design does is **stale**, for one
  of three reasons, each with its fixes: **renamed** (the stored id finds
  the part under another designator: Update to the new name), **not in the
  design** (Remove reference, Keep as plain text), **no such pin on the
  part** (Pick another pin). Update all lists every stale reference and
  updates the renamed ones; the others wait for a choice. A rename made
  outside Annotate leaves the text stale until it is updated (decided as
  drawn, mockup row 196).
- A fix rewrites every use in the file and keeps the stored id under the
  new name. Remove reference refuses a use inside code (a block's
  `target:`), where Keep as plain text writes the name as a string; both
  refuse a use read as a value (`#component("R7").value`). A fix edits the
  notebook tab's text and is written on Save, like any notebook edit.
- A reference's **hover card** lists, for a part, its value, footprint,
  sheet, nets, variants and the blocks that use it; for a pin, its name,
  net, connected pins and last measurement; for a net, its parts, sheets and
  last measurement; for a stale reference, why and its fixes. Its buttons:
  Cross-probe, Open sheet, Timeline for a part; Cross-probe, Timeline, Copy
  reference for a pin; Cross-probe, Highlight net, Timeline for a net.
- Typing `@` in the editor opens the **reference picker**: the parts, then
  the nets, whose name starts with what follows the `@`, then those that
  contain it; `U3.` lists the pins of U3. Picking one inserts the Typst
  call; the `@` is never written.

## Commands

Each names the project with `document` (the project or one of its
documents), reads the project's notebook and design off the update thread,
then answers from that read as JSON.

| Command | Operands | Answers |
|---|---|---|
| `notebook_references` | `file?` | Each file's references: kind, name, state (`resolved`, `renamed` with `now`, `deleted`, `missing_pin` with `pins`), stored id, lines, uses and fixes; the totals and what Update all would change. |
| `notebook_reference_card` | `kind` (`component`, `pin`, `net`), `name` (`R7`, `U3.3`, `+3V3`), `file?` | The hover card: facts by kind, the blocks that use it, the last measurement, the actions. |
| `notebook_reference_picker` | `text?`, `limit?` | What `@` and `text` list, each with the call it inserts. |
| `notebook_badges` | `part?` or `net?` | Every badge with its tone and blocks, or the one badge of a part (Properties ▸ Design notes) or net; whether the badges are shown. |
| `notebook_fix_reference` | `fix` (`update`, `remove`, `keep_text`, `pick_pin`, `update_all`), `kind?`, `name?`, `to?`, `pin?`, `file?` | The file and how many uses changed; refused while the notebook tab does not hold the file. |

`toggle_notebook_badges` is View ▸ Notebook Badges; it works in a window
only.

## Not built yet

These parts of the mock-up wait for the notebook tab and are tracked as
their own issues: Open in notebook (the Properties button and a badge click
say in Messages where they would open), New Notebook and Import .typ… in
the empty state, the New annotation action itself (the button is there,
greyed), calculation results computed from part parameters, the reference
style, the margin-lane placement of the card and the `@` picker in the
editor (their content and views are built), Back to notebook and the
notebook's own cross-probe strip. A badge does not yet move to a free
corner when it would cover a pin or a label. The header More button is left
out until its items are decided (decision 2026-11-24).
