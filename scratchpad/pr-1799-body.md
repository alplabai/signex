## Summary

A power port's designator (`#PWR01`) is never drawn, but F2 / Edit Designator and a Find "Designator" hit still edited or selected that hidden field. Now they act on what is drawn: the net name.

## Changes

- F2 / Edit Designator / double-click on a power port opens the Value editor (the net name) and commits through the same `UpdateText { SymbolValue }` command Properties' "Name" row uses, so there is no second rename path and it is one undo step.
- Find "Designator" skips power ports; the net name is still found as a Value hit.
- The reference field's Visible row in Properties reads "No" for a power port, matching the canvas.
- Tests: `f2_on_a_power_port_edits_its_net_name_in_one_undo_step`, `a_power_ports_hidden_designator_is_not_found_but_its_net_name_is` (both red on trunk).

## License compliance (issue #62)

- [x] This is original work, or derived only from sources whose licence I read
      and confirmed Apache-2.0-compatible.

**Derived from something? Name the source and its licence:** n/a

## Crates affected

- [x] signex-engine
- [x] signex-app

## Checklist

- [x] fmt clean; clippy `-D warnings` clean on signex-engine and signex-app
- [x] No growing over-cap file, no discarded tasks, arch guard clean
- [x] Changelog fragment added under `changelog.d/`
- [x] Milestone set

Closes #1799
