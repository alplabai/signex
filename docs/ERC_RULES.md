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
| `unannotated_designator` | Warning | A part shown under a designator that is empty or ends in `?`. |
| `single_pin_net` | Warning | A net named by a local label that reaches exactly one part pin. |
| `net_with_no_driver` | Warning | A net with an input pin and no pin that can drive it. |
| `power_input_not_driven` | Warning | A net with a power input pin, no power port or flag, and no pin that can drive it. |
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

`power_input_not_driven` uses the same "can drive" set, so a rail fed through
a `Passive` ferrite or 0R, or by a regulator whose output is typed `Output`,
is not reported. It does not overlap `missing_power_flag`: that rule looks at
power *ports* not cross-referenced by a label, and this one skips every net
that carries a power port or flag.

`single_pin_net` only looks at nets a local label names. A pin with a bare
wire stub to nowhere is already a `dangling_wire` finding, so it is not
reported twice.

These rules attach the net to their finding (as net-level findings do), so a
click highlights it and a waiver keys on its pins. The finding keeps its
own location, so a No-ERC directive on it still suppresses it.

### Designators of reused sheets

The part rules and `units_not_placed` read the designator each placement
shows: a symbol's instance entries (#829, what `designator_in` resolves to),
or its `reference` when it has none. A symbol whose base reference is `R?`
but whose placements are `R7` and `R107` is annotated. A sheet's own run does
not know which placements exist, so a symbol with entries is judged by them.

### Units across sheets

Units of one part are often on different sheets. A sheet checked on its own
reports every unit it does not hold; the app's ERC run replaces those findings
for the project's sheets with one pass over those sheets
(`project_units_not_placed`), so a unit placed on another sheet counts. An
open tab outside the project keeps its own findings, and a No-ERC directive
at the part still suppresses the project finding.

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
| `objects` | Sorted identity of the finding: the UUID of its primary object and `peer:<uuid>` of its peer (the role is kept, so A-against-B and B-against-A stay two findings); `pin:<number>@<x>,<y>` for a finding anchored at one pin (the tip in the symbol's own coordinates, in µm, so two pins sharing a number or name stay apart and moving the part changes nothing), `pin:<a>/<b>` for a pin pair; `tag:<x>` where a rule has several findings on the same objects (a part's placement designator, only for a symbol placed several times, so re-annotating a single placement lapses its waiver; `no-pins` / `not-loaded` on one sheet symbol, the child port a sheet pin disagrees with, `nets:<rails>` for a power-port short, which also carries the two ports' pin numbers); and for a net-level finding the net on its sheet, `net:<name>@<sheet id>`, or `pins:<R1.1,U2.3>@<sheet id>` for an unnamed net. |
| `fingerprint` | SHA-256 over the rule id and the canonical JSON (keys sorted) of each involved object. |
| `reason`, `author` | Required; an entry missing either is kept on disk but never honoured. |
| `date` | `YYYY-MM-DD` the waiver was made. |
| `message` | The finding's text when waived, so a lapsed waiver still says what it was about. |

The identity is `rule_id` + `objects`; positions are never part of it. The
sheet id is the sheet document's own UUID (`schematic_id`), not its path, so
one global net (VBUS, GND) reported on two sheets gives two findings, each
waived on its own; two placements of one sheet file share the id, so one
waiver covers the net in both. The pin number keeps two findings on two pins
of one part apart. A named net's parts are not part of the identity: a part
joining or leaving a waived net changes the fingerprint, so the waiver shows
as LAPSED, not as matching no finding (which would read as fixed). An unnamed
net has nothing but its pins to name it by, so there a membership change
leaves the old waiver matching no finding and the finding shows again. The
fingerprint is what makes a waiver lapse, and it covers only what the rule
depends on, so an unrelated edit never lapses a waiver:

| Finding | Fingerprint covers |
|---|---|
| Part rules (footprint, value, MPN, annotation, duplicate designator, units not placed) | Each part's designators, library id, unit, value, footprint, `on_board` and fields. Not its position, rotation or text placement. |
| Net-level findings (every finding that carries its net) | The net's name and pin set (designator, pin number, electrical type), plus any label or wire the finding names. Not the parts' other fields. |
| Everything else (off-grid, dangling wire, pin conflicts, ...) | The involved objects' connection geometry and names: for a part, the part fields above plus position, rotation, mirroring and body style; for a wire or bus its ends; for a label its text, type and position; for a sheet pin its name, direction and position. Cosmetics (stroke width, font, justification, colour, junction diameter) are left out, so restyling never lapses a waiver. |

Every number is hashed as the integer nanometres the file stores
(`mm_as_stored_nm`), never as the raw `f64`: a length such as 3.81 mm reads
back from disk as 3.8099999999999996, and a waiver must not lapse because the
sheet was saved and reopened. When the covered content changes, the finding
shows again and the panel marks the waiver "LAPSED". Waive again to accept
the new state.

If the first waiver goes into a project that stores no rule set, the project
gets one copied from the machine's preferences (so the waiver has a place to
live), and the Messages panel says so. Undo and redo of the rule set re-run
ERC so the results match.

Waived findings are never dropped silently: the ERC panel lists every stored
waiver with its reason, author, date and status (waiving N findings, lapsed,
matches no finding, or ignored as incomplete), each with a Restore button, and
the ERC report lists them in its waivers section.

Confirm checks the finding against the open sheet the row belongs to (the
Waive action carries that sheet and the fingerprint the row was drawn with).
If the newest run no longer reports the finding, an object it names is gone,
or the sheet changed since that run (the fingerprint differs), Confirm is
refused with a line in the Messages panel and the editor stays open: re-run
ERC and waive what is shown. A waiver therefore never accepts an edit the
user has not seen.

No two findings of one rule on one sheet share an identity (the two dangling
ends of one wire are tagged `start` / `end`, so a waived stub at one end
never hides a later dangle at the other), and identities do not
depend on the order of symbols in the document: which part of a pair is the
primary, and which pin a net finding is reported at, are chosen by UUID. A
guard test runs the
engine over every rule's fixture and over a sheet built to trip many rules at
once (two pin-type conflicts between one pair of parts on different nets, two
touching pin pairs, a part placed twice, two multi-pin power ports shorted
on two separate nets, two unnumbered pins sharing a name) and also re-runs
every case with the symbols reversed and fails on any shared identity (`waivers/violation/identity_guard.rs`).

A pin-anchored waiver names the pin by its number and its position in the
symbol, so switching the part's body style or a library update that moves or
renumbers the pin orphans the waiver: it shows as matching no finding, and
the finding itself is reported again.

Limits: a finding that names neither an object nor a net with a name or
pins, or whose objects are not on the sheet it is filed under, cannot be
waived. Two
placements of one sheet file share object UUIDs, so a waiver covers both.
Rule-specific No-ERC markers (an optional item of #1285) are not implemented.
Per-net waivers keep their existing shape and carry no reason; adding one
would change the stored format of `erc_net_waivers`.
