# Requirements section of the Project tab (#2339).
project-section-requirements = Requirements
requirements-help = Targets this project must meet. Each is a check that passes or fails with evidence; Run checks runs them all. They are stored in the project file and written by Save.
requirements-empty = This project lists no requirements. Pick a kind in Add requirement… to add one.
requirements-list-unreadable = The project's requirements are not a list. They are kept as written and cannot be edited here; fix the project file.
requirements-add = Add requirement…
requirements-enabled = Enabled
requirements-remove = Remove
requirements-run = Run checks
requirements-running = Checking…
requirements-last-run = Last run: { $time }
requirements-never-run = Not run yet.
requirements-run-failed = The checks did not finish: { $reason }
requirements-run-notes = Notes from the last run
requirements-passed = Passed
requirements-failed = Failed
requirements-not-checked = Not checked
requirements-off = Off
requirements-passed-count = { $count } Passed
requirements-failed-count = { $count } Failed
requirements-not-checked-count = { $count } Not checked
# A failed parts_state card: $missed of $of parts, $allowed the allowed
# states as a list, $lines one requirements-part-* clause per BOM line.
requirements-parts-unmet = { $missed } of { $of } { $of ->
        [one] part
       *[other] parts
    } { $missed ->
        [one] is
       *[other] are
    } not { $allowed }: { $lines }.
requirements-part-unplaced = { $parts } { $count ->
        [one] was
       *[other] were
    } not placed from a library row
requirements-part-no-library = { $count ->
        [one] { $parts }'s library row is
       *[other] the library row of { $parts } is
    } in no open library
requirements-part-state = { $parts } { $count ->
        [one] is
       *[other] are
    } { $state }
# Any other failed card: its findings' messages, joined.
requirements-findings = { $findings }.
requirements-findings-next = { $head }; { $next }
requirements-expand = Expand
requirements-collapse = Collapse
requirements-summary-clean = 0 errors or warnings
requirements-summary-clean-warnings-allowed = 0 errors · warnings allowed
requirements-summary-board-size = ≤ { $width } × { $height } mm
requirements-summary-parts-state = Only { $states } parts
requirements-summary-net-class = { $nets }: { $limits }
requirements-summary-class = class { $class }
requirements-summary-min-trace = track ≥ { $width } mm
requirements-summary-min-clearance = clearance ≥ { $clearance } mm
requirements-summary-bom-cost = ≤ { $cost } { $currency } per board at { $quantity } boards
requirements-unreadable = This entry cannot be read ({ $reason }). It is kept as written; remove it or fix the project file.
requirements-not-set = Not set
requirements-kind-erc-clean = ERC clean
requirements-kind-drc-clean = DRC clean
requirements-kind-board-size = Board size
requirements-kind-parts-state = Part lifecycle
requirements-kind-net-class = Net class
requirements-kind-bom-cost = BOM cost
requirements-field-id = Id
requirements-field-severity = Severity
requirements-field-allow-warnings = Allow warnings
requirements-field-max-width = Max width (mm)
requirements-field-max-height = Max height (mm)
requirements-field-states = Lifecycle states
requirements-field-nets = Nets (* and ?)
requirements-field-class = Net class
requirements-field-min-trace = Min track width (mm)
requirements-field-min-clearance = Min clearance (mm)
requirements-field-quantity = Boards built
requirements-field-currency = Currency
requirements-field-max-unit-cost = Max cost per board
requirements-problem-list-unreadable = The project's requirements are not a list; fix the project file first.
requirements-problem-no-entry = That requirement is no longer in the list.
requirements-problem-unknown-id = The project has no requirement { $id }.
requirements-problem-ambiguous-id = More than one requirement has the id { $id }.
requirements-problem-entry-unreadable = This entry cannot be read; it can only be removed.
requirements-problem-unknown-kind = { $kind } is no requirement kind.
requirements-problem-not-a-field = This kind of requirement has no field { $field }.
requirements-problem-id-empty = A requirement needs an id.
requirements-problem-id-too-long = An id is at most { $max } characters.
requirements-problem-id-invalid = An id cannot hold a control character.
requirements-problem-id-taken = Another requirement has the id { $id }.
requirements-problem-unknown-severity = { $text } is no severity: error, warning or info.
requirements-problem-not-yes-no = { $text } is not true or false.
requirements-problem-not-positive = { $text } is not a number above 0.
requirements-problem-not-a-count = { $text } is not a whole number of at least 1.
requirements-problem-empty = This field needs a value.
requirements-problem-unknown-state = { $state } is no lifecycle state: draft, in_review, released, deprecated or obsolete.
requirements-problem-net-class-asks-nothing = Set a net class, a minimum track width or a minimum clearance.
# requirements-board-size
requirements-summary-measured = · measured { $width } × { $height } mm
requirements-field-measured-from = Measured from
requirements-measured-from-outline = Board outline
requirements-board-preview = Board against the limit
requirements-board-preview-hint = measured on the last run
requirements-board-not-measured = Run checks to measure the board
requirements-board-measured = { $width } × { $height } mm measured
requirements-board-max-width = ≤ { $width } mm
requirements-board-max-height = ≤ { $height }
requirements-board-legend-limit = limit (fits the enclosure)
requirements-board-legend-outline = board outline, last run
requirements-board-margin = margin { $width } × { $height } mm
requirements-board-fits = the board fits
requirements-board-does-not-fit = the board does not fit
# Part lifecycle: the variant checked and fitted parts only (#2686)
requirements-field-variant = Variant
requirements-field-fitted-only = Fitted parts only
# The Variant choice that follows the project's active variant; $name is its name.
requirements-variant-active = Active variant ({ $name })
# A failed parts_state card that checks fitted parts only; as requirements-parts-unmet.
requirements-parts-unmet-fitted = { $missed } of { $of } fitted { $of ->
        [one] part
       *[other] parts
    } { $missed ->
        [one] is
       *[other] are
    } not { $allowed }: { $lines }.
requirements-problem-unknown-variant = { $name } is no variant of this project: Base or one of its variants.
# Beside a net class card's pill when its fields changed since the run that judged it.
requirements-edits-not-checked = last run · edits not checked yet
