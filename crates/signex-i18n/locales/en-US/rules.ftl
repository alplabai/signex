# The Rules document tab (#2398, #2530): the project's DRC rules as a constraint grid and as text.

rules-tab-title = Rules
rules-tab-kind = RUL

# Domain tabs, each with the findings of its rules in the last run.
rules-domain-clearance = Clearance
rules-domain-mask = Mask

# Toolbar.
rules-view-text = Text
rules-view-table = Table
rules-text-apply = Apply
rules-text-discard = Discard
rules-run-drc = Run DRC
rules-drc-running = Checking...
rules-remove = Remove

# Cell values that are not typed text.
rules-scope-all = All nets
rules-layer-all = All layers
rules-severity-error = Error
rules-severity-warning = Warning
rules-severity-info = Info
rules-cell-empty = —

# Why a rule's findings line has no count: the board has not been
# checked yet, changed since the run, the rule changed since the run, it
# is off, it covers no object, the board is not open; or it passed.
rules-blank-not-checked = Not checked yet
rules-blank-stale = Stale since the last edit
rules-blank-changed = Changed since the run
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

# Notes above the grid.
rules-origin-machine = These rules come from this machine's profile. The first edit stores them in the project.
rules-syntax-error = The rule text has a syntax error, so no rule applies: { $error }
rules-refused = Not changed: { $reason }
rules-no-project = This project is no longer open.

# A rule field being typed in.
rules-cell-cancel = Cancel

# The detail of the selected rule.
rules-detail-text = Rule text
rules-detail-problems = Problems
rules-detail-flags = Flags
rules-detail-scope = { $count ->
        [one] 1 object on the board
       *[other] { $count } objects on the board
    }
rules-detail-scope-unknown = Open the board to see what this rule applies to.
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

# The class-to-class clearance matrix (Clearance domain).
rules-view-matrix = Matrix
rules-matrix-no-classes = This project has no net classes yet, so the matrix is empty.
rules-matrix-default = ({ $value } mm)
rules-matrix-no-default = (board setup)
rules-matrix-objects-title = Objects between { $a } and { $b }
rules-matrix-all-objects = All objects
rules-matrix-collapse = Close

# Constraint sets.
rules-panel-sets = Constraint sets
rules-sets-hint = A constraint set is limits written once and applied to classes or nets. Changing a set rewrites every rule it wrote.
rules-sets-name = Set name, such as HV_08
rules-sets-values = Limits, such as min_clearance=0.8mm, min_width=0.3mm
rules-sets-severity = Severity: error, warning or info
rules-sets-scope = Scope to apply to, such as class == "HV"
rules-sets-create = Create set
rules-sets-apply = Apply to scope
rules-sets-redefine = Use these limits
rules-sets-empty = No constraint sets yet.
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

# Constraint grid (#2530): the domain tabs. Signal integrity arrives with
# the simulation band.
rules-domain-physical = Physical
rules-domain-high-speed = High speed
rules-domain-manufacturing = Manufacturing
rules-domain-smt = SMT
rules-domain-plane = Plane
rules-domain-testpoint = Testpoint
rules-domain-placement = Placement
rules-domain-signal-integrity = Signal integrity
rules-domain-signal-integrity-later = Signal integrity · later

