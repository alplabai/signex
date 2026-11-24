# The bake preview card of sketch mode, both hosts (mockup row 190): what it lists
# besides the count per target.

# Heading of the rows that say why the sketch does not bake.
sketch-bake-errors = Why it does not bake
# Heading of a footprint sketch's warnings: what the last bake left out,
# tool clicks that were refused and solver notes.
sketch-bake-warnings = Sketch warnings
# One sketch warning; $warning is the warning's own text.
sketch-bake-warning = { $warning }
# Heading of the failed outline checks of a board sketch (mockup row 179d).
sketch-bake-checks = Outline checks
# Under the failed checks: they do not stop Bake.
sketch-bake-checks-warn = A failing check warns only; Bake still applies the sketch.
# Under rows that name a place: what a click does.
sketch-bake-select-hint = Click a row to select its place in the sketch.
# Under the counts: the canvas shows the result.
sketch-bake-drawn = The canvas draws the result in its layer colors, the sketch faint on top.
# Close gap (mockup row 190), offered on an open profile on both hosts: the button,
# and why it is or is not offered. $gap and $tolerance are millimetres.
sketch-bake-close-gap = Close gap
sketch-bake-gap-closes = Gap { $gap } mm, closes under the { $tolerance } mm heal tolerance
sketch-bake-gap-too-wide = Gap { $gap } mm is wider than the { $tolerance } mm heal tolerance
# An open profile Close gap cannot be offered on: an end nothing pairs, or a
# sketch with no solve. $x and $y are millimetres.
sketch-bake-open-end = Open profile at ({ $x }, { $y }) mm
# One open profile in the preview card: $object is its role, $x and $y are
# millimetres, $heal says whether Close gap can join it.
sketch-bake-open-profile = { $object } is open at ({ $x }, { $y }) mm. { $heal }
# The fix on failing inside corners (mockup row 179d); $radius is millimetres.
sketch-bake-add-fillets = Add fillets R { $radius }
# The Roles section of a footprint sketch (mockup row 190): one row per closed
# profile. $index counts the profiles, $edges is its number of edges.
sketch-bake-roles = Roles
sketch-bake-roles-profile = Profile { $index } · { $edges ->
    [one] 1 edge
   *[other] { $edges } edges
}
# A closed profile that is a pad; $number is the pad's designator.
sketch-bake-roles-pad-profile = Pad { $number } profile · { $edges ->
    [one] 1 edge
   *[other] { $edges } edges
}
sketch-bake-roles-empty = No closed profiles yet.
sketch-bake-roles-mask = Solder mask: from the pad rule, no entity
# $count is the number of centerlines.
sketch-bake-roles-centerlines = Centerlines: { $count }, never bake
# The board sketch's Bake panel (mockup row 179): one Bake as row per closed
# profile, as the footprint's Roles rows.
sketch-bake-bake-as = Bake as
# Under the Slot header of a clicked slot edge (mockup row 179): $length end to
# end and $width across, in mm with two decimals.
sketch-bake-slot-size = { $length } × { $width } mm
# Properties after Bake (mockup row 190), until a non-pad object, a
# multi-selection, an edit or an empty click ends it.
sketch-bake-outcome-title = Baked · { $pads ->
    [one] 1 pad
   *[other] { $pads } pads
}
# $items lists what was written, $name is the footprint.
sketch-bake-outcome-wrote = Baked { $items } into { $name }.
sketch-bake-outcome-nothing = The bake wrote nothing into { $name }.
# One entry of $items: a bake target and how many it holds.
sketch-bake-outcome-item = { $target } { $count }
# The row linking a clicked pad to its sketch source. $number is the pad's
# designator, $source names the geometry that drives it.
sketch-bake-outcome-link = Pads ↔ Sketch
sketch-bake-outcome-link-hint = Click a pad
sketch-bake-outcome-link-pad = Pad { $number } · { $source }
sketch-bake-outcome-link-none = Pad { $number } · no sketch source
# The layer colour legend over a footprint's bake preview (mockup row 190).
sketch-bake-legend-top = Top
sketch-bake-legend-paste = Paste
sketch-bake-legend-mask = Mask
sketch-bake-legend-silk = Silk
sketch-bake-legend-courtyard = Courtyard
