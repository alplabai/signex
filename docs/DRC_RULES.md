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

## Mistakes in the text

Run DRC says in Messages what it could not apply, with the line and column,
then a short `DRC rules: …` line with the count, and the DRC panel says that
some rule text was not applied:

- a syntax error: no rule of that text is applied;
- an unknown target, field, condition, object or unit, a missing unit, a
  repeated name or limit: that rule is skipped, the others apply;
- a `hole` or `edge` pair: the rule is not checked yet.

A rule that compiles but whose condition matches no net checks nothing, and
Run DRC says so: a net class that does not exist (`class == "Hv"` for `HV`)
is a warning; a net or pattern this board does not have is an info line.

When two nets ask the same clearance, a finding keeps the more severe of the
two rules: a `then info` rule for one net never turns another net's error
into info.

## Not in this version

Region-scoped rules, clearance to holes and the board edge, annular ring,
differential pairs, arithmetic in lengths, and checking the board setup's
minimum track width and via sizes without a written rule. The DRC panel's
summary counts clearance, shorts and unconnected links only; width and via
findings are listed as rows.
