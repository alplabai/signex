# Board sketch mode of the PCB editor (#2234).

# Name of the mode: the bar over the canvas and the notice card heading.
board-sketch-title = Board Sketch
# Tooltip of the PCB Active Bar button that starts the mode.
board-sketch-tooltip = Board Sketch: draw the board outline, keepouts, cutouts and mounting holes
# Tooltip of the mode bar's Bake on a board (#2518).
board-sketch-bake-tooltip = Apply to board
# Messages panel line when the mode starts.
board-sketch-entered = Board sketch mode: draw with the sketch tools. Bake applies the changes to the board; Discard changes or Esc drops them, asking first.
# Notice and Messages line when the board does not take the sketch; the
# board is unchanged and the mode stays on.
board-sketch-refused = The sketch was not applied to the board: { $reason }
# Messages line when a board closes, or is read again from disk, while its
# sketch has changes that were never applied.
board-sketch-dropped = The board sketch changes of { $board } were not applied and are discarded: the board was closed or read again from disk.
# Messages line and notice when a save, close, quit or undock waits because
# the sketch could not be applied; the copy and the mode stay.
board-sketch-held = { $board } was not saved, closed or undocked: its sketch could not be applied to the board. Fix the sketch or use Discard changes, then try again.
# Messages line when the mode is asked for on a board shown in its own window.
board-sketch-undocked = Board Sketch runs in the main window: dock the board tab first.
# The Board section of the Properties panel.
board-sketch-section = Board
board-sketch-outline = Board outline
board-sketch-hole = Mounting hole
board-sketch-hole-drill = Drill (empty: circle diameter)
board-sketch-hole-plated = Plated
board-sketch-hole-net = Net
board-sketch-roles-hint = The role picker above also sets outline, cutout, keepout, V-score and mounting hole.
# Rows of the Board section when nothing is selected (#2356): the board's
# name and its outline. The size rows reuse the board Properties strings.
board-sketch-name = Name
board-sketch-area = Area
# Area the outline encloses; $area is a square-millimetre number.
board-sketch-area-value = { $area } mm²
board-sketch-entities = Sketch entities
board-sketch-constraints = Constraints
# The Outline checks section of Properties with nothing selected (mockup row 179d),
# live after every solve; the bake preview lists the failed ones.
board-sketch-checks-section = Outline checks
board-sketch-checks-caption = Live after every solve. A failing check warns; Bake still applies the sketch to the board.
board-sketch-checks-unsolved = Not checked: the sketch is not solved.
# A passed check.
board-sketch-check-closed = Closed: one loop
board-sketch-check-no-crossing = No self-intersection
board-sketch-check-openings-inside = Holes, cutouts and keepouts inside the outline
board-sketch-check-corners = Inside corners: the router can cut them
# A check's name, as $check in a check that did not run.
board-sketch-check-name-closed = Closed outline
board-sketch-check-name-crossing = Self-intersection
board-sketch-check-name-openings = Openings inside
board-sketch-check-name-corners = Inside corners
board-sketch-check-skipped-no-outline = { $check }: not checked, the sketch draws no outline (the board keeps its own)
board-sketch-check-skipped-not-closed = { $check }: not checked until the outline is one closed loop
board-sketch-check-skipped-unsolved = { $check }: not checked, the sketch is not solved
board-sketch-check-skipped-router = { $check }: not checked, the project's fab profile names no router tool
# A failed check and its place; $x and $y are millimetres, $gap the gap
# between the two open ends in millimetres.
board-sketch-check-open-gap = Outline open: { $gap } mm gap at ({ $x }, { $y })
board-sketch-check-open-at = Outline open at ({ $x }, { $y })
board-sketch-check-open = Outline open
board-sketch-check-branching = The outline forks: a corner has more than two edges
board-sketch-check-untraceable = The outline cannot be traced through this entity
board-sketch-check-loops = The outline is { $count } loops; it must be one
board-sketch-check-crossing = Self-intersection at ({ $x }, { $y })
board-sketch-check-cutout-outside = Cutout outside the outline
board-sketch-check-keepout-outside = Keepout outside the outline
board-sketch-check-hole-outside = Mounting hole outside the outline
board-sketch-check-opening-outside = Opening outside the outline
# Properties rows of the board targets of decision 2 (2026-11-20). A slot's
# size: $length end to end and $width across, in mm with two decimals.
board-sketch-slot = Slot
board-sketch-slot-size = { $length } × { $width } · routed
board-sketch-slot-not-yet = Not a slot yet: two parallel lines joined by two tangent arcs
board-sketch-pour-layer = Layer
board-sketch-mechanical-layer = Layer
# A mechanical layer by its number, 1 to 32.
board-sketch-mechanical-n = Mechanical { $n }
board-sketch-anchor-component = Component
board-sketch-anchor-pick = Pick a component
# Messages, when Bake meets a locked component; $reference is its designator.
board-sketch-anchor-locked = { $reference } is locked and stays where it is; unlock it to move it to its anchor
# An inside corner tighter than the router tool cuts (mockup row 179d); $radius is the
# corner's radius and $tool the tool's radius, both in millimetres.
board-sketch-check-corner-tight = Inside corner R { $radius } < R { $tool } at ({ $x }, { $y })
# The Fab profile group of the board sketch Properties (mockup row 179d); $diameter
# in millimetres.
board-sketch-fab-section = Fab profile
board-sketch-fab-router = Router tool Ø { $diameter } · from profile
board-sketch-fab-router-none = Router tool: the project's fab profile names none
# The Import DXF dialog of the board sketch (mockup row 179c).
board-sketch-dxf-open = Import DXF…
board-sketch-dxf-title = Import DXF into sketch
board-sketch-dxf-file = File
board-sketch-dxf-units = Units
board-sketch-dxf-units-file = From file
board-sketch-dxf-units-mm = mm
board-sketch-dxf-units-mil = mil
board-sketch-dxf-units-inch = in
board-sketch-dxf-scale = Scale
board-sketch-dxf-scale-hint = 1
board-sketch-dxf-skip = Skip
board-sketch-dxf-sketch-only = Sketch only
board-sketch-dxf-join = Add H / V and Coincident where found
# What Import would make; $healed gaps are closed under the heal tolerance.
board-sketch-dxf-check = { $edges } edges · { $healed } gaps healed · { $open } open ends
board-sketch-dxf-bad-scale = The scale must be a number above 0.
board-sketch-dxf-out-of-range = At this scale the drawing leaves the range a board file holds.
board-sketch-dxf-nothing = Every layer is on Skip: nothing to import.
board-sketch-dxf-import = Import
board-sketch-dxf-cancel = Cancel
board-sketch-dxf-imported = Imported { $edges } edges from { $file } into the sketch: { $healed } gaps healed, { $open } open ends.
board-sketch-dxf-rejected = { $file } was not imported: the sketch could not solve with it.
# The Properties Bake panel of a board sketch while the bake preview is open
# (mockup row 179). $board is the board's name.
board-sketch-bake-title = Bake preview
board-sketch-bake-status = { $board } is unchanged until Bake
board-sketch-bake-solve = Solve
# An outline's extent; $width and $height are millimetres.
board-sketch-bake-size = { $width } × { $height } mm
board-sketch-bake-outline-removed = Removed from the board
# A mounting hole; $drill is its diameter in millimetres.
board-sketch-bake-hole-plated = Ø { $drill } mm · plated
board-sketch-bake-hole-unplated = Ø { $drill } mm · unplated
# A keepout; $layers lists the layers it applies to.
board-sketch-bake-keepout-layers = On { $layers }
# A component anchor; $reference is the designator it moves.
board-sketch-bake-anchor = Moves { $reference }
board-sketch-bake-nothing = Bake writes nothing to the board.
board-sketch-bake-unsolved = The sketch does not bake: it is not solved.
board-sketch-bake-close-hint = Bake preview on the mode bar closes this panel.
# dxf-import-check-step
# The Import DXF profile check and result (mockup row 179c).
board-sketch-dxf-next = Next: check profiles
board-sketch-dxf-problems = { $problems ->
        [one] 1 problem
       *[other] { $problems } problems
    } in { $profiles ->
        [one] 1 profile
       *[other] { $profiles } profiles
    }
