# The Rules document tab (#2398): the project's DRC rules as a table and as text.

rules-tab-title = Rules
rules-tab-kind = RUL

# Sub-tabs, each with the findings of its rules in the last run.
rules-domain-clearance = Clearance
rules-domain-routing = Routing
rules-domain-holes = Holes
rules-domain-mask = Mask & paste
rules-domain-assembly = Silkscreen & courtyard
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
rules-add-hole = Add hole rule
rules-add-edge = Add board edge rule
rules-add-silk = Add silkscreen rule
rules-add-mask = Add mask rule
rules-add-paste = Add paste rule
rules-add-courtyard = Add courtyard rule
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
rules-col-min-pad-gap = Min pad gap
rules-col-min-hole = Min hole
rules-col-max-hole = Max hole
rules-col-min-annular = Min annular ring
rules-col-min-hole-spacing = Min hole spacing
rules-col-min-edge-clearance = Min edge clearance
rules-col-min-silk-pad = Min to pad
rules-col-min-silk-spacing = Min silk spacing
rules-col-min-sliver = Min sliver
rules-col-min-mask-expansion = Min mask expansion
rules-col-max-mask-expansion = Max mask expansion
rules-col-min-paste-expansion = Min paste expansion
rules-col-max-paste-expansion = Max paste expansion
rules-col-min-courtyard-spacing = Min courtyard spacing
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
rules-kind-hole = Hole
rules-kind-edge = Board edge
rules-kind-silk = Silkscreen
rules-kind-mask = Mask
rules-kind-paste = Paste
rules-kind-courtyard = Courtyard
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
rules-flag-below-fab = Below the fab floor { $floor }: { $from } → { $to } mm
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

# Rule drawings (#2530): the live preview under the grid and the matrix.
rules-preview-heading = Preview
rules-preview-expand = Expand preview
rules-preview-collapse = Shrink preview
rules-preview-hint = Click a value on the drawing to type it; Enter commits, Tab moves to the next value. Drag a ● handle to change a width, gap or spoke width. The grid cell follows.
rules-preview-pair = { $a } ↔ { $b }
rules-preview-object-track = Track
rules-preview-object-pad = Pad
rules-preview-object-via = Via
rules-preview-object-pour = Pour
rules-preview-object-edge = Board outline
rules-preview-cap-clearance = Clearance
rules-preview-cap-gap = Gap
rules-preview-cap-min = Min
rules-preview-cap-max = Max
rules-preview-cap-diameter = Min Ø
rules-preview-cap-drill = Min drill
rules-preview-cap-pad-gap = To pad
rules-preview-cap-min-hole = Min Ø
rules-preview-cap-max-hole = Max Ø
rules-preview-cap-annular = Ring
rules-preview-cap-hole-spacing = Hole to hole
rules-preview-cap-edge = To edge
rules-preview-cap-silk-pad = Silk to pad
rules-preview-cap-silk-spacing = Silk to silk
rules-preview-cap-sliver = Min sliver
rules-preview-title-clearance = Copper clearance
rules-preview-title-track = Track width
rules-preview-title-via = Via
rules-preview-title-hole = Holes
rules-preview-title-edge = Copper to board edge
rules-preview-title-silk = Silkscreen
rules-preview-title-mask = Solder mask
rules-preview-title-paste = Paste mask
rules-preview-title-courtyard = Courtyard spacing
rules-preview-note-clearance = Different nets only
rules-preview-note-track = Solid: the narrowest track. Dashed: the widest.
rules-preview-note-via = Tented: the mask covers the via
rules-preview-note-hole = Dashed: the allowed hole range
rules-preview-note-edge = Copper keeps back from the board outline
rules-preview-note-silk = Silkscreen lines beside a pad
rules-preview-note-mask = Dashed: the mask opening around each pad
rules-preview-note-mask-merged = Openings merge: no mask between the pads
rules-preview-note-paste = A negative expansion shrinks the aperture
rules-preview-note-courtyard = Dashed: courtyards. Parts are not to scale; the gap is.
# Clearance matrix (#2530): the caption, the legend, the object-pair card
# and the preview row.
rules-matrix-caption = One clearance per pair of net classes. A grey value is what the pair needs without a cell of its own. Type in a cell to set it; click it to open its object pairs.
rules-matrix-clear-cell = Clear: use the fallback
rules-matrix-clear-pair = Clear: use the All objects value
rules-matrix-fallback-tip = No value of its own: what the pair needs without a cell
rules-matrix-legend-written = written
rules-matrix-legend-fallback = fallback
rules-matrix-legend-mirror = mirror half
rules-matrix-col-pair = Pair
rules-matrix-col-clearance = Clearance
rules-matrix-pair = { $a } – { $b }
rules-object-track = Track
rules-object-pad = Pad
rules-object-via = Via
rules-object-pour = Pour
rules-object-any = Any
rules-matrix-preview = PREVIEW
rules-matrix-preview-none = Hover a cell or an object pair to draw it.
rules-matrix-preview-bare = { $a } ↔ { $b }
rules-matrix-preview-classes = Class pair { $a } ↔ { $b } · { $value } mm
rules-matrix-preview-objects-written = { $a } ↔ { $b } · { $pair } · { $value } mm (written)
rules-matrix-preview-objects-fallback = { $a } ↔ { $b } · { $pair } · { $value } mm (from All objects)
rules-matrix-preview-about = The preview draws the hovered cell with its value and the dashed clearance zone. A class pair is drawn as two tracks. An object pair draws those two objects (track, pad, via, pour). It redraws as you type a value, and the values on the drawing are editable too.

# Presets panel (#2530): tier cards and the preview pills.
rules-presets-close = Close presets
rules-preset-spec = clearance { $clearance } · width { $width } · drill { $drill } · annular { $annular } mm
rules-preset-pill-added = Added
rules-preset-pill-changed = Changed
rules-preset-pill-removed = Removed
rules-preset-from-to = { $from } → { $to }

# Text view (#2530): the problem banner and the undo note.
rules-text-problem-at = Line { $line }, column { $column }:
rules-text-undo-note = Edits apply as one undo step.
