# The footprint and board sketch canvas: words drawn on the drawing (snap
# badges, values, pattern glyphs, the conflict badge) and the menu a
# right-click on a constraint or pattern opens (#187, #188, #191).

## Snap badges at the cursor.
sketch-canvas-snap-coincident = Coincident
sketch-canvas-snap-horizontal = Horizontal
sketch-canvas-snap-vertical = Vertical
sketch-canvas-snap-angle = Angle
sketch-canvas-snap-intersection = Intersection
sketch-canvas-snap-guide = Guide
sketch-canvas-snap-point = Point
sketch-canvas-snap-pad = Pad
sketch-canvas-snap-board-outline = Board outline
sketch-canvas-snap-board-pad = Board pad
sketch-canvas-snap-board-hole = Board hole
sketch-canvas-snap-board-via = Via
sketch-canvas-snap-board-track = Track
sketch-canvas-snap-board-courtyard = Courtyard

## Values and glyphs on the drawing.
sketch-canvas-conflict-badge = { $count ->
    [one] { $count } conflict · see Properties
   *[other] { $count } conflicts · see Properties
}
sketch-canvas-unit-mm = mm
sketch-canvas-unit-deg = °
sketch-canvas-value-unresolved = ?
sketch-canvas-pattern-linear = L
sketch-canvas-pattern-grid = G
sketch-canvas-pattern-polar = P
sketch-canvas-pattern-count = count
sketch-canvas-pattern-columns = columns
sketch-canvas-pattern-rows = rows
sketch-canvas-pattern-spacing-x = spacing X
sketch-canvas-pattern-spacing-y = spacing Y
sketch-canvas-pattern-sweep = sweep

## The constraint menu.
sketch-canvas-menu-header-bound = { $kind } · { $name }
sketch-canvas-menu-edit-value = Edit value…
sketch-canvas-menu-edit-value-hint = click the value
sketch-canvas-menu-make-parameter = Make parameter…
sketch-canvas-menu-make-parameter-bound = This value is already the parameter { $name }.
sketch-canvas-menu-convert-reference = Convert to reference (driven)
sketch-canvas-menu-convert-reference-why = Not available yet: a sketch cannot store a driven (reference) dimension.
sketch-canvas-menu-suppress = Suppress
sketch-canvas-menu-suppress-why = Not available yet: a sketch cannot store a suppressed constraint.
sketch-canvas-menu-select-entities = Select constrained entities
sketch-canvas-menu-show-conflicts = Show conflicts
sketch-canvas-menu-delete = Delete

## Constraint names in the menu header.
sketch-canvas-kind-coincident = Coincident
sketch-canvas-kind-point-on-line = Point on line
sketch-canvas-kind-point-on-arc = Point on arc
sketch-canvas-kind-horizontal = Horizontal
sketch-canvas-kind-vertical = Vertical
sketch-canvas-kind-parallel = Parallel
sketch-canvas-kind-perpendicular = Perpendicular
sketch-canvas-kind-distance = Distance
sketch-canvas-kind-distance-to-line = Distance to line
sketch-canvas-kind-distance-to-circle = Distance to circle
sketch-canvas-kind-angle = Angle
sketch-canvas-kind-equal-length = Equal length
sketch-canvas-kind-equal-radius = Equal radius
sketch-canvas-kind-tangent = Tangent
sketch-canvas-kind-symmetric = Symmetric
sketch-canvas-kind-midpoint = Midpoint
sketch-canvas-kind-fixed = Fixed

## The pattern menu.
sketch-canvas-pattern-menu-linear = Linear pattern
sketch-canvas-pattern-menu-grid = Rectangular pattern
sketch-canvas-pattern-menu-polar = Circular pattern
sketch-canvas-menu-edit-count = Edit count…
sketch-canvas-menu-edit-columns = Edit columns…
sketch-canvas-menu-edit-rows = Edit rows…
sketch-canvas-menu-edit-spacing-x = Edit X spacing…
sketch-canvas-menu-edit-spacing-y = Edit Y spacing…
sketch-canvas-menu-edit-sweep = Edit sweep angle…
sketch-canvas-menu-delete-pattern = Delete pattern
