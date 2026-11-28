# The Export Job tab (#1299, mockup rows 114, 167 and 193b): a `.snxout` as a
# document tab, its run profiles, automation, the run flow, the summary and the
# history, and the Gerber Viewer banner of a run. Rules: docs/OUTPUT_JOBS.md.

# The tab's title; { $file } is the job's file name.
output-job-tab-title = { $file } — Export Job
output-job-reading = Reading the Export Job…
output-job-read-failed = The Export Job could not be read: { $why }

# Toolbar.
output-job-profile-button = Profile: { $name }
output-job-profile-none = Profile: all enabled outputs
output-job-auto = Auto
output-job-variant = Variant
output-job-page-settings = Settings
output-job-page-automation = Automation
output-job-page-flow = Run
output-job-page-summary = Summary
output-job-run-selected = Run Selected { $count }
output-job-run-profile = Run profile
output-job-stop = Stop
output-job-chip-generating = Outputs: generating { $done }/{ $total }…
output-job-chip-stale = Outputs: { $count } out of date
output-job-chip-stopped = Outputs: run stopped
output-job-chip-up-to-date = Outputs up to date

# The profile popover.
output-job-profiles = Profiles in this .snxout
output-job-profiles-note = A profile holds the ticked outputs, the variant and the output container. Profiles are saved in the .snxout, so everyone who opens the project gets them.
output-job-profile-save-as = Save as profile…
output-job-profile-rename = Rename…
output-job-profile-duplicate = Duplicate
output-job-profile-delete = Delete…
output-job-profile-default = Set as default
output-job-profile-name = Profile name
output-job-ok = OK
output-job-cancel = Cancel

# Outputs.
output-job-add-output = Add Output
output-job-empty-title = No outputs yet
output-job-empty-body = An output is one set of files, such as Gerber, a schematic PDF or a BOM. Add the first one to start.
output-job-group-fabrication = Fabrication
output-job-group-assembly = Assembly
output-job-group-documentation = Documentation
output-job-group-reports = Reports
output-job-group-count = { $ticked }/{ $total }
output-job-enabled = Enabled
output-job-tick-note = Ticked = in this profile. An output switched off is in no run.
output-job-unknown-kind = { $name }: a kind this Signex does not write ({ $kind }); kept in the file and left out of runs.
output-job-kind-schematic-pdf = Schematic PDF
output-job-kind-schematic-png = Schematic PNG
output-job-kind-bom = BOM
output-job-kind-netlist = Netlist
output-job-kind-gerber = Gerber
output-job-kind-nc-drill = NC Drill
output-job-kind-pick-and-place = Pick and Place
output-job-kind-test-points = Test points / IPC-D-356
output-job-kind-ipc2581 = IPC-2581
output-job-kind-odb = ODB++
output-job-kind-step = 3D STEP
output-job-kind-assembly-pdf = Assembly drawings PDF

