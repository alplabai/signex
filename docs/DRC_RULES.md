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
| `via` | `diameter >= <length>`, `drill >= <length>`, `pad_gap >= <length>` (via in pad) |
| `hole` | `diameter >= <length>`, `diameter <= <length>` (hole size), `annular >= <length>` (annular ring), `spacing >= <length>` (hole to hole) |
| `edge` | `clearance >= <length>` (copper to the board outline) |
| `silk` | `pad_clearance >= <length>`, `spacing >= <length>` (to other silkscreen) |
| `mask` | `sliver >= <length>`, `expansion >= <length>`, `expansion <= <length>` |
| `paste` | `expansion >= <length>`, `expansion <= <length>` |
| `courtyard` | `spacing >= <length>` |
| `component` | `height <= <length>` (component height), `clearance >= <length>` (3D body clearance) |

A `silk`, `mask`, `paste` or `courtyard` rule is board-wide: it takes no
`when` condition. A `component` rule takes one condition only, the side its
parts sit on: `layer == TopCopper` or `layer == BottomCopper`. What each one
measures is under [The base rules](#the-base-rules) and, for `component`,
[Component height and 3D body clearance](#component-height-and-3d-body-clearance).

A length is a number with its unit: `0.2mm` or `0.2 mm`, `8mil`, `150um`
(or `150µm`), `125000nm`. A limit of 0 is reported: a minimum of 0 checks
nothing, `width <= 0mm` reports every track in scope and `diameter <= 0mm`
every hole. A gap of 0
(`pad_gap`, a hole, silkscreen or courtyard `spacing`, `pad_clearance`, an
edge `clearance`) still finds objects that touch or overlap, a minimum
expansion of 0 finds negative ones and a maximum expansion of 0 finds
positive ones, so those are not reported. A
clearance over 10 mm is reported as a likely wrong unit, and one over 100 mm
is not applied.

### Conditions

| Condition | Applies to |
|---|---|
| `net == "GND"` | one net |
| `class == "Power"` | the nets of a net class |
| `net matches "USB_*"` | nets whose name matches (`*` any run, `?` one character) |
| `layer == outer`, `layer == inner`, `layer == TopCopper` | copper layers |
| `class == "HV" and other_class == "LV"` | a clearance between a net of each class, in either order (a class matrix cell) |

A rule takes at most one of the first three and one layer condition. With no
condition the rule is board-wide. `other_class` needs a `class` condition and
a clearance rule.

### Object pairs

`between <object> and <object>` limits a clearance rule to two kinds of
object, in either order: `track`, `pad`, `via`. `hole` and `edge` are
accepted but not checked yet: such a rule is reported in Messages and not
applied. Hole to hole is `on hole require spacing`, and copper to the board
outline is `on edge require clearance`.

## The base rules

The PCB Design scope of [`ROADMAP.md`](ROADMAP.md) names 15 base rules
(#2474). Twelve are written as rule text; the last three are always checked.
A test runs each one on a board that passes it and one that breaks it,
through `check_drc` and Run DRC. Only a limit a rule sets is checked: a board
with no rule text is checked for clearance, shorts, unconnected links and
keepouts alone.

| Base rule | Written as | What is measured |
|---|---|---|
| Clearance | `on clearance require clearance >= …` | copper of two nets, edge to edge |
| Track width | `on track require width >= … and width <= …` | each track and copper arc |
| Annular ring | `on hole require annular >= …` | (smallest copper size − drill) / 2 of each via and plated pad; an unplated hole has none |
| Hole size | `on hole require diameter >= … and diameter <= …` | the drill of each via and pad hole; the minimum skips a via a via `drill` rule covers, so a too-small via drill is one finding |
| Hole to hole | `on hole require spacing >= …` | two holes, edge to edge, whatever their nets; the pair needs the larger of its two holes' limits |
| Via in pad | `on via require pad_gap >= 0mm` | a via's copper against each surface-mount pad of its own net (or of no net) on a shared layer; `0mm` finds a via in a pad, a larger value also one too near it |
| Silkscreen to pad | `on silk require pad_clearance >= …` | silkscreen against each pad with copper on that side's outer layer |
| Silkscreen to silkscreen | `on silk require spacing >= …` | the silkscreen of two parts, or of a part and a board drawing or string; two board drawings or strings are not checked against each other |
| Mask sliver and expansion | `on mask require sliver >= … and expansion <= …` | the mask strip between two openings on one side (openings that touch leave none); the manual mask expansion of vias and tracks |
| Paste expansion | `on paste require expansion >= … and expansion <= …` | the manual paste expansion of tracks |
| Board outline clearance | `on edge require clearance >= …` | each copper item against the board outline, at its closest point; copper of no net included (#2515) |
| Courtyard overlap | `on courtyard require spacing >= 0mm` | the courtyards of two parts on one side |
| Keepout | always | the board sketch's keepouts |
| Unrouted net | always | the links the ratsnest still shows |
| Short circuit | always | copper of two nets that touches; copper of no net counts as another net |

What each measurement leaves out for now: a part's silkscreen is the strokes
of its drawings, and a part's own texts are not measured; a board string is
the box its glyphs fill. A mask opening is a pad's copper (a pad has no
expansion of its own), a via not tented on that side grown by its manual
expansion, or a track with a manual mask expansion. A courtyard is the turned
box of the part's courtyard drawings; a part without any is not checked. The
board outline is the saved outline; cutouts inside the board are not edges
yet, and copper wholly outside the outline is not reported.

Copper of no net (#2515): pads, free pads, tracks, copper arcs and vias of
net 0, and drawings and strings on copper (#2883), listed by
`pcb_geom::no_net_copper`. The clearance and short checks measure it
against the copper of every net, and the board outline clearance against
the edge; two items of no net are never a pair. It never makes a
connection: the ratsnest and the unrouted check read netted copper only, so
a track of no net across two pads of one net shorts to each and leaves
them unrouted. An unplated pad of no net is copper only on the copper
layers it lists, as the Gerber flashes it. A part's drawings on a copper
layer are not copper (see `docs/PCB_TOOLS.md`, "Copper of no net").

The DRC panel lists the base rules before the first run. A finding of each
kind is a row of the DRC panel and a `check_drc` finding filed under its
written rule (`user::<name>`), with the kind's built-in id
(`builtin::hole_size`, `builtin::annular_ring`, `builtin::hole_to_hole`,
`builtin::edge_clearance`, `builtin::via_in_pad`, `builtin::silk_to_pad`,
`builtin::silk_to_silk`, `builtin::mask_sliver`, `builtin::mask_expansion`,
`builtin::paste_expansion`, `builtin::courtyard_overlap`) for its severity
and waiver key.

**Part outside its room** (`builtin::part_outside_room`, #2457) is a
built-in kind with no rule text: error by default, its severity set like any
built-in's. It checks only rooms with Keep parts inside: a member part whose
body is not inside the room's outline shrunk by the room's margin is a
finding naming the part, the room and the distance ("R2 is 1.2 mm outside
room dcdc"). A board without such rooms gets the report it got before. Rooms:
[PCB_ROOMS.md](PCB_ROOMS.md#design-rule).

## Component height and 3D body clearance

Two limits of the `component` target (#2939) check the parts' 3D bodies.
Neither is a base rule: a board is checked for them only when a rule sets
them, and only then are the library models read.

```text
rule tall on component require height <= 6mm then error
rule bottom_tall on component when layer == BottomCopper require height <= 2mm then error
rule bodies on component require clearance >= 0.25mm then warning
```

A part's 3D body is an outline on the board and a span of heights above the
face it stands on, taken in this order:

1. the model file attached to its library footprint, measured as the box of
   its points (placed by the attachment's offset and turn, as the 3D view
   draws it): source `model`;
2. else the library footprint's 3D Body: its outline (or the part's
   courtyard, else its pads), raised by its offset and as tall as its height:
   source `body`;
3. else the Height stored on the part, from the board up over its courtyard
   (else its pads): source `stored`.

A part with none of these has no height and is not checked; a library that
is not open, or a model that cannot be read, leaves the stored Height, and
Run DRC and `check_drc` say which model was not read. `query_part_heights`
answers each part's height and source (`docs/CLI.md`, "Part heights").

- **Component height** (`builtin::component_height`): the tallest point of
  a part's body above its face over the limit is a finding naming the part,
  its height and the limit ("U3 is 7 mm tall, needs at most 6 mm"). A rule
  with a `layer` condition is for the parts on that side only; a part on a
  side with no such rule takes the board-wide one.
- **3D body clearance** (`builtin::body_clearance`): two parts on one side
  whose bodies come closer than the limit in 3D are a finding naming both,
  with the distance. The distance combines the gap between their outlines
  and the gap between their height spans, so a short part under a raised
  body (a body with an offset, over the part) is measured straight up, and
  is found when the gap is under the limit. `clearance >= 0mm` finds bodies
  that touch or overlap.

Both findings carry `designators` in their evidence beside `measured_nm`
and `required_nm`, are errors by default and are set on the Rules tab's
Placement domain: Height ▸ Max and Component Clearance ▸ 3D body. A region
condition is not supported yet.

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
specificity, the stricter one wins. A `class` and `other_class` rule is a
net-class rule with one more condition: for a clearance between a net of each
class it wins over a rule for either class alone, on both sides, and a rule
for one net still wins over it.

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

The constraint grid and the rule text are two views of one rule list. A
cell is a limit of one row of the object hierarchy; editing it rewrites the
row's own rule that the cell shows (the stricter of two equal rules), or adds
one (`all_clearance`, `class_HV_track`, `net_GND_via`, `layer_L1_track`), and
every other rule stays exactly as typed. A value the cell would still not
show, because another rule of the row or the fab floor is stricter, is
refused and the tab names that rule; a value equal to that rule's is taken.
Pressing a cell names its rule in the detail, or none. A rule the hierarchy
cannot express (a
pattern, an object pair, a layer on a class, a condition or limit the grid
has no column for), and a rule no cell shows (one that is off or that the
compiler refuses, or one another rule of its row beats on every limit it
sets, on any row), is a row of the Scoped rules group and is edited there or
in the text view, so its On and Remove stay in reach. Every edit (a
cell, ↺, a value on the drawing, On, Add, Remove, Apply in the text view) is
one undo step on the project's rule history, the same history the ERC dialog
steps; Ctrl+Z and Ctrl+Y in the tab step it, and ERC runs again when it has
results the step may outdate. An agent steps the same history with
`undo document=<.snxprj>`: of the rule history and the Project tab's, the one
with the project's newest edit first. An edit that would make the rule fail to
compile is refused and the tab says why; nothing changes.

While the project stores no rules, the tab shows this machine's rules (what
Run DRC reads) and says so; the first edit stores them in the project, in the
same undo step.

- **Domains.** The tabs over the grid: Clearance, Physical, High speed,
  Manufacturing, SMT, Mask, Plane, Testpoint, Placement and Signal
  integrity, each with the findings of its rules in the last run (a rule
  changed since the run is not counted). Each rule
  type is a column group of its domain, with its own findings badge; a click
  on a group header collapses it. Every rule type has its columns; a column
  Run DRC does not check yet is inert and says "Not checked yet". Signal
  integrity is greyed: its constraints arrive with the simulation band.
- **Rows.** All, then Net Classes (each project class with its board nets
  under it, and the nets of no class that have rules of their own), Layers
  (the board's copper layers by position: L1 is Top, the last is Bottom, the
  ones between are L2, L3, ...; then, by the name the rules use, a layer the
  rules write for that the board lacks, or every written layer with no board
  open) and Scoped rules. When the rows are laid out again (the rules or the
  board changed), the selected cell stays selected. A row group shows only on
  the domains whose columns apply to it: Placement has no net rows, and
  layers carry copper limits only. A column that does not apply to a row is
  hatched; a board-wide kind (silkscreen, mask, paste, courtyard) is set on
  All only.
- **Inheritance.** A cell resolves down the hierarchy: the row's own rule,
  else a constraint set applied to the row, else its parent (a net's class,
  then All), else the fab tier default. A clearance cell no rule sets shows,
  muted, the clearance Run DRC checks with anyway: the open board's setup
  clearance (0.2 mm when it sets none or no board is open), raised for a class's nets by the
  class's own clearance; All shows it even while inherited values are
  hidden. Every other All cell is empty until a fab tier is picked, then
  shows the tier's floor, also while inherited values are hidden.
  Short-circuit (Allowed) and Un-routed net (Check) show the fixed No and
  On on All, since Run DRC always checks them. A value of the row's own is bold
  with ↺, which drops that value only; an inherited value is muted; a value
  a constraint set wrote has a blue edge, and the row carries a ⧉ chip
  naming the set. The tooltip names where the value comes from. Show
  inherited values (on by default) and Overridden rows only (rows with a
  value of their own in the domain, and All) filter the view. A length is
  written as typed (`0.2mm`, `8mil`, `50µm`) and the cell shows it so. A bare
  number typed in a cell is in mm; an empty cell resets it.
- **Find a constraint.** Typing a rule type or column name (two letters list
  the matches, three jump) switches to its domain, opens its group, selects
  its column on the selected row (else All) and scrolls it into view; Enter
  steps to the next match.
- **Toolbar.** Table or Text, Run DRC (the tab stays and the counts fill in),
  Constraint sets (the set cards), Add (a board-wide rule of the selected
  column's kind) and Presets. On Clearance, Matrix switches to the class
  matrix.
- **Detail strip.** Under the grid, in three columns: the selected cell's
  live drawing with its row's resolved values, each value editable on the
  drawing (Enter or a dragged handle writes the cell, as one undo step; Tab
  and Shift+Tab move between values; a value of 0 keeps its label); the rule
  that governs the cell, with its text, problems, flags and its findings in
  the last run, or why it has no count: stale since the last edit (the board
  changed after the run), changed since the run (the rule), off, no objects in
  scope, open the board to check, not checked yet, or passes (Jump opens the
  board and selects the objects it covers); and **Which value wins, and
  why**, walking the row,
  its constraint set, its parents and the default, the winner marked, then
  the board's resolution for the first net the rule takes in.
- **Flags.** A rule that never applies on the board (another rule always
  wins, "shadowed"), two equally specific rules that ask different values of
  the same nets ("conflict"), a rule that lowers a net class's clearance, a
  scope that matches no net, and a rule below the fab floor ([Fab
  profiles](#fab-profiles)). Found by the check's own resolution over the
  board's nets, copper layers and object pairs.
- **Text view.** The same rules as text in a code field with line numbers and
  keyword colours. A line the compiler rejects is washed red with the bad span
  in the error ink, and each problem is a banner under the field ("Line 9,
  column 32: ..."); clean text shows "No problems in the text." in green.
  Apply stores the text as one undo step and is off while the text has an
  error; Discard drops what was typed. Its bar reads "All domains" with a
  "Text view" tag in place of the domain tabs, and keeps the Table and Text
  switch, Constraint sets, Presets and Run DRC. While typed text is not
  applied, Undo, Redo, the switch back to the table, a set's Use these limits
  and Apply to scope, and a preset's Apply wait and the tab says so; a text
  view nobody typed in follows the stored rules.

### Commands

Each takes `document`: the project or any document of it. Edits are one undo
step each and answer with data; refusals say why.

| Command | Answer |
|---|---|
| `rules_list [kind] [enabled]` | `stored_in` (`project` or `machine`), `board`, `syntax_errors`, `sets` (as `rules_set_create` answers each), `rules`: each with `id`, `name`, `enabled`, `applied`, `kind`, `domain` (the grid domain it shows on: `clearance`, `physical`, `high_speed`, `manufacturing`, `smt`, `mask`, `plane`, `testpoint`, `placement`), `scope`, `layer`, `pair`, `limits` (by column id: `min_clearance`, `min_width`, `max_width`, `min_diameter`, `min_drill`, `min_pad_gap`, `min_hole`, `max_hole`, `min_annular`, `min_hole_spacing`, `min_edge_clearance`, `min_silk_pad`, `min_silk_spacing`, `min_sliver`, `min_mask_expansion`, `max_mask_expansion`, `min_paste_expansion`, `max_paste_expansion`, `min_courtyard_spacing`, `max_height`, `min_body_clearance`), `severity` (the word written), `effective_severity` (what a finding gets: a severity entry's, or `off`), `message`, `text_only`, `problems` (with line and column) and `text` |
| `rules_get id` | one rule as above, plus `flags`, `scope_objects` and `result` (`violations`, `measured_mm`, `required_mm`, `margin_mm`, `stale`) when the board is open |
| `rules_add kind values [scope] [layer] [pair] [severity] [name]` | `id`, `name`; `kind` is a target (`clearance`, `track`, `via`, `hole`, `edge`, `silk`, `mask`, `paste`, `courtyard`, `component`); `values` is `min_width=0.1mm, max_width=12mil` or one bare length for the kind's first limit |
| `rules_set id field [value]` | `rule`; `field` is `name`, `scope`, `layer`, `pair`, `severity`, `message`, `enabled` or a limit; no `value` clears it |
| `rules_remove id` | `removed` |
| `rules_enable id on` | `id`, `enabled` |
| `rules_effective net [kind] [layer]` | `value_mm`, `source`, `without_rules_mm` and the `chain` of candidates with their `verdict` (`kind` is a limit id: `min_clearance`, `min_track_width`, `max_track_width`, `min_via_diameter`, `min_via_drill`, `min_hole_size`, `max_hole_size`, `min_annular_ring`, `min_hole_to_hole`, `min_edge_clearance`, `min_via_pad_gap`, `min_silk_to_pad`, `min_silk_to_silk`, `min_mask_sliver`, `min_mask_expansion`, `max_mask_expansion`, `min_paste_expansion`, `max_paste_expansion`, `min_courtyard_gap`, `max_component_height`, `min_body_clearance`) |
| `rules_explain violation` | for finding `violation` of the last run: `required_mm`, `rule` and one `sides` entry per net, as `rules_effective` answers |
| `rules_conflicts` | `flags`: `shadowed` (`by`), `conflict` (`with`), `lowers_class` (`class`, `from_mm`, `to_mm`), `no_nets`, `below_fab_floor` (`floor`, `limit`, `from_mm`, `to_mm`) |
| `rules_matrix_get` | `classes`, `cells` (each `class_a`, `class_b`, `object_pair`, `layer`, `value`, `rule`, `enabled`), `defaults` (each `class_a`, `class_b`, `object_pair`, `value_mm`) and `defaults_from_board` |
| `rules_matrix_set class_a class_b [object_pair] [layer] [value]` | `change` (`added`, `changed`, `removed`, `unchanged`), `rule` and `cell`; no `value` clears the cell |
| `rules_set_create name values [severity]` | `set`: `name`, `limits`, `severity`, `uses` (each `scope` and `rules`) |
| `rules_set_edit set values [severity]` | `set`; every rule the set wrote follows; no `severity` keeps the set's |
| `rules_set_apply set scope` | `set` and the `rules` written for `scope` |
| `rules_preset_diff preset` | `preset`, `label`, `mode` (`merge` or `replace`), the counts `added`, `changed`, `removed`, `changes` (each `rule`, `change`, `from`, `to`), `other` (severity and rule-set lines) and `fab` (`manufacturer`, `process`, `source`, `published`, or null) |
| `rules_preset_apply preset` | as `rules_preset_diff`, applied |

### Class matrix

On the Clearance domain, Matrix shows one row and one column per net
class of the project, one clearance per cell. A cell is a rule of the text,
`rule cc_<A>_<B> on clearance when class == "A" and other_class == "B"
require clearance >= <length> then error`, so the text view shows the same rule.
It applies to a clearance between a net of class A and a net of class B, in
either order, on both sides ([Which rule applies](#which-rule-applies)); the
diagonal cell (A and A) is between two nets of one class.

An empty cell shows, greyed, what the pair needs without class-pair rules:
the board's resolution of the two classes' first nets (the written rules,
else the setup and class clearances), or with no board open the larger of
the two class clearances, or "board setup". A cell below what a class asks
is applied and flagged as lowering that class, as any rule is.

Only the lower triangle is live; the mirror half is inert, and a pair named
in either order is the same cell. Clicking a cell types in it (Enter stores
the length as typed, `8mil` too; a bare number is in mm; an empty value
removes the rule, so the cell falls back), keeps the accent ring and opens its object pairs in a card
beside the matrix: All objects (the whole cell), then one row per object
pair (track-track, track-pad, track-via, pad-pad, pad-via, via-via), each
the same rule narrowed with `between`. An empty object-pair row shows the
All objects value it falls back to; ↺ on a written row clears it. Every
cell edit is one undo step.

The preview row under them follows the hovered cell or object pair, else
the clicked one (the pin). A value typed on the preview drawing writes the
pinned cell or object-pair row, as one undo step.

Differential-pair classes are not in the matrix yet: there is no
differential-pair class to name.

### Constraint sets

A constraint set holds limits written once (`min_clearance=0.8mm,
min_width=0.3mm`) and a severity. It is stored beside the rule text in
`rules.drc.sets` and is not a rule itself. It takes clearance, track, via,
hole and edge limits only; a board-wide limit (silkscreen, mask, paste,
courtyard) has no scope to apply to and is refused. Apply writes, for one scope (a
class, a net, a pattern or a class pair), one ordinary rule per kind the set
limits, named `<set>_<scope>_<kind>`; applying it again to the same scope
writes those rules anew. Giving the set new limits rewrites every rule it
wrote, everywhere it is applied: a kind it no longer limits loses its rule,
a kind it now limits gains one; each rule keeps its name, on/off, layer and
objects. Each create, apply and change is one undo step that holds the set
and its rules together. A rule a set wrote can still be edited in the
grid; the set's next change writes over it. A severity entry for a rule
whose `then` word the set changes is dropped in the same step, so the set's
severity governs it. A rule a set wrote that was edited in the text into one
the grid cannot show (a scoped rule) is not written over: the set's change
is refused and names the rule. A class pair naming a class the project does
not have is flagged like a one-class scope.

In the tab, Constraint sets (with the number of sets) shows one card per
set under the toolbar: its name, limits and severity, and the scopes it is
applied to. The form under the cards takes a name, limits, a severity
(error, warning or info) and a scope: Create set stores a new set, a card's
Use these limits gives that set the form's limits and severity, and its
Apply to scope applies it to the form's scope. A set is edited through the
form only, not on its card.

### Presets

The Presets panel lists the generic manufacturing tiers (Standard, Advanced
and HDI, the minimums the new-project setup offers; no fabricator is named)
and, once read, the rule profiles of this machine that hold DRC rules. A
tier merges seven board-wide rules, `fab_clearance`, `fab_track`, `fab_via`,
`fab_hole`, `fab_edge`, `fab_mask` and `fab_silk`, into the text by name: a
rule of that name is replaced, the rest stay. A profile replaces the
project's DRC rules (text, severity entries and rule sets) as "Update from
profile" does. Each tier is a card with its clearance, width, drill and
annular minimums, read from the tier table (`FabTemplate::capabilities`).
Picking one gives it the accent and lists the rules it adds, changes and
removes (spacing alone is not a change), each with its limits old → new,
and the other lines; a long list scrolls in the card, so Cancel and Apply
at the card's foot stay in view. Apply in that card stores it as one undo
step, and is refused when the rules changed since the list was made.

## Fab profiles

A board is built by one maker, whose published limits are the floor of
what can be built (#2311). A fab profile is a rule profile (`.snxrules`)
whose `drc.dsl` writes those limits as rules named `fab_*`, one per base
rule the maker limits, with an optional `fab` block saying whose limits they
are:

```json
{ "schema": 1, "name": "Two-layer standard", "version": "1",
  "fab": { "manufacturer": "", "process": "2 layers, 1 oz copper",
           "source": "the maker's capability page", "published": "2026-11-17" },
  "drc": { "dsl": "rule fab_clearance on clearance require clearance >= 0.15mm then error\n" } }
```

Every `fab` field is free text and may be left out; `rules_preset_diff`
answers it as `fab`, leaving out an empty field as the file does (a generic
tier's `fab` has no `manufacturer` key). Picking the profile (Presets, or "Update from profile")
sets the project's DRC rules from it, with the same drift check as any rule
profile. The generic tiers are fab profiles shipped with Signex: their
`fab` block names no maker, says which process class they stand for, and
dates the numbers; they are starting points, not one maker's limits.

| Tier | Clearance | Track | Via / drill | Hole | Annular ring | Hole to hole | Edge | Mask sliver | Silkscreen to pad | Router tool Ø | Min slot |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Standard | 0.15 | 0.15 | 0.5 / 0.25 | 0.25 | 0.125 | 0.5 | 0.3 | 0.1 | 0.15 | 2.0 | 0.8 |
| Advanced | 0.1 | 0.1 | 0.4 / 0.2 | 0.2 | 0.1 | 0.4 | 0.25 | 0.075 | 0.1 | 1.6 | 0.6 |
| HDI | 0.075 | 0.075 | 0.25 / 0.1 | 0.1 | 0.075 | 0.3 | 0.2 | 0.06 | 0.075 | 1.0 | 0.5 |

All in mm. Mask sliver and silkscreen rules are warnings, the rest errors.
The router tool diameter is no rule: it is the bit that cuts the board
outline, stored as `fab.router_tool_diameter` in a fab profile and copied
into the project's `.snxprj` (`router_tool_diameter`) when a tier is picked
in New Project, a tier is seeded for a new board, or a tier or a fab profile
naming one is applied from Presets (a profile naming none leaves the
project's value). The board sketch's inside-corner check warns at a corner
tighter than half of it (`PCB_SKETCH.md`, "Outline checks"). The tier values
are generic starting points (decision 3, Caner 2026-11-20); check your
maker's. The Project tab's Project setup tier pick does not set it yet.

The min slot is no rule either: the narrowest slot the maker routes,
across (DECISIONS 2026-11-23 and 2026-11-24). A fab profile stores it as
`fab.min_slot`; the project stores it with its rules, `rules.min_slot`, set
in the same places and the same undo step as the router tool, except that
New Project stores it with the seeded rules. The board sketch's min slot
check warns at a narrower Slot (`PCB_SKETCH.md`, "Outline checks").

Picking a tier in project setup (New Project, the Project tab's Project
setup, New PCB) writes the same seven rules as the tier preset: see
`docs/PROJECT_SETUP.md`, "Rule templates".

The annular ring and the via diameter are separate limits: the ring is
measured from the drill, the diameter across the copper, so a via under
both minimums is two findings.

Project rules can be stricter than the floor. A written rule that is not a
`fab_*` rule and asks less than the strictest `fab_*` rule of the same limit
whose scope it shares (a net, a copper layer, an object pair: a floor for
inner layers says nothing of a rule for the outer layers) is still applied (the most specific rule wins, as everywhere), but Run DRC
and `check_drc` say so in Messages, `DRC rule 'tight' asks min_clearance
0.1 mm, past the fab floor 'fab_clearance' of 0.15 mm`, and the Rules tab
flags it. A `fab_*` rule more specific than the written rule (one for inner
layers against a board-wide rule) is left out of that comparison: it wins
on its own scope, so the broad rule loosens nothing there.

## Not in this version

Region-scoped rules, clearance between copper and holes, differential pairs
and their classes, arithmetic in lengths, the what-if preview of a rule
edit, a fab profile that varies by layer count or copper weight inside one
file, and checking the board setup's minimum track width and via sizes
without a written rule. The DRC panel's summary counts clearance, shorts,
unconnected links and keepouts only; the findings of the other rules are
listed as rows, and Run DRC's Messages count them by kind.
