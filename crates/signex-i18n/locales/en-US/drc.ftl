# Design rule check results in the Messages panel (#2222).

drc-run = Run DRC
drc-heading = Design rule check
drc-default-clearance = No clearance rule set on this board; checked with { $clearance } mm. Set it in Board Setup.
drc-idle = Run DRC to check the board against its rules.
drc-running = Checking the board...
drc-clean = No violations.
drc-stale = The board changed after this check; run it again.
drc-summary =
    { $clearance ->
        [one] 1 clearance violation
       *[other] { $clearance } clearance violations
    }, { $short ->
        [one] 1 short
       *[other] { $short } shorts
    }, { $unconnected ->
        [one] 1 unconnected link
       *[other] { $unconnected } unconnected links
    }
drc-skipped =
    { $count ->
        [one] 1 item could not be checked.
       *[other] { $count } items could not be checked.
    }
drc-more = and { $count } more not shown
# A violation row was clicked while some of its items sit on hidden layers.
drc-pick-hidden =
    { $count ->
        [one] 1 item of this violation is on a hidden layer and was not selected.
       *[other] { $count } items of this violation are on hidden layers and were not selected.
    }

drc-kind-clearance = Clearance
drc-kind-short = Short
drc-kind-unconnected = Unconnected
drc-kind-keepout = Keepout

drc-detail-clearance = { $a } to { $b }: { $measured } mm, needs { $required } mm
drc-detail-short = { $a } shorted to { $b }
drc-detail-unconnected = { $net }: link of { $length } mm not routed
drc-detail-keepout = { $net }: inside a keepout that forbids it

drc-location = { $x }, { $y } mm
