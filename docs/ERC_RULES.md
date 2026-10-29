# ERC rules

Notes on built-in ERC rules whose behaviour needs more than a code comment.
The rule list itself is `RuleKind` in `crates/signex-erc/src/lib.rs`; every
rule's stable id, label and default severity live there and in `rule.rs` /
`project_rules.rs`.

## Completeness rules

Added for #1286. They are the usual pre-fabrication checks: is every part
complete, and is every net plausibly wired. Each is an ordinary built-in rule:
it has a default severity, can be overridden per project (`.snxprj`
`rules.erc.severity`) and appears in rule profiles. A project whose stored rule
set predates these rules runs them at their defaults (`resolve` lists them as
`missing`).

| Rule (key) | Default | What fires |
|---|---|---|
| `part_without_footprint` | Info | A part placed on the board (`on_board`) none of whose units has a footprint. |
| `part_without_value` | Info | A part none of whose units has a value. |
| `part_without_mpn` | Off | A part none of whose units carries an MPN field (`MPN`, `Manufacturer Part Number`, `Part Number`, the fields the BOM reads). |
| `unannotated_designator` | Warning | A part whose designator is empty or ends in `?`. |
| `single_pin_net` | Warning | A wired or labelled net with exactly one part pin. |
| `net_with_no_driver` | Warning | A net with an input pin and no pin that can drive it. |
| `power_input_not_driven` | Warning | A net with a power input pin and no power output, ground reference, power port or power flag. |
| `units_not_placed` | Warning | A multi-unit part with units placed on no sheet. |
| `off_grid` | Info | A pin tip or wire end off the 1.27 mm (50 mil) connection grid. |
| `labels_only_net` | Warning | A net with labels and wires but no pin. |

### Why these defaults

The defaults are chosen so an existing project does not suddenly drown in new
findings:

- Footprint and value are often filled in late, during capture they are
  expected gaps, so they are Info. Raise them to Warning or Error in the
  project's rule set before release.
- MPN is Off: many projects keep part numbers in a library or a BOM tool rather
  than on the symbol, and the BOM validator already reports it.
- Off-grid is Info: a design drawn on a 0.635 mm grid is legitimate, so the
  finding is advice, not a fault.
- The net rules and annotation are Warning. They point at wiring that is very
  likely wrong, and they skip every case they cannot judge (below), so they
  stay quiet on a sound design.

### What the net rules skip

All four net rules read membership from the shared connectivity
(`merged_connectivity`, the same derivation the netlist uses). A sheet's run
cannot see the rest of a net that leaves the sheet, so a net is skipped when it
carries a Global, Hierarchical or Power label, touches a sheet pin or a bus
entry, or carries a power port or power flag. Pins on a no-connect marker are
not counted.

"Can drive" is generous on purpose: only `Input`, `PowerInput` and
`DoNotConnect` pins cannot. `Passive`, `Unclassified`, `Clock` and the rest
are given the benefit of the doubt, since a resistor pull-up or an
unclassified library pin would otherwise flood the report.

`power_input_not_driven` does not overlap `missing_power_flag`: that rule looks
at power *ports* not cross-referenced by a label, and this one skips every net
that carries a power port or flag.

### Units across sheets

Units of one part are often on different sheets. A sheet checked on its own
reports every unit it does not hold; the app's ERC run replaces those findings
with one project-wide pass over every sheet of the run
(`project_units_not_placed`), so a unit placed on another sheet counts.

Unit numbers come from a file, so the rule never sizes a loop by them alone:
a library symbol claiming more than 256 units (`MAX_UNITS_PER_PART`) is
reported once as likely corrupt instead of being enumerated, and a message
lists at most eight missing units before "(+N more)".

## Waivers

Added for #1285. A designer can accept one finding with a reason instead of
switching its rule off or dropping a position-based No-ERC marker on it.

A waiver is stored in the project, in `rules.erc_waivers` of the `.snxprj`,
beside the per-net waivers (`rules.erc_net_waivers`, #1581) and outside
`rules.erc`, so waiving never changes the rule set's hash or its profile
provenance. Each entry holds:

| Field | Meaning |
|---|---|
| `rule_id` | Stable rule id, `builtin::...` or `user::...`. |
| `objects` | Sorted UUIDs of the objects the finding involves: its primary and peer object, and every part on the net for a net-level finding. |
| `fingerprint` | SHA-256 over the rule id and the canonical JSON (keys sorted) of each involved object. |
| `reason`, `author` | Required; an entry missing either is kept on disk but never honoured. |
| `date` | `YYYY-MM-DD` the waiver was made. |
| `message` | The finding's text when waived, so a lapsed waiver still says what it was about. |

The identity is `rule_id` + `objects`; positions are never part of it. The
fingerprint is what makes a waiver lapse: editing, moving or deleting any
involved object changes it, the finding shows again, and the panel marks the
waiver "LAPSED". Waive again to accept the new state.

Waived findings are never dropped silently: the ERC panel lists every stored
waiver with its reason, author, date and status (waiving N findings, lapsed,
matches no finding, or ignored as incomplete), each with a Restore button, and
the ERC report lists them in its waivers section.

Limits: a finding that names no object (a placeholder-located DSL finding) or
whose objects are not on the sheet it is filed under cannot be waived. Two
placements of one sheet file share object UUIDs, so a waiver covers both.
Rule-specific No-ERC markers (an optional item of #1285) are not implemented.
Per-net waivers keep their existing shape and carry no reason; adding one
would change the stored format of `erc_net_waivers`.
