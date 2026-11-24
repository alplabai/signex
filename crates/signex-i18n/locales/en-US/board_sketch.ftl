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
