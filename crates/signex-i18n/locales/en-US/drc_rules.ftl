# The DRC panel's words for the base rules of #2435 and its base-rule help (#2474).

drc-kind-hole-size = Hole size
drc-kind-annular-ring = Annular ring
drc-kind-hole-to-hole = Hole to hole
drc-kind-edge-clearance = Board edge
drc-kind-via-in-pad = Via in pad
drc-kind-silk-to-pad = Silkscreen to pad
drc-kind-silk-to-silk = Silkscreen to silkscreen
drc-kind-mask-sliver = Mask sliver
drc-kind-mask-expansion = Mask expansion
drc-kind-paste-expansion = Paste expansion
drc-kind-courtyard = Courtyard overlap
drc-kind-component-height = Component height
drc-kind-body-clearance = 3D body clearance
# A part taller than a rule's maximum (#2939).
drc-detail-height = { $measured } mm tall, needs at most { $required } mm

# A gap under a rule's minimum; at 0 the two touch or overlap.
drc-detail-gap = { $measured } mm apart, needs { $required } mm
drc-detail-overlap = Touching or overlapping, needs { $required } mm apart
# The net cell of a finding on an object of no net.
drc-detail-no-net = No net

# The base rules (#2474), shown before the first run: each rule's name, its
# rule text and how to write it in the Rules tab. Keepout, unrouted net and
# short are always checked and need no rule text.
drc-base-rules = The base rules
drc-base-clearance = Clearance
drc-base-clearance-text = on clearance require clearance >= …
# $clearance is the length DRC checks while no rule is written, in mm.
drc-base-clearance-how = Replace … with a length, such as 0.15mm. Until then DRC checks { $clearance }mm.
drc-base-track-width = Track width
drc-base-track-width-text = on track require width >= … and width <= …
drc-base-track-width-how = Two lengths, min then max, such as 0.15mm and 1mm.
drc-base-annular-ring = Annular ring
drc-base-annular-ring-text = on hole require annular >= …
drc-base-annular-ring-how = One length, such as 0.13mm.
drc-base-hole-size = Hole size
drc-base-hole-size-text = on hole require diameter >= … and diameter <= …
drc-base-hole-size-how = Min and max diameter, such as 0.2mm and 6.3mm.
drc-base-hole-to-hole = Hole to hole
drc-base-hole-to-hole-text = on hole require spacing >= …
drc-base-hole-to-hole-how = Edge-to-edge gap, such as 0.25mm.
drc-base-via-in-pad = Via in pad
drc-base-via-in-pad-text = on via require pad_gap >= 0mm
drc-base-via-in-pad-how = Keep 0mm: any via touching a pad is reported.
drc-base-silk-to-pad = Silkscreen to pad
drc-base-silk-to-pad-text = on silk require pad_clearance >= …
drc-base-silk-to-pad-how = One length, such as 0.1mm.
drc-base-silk-to-silk = Silkscreen to silkscreen
drc-base-silk-to-silk-text = on silk require spacing >= …
drc-base-silk-to-silk-how = One length, such as 0.1mm.
drc-base-mask = Mask sliver and expansion
drc-base-mask-text = on mask require sliver >= … and expansion <= …
drc-base-mask-how = Min sliver, then max expansion, such as 0.1mm and 0.05mm.
drc-base-paste = Paste expansion
drc-base-paste-text = on paste require expansion >= … and expansion <= …
drc-base-paste-how = Min and max; a negative value shrinks, such as -0.05mm and 0mm.
drc-base-edge = Board outline clearance
drc-base-edge-text = on edge require clearance >= …
drc-base-edge-how = Copper to board edge, such as 0.5mm.
drc-base-courtyard = Courtyard overlap
drc-base-courtyard-text = on courtyard require spacing >= 0mm
drc-base-courtyard-how = Keep 0mm: courtyards must not overlap.
drc-base-keepout = Keepout
drc-base-keepout-text = checked against the board sketch's keepouts
drc-base-keepout-how = Nothing to write: draw keepouts in the board sketch.
drc-base-unrouted = Unrouted net
drc-base-unrouted-how = Nothing to write: every net must be fully routed.
drc-base-short = Short circuit
drc-base-short-how = Nothing to write: two nets touching is always an error.
# Whether the board has a base rule written, uses the default, or checks it always.
drc-base-state-written = written
drc-base-state-default = default
drc-base-state-always = always
# A link that opens the rule in the Rules tab.
drc-base-write-it = Write it