# Column group headers, one per rule type.
rules-group-clearance = Clearance
rules-group-creepage = Creepage
rules-group-short-circuit = Short-circuit
rules-group-unrouted-net = Un-routed net
rules-group-unconnected-pin = Un-connected pin
rules-group-modified-pour = Modified pour
rules-group-width = Width
rules-group-via-style = Via style
rules-group-diff-pair = Diff pair
rules-group-neck-down = Neck-down
rules-group-corners = Corners
rules-group-routing = Routing
rules-group-length = Length
rules-group-matched-length = Matched length
rules-group-stubs-and-vias = Stubs and vias
rules-group-return-path = Return path
rules-group-parallel-segment = Parallel segment
rules-group-hole-size = Hole size
rules-group-annular-ring = Annular ring
rules-group-hole-to-hole = Hole to hole
rules-group-layer-pairs = Layer pairs
rules-group-acute-angle = Acute angle
rules-group-mask-sliver = Mask sliver
rules-group-silk = Silk
rules-group-net-antennae = Net antennae
rules-group-board-outline = Board outline
rules-group-smd-to-corner = SMD to corner
rules-group-smd-to-plane = SMD to plane
rules-group-smd-entry = SMD entry
rules-group-solder-mask = Solder mask
rules-group-paste-mask = Paste mask
rules-group-power-plane-connect = Power plane connect
rules-group-power-plane-clearance = Power plane clearance
rules-group-pour-connect = Pour connect
rules-group-fab-testpoint-style = Fabrication style
rules-group-fab-testpoint-usage = Fabrication usage
rules-group-assembly-testpoint-style = Assembly style
rules-group-assembly-testpoint-usage = Assembly usage
rules-group-component-clearance = Component clearance
rules-group-permitted-layers = Permitted layers
rules-group-height = Height
rules-group-orientation = Orientation
rules-group-room = Room
rules-group-nets-to-ignore = Nets to ignore
rules-group-collapse = { $type } · click to collapse
rules-group-expand = { $type } · click to expand

# The full rule-type name of a group, where it differs from the header
# (tooltips, the find box, the drawing caption).
rules-type-creepage = Creepage distance
rules-type-diff-pair = Differential pairs routing
rules-type-neck-down = SMD neck-down
rules-type-corners = Routing corners
rules-type-routing = Routing topology, priority, layers and fanout
rules-type-matched-length = Matched lengths
rules-type-stubs-and-vias = Daisy-chain stubs, via count, back drill and vias under SMD
rules-type-annular-ring = Minimum annular ring
rules-type-hole-to-hole = Hole to hole clearance
rules-type-mask-sliver = Minimum solder mask sliver
rules-type-silk = Silk to pad and silk to silk clearance
rules-type-board-outline = Board outline clearance
rules-type-solder-mask = Solder mask expansion
rules-type-paste-mask = Paste mask expansion
rules-type-power-plane-connect = Power plane connect style
rules-type-pour-connect = Pour connect style
rules-type-fab-testpoint-style = Fabrication testpoint style
rules-type-fab-testpoint-usage = Fabrication testpoint usage
rules-type-assembly-testpoint-style = Assembly testpoint style
rules-type-assembly-testpoint-usage = Assembly testpoint usage
rules-type-orientation = Component orientations
rules-type-room = Room definition

# Column headers under a group.
rules-column-min = Min
rules-column-max = Max
rules-column-preferred = Preferred
rules-column-diameter = Diameter
rules-column-hole = Hole
rules-column-pad-gap = Pad gap
rules-column-finish = Finish
rules-column-width = Width
rules-column-gap = Gap
rules-column-max-uncoupled = Max uncoupled
rules-column-min-of-pad = Min % of pad
rules-column-style = Style
rules-column-setback = Setback
rules-column-topology = Topology
rules-column-priority = Priority
rules-column-layers = Layers
rules-column-fanout = Fanout
rules-column-target = Target
rules-column-plus-minus = ± Tol
rules-column-tolerance = Tolerance
rules-column-max-skew = Max skew
rules-column-daisy-stub = Daisy stub
rules-column-max-vias = Max vias
rules-column-back-drill-stub = Back-drill stub
rules-column-under-smd = Under SMD
rules-column-max-gap = Max gap
rules-column-max-length = Max length
rules-column-enforce = Enforce
rules-column-to-pad = To pad
rules-column-to-silk = To silk
rules-column-min-expansion = Min expansion
rules-column-max-expansion = Max expansion
rules-column-tented = Tented
rules-column-spokes = Spokes
rules-column-air-gap = Air gap
rules-column-side = Side
rules-column-pad-diameter = Pad Ø
rules-column-grid = Grid
rules-column-spacing = Spacing
rules-column-required = Required
rules-column-sides = Sides
rules-column-allowed = Allowed
rules-column-keep = Keep
rules-column-ignore = Ignore
rules-column-check = Check
rules-column-entry = Entry

