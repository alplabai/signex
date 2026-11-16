# DRC rules

The design rule check (Run DRC) checks a board against its rules. Without any
rule text it checks what it always has: the board setup clearance (or 0.2 mm
when the board sets none), raised by net-class clearances, plus shorts,
unconnected links and keepouts. Rule text adds scoped rules on top. How the
check works and how rules are resolved: [`architecture/pcb-drc.md`](architecture/pcb-drc.md).

## Where rules live

Rule text sits in the rule layers, next to the DRC severities:

- a rule profile (`.snxrules`): `drc.dsl`, used when the project stores no
  rules (the machine's `User Default` profile);
- the project (`.snxprj`): `rules.drc.dsl`, used when the project stores its
  rules, so it checks the same on every machine.

The text is kept and written back exactly as typed.

## Writing a rule

```text
rule <name> on <target> [between <object> and <object>]
  [when <condition> and <condition> ...]
  require <limit> and <limit> ...
  then error|warning|info ["message"]
```

Spaces and line breaks between words are free. Names are letters, digits and
`_`; each rule's id is `user::<name>`.

### Examples

In order: `PWR_MAIN` asks 0.6 mm instead of the 1.0 mm of its class `Power`
(the rule for one net beats its own class; a neighbour that asks more still
gets its own value); every net of class `HV` keeps 0.8 mm; `DDR_*` tracks on
the outer layers are 0.1 mm to 12 mil wide; every via is at least 450 µm
across with a 0.2 mm drill; `SIG_*` tracks keep 0.25 mm from pads. Run
DRC says once that `pwr_main` lowers class `Power` for `PWR_MAIN`; nothing
else lowers a class. (A `track_pad` rule without its `SIG_*` condition would
apply to every net's track–pad pairs and lower `Power` too, and Run DRC
would say so.) A test reads this block and checks these outcomes.

```text
rule pwr_main on clearance when net == "PWR_MAIN" require clearance >= 0.6mm then warning
rule hv_spacing on clearance when class == "HV" require clearance >= 0.8mm
  then error "HV nets need 0.8 mm"
rule ddr_width on track when net matches "DDR_*" and layer == outer
  require width >= 0.1mm and width <= 12mil then warning
rule vias on via require diameter >= 450um and drill >= 0.2mm then error
rule track_pad on clearance between track and pad when net matches "SIG_*"
  require clearance >= 0.25mm then error
```

The language has no comments yet.

### Targets and limits

| Target | Limits |
|---|---|
| `clearance` | `clearance >= <length>` |
| `track` | `width >= <length>`, `width <= <length>` |
| `via` | `diameter >= <length>`, `drill >= <length>` |

A length is a number with its unit: `0.2mm` or `0.2 mm`, `8mil`, `150um`
(or `150µm`), `125000nm`. A limit of 0 is reported: a minimum of 0 checks
nothing, and `width <= 0mm` reports every track in scope. A
clearance over 10 mm is reported as a likely wrong unit, and one over 100 mm
is not applied.

### Conditions

| Condition | Applies to |
|---|---|
| `net == "GND"` | one net |
| `class == "Power"` | the nets of a net class |
| `net matches "USB_*"` | nets whose name matches (`*` any run, `?` one character) |
| `layer == outer`, `layer == inner`, `layer == TopCopper` | copper layers |

A rule takes at most one of the first three and one layer condition. With no
condition the rule is board-wide.

### Object pairs

`between <object> and <object>` limits a clearance rule to two kinds of
object, in either order: `track`, `pad`, `via`. `hole` and `edge` are
accepted but not checked yet: such a rule is reported in Messages and not
applied.

## Which rule applies

For each net, the most specific rule wins, even when it is looser:

1. a rule for one net,
2. a net-class rule,
3. a net-pattern rule,
4. a layer-only rule,
5. a board-wide rule,
6. the board setup clearance and net-class clearances (as without rule text).

Within each level, a rule with more conditions (a layer, an object pair) is
more specific: `class == "HV" and layer == inner` wins over `class == "HV"`
on inner layers. A layer condition and an object pair count the same: a
rule with only a layer condition and one with only an object pair are
equally specific, and the stricter one wins. Between two rules of the same
specificity, the stricter one wins.

A clearance is between two nets, and each net keeps its own rule: **a pair
needs the stricter of its two sides; a rule for one net can loosen that
net's own class value but never the other net's.** A side with no written
rule asks the board setup clearance or its own class's value, whichever is
larger, so a rule for one net below the board setup clearance does not
loosen any pair. A looser rule for `GND` never lowers what an `HV` net asks
of its neighbours.

When a written rule asks less than a net class's clearance for that class's
nets (a board-wide `clearance >= 0.15mm` over a class asking 0.8 mm, say),
Run DRC applies it and says so in Messages, once per rule and class:
`DRC rule 'base' lowers net class 'HV' clearance 0.8 → 0.15 mm`.

## Severity

A finding of a rule has the severity after `then`. A severity entry for the
rule's id (`user::<name>`) in the profile or project overrides it, as for the
built-in rules. The severity is recorded but not shown yet, and `off` does
not hide findings yet.

The Rules tab shows the severity that applies, the entry's when there is
one, and a rule an `off` entry switches off as Off. Its edits keep the entry
in step, in the same undo step: setting the severity (a cell or `rules_set`)
removes the entry, so the word written governs; switching a rule on removes
an `off` entry; renaming moves the entry to the new name; removing or adding
a rule drops any entry for its name.

## Mistakes in the text

Run DRC says in Messages what it could not apply, with the line and column,
then a short `DRC rules: …` line with the count, and the DRC panel says that
some rule text was not applied:

- a syntax error: no rule of that text is applied;
- an unknown target, field, condition, object or unit, a missing unit, a
  repeated name or limit: that rule is skipped, the others apply;
- a minimum above the maximum of the same field (`width >= 0.3mm and
  width <= 0.2mm`, which no track can meet): that rule is skipped;
- a `hole` or `edge` pair: the rule is not checked yet.

A rule that compiles but whose condition matches no net checks nothing, and
Run DRC says so: a net class that does not exist (`class == "Hv"` for `HV`)
is a warning; a net or pattern this board does not have is an info line.

When two nets ask the same clearance, a finding keeps the more severe of the
two rules: a `then info` rule for one net never turns another net's error
into info.

## Turning a rule off

`disabled` before `rule` keeps a rule written but not applied:

```text
disabled rule hv_spacing on clearance when class == "HV" require clearance >= 0.8mm then error
```

A disabled rule is still checked for mistakes and keeps its name, so a second
rule of that name is still the repeated one. It is not counted as "not
applied" in Messages. The Rules tab's On box writes and removes the keyword.

## Rules manager

Design ▸ Rules (also the command palette, `open_rules`, and the Rules button
of the DRC section in Messages) opens the project's rules as a document tab
named **Rules**. One tab per project; Save writes the `.snxprj`.

The table and the rule text are two views of one rule list. Each row is one
rule of the text. Editing a cell rewrites only that rule's text, on one line;
every other rule stays exactly as typed. A rule with a condition or limit the
table has no column for is still a row, marked "Text only", and is edited in
the text view. Every edit (a cell, On, Add, Remove, Apply in the text view) is
one undo step on the project's rule history, the same history the ERC dialog
steps; Ctrl+Z and Ctrl+Y in the tab step it, and ERC runs again when it has
results the step may outdate. An agent steps the same history with
`undo document=<.snxprj>`: of the rule history and the Project tab's, the one
with the project's newest edit first. An edit that would make the rule fail to
compile is refused and the tab says why; nothing changes.

While the project stores no rules, the tab shows this machine's rules (what
Run DRC reads) and says so; the first edit stores them in the project, in the
same undo step.

