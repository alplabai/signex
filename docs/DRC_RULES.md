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

In order: `GND` may sit 0.1 mm from other copper, closer than its class asks
(the rule for one net wins); every net of class `HV` keeps 0.8 mm; `DDR_*`
tracks on the outer layers are 0.1 mm to 12 mil wide; every via is at least
450 µm across with a 0.2 mm drill; tracks keep 0.25 mm from pads.

```text
rule gnd_close on clearance when net == "GND" require clearance >= 0.1mm then warning
rule hv_spacing on clearance when class == "HV" require clearance >= 0.8mm
  then error "HV nets need 0.8 mm"
rule ddr_width on track when net matches "DDR_*" and layer == outer
  require width >= 0.1mm and width <= 12mil then warning
rule vias on via require diameter >= 450um and drill >= 0.2mm then error
rule track_pad on clearance between track and pad require clearance >= 0.25mm then error
```

The language has no comments yet.

### Targets and limits

| Target | Limits |
|---|---|
| `clearance` | `clearance >= <length>` |
| `track` | `width >= <length>`, `width <= <length>` |
| `via` | `diameter >= <length>`, `drill >= <length>` |

A length is a number with its unit: `0.2mm` or `0.2 mm`, `8mil`, `150um`
(or `150µm`), `125000nm`. A limit of 0 is reported (it checks nothing). A
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
on inner layers. Between two rules of the same specificity, the stricter one
wins.

A clearance is between two nets, and each net keeps its own rule: the pair
needs the stricter of the two. A looser rule for `GND` never lowers what an
`HV` net asks of its neighbours.

## Severity

A finding of a rule has the severity after `then`. A severity entry for the
rule's id (`user::<name>`) in the profile or project overrides it, as for the
built-in rules. The severity is recorded but not shown yet, and `off` does
not hide findings yet.

## Mistakes in the text

Run DRC says in Messages what it could not apply, with the line and column,
and the DRC panel and the summary line say that some rule text was not
applied:

- a syntax error: no rule of that text is applied;
- an unknown target, field, condition, object or unit, a missing unit, a
  repeated name or limit: that rule is skipped, the others apply;
- a `hole` or `edge` pair: the rule is not checked yet.

## Not in this version

Region-scoped rules, clearance to holes and the board edge, annular ring,
differential pairs, arithmetic in lengths, and checking the board setup's
minimum track width and via sizes without a written rule. The DRC panel's
summary counts clearance, shorts and unconnected links only; width and via
findings are listed as rows.