board-sketch-dxf-no-problems = No problems in { $profiles ->
        [one] 1 profile
       *[other] { $profiles } profiles
    }
# $profile is the target the profile lands as, such as Board outline.
board-sketch-dxf-gap = { $profile } open: gap { $gap } mm at ({ $x }, { $y })
board-sketch-dxf-open-end = { $profile } open at ({ $x }, { $y }): no end within { $tolerance } mm
board-sketch-dxf-overlap = { $count } overlapping segments at ({ $x }, { $y })
board-sketch-dxf-circles-closed = { $count ->
        [one] 1 circle closed
       *[other] { $count } circles closed
    }
board-sketch-dxf-heal-under = Heal under
board-sketch-dxf-heal = Heal and merge
board-sketch-dxf-as-is = Import as is
board-sketch-dxf-result-title = Imported: { $outlines ->
        [one] 1 outline
       *[other] { $outlines } outlines
    }, { $holes ->
        [one] 1 hole
       *[other] { $holes } holes
    }
board-sketch-dxf-result-others = and { $others ->
        [one] 1 other profile
       *[other] { $others } other profiles
    }
board-sketch-dxf-result-healed = { $healed ->
        [one] 1 gap closed
       *[other] { $healed } gaps closed
    } (end points merged), { $merged ->
        [one] 1 overlapping segment
       *[other] { $merged } overlapping segments
    } merged into one line.
