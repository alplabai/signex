# Export Jobs and PNG pages

An Export Job is a `.snxout` file in a project's folder that lists the
outputs one run writes: schematic PDF and PNG pages, the bill of materials
and the netlist, and the board's fabrication files (Gerber, NC drill, pick
and place, IPC-D-356, IPC-2581, ODB++), its STEP model and its assembly
drawings (#1299; mockup rows 114, 167 and 193b). Run profiles pick the
outputs, the one variant and where the files go; a run goes Validate,
Generate, Package, Summary, and the runs from the window are kept in the
file's history. The UI calls the document "Export Job" (DECISIONS
2026-11-19); the file kind and the command ids keep the name `output_job`.

## The file

TOML under the format token `snxout/1` (`docs/FORMAT_VERSIONS.md`). Keys this
build does not know are kept when the file is read and written again: at the
top level, in each output, profile, profile's triggers and history row.

```toml
format = "snxout/1"
folder = "out"                    # beside the job file; the default
variant = "Lite"                  # with no profile; default: the project's active one
default_profile = "Production release"

[[outputs]]
name = "Schematic"                # unique in the job; a run can name one output by it
kind = "schematic_pdf"
enabled = true                    # off: no run writes it, and no profile can tick it
file = "dcdc-buck.pdf"            # default: the kind's name, below

[[outputs]]
name = "Gerber"
kind = "gerber"                   # a board output: `file` is the stem its files start with
gerber_units = "inch"             # the Gerber settings, each left out for the writer's own (below)
gerber_format = "2:5"

[[outputs]]
name = "ODB++"
kind = "odb"
odb_folder = true                 # the job as its folder tree, not one archive

[[profiles]]
name = "Production release"
outputs = ["Schematic", "Gerber"] # the ticked outputs
variant = "Base"                  # the one variant the profile builds
folder = "out/release"            # default: the job's folder
validate = true                   # ERC, DRC and the BOM checks first
on_errors = "stop"                # stop | ask
on_warnings = "continue"          # continue | ask | stop
zip = true                        # a zip of the run beside its folder
group_folders = true              # Fabrication/, Assembly/, Documentation/, Reports/
release_folder = true             # <stem>-rev<R>-<YYYY-MM-DD>/ in the folder
clear_folder = false              # delete the files the run did not write
naming = "{project}-{rev}"        # the stem of the default file names

[profiles.triggers]
on_save = true                    # run when a watched file is saved
sources = ["dcdc-buck.snxpcb", "*.snxsch"]  # empty: the board and every schematic
wait_seconds = 30                 # after the last save; 0 to 3600
keep_up_to_date = false           # run only the outputs made from the saved file
before_close = "off"              # off | ask | always
on_tag = "v*"                     # kept; no release or tag event exists yet

[[history]]                       # newest first, at most 20
started = "2026-11-18 14:41:08"
profile = "Production release"
variant = "Base"
trigger = "manual"                # manual | selected | on_save | keep_up_to_date | before_close
result = "complete"               # complete | stopped
outputs = 11
files = 22
warnings = 3
errors = 0
seconds = 18
folder = "out/release/dcdc-buck-revB-2026-11-18"
zip = "dcdc-buck-revB-2026-11-18.zip"
viewable = ["Fabrication/dcdc-buck-copper-l1-top.gbr"]
```

| `kind` | Group | Writes | Default file |
|---|---|---|---|
| `schematic_pdf` | Documentation | every page of the project as one PDF | `<stem>.pdf` |
| `schematic_png` | Documentation | every page as its own PNG, `<file stem>-<n>.png` in print order | `<stem>.png` gives `<stem>-1.png`, ... |
| `bom` | Reports | the bill of materials; the file's extension picks CSV, HTML or XLSX | `<stem>-bom.csv` |
| `netlist` | Reports | the `.NET` netlist | `<stem>.NET` |
| `gerber` | Fabrication | the Gerber X2 layers and their job file | files start `<stem>` |
| `nc_drill` | Fabrication | the drill files | files start `<stem>` |
| `test_points` | Fabrication | the IPC-D-356 test netlist | files start `<stem>` |
| `ipc2581` | Fabrication | the IPC-2581 file | files start `<stem>` |
| `odb` | Fabrication | the ODB++ job, one archive or (`odb_folder`) its folder tree | files start `<stem>` |
| `pick_and_place` | Assembly | the pick-and-place file | files start `<stem>` |
| `assembly_pdf` | Assembly | both sides' assembly drawings as one PDF | `<stem>-assembly.pdf` |
| `step` | Documentation | the board and its parts as a STEP model | `<stem>-board.step` |