# The grid's frame, rows and units.
rules-grid-unnamed = Unnamed
rules-grid-object = Object
rules-grid-collapsed = …
rules-grid-unit-mm = mm
rules-grid-unit-percent = %
rules-grid-unit-degrees = °
rules-grid-row-all = All
rules-grid-row-nets = { $count ->
        [one] 1 net
       *[other] { $count } nets
    }
rules-grid-row-scope = { $scope }
rules-grid-rows-net-classes = Net Classes
rules-grid-rows-layers = Layers
rules-grid-rows-scoped = Scoped rules
rules-grid-layer-top = L{ $n } · Top
rules-grid-layer-inner = L{ $n }
rules-grid-layer-bottom = L{ $n } · Bottom
rules-grid-kind-all = All
rules-grid-kind-class = Class
rules-grid-kind-net = Net
rules-grid-kind-layer = Layer
rules-grid-kind-rule = Rule
rules-grid-set-chip = ⧉ { $set }
rules-grid-reset-glyph = ↺
rules-grid-add = Add
rules-grid-signal-integrity-later = Later (simulation band). Signal integrity constraints need the simulator, so they arrive with it.
rules-grid-preview-caption = { $row } · { $type }
rules-grid-preview-none = This rule has no geometry to draw. Its value is in the selected cell.
rules-grid-preview-select = Select a cell to draw its rule.

# Where a cell's value comes from (its tooltip).
rules-cell-from-all = Default for every object
rules-cell-from-row = Set on this row (reset to { $value })
rules-cell-from-set = From constraint set { $set }
rules-cell-from-parent = Inherited from { $row }
rules-cell-from-default = The built-in default
rules-cell-not-applicable = Does not apply to this row
rules-cell-not-checked = Not checked yet
rules-cell-not-set = Not set

# The filter chips and the find box.
rules-chip-inherited = Show inherited values
rules-chip-overridden-only = Overridden rows only
rules-find-placeholder = Find a constraint, such as creepage
rules-find-at = { $type } · { $domain } › { $group } › { $column }
rules-find-at-of = { $place } ({ $at } of { $count }, Enter for next)
rules-find-count = { $count ->
        [one] 1 match
       *[other] { $count } matches
    }
rules-find-none = No constraint by that name

# Constraint set cards.
rules-sets-applied-to = Applied to
rules-sets-card-values = { $values } · { $severity }

# The detail strip under the grid.
rules-detail-no-rule = No rule sets this cell yet. Type a value in it to write one.
rules-detail-violations = Violations
rules-detail-findings = { $count ->
        [one] 1 on the board
       *[other] { $count } on the board
    }
rules-detail-jump = Jump
rules-why-wins = Which value wins, and why
rules-why-cell = For { $row }, { $group } { $column }: { $value }, { $how }.
rules-why-how-own = set on this row
rules-why-how-set = from constraint set { $set }
rules-why-how-inherited = inherited from { $row }
rules-why-how-default = the built-in default
rules-why-how-not-set = not set
rules-why-level = { $row } · { $value } · { $how }
rules-why-level-wins = ✓ { $row } · { $value } · applies, { $how }
rules-why-level-own = set on the row
rules-why-level-set = set { $set }
rules-why-level-default = default
rules-why-level-not-set = not set
rules-why-extra-hierarchy = A lower row beats the rows above it; a constraint set sits between a row and its parent.
rules-why-extra-layer = A layer value beats the class value on that layer; a net value still beats both.
rules-why-extra-scoped = A scoped rule applies to whatever its scope matches; it beats the hierarchy there.
