# Design Notebook measurements (mockup row 199): the timeline of a net, part or pin in
# the notebook's margin lane, the card of a stale attachment, and the status strip's
# measurement counts.

## Timeline
notebook-timeline-title = Timeline
# The tabs of the timeline: what its readings were taken on.
notebook-timeline-target-net = Net { $name }
notebook-timeline-target-part = Part { $name }
notebook-timeline-target-pin = Pin { $name }
# Over the chart: the limit and the calculated value, with their unit.
notebook-timeline-limit = limit { $value }
notebook-timeline-computed = computed { $value }
# In a reading's first line: the design revision and the board serial.
notebook-timeline-rev = rev { $rev }
notebook-timeline-board = board { $board }
notebook-timeline-fail = fail, limit { $limit }
# A reading taken with another value of a part than the schematic holds now.
notebook-timeline-stale = stale: taken with { $part } { $was }
notebook-timeline-empty = No readings of { $name } yet.
notebook-timeline-summary =
    { $readings ->
        [one] 1 reading
       *[other] { $readings } readings
    } · { $fails } fail · { $stale } stale
# A link under the list: write it as a CSV file.
notebook-timeline-export = export CSV

## Stale attachment
notebook-stale-title = { $name } is stale
notebook-stale-why = a part it depends on changed after it was taken
notebook-stale-taken = Taken { $date }
notebook-stale-taken-rev = Taken { $date }, rev { $rev }
notebook-stale-changed = Changed { $part } { $was } → { $now }
notebook-stale-gone = Changed { $part } { $was } → no longer in the design
# Followed by the ids of the readings taken with the attachment.
notebook-stale-also = Also stale
notebook-stale-remeasure = Re-measure…
notebook-stale-mark-valid = Mark still valid
notebook-stale-show-change = Show change
notebook-stale-note = Each attachment stores the values of the parts named in its target when it was added.

## Status strip
notebook-strip-measurements =
    { $readings ->
        [one] 1 measurement
       *[other] { $readings } measurements
    } · { $pass } pass · { $fail } fail
notebook-strip-stale-attachments =
    { $count ->
        [one] 1 stale attachment
       *[other] { $count } stale attachments
    }