board-sketch-dxf-result-as-is = Imported as drawn: no gap closed, no overlap merged.
board-sketch-dxf-result-constraints = { $constraints ->
        [one] 1 constraint added
       *[other] { $constraints } constraints added
    }; DOF { $dof } left.
board-sketch-dxf-result-unlinked = The DXF is not linked: a later change to the file does not reach this sketch.
board-sketch-dxf-done = Done
# Marks on the preview of the drawing.
board-sketch-dxf-mark-gap = gap { $gap } mm
board-sketch-dxf-mark-overlap = { $count } overlapping segments
# The layer caption under the preview; $list joins the layer entries.
board-sketch-dxf-layers = layer { $list }
board-sketch-dxf-layer-target = { $layer } → { $target }
board-sketch-dxf-layer-skipped = { $layer } ignored
# min-slot
# The min slot outline check (mockup row 179d): every Slot opening at least as
# wide as the fab routes.
board-sketch-check-min-slot = Slots: none narrower than the min slot
board-sketch-check-name-min-slot = Min slot
board-sketch-check-skipped-min-slot = { $check }: not checked, the project's fab profile names no min slot
# A slot narrower than the min slot; $width and $min in millimetres.
board-sketch-check-slot-narrow = Slot { $width } mm wide < min slot { $min } mm
# The Fab profile group's min slot row; $width in millimetres.
board-sketch-fab-min-slot = Min slot Ø { $width } · from profile
board-sketch-fab-min-slot-none = Min slot: the project's fab profile names none

# board-sketch-properties
# The DOF card of the Sketch solve section (mockup row 179). $dof is the
# sketch's free degrees of freedom, $count a whole number, $ms milliseconds.
sketch-dof-number = { $dof }
sketch-dof-number-over = −{ $count }
sketch-dof-under = Under-constrained · { $dof } DOF
sketch-dof-full = Fully constrained
sketch-dof-over =
    { $count ->
        [one] Over-constrained · { $count } conflict
       *[other] Over-constrained · { $count } conflicts
    }
sketch-dof-solved =
    { $count ->
        [one] { $count } constraint · solved in { $ms } ms
       *[other] { $count } constraints · solved in { $ms } ms
    }
sketch-dof-kept =
    { $count ->
        [one] { $count } constraint · last good solve kept
       *[other] { $count } constraints · last good solve kept
    }
# A free motion of the sketch; $count is how many points or radii it moves,
# $angle its direction in degrees from X.
sketch-dof-slide-x =
    { $count ->
        [one] One point slides along X
       *[other] { $count } points slide along X
    }
sketch-dof-slide-y =
    { $count ->
        [one] One point slides along Y
       *[other] { $count } points slide along Y
    }
sketch-dof-slide-angle =
    { $count ->
        [one] One point slides at { $angle }°
       *[other] { $count } points slide at { $angle }°
    }
sketch-dof-any =
    { $count ->
        [one] One point moves in any direction
       *[other] { $count } points move in any direction
    }
sketch-dof-reshape =
    { $count ->
        [one] One entity can change shape
       *[other] { $count } entities can change shape
    }
