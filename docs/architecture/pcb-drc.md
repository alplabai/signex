# PCB design-rule check (#2222)

`signex_drc::run(&PcbBoard, &DrcRules) -> DrcReport`: a read-only check of
a board in integer nm. Nothing in it edits the board, so it does not wait on
the document-hosting decision (#661).

The check lives in its own domain crate, `crates/signex-drc` (#2307), the
DRC twin of `signex-erc`. It measures with `pcb-geom` (index, shapes,
distances, copper layers, ratsnest); `pcb-geom` does not depend on it.

## What it reports

| Kind | Meaning |
|---|---|
| `Clearance` | Copper of two different nets that share a copper layer and are closer than the rule. `measured_nm` is the gap, rounded down, always under `required_nm`. |
| `Short` | Copper of two different nets that touches or overlaps on a shared layer (`Shape::overlaps`). Reported once, never also as a clearance. |
| `Unconnected` | One violation per link `ratsnest` still draws. `measured_nm` is the link length, `required_nm` is 0, `items` are the pads at its ends. |
| `Keepout` | A track, via, pad or footprint inside a board-sketch keepout that forbids it, on a copper layer the keepout lists (#2234). One per keepout and item; `items` is the item, `keepout` the keepout's uuid, both distances 0. See "Keepouts" below. |
| `TrackWidth` | A track narrower than a written rule's minimum or wider than its maximum (#2307 step 2b). `measured_nm` is its width, `required_nm` the limit broken. |
| `ViaDiameter` | A via whose diameter is under a written rule's minimum. |
| `ViaDrill` | A via whose drill is under a written rule's minimum. |

Each violation also names the written rule that set its limit (`rule`,
`user::<name>`), or none when the board setup or a net class did. New kinds
are declared after the old ones, so the old rows keep their order.

Each violation carries its items (sorted), the two nets, the shared layer
mask (`copper_layers` lists them), a location and the two distances. The
report is sorted, so a run is the same on every machine.

A gap equal to the rule passes; one nm under fails. The comparison is exact
(`Separation::is_under` compares squared nm) for every pad shape:

| Pad shape | Read as | Effect on the check |
|---|---|---|
| Rect, round pad, oval, track, via | itself | exact; the only rounding is the foot of a perpendicular on a diagonal, under 1 nm |
| Round-rect | its inset rectangle swollen by the corner radius | exact, the same way |

Pads read their real, rotated outline from `pcb_geom::pad_shape` (#680), so
the check measures the copper as drawn. Trapezoid and custom pads are not in
the format (#2242).

## Rules

`DrcRules::for_board(board, net_classes)`:

- the board-wide minimum is `PcbSetup.clearance` (mm, in the `.snxpcb`
  setup); **0 or absent means unset and the default 0.2 mm
  (`DEFAULT_CLEARANCE_MM`) applies**. No file format changed.
- when the default is used, the report says so (`DrcRules::from_default`): the
  Messages panel shows an info line at the top of the results and the log
  summary repeats it. Rules are to be asked for at board creation (#2232).
- a net class (`NetClass.clearance`, the project's net-class table) raises the
  rule for its nets: a pair of nets needs the largest of the board value and
  the two nets' class values. The class of a net is the first matching one,
  as everywhere else. A class value of 0 is unset.
- a value that is not a finite length is an error, not a silent default.

## Rule ids and severities (#2307 step 2a)

Each violation kind is a rule with a stable id, following ERC's `builtin::` /
`user::` convention (`ViolationKind::rule_id`). Ids are stored in profiles and
projects and are never renamed.

| Kind | Rule id | Built-in severity |
|---|---|---|
| `Short` | `builtin::short` | error |
| `Clearance` | `builtin::clearance` | error |
| `Unconnected` | `builtin::unconnected` | warning |
| `Keepout` | `builtin::keepout` | error |
| `TrackWidth` | `builtin::track_width` | error |
| `ViaDiameter` | `builtin::via_diameter` | error |
| `ViaDrill` | `builtin::via_drill` | error |

The last three are only ever found through a written rule, whose own id and
severity a violation carries (see "Rule language" below); their built-in ids
exist so a later check of the board setup's minimums can use them.

Severities follow the #1085 rule layers, a later layer overriding an earlier
one (the precedence between rules of the rule language is a different thing,
see "Precedence" below):

1. the built-in defaults above (`ViolationKind::default_severity`);
2. a profile's `drc.severity` (`.snxrules`, see
   [`snxrules-format.md`](snxrules-format.md));
3. the project's `rules.drc.severity` (`.snxprj`, see
   [`snxprj-format.md`](snxprj-format.md)).

`signex_drc::resolve_severities(profile, project)` applies them. A layer maps
a rule id to `error`, `warning`, `info` or `off`; `off` leaves the rule
without a severity and a later layer can set one again. An id or a severity
word this build does not know is listed in `DrcSeverities::unrecognised`,
kept in the file and not applied; the app logs one line per entry
(`DRC rules: ignoring unrecognised entry (...)`).

The resolved table travels in `DrcRules::severities` (the built-in layer
unless the caller resolved others), so `DrcReport::severity_of(violation)`
gives each violation its severity. A severity never filters or reorders
violations: the report finds the same violations whatever the layers say, and
the Messages panel and log lines do not show severity yet.

The app picks the layers the way ERC picks its rules: a project that stores a
`rules` section uses its own `rules.drc` alone (the project checks the same on
every machine); a board with no project, or a project without a `rules`
section, uses the machine's `User Default` profile as the profile layer. Every
path that makes a project start storing rules carries a `drc` section with it,
so the DRC layer never changes by itself: a new project copies the `drc`
section of the profile it is seeded from, and "Store ERC rules in this
project" and a first ERC waiver copy the machine's `User Default` `drc`
section along with its ERC rules. "Save as profile", "Update from profile" and
the drift check still handle only `erc`; the `drc` section joins them in
step 2b-2.

## Rule language (#2307 step 2b)

Rules are written as text in the rule layers: a profile's `drc.dsl`
(`.snxrules`) and the project's `rules.drc.dsl` (`.snxprj`), never a loose
file. The user reference with examples is [`DRC_RULES.md`](../DRC_RULES.md).
`crates/signex-drc-dsl` parses, validates and compiles the text into
`signex_drc::UserRule`s, in the style of `signex-erc-dsl` (chumsky 0.12,
parse, then validate and compile, byte-span diagnostics); `signex-erc-dsl` is
not touched and a shared core can come later.

```text
program  := rule*
rule     := "rule" NAME "on" TARGET pair? when? require "then" severity STRING?
pair     := "between" OBJECT "and" OBJECT
when     := "when" cond ("and" cond)*
cond     := SUBJECT ("==" | "matches") (STRING | WORD)
require  := "require" limit ("and" limit)*
limit    := FIELD (">=" | "<=") NUMBER UNIT        (a space before UNIT only for a known unit)
severity := "error" | "warning" | "info"
```

| Target | Fields | Pair | v1 |
|---|---|---|---|
| `clearance` | `clearance >=` | `between` two of `track`, `pad`, `via`, `hole`, `edge` | checked between copper items (track, pad, via); a pair with `hole` or `edge` is valid but not checked yet: a warning says so and the rule is not applied |
| `track` | `width >=`, `width <=` | none | checked |
| `via` | `diameter >=`, `drill >=` | none | checked |

| Condition | Scope | Net-scope rank |
|---|---|---|
| `net == "GND"` | one net, by name | 3 |
| `class == "Power"` | the nets of a net class (first matching class) | 2 |
| `net matches "USB_*"` | a net-name glob (`*`, `?`, as net classes) | 1 |
| none of the above | every net | 0 |
| `layer == outer` / `inner` / `TopCopper` | copper layers; a narrowing condition | (adds 1) |

A rule takes at most one net, class or pattern condition and one layer
condition. Words the grammar keeps open on purpose: a region scope (v2) is one
more condition subject (`region == "BGA1"`, reported today as "not supported
yet"), mask and silkscreen are more pair objects, and more targets or fields
join the tables above, all without a grammar change.

Units are `mm`, `mil`, `um` (or `µm`) and `nm`, converted to integer nm with
`pcb_geom::mm_to_nm`. A unit may follow the number directly (`0.2mm`) or after
spaces (`0.2 mm`); after a space only these unit words count, so `0.2 and`
is a missing unit, not a unit called `and`. There is no expression
arithmetic (#1984, #1989). A limit of 0 is a warning (it checks nothing); a
clearance over 10 mm is a warning and one over 100 mm is refused, since a
clearance widens the search around every copper item it covers.

### Precedence

Specificity first, strictness only between equals (maintainer, 2026-11-13).
A rule's specificity is its net-scope rank, then the number of narrowing
conditions it adds (a layer condition, an object pair): `class == "HV" and
layer == inner` is more specific than `class == "HV"`, and a layer-only rule
is above a board-wide one. A layer condition and an object pair count the
same, so a layer-only and a pair-only rule are equally specific and the
stricter decides. The most specific written rule that covers an
object applies, even when it is looser than a less specific one: a rule for
one net beats its class. Among rules of equal specificity the strictest wins
(the largest minimum, the smallest maximum); identical ones go to the first
written.

Specificity is per object. A clearance has two sides: for each side's net, on
each shared copper layer, the most specific written rule covering that net
(and the pair's object kinds) gives the side's value, else the net's implicit
value; the pair needs the stricter of its two sides and the most of its
layers. **A pair needs the stricter of its two sides; a rule for one net can
loosen that net's own class value but never the other net's**, written or
implicit, exactly as the implicit tier has always combined two nets' class
values. Since a side with no written rule falls back to at least the setup
value, a rule for one net below the setup value loosens no pair. This
per-side reading of decision 6837552967 §1 is an implementation choice
pending the maintainer's confirmation. On equal values a written rule is
named over the implicit one, so the finding carries its id and severity.

A written rule that wins for a net below that net's class clearance is
applied and said, never silently clamped: one Messages warning per rule and
class, `DRC rule 'base' lowers net class 'HV' clearance 0.8 → 0.15 mm`,
checked on every copper layer and object pair a rule names.

Today's values are the lowest, implicit rules, below every written rule: the
board setup clearance (or the 0.2 mm default) and the net-class clearances.
A side with no written rule takes the larger of the setup value and its
class's value, so a pair no written rule covers needs what it always did and
a board with no rule text gives the same results, in the same order, with
the same Messages lines.

The broad phase searches around each copper item as far as the widest
clearance any pair with that net can need: its implicit value and the
widest written rule covering the net. A pair whose other side asks for more
is found from that other item.

Track width and via size are checked only against a limit a written rule
sets. The board setup's `track_min_width`, `via_min_diameter` and
`via_min_drill`, and the net classes' routing widths and via sizes, are not
enforced: doing so would report on existing boards, a separate behaviour
change for the maintainer to decide.

### Ids, severities, layers and Messages

A written rule's id is `user::<name>` (ERC's convention); its severity is the
one after `then`, and a `drc.severity` entry for that id overrides it like a
built-in rule's, a later layer winning (`resolve_severities_with`). An entry
for a `user::` id no rule has stays unrecognised. As for built-in rules the
severity is recorded on the report only: it is not shown yet and `off` does
not hide a finding.

The app compiles the same layers it takes severities from (the project's
`rules.drc` when the project stores rules, else the machine's `User Default`),
inside the `spawn_blocking` job, never in `update`. `compile_layers` reads the
profile then the project text; a project rule replaces a profile rule of the
same name. A syntax error applies no rule of that text, an invalid rule (or a
second rule of one name) is dropped alone, and every such case is one
Messages warning naming the layer, line, column and what was not applied, for
example `DRC DSL (project): line 2, column 33: rule 'vias': unknown unit 'in'
(mm, mil, um or nm); the rule is skipped`. A run with rules logs `DRC DSL:
applying N rule(s)`, a `DRC: N track width, N via diameter, N via drill`
summary when any is found, and the rule id after each logged finding of a
written rule.

Rule text that was not applied is also said where the results are read, so
a typo never passes as a clean board: a short warning line of its own
(`DRC rules: N written rule(s) not applied`, `DRC rules: the rule text was
not applied (syntax error)`, kept apart from the summary so the Messages
length cap never cuts it off), and a warning line in the DRC panel above
the results. With
written rules the default-clearance note and the summary say the board value
holds only where no written rule applies. All of these appear only when rule
text is present, so a run without it logs and shows what it always did.

Known limit: the panel's summary counts clearance, shorts and unconnected
links only; keepout (#2300), track width and via size findings are listed as
rows but not counted there yet.

## How it stays fast

Candidate pairs come from the `BoardIndex` R-tree: one bounding-box query per
item, grown by the largest rule, then the exact test on the pairs it returns.
`crates/signex-drc/tests/drc_corpus.rs` times a 10,000-track board in both a
clean and a 29,502-violation run and asserts a loose bound (10 s in debug CI;
a debug run takes about 0.35 s).

## In the app

`run_drc` (command catalog, group PCB, enabled with a PCB document), the
Tools > Design Rule Check menu row (greyed with no PCB), the command palette
and the Run DRC button in the Messages panel send `Message::Pcb(PcbMsg::DrcRequested)`.
The board is copied and checked in `spawn_blocking`; a result is applied only
if no newer run began. The Messages panel lists the violations (first 500)
above the log, and clicking a row selects its items with the #2191
selection. The summary and the first 20 violations are also logged, which is
what a headless run reports. A report is marked stale when the board's
revision moved after it.

## Zones

`Unconnected` is the ratsnest, so it joins copper through zones the way the
ratsnest does (#2215): a pad island joined only through a same-net pour is not
reported open. The pour itself is not checked: a zone's clearance to other
nets (#2216) and its fill are not modelled, so a zone is never a `Clearance`
or `Short` party.

## Keepouts

The keepouts are the ones the board sketch baked (`PcbBoard::sketch`,
`docs/PCB_SKETCH.md`). Any net counts, net 0 included: a keepout keeps out
copper, not a net.

| Keepout forbids | Reported |
|---|---|
| `no_routing` | tracks on a listed layer |
| `no_vias` | vias reaching a listed layer |
| `no_copper` | tracks, vias and pads on a listed layer |
| `no_drilling` | vias and drilled pads, whatever their layers |
| `no_components` | a footprint whose body (`footprint_body`) meets the keepout, when the outer copper layer of its side is listed |

An item meets a keepout when its copper touches the keepout polygon or lies
inside it (`Shape::overlaps_polygon`). A keepout that lists no copper layer
forbids nothing this check sees. Each keepout is tested against every item,
with a bounding-box test first; an index lookup is a follow-up if boards with
many keepouts make it slow.

## Not checked yet

Hole-to-hole, silk-to-pad, courtyard overlap, zone clearance (#2216), annular
ring, copper with no net (net 0 is skipped, like in the ratsnest), clearance
to holes and the board edge (the rule language parses them), region-scoped
rules (v2) and the board setup's minimum width and via sizes. Each is its own
follow-up.

From the board sketch (#2234): `no_pours` keepouts (zones are not tested
against keepouts), board mounting holes in a `no_drilling` keepout, and
copper-to-board-edge and copper-to-cutout clearance: the rules hold no edge
clearance yet, so the baked outline and cutouts are not checked.