# The picked output's form.
output-job-form-nothing = Nothing selected
output-job-form-nothing-body = Settings for the selected output show here.
output-job-form-enabled = Enabled: run this output
output-job-source-sheets = Source: the project's schematic pages
output-job-source-board = Source: the project's board
output-job-form-file = File
output-job-form-file-note = The file name in the output folder. Empty takes the default.
output-job-form-stem = File name start
output-job-form-stem-note = Every file of this output starts with it. Empty takes the default.
output-job-form-default-file = Default
output-job-form-dpi = Resolution (dpi)
output-job-form-dpi-note = 36 to 1200 dpi; one PNG file per page.
output-job-form-odb-folder = Write the job as its folder tree instead of one archive
output-job-form-gerber-fixed = Gerber X2 with file attributes, as File ▸ Fabrication Outputs writes it. The settings start at that writer's own: millimetres, 4:6, leading zeros left out, every layer, standard apertures.
output-job-form-gerber-units = Units
output-job-form-gerber-mm = Millimetres
output-job-form-gerber-inch = Inches
output-job-form-gerber-format = Number format
output-job-form-gerber-format-value = { $integer }:{ $decimal }
output-job-form-gerber-format-note = Digits before and after the point of every coordinate.
output-job-form-gerber-zeros = Zero suppression
output-job-form-gerber-zeros-leading = Leading
output-job-form-gerber-zeros-trailing = Trailing
output-job-form-gerber-zeros-none = None
output-job-form-gerber-layers = Layers
output-job-form-gerber-layers-all = Every layer
output-job-form-gerber-layer-copper = All copper
output-job-form-gerber-layer-copper-top = Top copper
output-job-form-gerber-layer-copper-bottom = Bottom copper
output-job-form-gerber-layer-legend-top = Top legend
output-job-form-gerber-layer-legend-bottom = Bottom legend
output-job-form-gerber-layer-soldermask-top = Top solder mask
output-job-form-gerber-layer-soldermask-bottom = Bottom solder mask
output-job-form-gerber-layer-paste-top = Top paste
output-job-form-gerber-layer-paste-bottom = Bottom paste
output-job-form-gerber-layer-profile = Board outline
output-job-form-gerber-layer-vscore = V-score lines
output-job-form-gerber-layers-note = An inner copper layer alone is named copper-inner2, copper-inner3 and so on, from the command line.
output-job-form-gerber-apertures = Apertures
output-job-form-gerber-apertures-standard = Standard
output-job-form-gerber-apertures-macros = Macros only
output-job-form-drill-fixed = Excellon drill files, plated and non-plated apart, as File ▸ Fabrication Outputs writes them.
output-job-form-bom-format = CSV, HTML or XLSX, by the file's extension.
output-job-form-pages-fixed = Every page in print order, on the first page's paper, in the print colors.
output-job-form-step-fixed = The board and its parts' models, as File ▸ Export ▸ STEP 3D Model makes it.
output-job-form-remove = Remove output

# The container and the pipeline.
output-job-container = Output container
output-job-folder = Folder beside the job file
output-job-folder-of-job = Folder: { $folder }
output-job-no-profile = This job has no profile yet: a run writes every enabled output. Tick an output or pick a variant to make the first profile.
output-job-zip = Also write a zip file
output-job-group-folders = One subfolder per group
output-job-naming = Naming pattern
output-job-naming-hint = { "{" }project{ "}" }-{ "{" }rev{ "}" }
output-job-naming-example = Example: { $example }
output-job-release-folder = Release folder named with rev and date
output-job-clear-folder = Clear the folder before each run
output-job-clear-folder-note = Deletes the files in it that this run did not write. Signex documents are never deleted.
output-job-open-folder = Open folder
output-job-pipeline = Run pipeline
output-job-step-validate = Validate
output-job-step-generate = Generate
output-job-step-package = Package
output-job-step-validate-long = 1 · Validate: ERC, DRC, BOM checks
output-job-step-generate-long = 2 · Generate the ticked outputs
output-job-step-package-long = 3 · Package: folder and zip
output-job-step-summary-long = 4 · Summary
output-job-on-errors = On errors
output-job-on-warnings = On warnings
output-job-policy-stop = Stop
output-job-policy-ask = Ask
output-job-policy-continue = Continue
output-job-last-run = Last run
output-job-last-run-line = { $started } · { $outputs } outputs, { $files } files
output-job-never-run = Never run.

