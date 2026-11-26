# Schematic directives

Four objects a sheet can carry that are not circuit (#2420, #1281, #1489). None of
them makes a connection; the netlist and ERC read them.

| Directive | Shape | What it does |
|---|---|---|
| Net Blanket | a drawn region | Gives every net it catches a net class and a track width, says how copper pours treat them, and drops the ERC rules it names inside it. |
| Compile Mask | a drawn region | ERC and the netlist skip what it covers. |
| Differential Pair | a point on a wire | Pairs the net it sits on with its partner and names the pair's diff-pair class, for the differential-pair classes and the board router (#2389). |
| Net Parameters | a flag on one wire | Gives the net it sits on a net class, a diff-pair class and routing rules (width, clearance, via diameter and hole, current). |

The UI names are Net Blanket and Net Parameters (DECISIONS 2026-11-17); the
file and the command ids keep `blanket` and `place_parameter_set`. The
Generic No-ERC directive (`no_erc_directives`, a point) is older and stays as
it is.

## Model and file

`signex_types::schematic::SchDirective`, one enum tagged by `type`, held in
`SchematicSheet::directives` in placement order. A region is two opposite
corners (`start`, `end`, any order), a Differential Pair and a Net
Parameters directive a `position`; all in mm like every other schematic
point.

On disk the list is `[[extras.sheet.directives]]`, a TOML array of tables:

```toml
[[extras.sheet.directives]]
type = "blanket"
uuid = "0192a8c0-0004-7000-8000-000000000001"
net_class = "SW_NODE"
erc_exclusions = ["builtin::unused_pin"]
track_width = 1.0
pour = "keep_short"
include_edge_nets = true

[extras.sheet.directives.start]
x = 139.7
y = 15.24

[extras.sheet.directives.end]
x = 190.5
y = 45.72

[[extras.sheet.directives]]
type = "net_parameters"
uuid = "0192a8c0-0004-7000-8000-000000000013"
net_class = "POWER_IN"
show_on_flag = true

[extras.sheet.directives.position]
x = 48.26
y = 22.86

[extras.sheet.directives.rules]
width_mm = 0.8
clearance_mm = 0.3
via_diameter_mm = 0.6
via_hole_mm = 0.3
current_a = 3.0
```

(The writer's table layout, as for `[[extras.sheet.drawings]]`.)

Every optional field is left out when unset: a Blanket's `net_class`,
`erc_exclusions`, `track_width`, `pour` (`normal`) and `include_edge_nets`
(`false`); a Differential Pair's `class`; a Net Parameters directive's
`net_class`, `diff_pair_class`, `rules` (and each rule in it) and
`show_on_flag`. The whole key is left out when a sheet has no directive, so
a sheet without one re-saves byte-identical. A directive whose uuid repeats
another object's is re-minted on load like any other object
(`kind::DIRECTIVE`).

A Net Parameters directive, a Blanket using `track_width`, `pour` or
`include_edge_nets`, and a Differential Pair naming a `class` need
`snxsch/1.3` (`SchDirective::needs_format_1_3`); a sheet without them keeps
its older token, and an older build opens a 1.3 sheet read-only instead of
dropping them (`docs/FORMAT_VERSIONS.md`).

## Meaning

All of it is derived once, in `signex-net` (`directives.rs`,
`diff_pair_classes.rs`):

- **Compile Mask** (`compile_masked`). Covered means: a symbol's origin, both
  ends of a wire or bus, the position of a label, junction, no-connect,
  No-ERC directive or bus entry, or the whole box of a sheet symbol lies
  inside the mask, its edge included. `build_netlist`, the project stitch
  (`build_project_netlist`, `multi_named_nets`, the net index) and the ERC
  projection (`ErcContext::project`) all read the masked sheet, so they
  agree. A finding located inside a mask (on the sheet, or a net finding
  anchored there) is dropped too, which covers DSL rules.
- **Net Blanket** (`blanket_nets`, `blanket_net_classes`). A Blanket catches
  a net when one of its wires lies wholly inside it or one of its junctions
  does. A net whose wires only cross the outline is listed as "edge only"
  and not caught, unless the Blanket's `include_edge_nets` is set (off by
  default). A power net, one named by a power port or a power label (`GND`),
  is never caught. The first Blanket in sheet order that catches a net and
  sets a class gives it that class. ERC drops a finding located inside a
  Blanket whose `erc_exclusions` holds the finding's rule id
  (`builtin::...`, `user::...`) or `*`. `track_width` and `pour` are stored
  and shown; nothing on the board reads them yet.
- **Net Parameters** (`net_parameters`). The directive's wire is found the
  way a label finds its wire (`wire_at_point`); its net takes the class, the
  diff-pair class and the rules. A directive on no wire names no net.
- **Class order** (`directive_net_classes`, `fill_pattern_classes`). A net's
  class is the one a Net Parameters directive on it sets (the first in sheet
  order), else the one a Net Blanket gives it, else the first project class
  whose names or patterns match it. `build_netlist` writes the directive
  class to `Net::class`, and the project netlist does the same from the
  sheets each net's wires are on, read in sheet-key order
  (`project_classes.rs`).
- **Class conflicts** (`class_assignments`, `net_class_conflicts`, #2656).
  Every class the directives of a sheet give a net is listed in rank order:
  the Net Parameters directives on it in sheet order, then the Net Blankets
  catching it in sheet order. The first is the net's class (the order
  above, from the same list). When a later one names another class, the
  resolution stays the same and ERC reports `net_class_conflict` at the
  overridden directive (`docs/ERC_RULES.md`). Directives naming the same
  class agree. The check reads the sheet with its Compile Masks applied,
  as the netlist does.
- **Class conflicts across sheets** (`project_net_class_conflicts`). The
  project netlist ranks the directives of all its sheets for each stitched
  net: the Net Parameters directives on one of its wires (sheets in key
  order, each in sheet order), then the Net Blankets catching it (the same
  order). Restricted to one sheet this is the sheet's order above. The
  first is the class the project netlist writes, and ERC's project run
  reports every later one naming another class, on the overridden
  directive's sheet, in place of that sheet's own answer on each net the
  project netlist carries. The sheet keeps its own answer on a net the
  project netlist does not carry, as does a sheet the project roots never
  reach. Each net is its own finding: two nets the same directives class
  give two (#2961).

## Differential pairs {#differential-pairs}

`diff_pairs_with_classes`: a Differential Pair directive, or a Net Parameters
directive that names a diff-pair class, on one half of a pair names the
pair. The partner is found by name: `X_P` / `X_N`, `XDP` / `XDN` (pair
`XD`) or `X+` / `X-`. A pair whose class turns `pair_by_suffix` off is
matched by `_P` / `_N` only. A directive on no wire, or on a net with no
suffix, pairs nothing; a half whose partner is not on the sheet is an
incomplete pair.

A net named as a positive half (`_P`, `DP`, `+`) is a pair member by name
alone, with no directive; a negative name alone (`RESET_N`) is not, since it
is an active-low signal far more often. The ERC rule
`diff_pair_partner_missing` reports a half whose partner is missing (#2951,
`signex_net::lone_pair_members`), and `list_diff_pairs` lists every pair of
the project, by directive or by name, with its class (#2952,
`signex_net::listed_pairs`; `docs/CLI.md`, "Differential pairs").

### Differential Pair Classes {#differential-pair-classes}

Design ▸ Differential Pair Classes… (`open_diff_pair_classes`) edits the
project's classes, `ProjectData.schematic_design.diff_pair_classes`: name,
impedance and tolerance, width and gap (on Top), and pairing by suffix. The
dialog works on a copy: OK writes it as one step of the project's General
history, Cancel and Esc drop it, and a blank or repeated name keeps it open.
A class with pairs cannot be deleted; its row menu says how many. The
Members table lists the classed pairs on the project's open sheets with the
sheet each directive is on. Max skew and max uncoupled length are rules of
the Rules tab (High Speed); the dialog shows them read-only, "—" while no
such rule exists, with a link to the Rules tab (DECISIONS 2026-11-19). The pairs
are shown in the PCB panel's Differential Pairs mode.

The board-style preview under the fields is edited too (mockup row 153,
#2661). A press on the width, the gap, the impedance or the tolerance written
on it opens a field there: Enter commits, Tab commits and opens the next
value, Esc closes it and leaves the dialog open, and a press elsewhere drops
the typed text. A value not set opens at what the drawing shows (the drawn
0.2 mm width or 0.15 mm gap, 0 for the impedance and tolerance) and commits
nothing when left unchanged. A handle on the N trace drags the gap: it snaps
to 0.005 mm, keeps the N trace inside the drawing, and the drawing keeps its
scale while the handle is held; Esc puts the gap back. The expand button
draws the pair twice as tall. A value set on the drawing is the same edit as
typing it in its field, so the field shows it and OK applies it with the rest
as one step. Where the drawing puts each value is decided once, in
`app/diff_pair_classes/preview.rs`; the view only draws it.

## Editing

Every placement and edit is one engine command: `Command::PlaceDirective`
and `Command::UpdateDirective`, patch `DocumentPatch::DIRECTIVES` (part of
`ELECTRICAL`, so connectivity caches refresh). Move, rotate, mirror and
delete work on a selected directive like on any other item
(`SelectedKind::Directive`). Consecutive updates of one directive share one
undo step, like typing a field; a command, a pick, a segment or a checkbox
is a step of its own. A rule value or track width must be blank or a finite
number of zero or more, and a blanket's width and height above zero. Anything
else is refused: the sheet does not change, and the box keeps the text and
shows it is invalid.

| Surface | Net Blanket | Compile Mask | Differential Pair | Net Parameters |
|---|---|---|---|---|
| Active Bar (Directives), Place ▸ Directives and the canvas Place flyout | two corner clicks | two corner clicks | one click | one click on a wire; off a wire nothing is placed and Messages says so |
| Command | `place_blanket start end [class] [exclude] [document]` | `place_compile_mask start end [document]` | `place_diff_pair at [document]` | `place_parameter_set at [class] [diff_pair_class] [width] [clearance] [via_diameter] [via_hole] [current] [document]`, refused off a wire |
| Edit | Properties, or `edit_blanket id [start] [end] [class] [exclude] [document]` | move or delete | move or delete | Properties |

`exclude` is a comma-separated list of ERC rule ids; an unknown id is
refused. Lengths are mm, `current` is A. The commands answer with the
directive as stored (its `id` for a later `edit_blanket`); `place_diff_pair`
also answers with the pair it found, `place_parameter_set` with its net.
`read_document kinds=directive` lists them.

### Properties

**Net Parameters**: a caption with the net and the sheet; a line-art
illustration of the flag on its wire whose Width, Clearance and Via are
linked to the rule rows (hovering either lights the other), drawn with or
without the values as "Show parameters on the flag" says; Classes (Net class,
and Diff-pair class, greyed as "None (single net)" while the net is not one
half of a pair); Rules (Width, Clearance, Via dia / hole as two inputs,
Current as information); a caption naming the rules Update PCB makes; and
Display (Show parameters on the flag).

**Net Blanket**: a caption with the sheet and the outline size; an
illustration of the outline over the wires around it, the caught nets in the
accent and the size called out; Outline (X / Y of the start corner, Width /
Height from it); Directive (Net class, Track width, Pour on nets Normal /
Keep short); Nets under the blanket, with how many it catches, each net with
its pins or an "edge only" / "edge · included" tag, hovering a row lighting
the net on the sheet; Include nets that only cross the edge; then ERC
Exclusions (one checkbox per rule, plus All Rules) as before.

A class pick lists None, the project's net classes, and the stored name when
the table no longer has it. A typed value keeps its text while it is typed,
so `0.` is not reformatted under the cursor.

## Update PCB rules {#update-pcb-rules}

Each Net Parameters directive asks for design rules on its class (on its
net when it sets no class): `Width_<CLASS>` (track, minimum width),
`Clearance_<CLASS>` (minimum clearance) and `Via_<CLASS>` (minimum via
diameter and drill). `current` makes no rule. Update PCB lists them below
the board's rows (built in `signex_drc_dsl::schematic_rules`): a rule of that name
written differently in the Rules tab first, with its text now, then new
rules, then those already up to date. Execute writes the changed and new
rules into the project's DRC rule text as one step of the rule history and
records their names in `rules.drc.from_schematic`; a build without that key
keeps it as an unknown key. The schematic wins: a rule edited in the Rules
tab is rewritten on the next Update PCB unless the directive changes too.
The Rules tab shows "from schematic" under such a rule.

A rule for a net (a directive with no class) is made only on a root page,
and only when the board has a net of that name: on a sheet placed under a
sheet symbol, and on every channel of a repeated sheet, the net takes
another name on the board, so the rule would match nothing. Update PCB
leaves such a rule out and says so in Messages; a net class carries the
rule instead. The rule name keeps letters, digits and `_` of the class or
net name, so two names can give one rule name (`V+BUS` and `V-BUS` both
give `Width_V_BUS`): the first keeps it and Messages names the other.

## Update PCB classes {#update-pcb-classes}

The board's net classes are the project's net-class table (the board's
nets are classed by name through it, as DRC reads them). Execute also puts
each class a Net Blanket or a Net Parameters directive gives into that
table, as one step of the table's history (built in
`signex_engine::net_class_table::schematic_class_edit`):

- a class the table lacks is added at its end;
- each net the netlist puts in the class is assigned to it by name, out of
  every other class's member list (a pattern of an earlier class still
  wins, as the table's order decides);
- the class's board rules are set from its first Net Parameters directive
  (width, clearance, via diameter and drill), a Net Blanket's track width
  when no Net Parameters directive sets a width, and the diff-pair width
  and gap of the project's diff-pair class that directive names. A rule the
  directives leave unset keeps the table's value.

A diff-pair class on a directive with no net class makes no board class.
A later Update PCB with nothing new changes nothing.

## Drawing

All four use the directive colour the No-ERC diamond uses (`no_connect`): a
Net Blanket is a dashed outline, a Compile Mask a dotted one, a Differential
Pair two short parallel strokes, a Net Parameters directive a flag on a
stem above its wire. The flag reads "Net Class = <class>" ("Net Parameters"
with no class) and, with Show parameters on the flag, the width, clearance,
via and diff-pair class; it is sized to its text. A Net Blanket with a class
carries a "Net Class = <class>" flag below its outline's right end, clear of
its edge; a Differential Pair with a class a "Diff Pair · <class>" flag
below the wire. Where such a stem stands on a wire, the engine adds an
automatic junction dot at its foot, as at a wire T (`transform::stem_dots`,
decision of 2026-11-21); a net label gets none. The flag text is fixed English, like every sheet output. A
region is picked by a click near its edge, so a click inside it still
reaches what it covers; a point directive by its mark or its flag
(`schematic_runtime/directives.rs`).

Exports draw all four the same way (#2656): SVG, PDF and print are lowered
from one page model, and the flag's lines and box come from the one function
the canvas uses (`signex_types::schematic::directive_flag`). The first flag
line is bold, as on the canvas. Each kind has its own box under Schematics
include in the export options: "Net Blankets" and "Net Parameters" are on by
default, "Compile Masks" and "Differential Pairs" off (decision of
2026-11-25). Print Preview redraws when any of them changes.

## No-ERC rule list

A No-ERC directive (`signex_types::schematic::NoErcDirective`, held in
`SchematicSheet::no_erc_directives`) is a point marker, not one of the
directives above. It silences ERC findings located on its point. Its `rules`
list names the ERC rule ids it silences (`builtin::...`, `user::...`, or `*`);
an empty list silences every rule (#1699). On disk it is an entry of
`no_erc_directives` in `[extras.sheet]`; `rules` is left out when empty, so a
marker without a list, including every marker saved before the list existed,
re-saves byte-identical and loads as "every rule". A sheet where any marker
has a list is written as `snxsch/1.2` (a sheet without one stays
`snxsch/1.1`). A build that predates the list reads only up to 1.1, so it
opens a 1.2 sheet read-only and refuses every save, rather than dropping the
list and letting the marker silence every rule (`docs/FORMAT_VERSIONS.md`).

Every suppression path asks the one test, `no_erc_rules_suppress`: the
per-sheet run (`drop_suppressed` and the rules that check the marker
themselves, "Pin type conflict" and "Net has multiple names") and the
project-wide "Net has multiple names" and "Part units not placed" passes.

Properties lists one checkbox per ERC rule under Suppressed rules, after an
All rules box that is ticked while the list is empty. While All rules is
ticked the rule filter and the rule boxes are disabled; unticking it opens the
list (view state only, the marker still silences every rule) and ticking a
rule then narrows the marker to it. Unticking the last listed rule leaves the
list empty, which is every rule again. A Blanket shows the same section.
Each checkbox is one undo step (`Command::SetNoErcRules`, patch
`DocumentPatch::NO_ERC_DIRECTIVES`).
The engine stores every list sorted without repeats, and a list naming `*`
as empty (`canonical_no_erc_rules`), so a file does not depend on the order
the rules were picked in.

Automation: `place_no_erc at [rules] [document]` places a marker with the
comma-separated `rules` (left out: every rule), and
`edit_no_erc_rules id [rules] [document]` sets the list of an existing
marker (`rules` left out or empty: every rule) through the same
`SetNoErcRules` as a checkbox. An unknown rule id is refused, as for a
Blanket's `exclude`.
Copy, cut and paste carry No-ERC directives with their lists.

## Not yet

- Copy and paste do not carry Net Blankets, Compile Masks, Differential Pair
  or Net Parameters directives.
- Compile Masks and Differential Pairs are not printed (open for the
  maintainer, #2656).
- The board does not read a Blanket's `track_width` or `pour` (Keep short).
- A class a directive gives reaches the board's rules by name only: a
  `Width_<CLASS>` rule applies to the board nets the project's net-class
  table puts in that class.
- The class dialog's preview shows the width and gap; they are typed in the
  fields, not on the drawing, and it has no large view.
- The class dialog's Members list reads the project's open sheets only.