# Selects what a free motion moves.
sketch-dof-show = Show
# Bake panel rows (mockup row 179). $count holes of one drill and plating;
# $drill is the diameter in millimetres.
board-sketch-bake-holes-plated = { $count } × Ø { $drill } mm · plated
board-sketch-bake-holes-unplated = { $count } × Ø { $drill } mm · unplated
# A keepout; $layers lists its layers, $kinds what it keeps out.
board-sketch-bake-keepout-kinds = On { $layers } · { $kinds }
# The Size row of a rectangular board (mockup row 179e); $width and $height
# are lengths in $unit.
board-sketch-size-value = { $width } × { $height } { $unit }
# A pattern's spacing as its expressions resolve (mockup row 179e); $dx and
# $dy are millimetres.
sketch-pattern-spacing-value = Spacing { $dx } × { $dy } mm
# The Sketch solve section title on a board (mockup row 179): $dof free
# degrees of freedom, $count conflicts.
sketch-dof-section = Sketch DOF
sketch-dof-section-under = Sketch: { $dof } DOF
sketch-dof-section-over =
    { $count ->
        [one] Sketch DOF: { $count } conflict
       *[other] Sketch DOF: { $count } conflicts
    }
# The Fab profile group's profile row (mockup row 179d); $name is the rule
# profile the project's rules were seeded from.
board-sketch-fab-profile = Profile { $name }
# The pattern form of a board sketch (mockup row 179e), titled by its kind.
board-pattern-title-rectangular = Rectangular pattern
board-pattern-title-linear = Linear pattern
board-pattern-title-circular = Circular pattern
board-pattern-source = Source
# The source as "Circle · Mounting hole"; $kind and $role are localized.
board-pattern-source-role = { $kind } · { $role }
board-pattern-count = Count
board-pattern-spacing-x = Spacing X
board-pattern-spacing-y = Spacing Y
# What a spacing expression resolves to; $value is millimetres.
board-pattern-resolved = { $value } mm
# What a bake of the pattern writes; $count is the instances it keeps.
board-pattern-bakes-holes =
    { $count ->
        [one] Bakes into { $count } hole
       *[other] Bakes into { $count } holes
    }
board-pattern-bakes-keepouts =
    { $count ->
        [one] Bakes into { $count } keepout
       *[other] Bakes into { $count } keepouts
    }
board-pattern-bakes-cutouts =
    { $count ->
        [one] Bakes into { $count } cutout
       *[other] Bakes into { $count } cutouts
    }
board-pattern-bakes-slots =
    { $count ->
        [one] Bakes into { $count } slot
       *[other] Bakes into { $count } slots
    }
board-pattern-bakes-pours =
    { $count ->
        [one] Bakes into { $count } pour
       *[other] Bakes into { $count } pours
    }
board-pattern-bakes-v-scores =
    { $count ->
        [one] Bakes into { $count } V-score
       *[other] Bakes into { $count } V-scores
    }
board-pattern-bakes-drawings =
    { $count ->
        [one] Bakes into { $count } mechanical drawing
       *[other] Bakes into { $count } mechanical drawings
    }
board-pattern-bakes-refused = Bake refuses a pattern of the board outline or a component anchor
board-pattern-bakes-unresolved = Bakes once its count and spacing resolve
# The suggestion under an over-constrained sketch's conflict rows (mockup
# rows 179 and 187); $kind names a constraint kind, such as Distance.
sketch-dof-suggest = Suggested: remove the conflicting { $kind }.
sketch-dof-suggest-remove = Remove conflicting constraint
sketch-dof-suggest-convert = Convert to reference
board-sketch-fab-profile-none = Profile: none set. Set one in the project's Rules (Presets) to check corners and slots against it.
# The Inside corners and Min slot lines naming the fab profile they were run
# against (mockup row 179d); $profile is its name.
board-sketch-check-corners-profile = Inside corners: the { $profile } router can cut them
board-sketch-check-min-slot-profile = Slots: none narrower than the { $profile } min slot
board-sketch-check-skipped-router-profile = { $check }: not checked, { $profile } names no router tool
board-sketch-check-skipped-min-slot-profile = { $check }: not checked, { $profile } names no min slot
board-sketch-check-corner-tight-profile = Inside corner R { $radius } < R { $tool } of the { $profile } router at ({ $x }, { $y })
board-sketch-check-slot-narrow-profile = Slot { $width } mm wide < { $profile } min slot { $min } mm
# The bake preview's degrees-of-freedom heading (mockup row 179).
board-sketch-bake-dof = DOF