- **Sub-tabs.** Clearance (clearance rules, object pairs included) and Routing
  (track width, via diameter and drill), each with the findings of its rules
  in the last run. Only what Run DRC checks has a column.
- **Columns.** On, Name, Kind (Routing), Scope, Layer, Objects (Clearance),
  one column per limit, Severity, Actual, Margin, Violations and Flags. A
  length is written as typed (`0.2mm`, `8mil`).
- **Scope cell.** `net == "GND"`, `class == "HV"` or `net matches "USB_*"`;
  empty for every net. While it is typed in, the board's nets and the
  project's classes are offered, and a scope that takes in no net is said.
- **Actual, Margin, Violations.** From the last Run DRC of the project's
  board: the number of findings of the rule, the worst measured value and how
  far past the limit it is. A blank cell says why: not checked yet, stale
  since the last edit (the board or the rule changed after the run), no
  objects in scope, off, or the board is not open. "Passes" is a checked rule
  with no finding.
- **Flags.** A rule that never applies on the board (another rule always
  wins, "shadowed"), two equally specific rules that ask different values of
  the same nets ("conflict"), a rule that lowers a net class's clearance, and
  a scope that matches no net. Found by the check's own resolution over the
  board's nets, copper layers and object pairs.
- **Detail.** The selected rule's text, its problems and flags, how many board
  objects it applies to (Show on board opens the board and selects them), and
  **Why this value?**: for the first net the rule takes in, the value that
  governs it, where it comes from (a rule, the net class, the board setup),
  and how each rule of that limit fared (applies, a more specific rule
  applies, an equally specific stricter rule applies, other nets, layers or
  objects).
