# The Rules document tab (#2398): the project's DRC rules as a table and as text.

rules-tab-title = Rules
rules-tab-kind = RUL

# Sub-tabs, each with the findings of its rules in the last run.
rules-domain-clearance = Clearance
rules-domain-routing = Routing
rules-domain-count = { $count }

# Toolbar.
rules-search-placeholder = Search name, net, class or layer
rules-filter-all = All
rules-filter-violations = With violations
rules-filter-flagged = Flagged
rules-filter-disabled = Off
rules-view-text = Text
rules-view-table = Table
rules-text-apply = Apply
rules-text-discard = Discard
rules-run-drc = Run DRC
rules-drc-running = Checking...
rules-add-clearance = Add clearance rule
rules-add-track = Add width rule
rules-add-via = Add via rule
rules-remove = Remove

# Column headings.
rules-col-on = On
rules-col-name = Name
rules-col-kind = Kind
rules-col-scope = Scope
rules-col-layer = Layer
rules-col-objects = Objects
rules-col-min-clearance = Min clearance
rules-col-min-width = Min width
rules-col-max-width = Max width
rules-col-min-diameter = Min diameter
rules-col-min-drill = Min drill
rules-col-severity = Severity
rules-col-actual = Actual
rules-col-margin = Margin
rules-col-violations = Violations
rules-col-flags = Flags

# Cell values that are not typed text.
rules-scope-all = All nets
rules-layer-all = All layers
rules-objects-any = Any
rules-kind-clearance = Clearance
rules-kind-track = Width
rules-kind-via = Via
rules-severity-error = Error
rules-severity-warning = Warning
rules-severity-info = Info
rules-cell-empty = —

# Why an Actual or Margin cell is blank.
rules-blank-not-checked = Not checked yet
rules-blank-stale = Stale since the last edit
rules-blank-no-objects = No objects in scope
rules-blank-off = Off
rules-blank-passes = Passes
rules-blank-no-board = Open the board to check

# Flags on a row.
rules-flag-shadowed = Never applies: { $by } wins
rules-flag-conflict = Disagrees with { $with }
rules-flag-lowers = Lowers { $class } { $from } → { $to } mm
rules-flag-no-nets = Matches no net
rules-badge-text-only = Text only
rules-badge-problems = { $count ->
        [one] 1 problem
       *[other] { $count } problems
    }

# Notes above the table.
rules-origin-machine = These rules come from this machine's profile. The first edit stores them in the project.
rules-syntax-error = The rule text has a syntax error, so no rule applies: { $error }
rules-refused = Not changed: { $reason }
rules-empty = No rules here yet. Add one, or write rules in the text view.
rules-no-project = This project is no longer open.

# The cell being typed in.
rules-cell-cancel = Cancel
rules-scope-hint = net == "GND", class == "HV" or net matches "USB_*"; empty for all nets
rules-scope-no-net = No net of the board is called { $net }
rules-scope-no-class = There is no net class { $class }
rules-scope-no-match = No net of the board matches { $pattern }

# The detail of the selected rule.
rules-detail-text = Rule text
rules-detail-problems = Problems
rules-detail-flags = Flags
rules-detail-scope = { $count ->
        [one] 1 object on the board
       *[other] { $count } objects on the board
    }
rules-detail-scope-unknown = Open the board to see what this rule applies to.
rules-detail-show = Show on board
rules-why-title = Why this value?
rules-why-for = For net { $net } on { $layer }: { $value } mm, from { $source }.
rules-why-source-rule = rule { $rule }
rules-why-source-class = its net class
rules-why-source-setup = the board setup
rules-why-source-none = no rule
rules-why-candidate = { $rule }: { $value } mm, { $verdict }
rules-why-none = No net of the board is in this rule's scope.
rules-verdict-won = applies
rules-verdict-less-specific = a more specific rule applies
rules-verdict-less-strict = an equally specific, stricter rule applies
rules-verdict-other-nets = other nets
rules-verdict-other-layers = other layers
rules-verdict-other-objects = other objects

# The text view.
rules-text-problem = Line { $line }, column { $column }: { $message }
rules-text-clean = No problems in the text.

# The class-to-class clearance matrix (Clearance sub-tab).
rules-view-matrix = Matrix
rules-view-list = List
rules-matrix-hint = One clearance per pair of net classes. A grey value is what the pair needs without a cell of its own; click a cell to set it, or to open its object pairs.
rules-matrix-no-classes = This project has no net classes yet, so the matrix is empty.
rules-matrix-default = ({ $value } mm)
rules-matrix-no-default = (board setup)
rules-matrix-objects-title = Objects between { $a } and { $b }
rules-matrix-all-objects = All objects
rules-matrix-clear = Clear
rules-matrix-collapse = Close

# Named rule sets.
rules-panel-sets = Rule sets
rules-sets-title = Rule sets
rules-sets-hint = A rule set is limits written once and applied to classes or nets. Changing a set rewrites every rule it wrote.
rules-sets-name = Set name, such as HV_08
rules-sets-values = Limits, such as min_clearance=0.8mm, min_width=0.3mm
rules-sets-severity = Severity: error, warning or info
rules-sets-scope = Scope to apply to, such as class == "HV"
rules-sets-create = Create set
rules-sets-apply = Apply to scope
rules-sets-redefine = Use these limits
rules-sets-empty = No rule sets yet.
rules-sets-uses = Applied to: { $scopes }
rules-sets-unused = Not applied yet.

# Presets: generic tiers and rule profiles.
rules-panel-presets = Presets
rules-presets-title = Presets
rules-presets-hint = A generic tier merges its minimums into the rules; a rule profile replaces them. Each shows what it changes before it is applied.
rules-preset-standard = Standard tier
rules-preset-advanced = Advanced tier
rules-preset-hdi = HDI tier
rules-presets-load = Show rule profiles
rules-presets-no-profiles = No rule profile holds DRC rules.
rules-preset-preview = Applying { $preset } changes:
rules-preset-no-change = Nothing changes.
rules-preset-added = Added { $rule }
rules-preset-changed = Changed { $rule }
rules-preset-removed = Removed { $rule }
rules-preset-apply = Apply
rules-preset-cancel = Cancel
