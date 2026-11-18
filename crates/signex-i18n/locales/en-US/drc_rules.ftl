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

# A gap under a rule's minimum; at 0 the two touch or overlap.
drc-detail-gap = { $measured } mm apart, needs { $required } mm
drc-detail-overlap = Touching or overlapping, needs { $required } mm apart

# The base rules, shown before the first run. Each names how to write it
# in the Rules tab; the last four need no rule text.
drc-base-rules = The base rules:
drc-base-clearance = Clearance: on clearance require clearance >= …
drc-base-track-width = Track width: on track require width >= … and width <= …
drc-base-annular-ring = Annular ring: on hole require annular >= …
drc-base-hole-size = Hole size: on hole require diameter >= … and diameter <= …
drc-base-hole-to-hole = Hole to hole: on hole require spacing >= …
drc-base-via-in-pad = Via in pad: on via require pad_gap >= 0mm
drc-base-silk-to-pad = Silkscreen to pad: on silk require pad_clearance >= …
drc-base-silk-to-silk = Silkscreen to silkscreen: on silk require spacing >= …
drc-base-mask = Mask sliver and expansion: on mask require sliver >= … and expansion <= …
drc-base-paste = Paste expansion: on paste require expansion >= … and expansion <= …
drc-base-edge = Board outline clearance: on edge require clearance >= …
drc-base-courtyard = Courtyard overlap: on courtyard require spacing >= 0mm
drc-base-keepout = Keepout: always checked against the board sketch's keepouts
drc-base-unrouted = Unrouted net: always checked
drc-base-short = Short circuit: always checked
