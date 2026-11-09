# Design rule check results in the Messages panel (#2222).

drc-run = Run DRC
drc-heading = Design rule check
drc-idle = Run DRC to check the board against its rules.
drc-running = Checking the board...
drc-clean = No violations.
drc-stale = The board changed after this check; run it again.
drc-summary = { $clearance } clearance, { $short } short, { $unconnected } unconnected
drc-skipped = { $count } items could not be checked.
drc-more = and { $count } more not shown

drc-kind-clearance = Clearance
drc-kind-short = Short
drc-kind-unconnected = Unconnected

drc-detail-clearance = { $a } to { $b }: { $measured } mm, needs { $required } mm
drc-detail-short = { $a } shorted to { $b }
drc-detail-unconnected = { $net }: link of { $length } mm not routed

drc-location = { $x }, { $y } mm
drc-layer-top = Top
drc-layer-bottom = Bottom
drc-layer-inner = Inner { $index }