`<stem>` is the `.snxprj` file's name without its extension, or the profile's
`naming` pattern with `{project}`, `{rev}` (the project's `rev` parameter)
and `{variant}` filled in.

A job cannot run (and is not saved) when a `folder` leaves the job's folder
(an absolute path, a drive, `..`, an empty step) or a `file` is not a plain
file name, when two outputs share a name or a file (names compare exactly,
files without case; two board outputs of one kind share a stem), when a
`dpi` is out of range, when a `gerber_layers` list is empty or holds a word
that is not a layer id, when two profiles share a name (without case), when
`default_profile` names no profile, or when a wait is longer than an hour.
An output of a `kind` this build does not know is kept in the file; a run of
every output leaves it out and says so, and a run of that output alone is
refused.

## Gerber settings

A Gerber output takes five settings (#1299). Each one left out of the file
is the Fabrication Outputs writer's own, and the defaults write the bytes
that writer writes, so a job saved before the settings existed writes the
same files. The Export Job tab's form of a Gerber output shows a field for
each; picking a default stores the setting as left out.

| Key | Values | Default |
|---|---|---|
| `gerber_units` | `mm`, `inch` (`%MOMM`, `%MOIN`) | `mm` |
| `gerber_format` | integer and decimal digits, `I:D`, integer 1 to 6, decimal 4 to 6 (`%FSLAXIDYID`) | `4:6` |
| `gerber_zeros` | `leading` (leading zeros left out, a plain integer), `trailing` (`%FST`, trailing zeros left out, every integer digit written), `none` (every digit written) | `leading` |
| `gerber_layers` | a list of layer ids; left out, every layer | every layer |
| `gerber_apertures` | `standard` (a standard circle, rectangle or obround where a pad has one, a macro for the rest), `macros` (every aperture a macro) | `standard` |

A layer id is the file's suffix without the copper layer's number:
`copper-top`, `copper-bottom`, `copper-inner2` (and on), `legend-top`,
`legend-bottom`, `soldermask-top`, `soldermask-bottom`, `paste-top`,
`paste-bottom`, `profile` and `vscore`; `copper` takes every copper layer.
An id the board has no image for writes nothing. The drill files and the
other board outputs do not follow these settings. In inches a coordinate is
rounded to the format's last digit and an aperture to a millionth of an
inch; the job file (`.gbrjob`) lists the images written. With `trailing`
and a board wider than the format's integer digits hold, the integer part
is written in full. A `gerber_units`, `gerber_format`, `gerber_zeros` or
`gerber_apertures` value outside the table makes the job file unreadable:
it is refused with the reason and never written over.

## Profiles

A run runs the profile it names, else the job's default profile. A job with
no profile runs every enabled output into its `folder`, in its `variant`,
with no Validate and no Package options, as the first pass did. Run Selected
runs the shown profile's ticked outputs without Validate and without the zip.
A run of one output by name ignores the ticks.

The variant is the profile's; every output follows it. The board writer
takes no variant yet, so for a variant other than the base one the board
outputs that list parts (pick and place, the assembly PDF, IPC-2581, ODB++,
STEP) are refused and the run writes nothing; Gerber, drill and IPC-D-356
are variant-free.

## Runs

- The job must be in the folder of an open project, and a schematic of that
  project must be open (the root sheet when it is, else the first open one).
  A board output needs the project's board (`pcb_file`) open; it is made
  from a copy of the board as it is in memory.
- One run of a job goes at a time.
- **Validate** (a profile with `validate`): ERC over the project, DRC over
  its board when the run makes a board output, and the BOM checks of a BOM
  output, run off the UI thread with the checks `check_erc` and `check_drc`
  run. Errors: `stop` stops the run; `ask` asks the person (Continue anyway
  or Stop). Warnings: `continue`, `ask` or `stop`. A run nobody is there to
  ask (a trigger, the CLI, an agent) stops where it would ask.
- **Generate**: each output is made off the UI thread in turn, so the flow
  lights node by node. Nothing is written yet. An output that cannot be made
  stops the run.
- **Package**: every file is written crash-safely, into the release folder
  when the profile has one (`-2`, `-3`, ... when a run the same day made one
  already), in group subfolders when asked; the folder must resolve, links
  followed, inside the project folder. Clear the folder then deletes the
  files there (and one level down) that the run did not write; it never
  deletes a Signex document (`.snx*`) and does nothing in the job's own
  folder or the project folder. The zip holds the same files under the same
  names, with a fixed date, beside the folder.
- **Summary**: the tab shows outputs, files, warnings, errors and time, where
  the files went, Open folder, Copy path and View in Gerber Viewer. A run
  from the window (a person's, or a trigger's) is kept in the job's history;
  a run started by the CLI or an agent leaves the file as it is and is
  answered when it ends.
- Stop ends the run going now; nothing of it is written. A package already
  writing finishes.
- The pages print on the first page's paper in the default print palette, so
  two runs on the same project write the same bytes on any machine. A
  netlist with a hole in it stops the run.

## The Export Job tab

`View ▸ Panels ▸ Export Jobs` (the panel list row, or `open_output_job`)
opens the project's `.snxout` in a document tab (the first by name), or
focuses it; with none, it opens a new unsaved one named after the project.
A double-click on a `.snxout` row in Projects opens it. The tab is titled
`<file> — Export Job` (mockup row 167) and is a document: an edit marks it
unsaved (the dot, Save All and the quit gate see it), Ctrl+S writes it, and a
closed tab with unsaved edits keeps them for Save All. Run profile and Run
Selected on a job with unsaved edits save it first and run what was saved.

- **Settings**: the outputs by group, each with its tick (in the shown
  profile; disabled while the output is switched off) and its Enabled
  switch; Add Output by group; the picked output's form (file or stem, PNG
  resolution, the ODB++ folder form, what the writer fixes, Remove); the
  profile's container and pipeline; the last run.
- **Toolbar**: the profile (its popover: pick, Save as profile…, Rename…,
  Duplicate, Delete…, Set as default) with an Auto badge, the one variant,
  the pages, the outputs chip (generating, out of date, stopped, up to date),
  Run Selected, Run profile and Stop.
- **Automation**: the triggers below, the command line for CI with Copy, and
  the recent auto runs.
- **Run**: the run as a flow (Validate, the outputs by group, Package), each
  node lit by its state and its log on a click; the question Validate asks,
  or the stopped banner with Show the findings, Run again and Ask instead of
  stopping next time.
- **Summary**: as above, with the Outputs card (a View link on the rows the
  Gerber Viewer reads) and the history; a history row opens its summary.

## Automation

- **Run on save**: when a file the profile watches is saved, the profile runs
  after its wait from the saved `.snxout`; a newer save restarts the wait. A
  run of the job going then waits a few seconds more.
- **Keep outputs up to date**: a board save runs the profile's ticked board
  outputs, a schematic save its schematic ones.
- **Before closing the project**: when ticked outputs are out of date (a
  source was saved since their last run this session), `always` starts the
  run before the project closes, from what it captures then; `ask` names them
  in a notice and the close goes on.
- **Release or git tag**: the pattern is kept and shown; Signex makes no
  release or tag yet, so nothing starts it.

Auto runs work in the background and never ask; a headless run starts none.

## View in Gerber Viewer

Mockup row 193b: the summary's View in Gerber Viewer (and `view_output_job_run`)
opens exactly the files the run wrote that the viewer reads, with a banner
naming the run (profile, variant, time), Back to run summary and Close.
Opening the same run again focuses the tab. Files moved since leave the
viewer empty with a line saying where they were. A stopped run wrote
nothing, so it has none to view.

## Commands

| Id | Without operands | With operands |
|---|---|---|
| `run_output_job` | picks a `.snxout` in the active project's folder, then runs it | `path=<job.snxout> [name=<output>] [profile=<p>] [selected=true]` |
| `open_output_job` | the project's job in its tab (View ▸ Panels ▸ Export Jobs) | `path=<job.snxout>` |
| `new_output_job` | a new unsaved job in its tab | `path=<job.snxout>`: writes an empty job; refused over a file |
| `read_output_job` | — | `path=`: the job as JSON, `unsaved`, and this session's `run` |
| `add_job_output` | — | `path= kind= [name=] [profile=]` |
| `set_job_output` | — | `path= name= [enabled=] [file=] [dpi=] [odb_folder=] [remove=]` |
| `set_job_profile` | — | `path= profile= [from=] [to=] [remove=] [default=] [variant=] [outputs=a,b] [validate=] [on_errors=] [on_warnings=] [folder=] [naming=] [zip=] [group_folders=] [release_folder=] [clear_folder=]` |
| `set_job_triggers` | — | `path= profile= [on_save=] [sources=a,b] [wait=] [keep_up_to_date=] [before_close=] [tag=]` |
| `stop_output_job` | — | `path=` |
| `view_output_job_run` | — | `path= [run=<history row>]` |
| `set_job_gerber_units` | — | `path= name=<Gerber output> units=<mm or inch>` |
| `set_job_gerber_format` | — | `path= name= format=<I:D>` |
| `set_job_gerber_zeros` | — | `path= name= zeros=<leading, trailing or none>` |
| `set_job_gerber_layers` | — | `path= name= layers=<all, or ids comma separated>` |
| `set_job_gerber_apertures` | — | `path= name= apertures=<standard or macros>` |
| `export_sheet_png` | picks a folder for the pages of the schematic in view, at 300 dpi | `document=<sheet> path=<folder> [dpi=<n>]` |

The edits of a job open in a tab change its draft and answer `saved: false`;
any other job file is read, changed and written, and answered `saved: true`.
`run_output_job` and `export_sheet_png` need `--allow export`; the commands
that write the job file (`new_output_job`, `add_job_output`,
`set_job_output`, `set_job_profile`, `set_job_triggers` and the five
`set_job_gerber_*`) need `--save`. A `set_job_gerber_*` command answers with
the output's five settings (`output`, `units`, `format`, `zeros`, `layers`,
`apertures`) and is refused for an output that is not a Gerber output.
`run_output_job` answers, once the run ends, with:

```json
{"job": "/p/fab.snxout", "folder": "/p/out/release", "variant": "Base",
 "profile": "Production release", "checks": {"errors": 0, "warnings": 3},
 "outputs": [{"name": "Gerber", "files": ["Fabrication/dcdc-buck-copper-l1-top.gbr"]}],
 "zip": "release.zip", "skipped": [], "warnings": []}
```

`profile`, `checks`, `zip` and `removed` are left out when they do not
apply. `job` is empty for `export_sheet_png`, whose one output is named
`PNG`. The tab's Automation page copies the command line for CI:

```sh
printf '%s\n' '{"id":"open_document","args":{"path":"dcdc-buck.snxsch"}}' \
  '{"id":"run_output_job","args":{"path":"fab.snxout","profile":"Production release"}}' \
  | signex-cli run dcdc-buck.snxprj --headless --allow export
```

It runs `--headless`: the agent channel of a running Signex takes no file
path, so a command that names the job by `path` runs without a window.

A page larger than 4096 pixels a side at the asked resolution is drawn at a
lower one, and the run says so in its warnings.
