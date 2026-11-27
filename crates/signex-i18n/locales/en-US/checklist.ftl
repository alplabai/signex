# The Design Checklist tab (#1291, mockup row 206).
checklist-tab-title = Design Checklist
checklist-tab-kind = CHK
checklist-template-release-review = Release review
checklist-template-prototype-build = Prototype build
checklist-template-blank = Blank
checklist-template-release-review-help = { $count } items for a production release
checklist-template-prototype-build-help = { $count } items for a first board spin
checklist-template-blank-help = The three stages, no items
checklist-own-list = Own list
checklist-summary = { $total } { $total ->
        [one] item
       *[other] items
    } · { $open } open · { $waived } waived · stored in the project file
checklist-search = Filter items, owners, links
checklist-add = Add item
checklist-run = Run checks
checklist-running = Checking…
checklist-run-failed = The checks did not finish: { $reason }
checklist-stage-schematic = Schematic
checklist-stage-layout = Layout
checklist-stage-fabrication = Fabrication
checklist-stage-release = Release
checklist-stage-progress = { $closed } / { $total }
checklist-stage-counts = { $done } done · { $waived } waived · { $open } open
checklist-state-open = Open
checklist-state-done = Done
checklist-state-waived = Waived
checklist-waive = Waived…
checklist-column-state = State
checklist-column-item = Item
checklist-column-owner = Owner
checklist-column-link = Linked to
checklist-column-check = Check
checklist-column-updated = Updated
checklist-result-passes = pass
checklist-result-fails = { $count } { $count ->
        [one] finding
       *[other] findings
    }
checklist-result-not-run = not run
checklist-check-passes = Check passes
checklist-mark-done = Mark done
checklist-link-erc = ERC run
checklist-link-drc = DRC run
checklist-link-title-block = Title block
checklist-link-requirement = Requirement { $id }
checklist-link-none = Not linked
checklist-detail-title = Item
checklist-field-title = Title
checklist-field-stage = Stage
checklist-field-owner = Owner
checklist-field-link = Linked to
checklist-field-link-hint = erc, drc, title_block, requirement:ID or object:what
checklist-field-notes = Notes
checklist-field-reason = Reason
checklist-reason-hint = Why is this item waived?
checklist-waive-confirm = Waive
checklist-waived-because = Waived: { $reason }
checklist-remove = Remove item
checklist-close-detail = Close
checklist-empty-title = No checklist for this project yet
checklist-empty-help = A checklist holds the review items to close before release, grouped by stage, each with an owner and a link to what proves it.
checklist-start = Start from template
checklist-empty-note = Nothing is written until you save the project.
checklist-unreadable = This project's design checklist was written by a newer version of Signex. It is kept as written and cannot be edited here.
checklist-no-match = No item matches the filter.
checklist-new-item = New item
checklist-problem-unreadable = The design checklist cannot be read by this version, so it cannot be edited.
checklist-problem-already-started = This project already has a design checklist.
checklist-problem-unknown-template = { $text } is no checklist template.
checklist-problem-unknown-item = The design checklist has no item { $id }.
checklist-problem-unknown-stage = { $text } is no stage.
checklist-problem-unknown-state = { $text } is no state.
checklist-problem-unknown-field = { $text } is no checklist field.
checklist-problem-unknown-link = { $text } is not a link. Write erc, drc, title_block, requirement:ID or object:what.
checklist-problem-empty-title = An item needs a title.
checklist-problem-needs-reason = A waived item needs a reason.
checklist-problem-not-stored = The design checklist was not stored: { $reason }
# The titles of the template items; a started checklist keeps them as text.
checklist-item-erc-clean = ERC passes with no errors
checklist-item-title-block = Title block filled in
checklist-item-parts-have-mpn = Every part has a manufacturer part number
checklist-item-fitted-parts-reviewed = Fitted and not fitted parts reviewed
checklist-item-requirements-met = Project requirements pass
checklist-item-drc-clean = DRC passes with no errors
checklist-item-board-outline = Board outline matches the enclosure
checklist-item-placement-reviewed = Component placement reviewed
checklist-item-fab-outputs-written = Fabrication outputs written
checklist-item-gerbers-viewed = Gerber set checked in the viewer
checklist-item-bom-released = Bill of materials released
checklist-item-assembly-drawings = Assembly drawings written
# Filter chips with their counts (mockup row 206).
checklist-chip-all = All { $count }
checklist-chip-open = Open { $count }
checklist-chip-done = Done { $count }
checklist-chip-waived = Waived { $count }
checklist-chip-mine = Mine { $count }
# The More menu, Save as template and the user's own templates.
checklist-more = More
checklist-save-as-template = Save as template…
checklist-export-csv = Export CSV…
checklist-reset = Reset from template
checklist-template-name = Template name
checklist-template-name-help = Saved to your templates, not to the project.
checklist-template-save = Save template
checklist-cancel = Cancel
checklist-template-stages = Schematic { $schematic } · Layout { $layout } · Fabrication { $fabrication }
checklist-your-templates = Your templates
checklist-saved-template = { $name }
checklist-saved-template-help = { $count } { $count ->
        [one] item
       *[other] items
    }