# Automation.
output-job-back-to-settings = Back to settings
output-job-automation-note = Triggers run this profile without a click. Auto runs work in the background, so editing is never blocked.
output-job-trigger-on-save = Run on save
output-job-trigger-on-save-note = When the board or a schematic is saved. Waits until saving stops, so a burst of saves runs once.
output-job-trigger-sources = Files, comma separated
output-job-trigger-sources-note = File names or *.ext patterns; empty is the board and every schematic.
output-job-trigger-wait = Wait in seconds
output-job-trigger-wait-note = Wait { $seconds } s after the last save.
output-job-trigger-keep = Keep outputs up to date
output-job-trigger-keep-note = When a source changes, re-run only the outputs made from it: a board save re-runs the board outputs, a schematic save the schematic ones.
output-job-trigger-before-close = Before closing the project
output-job-trigger-before-close-note = If outputs are out of date when the project closes: Ask names them in a notice, Always runs the profile.
output-job-trigger-off = Off
output-job-trigger-always = Always run
output-job-trigger-tag = Run on release or git tag
output-job-trigger-tag-hint = Tag pattern, such as v*
output-job-trigger-tag-note = Needs version control. Signex makes no release or tag yet, so the pattern is kept for when it does.
output-job-command-line = Command line, for CI
output-job-command-line-note = Runs the same pipeline with no window. It stops with a non-zero exit code when a step stops on an error.
output-job-copy = Copy
output-job-recent-auto = Recent auto runs
output-job-recent-auto-none = No auto run yet.
output-job-recent-auto-line = { $started } · { $trigger } · { $profile } · { $outputs } outputs

# The run flow.
output-job-no-run = No run yet this session.
output-job-flow-head = { $profile } · variant { $variant } · started { $started }
output-job-flow-progress = { $done } of { $total } steps · { $outputs_done } of { $outputs } outputs
output-job-flow-policy = On errors: { $errors } · On warnings: { $warnings }
output-job-node-pending = waiting
output-job-node-running = running
output-job-node-done = { $files } files
output-job-node-warning = warnings
output-job-node-failed = failed
output-job-ask = { $summary }. Continue anyway, or stop with nothing written?
output-job-ask-continue = Continue anyway
output-job-ask-stop = Stop
output-job-stopped-banner = Run stopped at { $at }: { $reason }. No files were written.
output-job-show-in-check = Show the findings
output-job-run-again = Run again
output-job-ask-next-time = Ask instead of stopping next time (this profile)
output-job-log = Log
output-job-log-empty = Click a node to see its log.

# The summary and the history.
output-job-show-flow = Show the flow
output-job-summary-complete = Run complete · { $profile } · variant { $variant } · { $started }
output-job-summary-stopped = Run stopped · { $profile } · variant { $variant } · { $started }
output-job-summary-counts = Outputs written { $outputs } · Files { $files } · Warnings { $warnings } · Errors { $errors } · Total time { $seconds } s
output-job-written-to = Written to
output-job-plus-zip = plus { $zip }
output-job-view-in-gerber = View in Gerber Viewer
output-job-view-in-gerber-note = View in Gerber Viewer opens exactly the files this run wrote, so you check the output, not the board.
output-job-copy-path = Copy path
output-job-outputs = Outputs
output-job-file-count = { $count ->
    [one] 1 file
   *[other] { $count } files
}
output-job-view = View
output-job-history = History kept in this .snxout
output-job-history-row = { $started } · { $profile } { $variant } · { $trigger } · { $result } · { $seconds } s
output-job-history-complete = { $outputs } out · { $warnings } warn
output-job-history-stopped = Stopped · { $at }
output-job-history-note = Click a run to open its summary. The files stay wherever that run wrote them.
output-job-trigger-manual = Manual
output-job-trigger-selected = Run Selected
output-job-trigger-saved = Auto, on save
output-job-trigger-kept = Keep up to date
output-job-trigger-closed = Before close
output-job-trigger-command = Command line

# The footer.
output-job-status-ready = Ready
output-job-status-empty = No outputs. Add one, then save with Ctrl+S.
output-job-status-running = Running { $profile } · { $step } · { $done } of { $total } outputs
output-job-status-asking = Waiting for your answer
output-job-status-stopped = Stopped at { $at } · nothing written
output-job-status-complete = Run complete · variant { $variant } · { $outputs } outputs, { $files } files · { $warnings } warnings · { $seconds } s
output-job-fault = Cannot be saved or run yet: { $fault }

# The Gerber Viewer opened from a run (mockup row 193b).
output-job-viewer-from-run = From Export Job run: { $profile } · variant { $variant } · { $started }
output-job-viewer-files-missing = The files of this run are no longer at { $folder }. Open another folder.
output-job-viewer-back = Back to run summary
output-job-viewer-close = Close