- **Text view.** The same rules as text, with what the compiler says about the
  text as typed; Apply stores it as one undo step, Discard drops what was
  typed. While typed text is not applied, Undo, Redo and the switch back to
  the table wait and the tab says so; a text view nobody typed in follows the
  stored rules.
- **Search and filter.** By name, scope, layer or message; All, With
  violations, Flagged, Off.

### Commands

Each takes `document`: the project or any document of it. Edits are one undo
step each and answer with data; refusals say why.

| Command | Answer |
|---|---|
| `rules_list [kind] [enabled]` | `stored_in` (`project` or `machine`), `board`, `syntax_errors`, `rules`: each with `id`, `name`, `enabled`, `applied`, `kind`, `domain`, `scope`, `layer`, `pair`, `limits` (`min_clearance`, `min_width`, `max_width`, `min_diameter`, `min_drill`), `severity` (the word written), `effective_severity` (what a finding gets: a severity entry's, or `off`), `message`, `text_only`, `problems` (with line and column) and `text` |
| `rules_get id` | one rule as above, plus `flags`, `scope_objects` and `result` (`violations`, `measured_mm`, `required_mm`, `margin_mm`, `stale`) when the board is open |
| `rules_add kind values [scope] [layer] [pair] [severity] [name]` | `id`, `name`; `values` is `min_width=0.1mm, max_width=12mil` or one bare length for the kind's first limit |
| `rules_set id field [value]` | `rule`; `field` is `name`, `scope`, `layer`, `pair`, `severity`, `message`, `enabled` or a limit; no `value` clears it |
| `rules_remove id` | `removed` |
| `rules_enable id on` | `id`, `enabled` |
| `rules_effective net [kind] [layer]` | `value_mm`, `source`, `without_rules_mm` and the `chain` of candidates with their `verdict` (`kind` is `min_clearance`, `min_track_width`, `max_track_width`, `min_via_diameter`, `min_via_drill`) |
| `rules_explain violation` | for finding `violation` of the last run: `required_mm`, `rule` and one `sides` entry per net, as `rules_effective` answers |
| `rules_conflicts` | `flags`: `shadowed` (`by`), `conflict` (`with`), `lowers_class` (`class`, `from_mm`, `to_mm`), `no_nets` |

## Not in this version

Region-scoped rules, clearance to holes and the board edge, annular ring,
differential pairs, arithmetic in lengths, and checking the board setup's
minimum track width and via sizes without a written rule. The DRC panel's
summary counts clearance, shorts, unconnected links and keepouts only; width
and via findings are listed as rows.
