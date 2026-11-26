# Design notebook generation

Generate design notebook (#2945, mockup row 201) assembles a design report
from what the project already knows. It is one catalog command,
`generate_design_notebook`, reached from command search (the options
dialog), its operands (`signex-cli run`, an agent) and the command line
(`signex-cli notebook generate`); all of them run the same start. Command
reference: [CLI.md](CLI.md), "Design notebook generation". Decisions:
[DECISIONS.md](DECISIONS.md), the 2026-11-25 Generate rows.

## Where the report goes

The report is its own notebook file, `notebook/generated/design-report.snxnbk`
(`snxnbk/1`, an empty header). Regenerate replaces it as a whole; the
hand-written `notebook/design.snxnbk` and its chapters are read, never
written. In the app the report is held unsaved, its path among the unsaved
documents, so Save All and the close and quit prompts list it; their Save
writes it (replacing the report made before) and Discard drops it. With `save=true` (always, from `signex-cli notebook generate`) the run
writes it, and refuses when one is there unless `update=true` (`--force`).

## How it is built

The domain is `signex_notebook::generate`; the app only gathers.

1. In `update`, the slice (`app/notebook_generate/`) captures the project
   file's facts (sheets, revision, variant, requirements), the three check
   jobs (or the newest kept reports, for "use the last results"), the BOM
   inputs and the open board.
2. Off the UI thread, `job::run` runs the checks, builds the BOM, reads the
   notebook and its chapters (within the project folder in a headless run or
   for an agent), and calls `generate`.
3. `generate` writes the Typst body section by section, compiles it with the
   bundled package and fonts only to count the pages, and returns the file
   text and the summary. Every piece of project text goes in as a quoted
   Typst string, so a name with `#`, `*` or `]` prints as written.
4. Back in `update`, the check runs are numbered and kept as the newest of
   their check, then the command call is answered (or the dialog shows the
   result).

Each source is a `Fact`: read, with where it came from, or not read, with why.
A section whose source was not read says so in the report and in the
answer's `not_read`; nothing is filled in.

## How each requirement was met

A block meets a requirement when its title or text names the requirement's id
as a whole word (`REQ-001`, not `REQ-0012`). The measurements that name it are
its evidence, each with its value and unit; with none, the calculations that
name it are. The parts those blocks reference, then the other calculations and
decisions that name it, are what meets it. The result is the newest
requirement run's: a `req::<id>` finding fails it, its rule in `passed`
passes it, anything else is Not checked.

## Not read yet

Signex keeps no record of simulation runs, output runs or the project version
history yet, so those sections say they could not be read. Power nets are not
gathered yet, so the architecture figure is the sheet list.