checklist-saved-template-from = { $count } { $count ->
        [one] item
       *[other] items
    } · saved from { $project }
checklist-import = Import from another project…
checklist-import-title = Import a design checklist from a project
checklist-export-title = Export the design checklist
checklist-notice-template-saved = Saved { $name } to your templates.
checklist-notice-template-not-saved = The template was not saved: { $reason }
checklist-notice-library-unreadable = Your templates could not be read: { $reason }
checklist-notice-import-failed = The project could not be read: { $reason }
checklist-notice-nothing-to-import = { $project } has no design checklist to import.
checklist-notice-link-not-found = { $what } is not in the project's open documents.
checklist-problem-not-started = This project has no design checklist yet.
checklist-problem-no-template = This checklist was not started from a template, so there is none to reset from.
checklist-problem-unknown-saved-template = None of your templates is called { $name }.
checklist-problem-empty-name = A template needs a name.
# What a design-object link names, and the link picker.
checklist-link-part = Part { $part }
checklist-link-pin = { $part } pin { $pin }
checklist-link-net = Net { $net }
checklist-link-sheet = Sheet { $sheet }
checklist-link-board = The board
checklist-link-object = { $what }
checklist-link-pick = Link a design object or check…
checklist-link-clear = Remove link
checklist-link-search = Filter checks, requirements, parts, nets, sheets
checklist-link-typed = Link to { $what }
checklist-group-checks = Checks
checklist-group-requirements = Requirements
checklist-group-parts = Parts
checklist-group-nets = Nets
checklist-group-sheets = Sheets
checklist-group-board = Board
checklist-open-in-requirements = Open in Requirements
checklist-open-results = Open results
checklist-open-parameters = Open in Parameters
checklist-show-object = Show
checklist-run-check = Run check
# An item's history.
checklist-history = History
checklist-history-created = Created
checklist-history-created-from = Created from the template { $template }
checklist-history-imported = Imported from { $project }
checklist-history-title = Renamed to { $title }
checklist-history-stage = Moved to { $stage }
checklist-history-owner = Owner set to { $owner }
checklist-history-owner-cleared = Owner cleared
checklist-history-link = Linked to { $link }
checklist-history-link-cleared = Link removed
checklist-history-notes = Notes edited
checklist-history-opened = Opened again
checklist-history-done = Marked done
checklist-history-waived = Waived: { $reason }
checklist-history-other = { $event }
checklist-history-by-on = { $by } · { $on }
checklist-history-by = { $by }
checklist-history-on = { $on }

## Why a checklist command was refused (#1291), as an agent or the CLI reads it.
checklist-refusal-unreadable = the project's design checklist cannot be read by this version; it is kept as written
checklist-refusal-already-started = the project already has a design checklist
checklist-refusal-not-started = the project has no design checklist yet
checklist-refusal-no-template = the design checklist was not started from a template
checklist-refusal-unknown-template = `{ $text }` is no checklist template; release_review, prototype_build, blank or user:<name>
checklist-refusal-unknown-saved-template = no saved checklist template is called `{ $name }`
checklist-refusal-empty-name = a checklist template needs a name
checklist-refusal-unknown-item = the design checklist has no item `{ $id }`
checklist-refusal-unknown-stage = `{ $text }` is no stage; schematic, layout or fabrication
checklist-refusal-unknown-state = `{ $text }` is no state; open, done or waived
checklist-refusal-unknown-field = `{ $text }` is no checklist field; title, stage, owner, link or notes
checklist-refusal-unknown-link = `{ $text }` is no link; erc, drc, title_block, requirement:<id> or object:<what>
checklist-refusal-empty-title = a checklist item needs a title
checklist-refusal-needs-reason = a waived item needs a reason
checklist-refusal-not-stored = the design checklist was not stored: { $reason }
checklist-refusal-project = no project for the design checklist: { $reason }
checklist-refusal-needs-operand = needs the operand `{ $name }`: { $what }
checklist-refusal-bad-filter = `{ $filter }` is no filter; all, open, done, waived or mine
checklist-refusal-template-not-saved = the template was not saved: { $reason }
checklist-refusal-library-unreadable = the checklist templates could not be read: { $reason }
checklist-refusal-nothing-to-import = { $project } has no design checklist
checklist-refusal-import-failed = the project could not be read: { $reason }
checklist-refusal-no-link = item `{ $id }` links to nothing
checklist-refusal-project-closed = the project was closed
checklist-refusal-command = the command could not wait for its files: { $reason }
checklist-operand-template = release_review, prototype_build, blank or user:<name>
checklist-operand-stage = schematic, layout or fabrication
checklist-operand-title = the item's title
checklist-operand-item = an item id
checklist-operand-field = title, stage, owner, link or notes
checklist-operand-state = open, done or waived
checklist-operand-template-name = the template's name
checklist-operand-csv = the CSV file
checklist-operand-import = the project to import from
