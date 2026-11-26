# Export Jobs and PNG pages

An Export Job is a `.snxout` file in a project's folder that lists the
outputs one run writes: a schematic PDF, the pages as PNG images, a bill of
materials and a netlist (#1299; mockup rows 167 and 193b). Running the job
writes every enabled output, or the one output it is asked for, into the
job's folder. The UI calls the document "Export Job" (DECISIONS 2026-11-19);
the file kind and the command ids keep the name `output_job`.

The Export Job tab of mockup row 114 (profiles, automation, the run flow and
the run history) is not built yet. Until it is, a job file is written by
hand or by a script, and run from the command palette or the CLI.

## The file

TOML under the format token `snxout/1` (`docs/FORMAT_VERSIONS.md`). Keys this
build does not know are kept when the file is read and written again, at the
top level and in each output.

```toml
format = "snxout/1"
folder = "out"          # beside the job file; the default
variant = "Lite"        # every output shows this variant; default: the project's active one

[[outputs]]
name = "Schematic"      # unique in the job; a run can name one output by it
kind = "schematic_pdf"
enabled = true          # off: no run writes it
file = "dcdc-buck.pdf"  # default: the kind's name, below

[[outputs]]
name = "Pages"
kind = "schematic_png"
dpi = 150               # 36 to 1200; default 300
```

| `kind` | Writes | Default file |
|---|---|---|
| `schematic_pdf` | every page of the project as one PDF | `<project>.pdf` |
| `schematic_png` | every page as its own PNG, `<file stem>-<n>.png` in print order | `<project>.png` gives `<project>-1.png`, ... |
| `bom` | the bill of materials; the file's extension picks CSV, HTML or XLSX | `<project>-bom.csv` |
| `netlist` | the `.NET` netlist | `<project>.NET` |

`<project>` is the `.snxprj` file's name without its extension.

A job cannot run (and is not saved) when its `folder` leaves the job's folder
(an absolute path, a drive, `..`, an empty step) or a `file` is not a plain
file name, when two outputs share a name or a file (names compare exactly,
files without case), or when a `dpi` is out of range. An output of a `kind`
this build does not know is kept in the file; a run of every output leaves it
out and says so, and a run of that output alone is refused.

## A run

- The job must be in the folder of an open project, and a schematic of that
  project must be open (the root sheet when it is, else the first open one).
- The outputs come from the same scope as the PDF, BOM and netlist dialogs:
  every page of the project, the BOM with the library prices and the board's
  left-out parts, the netlist from the one project derivation.
- The pages print on the first page's paper in the default print palette,
  not the window's theme, so two runs on the same project write the same
  bytes on any machine. The test that pins this writes a CSV BOM; pick CSV
  or HTML where the bytes must repeat, as a spreadsheet file may carry the
  time it was made.
- A netlist with a hole in it (a missing or unreadable sheet, a placement
  that shares designators) stops the run, as the netlist dialog refuses it
  by default.
- Every output is built before anything is written. When one cannot be
  built, nothing is written and the run says why. The files are written
  crash-safely, and the folder must resolve, links followed, inside the
  project folder.
- A run started by a person ends in the Messages panel (what was written
  where, each warning) and, when it wrote nothing, the notice card. A run
  started by the CLI or an agent is answered when it ends.

## Commands

| Id | Without operands | With operands |
|---|---|---|
| `run_output_job` | picks a `.snxout` in the active project's folder, then runs it | `path=<job.snxout> [name=<output>]` |
| `export_sheet_png` | picks a folder for the pages of the schematic in view, at 300 dpi | `document=<sheet> path=<folder> [dpi=<n>]` |

Both are in the `reports` category, so a CLI run needs `--allow export`.
Both answer, once the files are written, with:

```json
{"job": "/p/fab.snxout", "folder": "/p/out", "variant": "Base",
 "outputs": [{"name": "Schematic", "files": ["dcdc-buck.pdf"]}],
 "skipped": [], "warnings": []}
```

`job` is empty for `export_sheet_png`, whose one output is named `PNG`.
`export_sheet_png` writes the pages of the sheet's project (or of the open
loose sheets) in the project's active variant into the folder it names.

```sh
signex-cli run dcdc-buck.snxprj --allow export <<'EOF'
{"id": "open_document", "args": {"path": "dcdc-buck.snxsch"}}
{"id": "run_output_job", "args": {"path": "fab.snxout"}}
EOF
```

A page larger than 4096 pixels a side at the asked resolution is drawn at a
lower one, and the run says so in its warnings.
