# signex-cli: running Signex commands without a window

`signex-cli` runs catalog commands by id against a project with no window, as
a script or an AI agent would (#1923). It boots the same application the GUI
runs (`signex_app::headless`), so a command id has the same effect as the menu,
the palette and the keymap. The GUI binary (`signex`) stays argument-free.

## Usage

```
signex-cli run <project> [options] < commands.jsonl
signex-cli run <project> <command-id> [--arg key=value]... [options]
signex-cli commands [--json]
signex-cli agent-log <project> [--access]
```

`agent-log` prints the project's agent journal as a table: the edits agents
made (`.signex/agent-edits.jsonl`), or with `--access` who connected and with
what rights (`.signex/agent-access.jsonl`). It only reads; what it skipped (a
partial last line, a line of a newer version) goes to standard error. See
`docs/AGENT_JOURNALS.md`.

`<project>` is a `.snxprj` (or a loose `.snxsch`). The exit code is `0` when
every command was ok, `1` when a command was refused or failed, `2` for a
usage error, a project that cannot be opened or a run that cannot be placed
(`--attach` or `--headless` below, an unsafe token file), and `2` for an
internal error (the run's thread could not start, or the run panicked; the
panic message is on standard error).

A run works in one of two places (#1924): in a running Signex that has the
project open, or headless, as described above. See "Working through a running
Signex".

| Option | Meaning |
|---|---|
| `--attach` | only work through a running Signex; fail (exit 2) if none has the project open |
| `--headless` | only run headless; fail (exit 2) if a running Signex has the project open |
| `--keep-going` | run the rest after a refused or failed command (default: stop at the first) |
| `--save` | allow commands that write files (capability `save`) |
| `--dry-run` | run every command with `dry_run=true`: a wire or bus run reports and changes nothing. A read-only query and `open_document` run as given; any other command without that operand is refused, and so is `--dry-run` together with `--save` or `--allow save`. `--dry-run` cannot dry-run a placement in an attached run (placements reach a running Signex only through `apply_edits`, which has no `dry_run` operand); put `dry_run=true` on a wire or bus edit inside `apply_edits` instead (the other placements take no `dry_run`) |
| `--allow <caps>` | also allow capabilities (comma list) |
| `--allow-path <dir>` | also allow paths inside `<dir>` (repeatable) |
| `--max-line-bytes`, `--max-commands`, `--max-file-bytes` | tighten or raise the input limits |

## JSON Lines

One command per input line; one result line per command on standard output.
Blank lines are skipped.

```
{"id": "open_document", "args": {"path": "main.snxsch"}}
{"id": "color_net", "args": {"at": [5, 0], "color": "#336699", "document": "main.snxsch"}}
{"id": "place_junction", "args": {"at": [5.08, 0]}}
{"id": "save_document", "args": {"document": "main.snxsch"}}
```

The project-wide numbering commands (`annotate_schematic_quietly`,
`force_annotate_all_schematics`) write no file: each sheet they change, opened
or not, stays unsaved in memory. To keep the result, run with `--save` and send
one `save_document` naming each changed sheet (`list_documents` lists them). A
run that ends with changes unsaved names those sheets, and they are lost with
the run. A save never overwrites a file that changed since it was read.
`undo document=<sheet>` never steps such a run, saved or not: it moves other
sheets too, so the command is refused with `the next step belongs to a
project-wide run (annotate, replace or re-prefix) and moves other sheets too;
undo it from the sheet it was run on`. Undo the run in the app window, where
Ctrl+Z restores every sheet until you save.

An argument is a string, a number, a boolean, or `[x, y]` for a point (mm).
The names, kinds and units of a command's arguments are in
`signex-cli commands --json`; the same checks as `CommandArgs::parse_operands`
apply. A relative `path` is read against the project folder.

A command that acts on a document (`undo`, `redo`, `save_document`,
`save_document_as`, `close_active_document`, `color_net`,
`clear_all_net_colors`, `assign_net_class`) names it with `document`: a
document id (the hyphenated uuid, as `list_documents` prints it) or a path. A
`document` given as a path is confined and refused outside the project like a
`path`; an id is not a path and is never confined. A run never falls back to
the active document. `list_documents`, `document_info`, `resolve`,
`find_items`, `read_document` and `query_pins` are queries: their result line is
`answered` with the answer in `data`. `query_pins document=<doc> designator=U1`
answers with `pins`, one entry per pin of every placed unit of that part
(`number`, `name`, `electrical_type`, `hidden`, `unit`, `shared`, `position` as
`[x, y]` in mm; `shared` is true for a pin every unit of a multi-unit part
carries, which has an entry at each placed unit's tip). The designator is the
one the netlist prints, matched exactly. An unknown or ambiguous designator, a
power port, a document that is not a schematic or one that cannot be read, or
a project whose sheet hierarchy is not read yet, is refused with the reason.
`open_document` and `new_document` answer with the new document's
`{id, path, kind}`. A wire or bus run from operands also answers (see "Wire
intent").

`apply_edits` takes a list of edit commands as its `edits` argument and
applies them to one sheet as one undo step, and `revert` takes that step back;
both answer in `data`. See [AGENT_EDITS.md](AGENT_EDITS.md).

```
{"run": "<uuid>", "seq": 2, "mode": "headless", "id": "color_net", "status": "ran",
 "messages": [{"level": "INFO", "message": "..."}], "changed": ["<path>"]}
```

| Field | Meaning |
|---|---|
| `run`, `seq` | the run id (also in the audit log) and the line's 1-based place in the run |
| `mode` | where the command ran: `headless` (this process, no window) or `attached` (a running Signex, below) |
| `id` | the command id, `null` when the line was not a command |
| `status` | `ran`, `answered` (a query, or a wire or bus run; the answer is in `data`), `refused` (nothing ran; `reason` says why) or `failed` (it ran and did not succeed; `reason`) |
| `messages` | what the app logged to its Messages panel during the command |
| `changed` | documents whose content or unsaved state differs after the command |

`open_document` and `new_document` take a `path` (a new project's `.snxprj`)
and do what the menu entry does after its dialog. A `new_document` whose path
already exists is `refused` without `overwrite: true`. `save_document_as` takes
`document`, `path` and `overwrite`: an existing target without
`overwrite: true` is `refused`. An open
or new that makes nothing, with an error notice, is `failed` with that notice.
A command with no operand that opens a dialog is `refused`, never `ran`, when
the dialog cannot be answered.

## Naming the sheet of a placement

The placements (`place_net_label` and the other labels, `place_power_symbol`,
`place_junction`, `place_no_connect`, `place_no_erc`, `place_wire`, `draw_wire`,
`place_bus`, `draw_bus`, `place_text`, `place_text_string`, `draw_line`,
`draw_graphic_line`, `draw_rectangle`, and the directives `place_blanket`,
`place_compile_mask`, `place_diff_pair` and `edit_blanket` (#2420) and
`place_parameter_set` (#1489: `at` on a wire, refused off one; optional
`class`, `diff_pair_class`, `width`, `clearance`, `via_diameter` and
`via_hole` in mm, and `current` in A, information only), and
`edit_no_erc_rules` (#1699, see
[SCHEMATIC_DIRECTIVES.md](SCHEMATIC_DIRECTIVES.md))) take an optional
`document`, an id or a path as for `apply_edits`. Given, the command runs on that sheet and never on
the active tab: pin references, the intent checks and their scratch copies, the
commit, undo and the dirty flag all go to it, and no tab or focus changes. An
unknown, unloaded, non-schematic or read-only document is refused with its
reason. Without it, the command acts on the sheet the edit targets (the
`apply_edits` document, else the session's own active sheet). Inside
`apply_edits` an edit may name another sheet of the call's project (the call is
then one group across the sheets, see [AGENT_EDITS.md](AGENT_EDITS.md)), but
not a sheet of another project. The optional `document` does
not put the placements on the agent channel: they stay reachable through
`apply_edits` only. When the run works through a running Signex (see "Working
through a running Signex"), a placement without `document` is refused there,
as every command that takes one is, so it never acts on the person's active tab.

## Pin references

A point operand (`at`, `start`, `end`) also takes `<designator>.<pin>`, split at
the first `.`: `R1.2`, `U1.VCC`. The pin is its number first, else a pin name
only one pin has (a pin with no number is known by its name, as in the
netlist). A multi-unit part is named by its bare designator (there is no `U1A.3`
form). The reference is the pin's tip on the sheet the edit targets (the
`document` of `apply_edits`, else the active sheet), never on whichever tab is
showing, and is accepted off the grid; the grid check stays for coordinates.
Coordinates keep working: a value with a comma is a coordinate. `color_net`
takes a coordinate only, and a bus no pin reference ("a bus does not connect to
pins").

The same position also takes `<sheet name>.<entry name>`: `Charger.SDA` is the
connection point of the entry (sheet pin) `SDA` of the sheet symbol `Charger`
placed on the target sheet, accepted off the grid like a pin tip. The sheet name
is the symbol's name as `read_document` reports it (`name` of the `sheet_entry`
element), the entry the sheet pin's `name`, both exact. The value is split at the
FIRST `.`, so an entry or pin name may contain a `.` (`U1.A.B` is pin `A.B`) but
a sheet name or designator may not; a space inside a name is kept
(`Power Stage.VIN`), the ends of the value are trimmed, and a value with a comma
is a coordinate. An end given so names exactly that sheet pin: another item on its point
(a part pin, another entry) is extra, and the project name check applies as for
any end. `ends` in an answer carries the reference as given. It is refused for a
sheet or entry that is not on the sheet, two sheet symbols with that name, two
entries with that name, and a name that a part designator and a sheet symbol
both carry (ambiguous); a bus takes none. Placing sheet symbols and entries
from operands is not part of this.

A reference is refused, with the reason, for a designator that is not on the
sheet, not annotated (`R?`), ambiguous or a power port; a pin that does not
exist, whose name matches several pins, that is hidden, or whose unit is not
placed on the sheet; a pin shared by every unit of a part that has several
units placed (it has several tips, which are named); a project whose sheet
hierarchy is not read yet or is being re-read ("ask again"). `query_pins`
refuses only a hierarchy that is not read yet.

## Wire intent

A person clicking a wire sees what its path touches; a CLI or an agent does not.
So `place_wire` and `draw_wire` from operands apply the run to a copy of the
sheet first and judge the result with the netlist derivation
(`signex_net::wire_intent`; there is no second connectivity model):

1. Only horizontal and vertical segments: the draw mode bends a run as it does a
   click run, and a segment that is still diagonal (Angle45, FreeAngle) is
   refused.
2. No segment crosses the open interior of a symbol body, a power port
   included; running along a body edge or ending on one does not count.
3. **Extra connection** (never allowed): the run pulls another pin, label,
   junction or wire into the net, wherever on its path or bend ("the wire also
   touches R5.1 at 20.32,5.08"). "Touches" is what the netlist joins. A pin
   end the run passes over gets an automatic junction (2026-11-18), so it is
   joined and named like a pin the run ends on. What an end intends is what is
   connected there before the run (wire ends, junctions, labels, connected pins,
   sheet pins) and what the caller named: a pin reference names only that pin
   (another pin on the same tip is extra), a coordinate names the pins at the
   point. A start
   or end on a wire's interior is a T the caller asked for only for a
   coordinate with no pin tip there; for a pin reference, or any end on a pin
   tip, a wire passing under the tip is not intended, so its net is extra.
4. **Short**: the two nets the run joins are a short by the name rule below
   (every label text and power-port value of each net counts, not only the
   best name: own names `{DATA, P}` and `{DATA, Q}` differ); refused unless
   `allow_merge=true` ([Why `allow_merge`](#why-allow_merge)). On every sheet
   a hierarchical label is an alias, not an own name, so on a child a wire
   from the hierarchical `SDA` net to the net labelled `I2C_SDA` is accepted
   on that sheet; the project check may still refuse it (rule 6 below).
5. **Open end**: an end with no pin tip, wire, junction, label, sheet pin or
   bus entry (either end of it) under it; refused unless `allow_open=true`.
6. The two ends must be on one net afterwards; a run that is not is refused
   whatever the flags (it cannot happen for a connected run, so it signals a
   bug rather than a choice).

Names are judged across the project, by net identity, not by name text. When
the sheet belongs to a project, rule 4 (and a placement's name change) compares
the project's stitched nets (`signex_net::project_name_changes`; net scope is
`docs/NET_NAMING.md`: net labels local, global labels and power ports joined
project-wide by name, hierarchical labels through the matching sheet pin, each
placement of a child sheet with its own nets) before and after. The name rule
is frozen by [#2199 comment 6761836527](https://github.com/alplabai/signex/issues/2199#issuecomment-6761836527);
it is a heuristic that catches what names can tell apart and leaves the rest
(a signal tied to a rail through an entry, two outputs tied together) to ERC:

1. **Own names and aliases.** A net's own names are its net labels, global
   labels/ports and power ports (power flags name nothing). Its aliases are
   the names of hierarchical labels.
2. **Two own-named nets** joined whose own-name sets differ are a short
   (`VCC` + `GND`, `I2C1_SDA` + `I2C2_SDA`); the same own names join freely.
3. **An own-named net joined to an alias-only net** joins freely, and the
   answer lists the aliases (`+5V` to `Regulator.VIN`; `I2C1_SDA` to
   `SensorA.SDA` and `SensorB.SDA`).
4. **Two alias-only nets** joined that share no name are a short (`SDA` +
   `SCL`); sharing a name they join freely (`SDA` + `SDA`, `{SDA, I2C_DATA}` +
   `SDA`).
5. **Rename:** a label or power port that adds an own name to a net that
   already has own names and does not carry that name (own or alias). A
   hierarchical label never renames.
6. **Each edit is judged by what it changes.** A child's local names that
   were already behind a hierarchical label before the edit play no part on
   the parent (a sensor's internal `SDA_INT`). Names that the edit itself
   brings behind a port do count: a new hierarchical `GND` on a child net
   labelled `VCC`, whose entry the parent wired to `GND`, is a short. "The
   parent" is seen from the edited sheet: only names below or beside it are
   hidden. On the edited sheet and its ancestors local names stay names (a
   parent's `PNET` behind its own port is a name of the child's net, so a
   label `Y` on that net in the child is a rename). On the child itself,
   a wire from the
   `SDA`/`SDA_INT` net to a net labelled `GND` is a short, and a label `FOO`
   on it a rename, as on a loose sheet (while a global `SDA_INT` there
   carries a name the net has). Where this project check runs (the sheet is
   in a project and the net reaches another sheet), its net-name verdict is
   the only one; the per-sheet one decides loose sheets and the rest. So the
   same final design reached by a different order of edits can get a
   different verdict; the check judges the edit, not the design.
7. **Buses** follow the same rule with bus names (hierarchical bus labels are
   aliases), and a bus run ending on a sheet entry is connected, not open.
   Limit: bus names are judged on the edited sheet only. The project check
   compares member nets, not bus names, so a bus name that reaches the run
   from another sheet (the parent's `ADDR[0..7]` on the entry the child
   labels `D[0..7]`) is not seen: `DATA[0..7]` placed on that child run is
   not refused.

Limit: a sheet counts as part of a project for these checks when the project
file lists it or an open sheet of that project references it as a child. A
sheet that is neither (for example a grandchild added only through its parent
sheet, opened without that parent) is judged on its own sheet only, as a loose
sheet is; open its parent sheet too to have names judged across the project.

Limit: in a project with several root pages, every root page counts as an
ancestor of the edited sheet, and a sheet placed both above and beside the
edited sheet counts as above it. Both can only keep a local name visible that
would otherwise be hidden, so the check errs towards refusing (pass
`allow_merge=true`).

A short or rename is refused unless `allow_merge=true`. The rule is one
function (`signex_net::join_rule`), read by the per-sheet checks, the bus
checks (bus names, on the edited sheet) and the project check (net names). A run's answer lists the names the joined net
carries only as aliases in `aliases`. The extra rule stays per sheet. A net that reaches no sheet pin,
hierarchical or global label or power port, and a bus run that reaches no
hierarchical or global bus-name label or sheet pin (by the bus analysis), is
judged on its sheet only and costs no project read. A no-connect and a power
flag are not judged. A bus run is judged like a wire. When the project cannot be read
completely (a sheet missing or unreadable, a child reference leaving its
folder, or the edited sheet not among the project's sheets) and the net reaches
another sheet, the edit is refused with the reason, and `allow_merge=true`
places it anyway. A route sent
as chained `place_wire` edits (A to B, then B to C) is judged per edit against
the sheet as it stands, so every edit but the last needs `allow_open=true`.

`dry_run=true` (`--dry-run`) answers the report and changes nothing: no undo
step, no dirty flag, and no Messages line from the run or its checks (reading a
closed sheet of the project may still show that sheet's own load warnings). A
run that lands answers
`{"dry_run": false, "ends": [{"ref": "R1.2", "at": [x, y]}, {"at": [x, y]}],
"segments": [[x1,y1,x2,y2],...], "net": {"name": "N$3", "pins":
["C1.1","R1.2"]} | null, "merged": [names], "aliases": [names]}`; `net` is
`null` when no pin is on the run's net; `aliases` are the names the joined net
carries only through a child sheet's hierarchical label. `ends` records both ends as given: the pin reference (`ref`,
only for an end given as one) and the point it resolved to (`at`), so a
journal of the run holds both; it carries no file path. `dry_run` is in every
answer, dry or not. A bus answers the same with `"bus": [names]` for `net`.
`place_wire`, `draw_wire`, `place_bus` and `draw_bus` answer `answered`, not
`ran`; the point placements (label, power port, junction, no-connect) still
answer `ran`.

## Why `allow_merge`

Merging two named nets or buses on purpose does happen: giving a signal a
second name, or tying AGND to GND at a single point. An agent must never do it
silently, so the caller states it: `allow_merge=true` is the explicit "yes, join
them". Without it a wire, a bus, a junction, a label or a power port that would
short differently named nets is refused with the names it would join. It is
not an escape for the other refusals: touching things the run was not drawn to
(extra connections) has no escape.

## Labels, power ports, junctions and no-connects

`place_net_label`, `place_local_net_label`, `place_global_label`,
`place_hierarchical_label`, `place_power_symbol`, `place_junction` and
`place_no_connect` from operands, at a coordinate or a pin reference, are all
checked by one rule, the way a wire is: the placement's real engine command is
applied to a copy of the sheet, and the copy is compared with the sheet by the
netlist's own connectivity and by the bus analysis (`signex_net::placement_intent`).

- **Name change.** A short or a rename by the name rule of "Wire intent"
  (points 2 to 5), for nets and, with bus names (judged on the edited sheet
  only, point 7), for bus runs: a hierarchical
  label is an alias and renames nothing on its sheet (a short it makes
  through a wired entry is the project check's, point 6): refused ("a
  label at 12.7,25.4 would join the nets {VCC} and {GND}", "a label at
  57.15,38.1 would add D[0..7] to the bus {A[0..15]}"), unless
  `allow_merge=true` ([Why `allow_merge`](#why-allow_merge)). A name the net
  already carries (an alias included), an unnamed net, nothing at `at`, and
  unnamed nets tied by a junction stay allowed. A power flag
  (`power:PWR_FLAG`) never names a net and is never refused for names, so it
  reads no project.
- **Extra, by pin reference only (no escape).** Anything not on the pin's net
  before that ends up on it: another pin on the tip, a power port, a sheet pin,
  a wire passing under the tip ("the label on R1.2 also touches
  pin C1.1 ... would join them to the net"). This is the end rule of "Wire
  intent", applied to the tip. A coordinate has no extra check: it is the
  caller's own T.

There is no `dry_run` for these commands, and they answer `ran`. The
pin-reference placements that are join-checked are the wire, the label, the
power port, the junction and the no-connect.

One more refusal is for a power port by pin reference:
- A power port on a pin whose tip has no wire end, junction or label is refused
  ("would touch the pin without connecting"): pin to pin is no connection, so
  draw a wire to it first or place a label. A no-connect flag does not count as
  a connection.

## Bus geometry

`place_bus` and `draw_bus` from operands follow the geometry rules of a wire
(orthogonal segments, no body crossing, no pin references) and refuse a
dangling end: an end must sit on an existing bus (an end or its interior), a
bus entry (either end), a bus-name label or a sheet entry, unless
`allow_open=true`.

They are intent-checked too, with the one bus analysis (`analyse_buses`) applied
to the sheet before and after the run (`signex_net::bus_intent`):

- **Extra connection** (never allowed): another bus run touched by its end or
  interior along the path or at a bend, a bus entry, or a bus-name label that
  ends up on the new run and was not on a run an end attaches to ("the bus also
  touches bus D[0..7] at 12.7,38.1 ..."). What an end attaches to is the intent.
- **Merge**: the runs the two ends attach to, and a bus-name label at an end
  that sits on no run yet, are a short by the name rule of "Wire intent"
  with bus names (hierarchical bus labels are aliases): two runs with
  different own bus names, or two alias-only runs that share no name;
  refused unless `allow_merge=true` ([Why `allow_merge`](#why-allow_merge)).
  One name and one unnamed run, or an own-named run and an alias-only run,
  is allowed. Bus names are judged on the edited sheet only (point 7 of the
  name rule): a bus name from another sheet is not seen.

There is no net diff: bus connectivity is the bus analysis, not the netlist.
`dry_run` works as for a wire; the answer is `{"dry_run", "ends", "segments",
"bus": [names of the run], "merged": [names], "aliases": [names]}`, `aliases`
as for a wire (the member nets' names that reach them only through a child's
hierarchical label).

## Placing parts and hierarchy

`place_component`, `place_sheet_symbol`, `place_sheet_entry` and
`sync_sheet_pins` run from operands (#2199, decision
https://github.com/alplabai/signex/issues/2199#issuecomment-6769277106). Each
is one undoable edit of the sheet the edit targets (the named `document`, else
the active sheet) and none writes a file. Without operands, in a window, they
do what they always did: `place_component` opens the component picker, the
sheet tools arm, and Sync Sheet Pins acts on the selection. A headless run
without operands is refused with the operand it needs.

**The connection rule.** An edit from operands connects nothing it does not
name. A placed part whose pin tip, or a sheet entry (placed or synced) whose
connection point, lands on a wire (its end or its run), a junction, a label, a
sheet entry or another pin tip is refused with no escape ("the part would land
on another wire at 12.7,25.4 ..."); place it clear and wire it afterwards with
`place_wire start=U1.1 ...`. The edit's real commands are applied to a copy of
the sheet and the new connection points are read from it
(`signex_net::placement_contacts`). A part body over a wire is not a
connection and is not judged; pin to pin counts here, though the netlist joins
nothing pin to pin, because the caller did not name it. A bus-named sheet
entry (`D[0..7]`) placed on a bus, at an end or mid-span, is refused: the
netlist would join the bus to the child's bus. A part pin tip, or an entry
with a plain name, on a bus connects nothing and is accepted. A pin tip or
sheet entry placed on a no-connect marker is refused: it would mark the new
point unconnected. The connection rule has no escape; only
`place_sheet_symbol` takes `allow_merge`, for the names its file brings in
(below). None takes `dry_run`, so `--dry-run` refuses them.

### `place_component`

```text
place_component library=<id or path> part=<row id or part number> at=x,y
                [unit=N] [designator=U1] [value=..] [rotation=0|90|180|270]
                [mirror=x|y] [document=..]
```

- `library` names an open library by its id or by the path of its `.snxlib`
  file or folder; a relative path is read from the sheet's project folder (a
  loose sheet's folder), and paths are compared with `..` folded and the
  project's case rule, without looking at the disk. A project's libraries open
  with the project (`open_document`
  of the `.snxprj`), and the bundled stock library opens with the app; a
  library that is not open is refused ("library `...` is not open"). Nothing
  here mounts a library.
- `part` is a row id, else a part number (internal or manufacturer) that
  exactly one row carries. None, or several ("matches 2 rows ... give the row
  id"), is refused.
- `at` is a coordinate where a click lands (the grid), never a pin reference.
- `unit` (default 1) must be a unit of the part. `designator` is used as given;
  without it the next free designator is used, as a click numbers it, counting
  the project's closed sheets. A `designator` already in the project is
  accepted only for another unit of that same library row placed on no sheet
  of the project yet (closed sheets included), so units of one part share it:
  `place_component ... unit=2 designator=U1`. A unit placed twice, or another
  part's designator, is refused.
- `value` replaces the row's value; `rotation` is the stored rotation;
  `mirror=x` flips up/down (the X key), `mirror=y` left/right (the Y key).

The row's symbol and the project's closed sheets are read off the UI thread
before the edit runs, and the answer comes when it has run (see "Edits that
read the disk" in `docs/AGENT_EDITS.md`); it can be one of an `apply_edits`
list. The commit is the click's
(`place_part_at`): one undo step, the same document a pick and click make.
The answer is `{"designator", "unit", "lib_id", "at", "pins": [{"number",
"name", "at"}]}`, the pins of the placed unit (shared pins included), so the
next edit can wire them.

### `place_sheet_symbol`

```text
place_sheet_symbol start=x,y end=x,y name=Charger file=charger.snxsch
                   [allow_merge=true] [document=..]
```

`start` and `end` are the two corners (on the grid, each edge at least 5.08
mm), as for `draw_rectangle`. `file` is the child sheet's reference, relative
to the sheet's folder: a `.snxsch` inside the project folder (a link that
leaves it included), checked as the Create Sheet dialog checks it. It is text,
not a path operand: nothing is opened, created or written, so the CLI does not
confine it, and the handler refuses any reference that leaves the project
folder (whether a link leaves it is read off the UI thread first). A name
another sheet symbol on the sheet has, or a file another one refers to, is
refused. `place_sheet_symbol` refuses a `file` that is the edited sheet or one
above it in the project hierarchy (a sheet cannot contain itself). Ancestors
come from the project as the edit reads it (its closed sheets read off the UI
thread first), and from the hierarchy a window already shows for the sheet;
a `file` whose own sheet symbols place the edited sheet below it is refused
too, on a loose sheet as well (the file's subtree is read first). The nets
the file brings into the project are judged by the name rule of "Wire
intent" ("Why `allow_merge`"): a file whose labels or power ports would short
two project nets or rename one (a child wire carrying the globals `VBUS` and
`VBAT`, each naming a net elsewhere) is refused unless `allow_merge=true`,
whether the file is open or closed. A file that does not exist yet brings no
nets. In a project that cannot be read completely (a sheet missing or
unreadable), the names on the other sheets are unknown: a file whose sheets
carry a global label or a power port is refused unless `allow_merge=true`,
and one that carries neither is placed. A sheet symbol on a loose sheet (no
project) is not judged for names. A `file` that is a page of the project, or
that a sheet symbol on another sheet of it already places, is refused unless
`allow_merge=true`: placing it here changes its nets (its local names become
this placement's). A `file` that cannot be read, or that places a sheet that
cannot be read, is refused unless `allow_merge=true`, which passes both its
unknown names and its unknown subtree (a loop through an unreadable sheet is
not seen); a loop through sheets that read is refused with no escape.
Creating the child file stays a window action
(`create_sheet_from_sheet_symbol`, `gui_only`). The answer is `{"id", "name",
"file", "at", "size"}`.

### `place_sheet_entry`

```text
place_sheet_entry sheet=Charger at=x,y name=SDA [direction=input] [document=..]
```

`sheet` names one sheet symbol on the sheet (none or two of that name is
refused). `at` must be on its border, away from the corners, on the grid; a
point that is not is refused with the nearest border point. `name` must be
new on that symbol. `direction` is `input`, `output`, `bidirectional`,
`tri_state` or `passive`; without it the edge decides (input on the left,
output on the right, bidirectional on top and bottom). The answer is
`{"sheet", "name", "direction", "at"}`, and `Charger.SDA` names the entry in
the next edit.

### `sync_sheet_pins`

```text
sync_sheet_pins sheet=Charger [document=..]
```

Sync Sheet Pins for the sheet symbol named `sheet`, with no selection: its
entries are matched to the hierarchical and global labels of its child file,
as the menu action does (every symbol on the sheet that refers to that file
follows). A closed child file is read off the UI thread before the sync runs;
one that does not exist or cannot be read is refused with the reason. The
menu action, run by a person, keeps its read in place. The answer is
`{"sheet", "file", "added", "removed", "kept"}`
(entry names); Messages reports it as the menu action does.

### Bottom-up hierarchy {#bottom-up-hierarchy}

```text
create_sheet_symbol_from_sheet path=power.snxsch [at=x,y] [allow_merge=..] [document=..]
sync_sheet_entries symbol=<id or name> direction=to_symbol|to_sheet [document=..]
```

The operand forms of Design ▸ Create Sheet Symbol From Sheet and Design ▸
Synchronize Sheet Entries and Ports (#1276); rules in
[MENU_BAR.md](MENU_BAR.md#bottom-up-hierarchy).

`create_sheet_symbol_from_sheet` places one sheet symbol for the sheet file
`path` with an auto-generated entry for every port of the file, as one undo
step. `path` is checked as `place_sheet_symbol`'s `file` is: inside the
project folder, not through a link out of it, not this sheet, not a sheet
above it at any depth, not a file whose subtree places this sheet, and the
frozen name rule (a page of the project, a file placed elsewhere, or names
that would short or rename a net need `allow_merge=true`). A file this sheet
already places is refused. `at` is the top-left corner (a grid coordinate,
never a pin reference); without it the symbol goes under the sheet's content.
The file is read off the UI thread before the edit runs. The answer is
`{"id", "name", "file", "at", "size", "entries"}`. It takes a file path, so it
is not an edit of `apply_edits`, as `place_design_block` is not. Without
`path` a headless run is refused for the missing operand; the file picker of
the menu row is never opened.

`sync_sheet_entries` applies what the dialog's preview would show for
`direction`, at once, on the sheet symbol `symbol` (its id, or its unique
name): `to_symbol` changes the symbol's entries (a renamed entry keeps its
place and wires; only auto-generated entries of `symbol` are renamed, and
only auto-generated entries are removed), `to_sheet`
changes the child sheet's Hierarchical labels (never a Global label) as one
undo step of the child, left unsaved; a closed child goes to a tabless
engine. The closed child file is read off the UI thread first; a sheet that
changed while it was read is refused with nothing applied, as any edit is.
`to_sheet` is refused inside `apply_edits` and an agent's one-step run,
whose undo step covers the edited sheet only. The answer is
`{"symbol", "name", "file", "direction", "added", "removed", "renamed"}`,
`renamed` a list of `{"from", "to"}`; empty lists when the two sides agree.

## Editing placed objects

The Location and Parameters rows of a placed object in Properties run these
commands' edits (#2418). Each is one engine edit and one undo step; `id` is
the object's uuid (from `find_items` or `read_document`) on the sheet the
edit targets, and `document` names that sheet as for a placement. Points are
stored mm, Y down from the paper's top, as every operand point is; the
Properties rows read the panel's unit and the page origin instead. The answer
is `{"id", "kind", "position", "rotation"?, "size"?}` after the edit.

```text
set_object_position id=<uuid> at=x,y [keep_wires=true|false] [wiring=auto|horizontal_first|vertical_first|straight] [document=..]
set_object_rotation id=<uuid> rotation=0|90|180|270 [document=..]
set_sheet_symbol_size id=<uuid> size=w,h [document=..]
set_symbol_parameter id=<uuid> name=Manufacturer [value=..] [document=..]
```

- `set_object_position` moves a part, label, sheet symbol, sheet entry, wire
  or bus (its start), bus entry, junction, no-connect, No-ERC directive or
  text note so its position is `at`. Its wire handling is in its operands,
  never the person's settings (#2552): `keep_wires` (default `true`) makes
  the wires on a moved part follow it, and `wiring` (default `auto`) bends
  them as the drag wiring mode of that name does. It is the edit a drag of
  the same distance commits, with the same rubber band and guard. A contact
  the guard cannot keep is left as a plain move; the answer adds
  `moved_wires` (ids of wires whose ends changed and of wires the move
  added), `removed_wires` (ids of wires it removed) and `fallbacks`, one
  `{"kind", "names", "message"}` per notice (`pins_not_kept` names the pins,
  `grip_end_left` carries `at` instead of `names`, `message` is the Messages
  line). A sheet entry stays on its symbol's
  border. A pin reference is refused here. The Position row in Properties
  makes the same move with the "keep wires" preference and `auto`.
- `set_object_rotation` turns a part in place (its wires follow when "keep
  wires" is on) or sets a label's angle. A power port takes the absolute angle its
  Rotation row always set. Any other kind is refused.
- `set_sheet_symbol_size` keeps the top-left corner; entries on the right and
  bottom edges travel with them. Refused with the reason, nothing changed,
  when an edge is under 5.08 mm, an entry would fall off its shortened edge,
  or a moving entry is connected or would land on a connection.
- `set_symbol_parameter` sets one custom parameter of a placed part (left out
  or empty `value`: the parameter stays, with no value). `reference`, `value`
  and `footprint` have their own rows and commands and are refused here.

`set_sheet_variable name=<NAME> [value=..] [remove=true] [document=..]` sets
one variable of the sheet (what `${NAME}` and expressions on it resolve), as
the Sheet Variables rows in Properties do, in one undo step. A left-out or
empty `value` keeps the variable with no value; `remove=true` deletes it and
takes no `value`. A name is letters, digits and `_` and does not start with a
digit. Variables that then do not evaluate (a cycle, an unknown name) are
reported in Messages. The answer is `{"name", "value", "variables"}`.

`repair_pin_junctions [document=..]` adds a junction dot on every pin end
of the sheet that lies inside a wire with none, as Tools ▸ Repair Pin
Junctions does, in one undo step; the answer is `{"added"}`.
`reset_board_origin document=..` and `reset_layer_colors document=..` are
the Reset rows of the board menus on the board named: the origin to the
bottom-left of the outline (answer `{"document", "origin"}`), and every
layer back to the palette colour. A read-only board is refused.

A locked object is refused with the lock's reason before anything is sent.
Without operands (the palette, a key) each says which Properties row to use.

### Drawing order and Delete by uuid

The Arrange rows and Delete act on the selection in a window; with operands
they act on the items `ids` names, by uuid, on the document named (#2326).
Each is one undo step; the answer is `{"document", "ids", "changed"}`.

```text
bring_drawings_forward document=<sheet> ids=<drawing uuids>
bring_drawings_to_front document=<sheet> ids=<drawing uuids>
send_drawings_backward document=<sheet> ids=<drawing uuids>
send_drawings_to_back document=<sheet> ids=<drawing uuids>
delete_selection document=<sheet or board> ids=<uuids>
```

- The drawing order moves only drawings: a uuid that is no drawing of the
  sheet refuses the whole command (`sheet-target-unknown`).
- `delete_selection` deletes objects of an open sheet (a part's uuid is the
  part, never only its designator or value field) or items of an open board
  (a footprint, track, via, area, text, arc, free pad, dimension or drawing;
  a pad's uuid is no board item). A uuid that names nothing there refuses
  the whole command (`sheet-target-unknown`, `board-target-unknown`); any
  other document is refused (`delete-document-not-open`). Locked objects on
  a sheet are skipped and named in Messages, as a key press does; a locked
  footprint refuses a board delete with the placement's reason.
- A headless run without operands acts on the session's own selection, as
  the window's key does. Delete in a footprint editor stays the editor's own
  route until slice S-L and takes no operands.

## Placement extras

The operand forms of three placement extras (#1667). Each is one undo step
and ends in the commit its key and its click end in.

```text
autoplace_fields id=<part uuid> document=..
place_design_block path=<sheet .snxsch> at=x,y [document=..]
import_graphics path=<drawing .dxf> at=x,y [document=..]
```

- `autoplace_fields` puts the reference and value of the part `id` where the
  autoplacer puts them, also when the user had moved them, and answers
  `{"id", "moved"}`. It names its `document`, since an agent has no
  selection; a locked part is refused with the lock's reason.
- `place_design_block` places the parts, wires, buses, labels, junctions,
  no-connects, text notes and drawings of the sheet file `path` (with the
  bodies its parts use), and answers `{"placed", "skipped_sheet_symbols"}`.
  Sheet symbols in that file are left out: each names a child file relative
  to its own sheet. Designators are numbered as a paste numbers them.
- `import_graphics` places the LINE, CIRCLE, ARC, LWPOLYLINE and POLYLINE
  entities of the ASCII DXF file `path` as sheet drawings in the project's
  shape line width, scaled from the file's `$INSUNITS` to mm (no units: mm),
  Y turned down. It answers `{"placed", "skipped"}`; `skipped` counts the
  entities it does not read (text, blocks, hatches, splines). A polyline's
  bulges are read as straight segments. A circle, arc or polyline whose
  plane faces down Z (extrusion `0,0,-1`) is mirrored back in X. A file over
  64 MiB is refused before it is read.

`at` is where the grid-snapped centre of the content lands, as the cursor
holds it; a point off the grid is refused as for any placement. The file is
read off the UI thread before the edit runs. Without operands, in a window,
`place_design_block` and `import_graphics` open a file dialog and put the
content on the cursor; `autoplace_fields` acts on the selected parts.

## Checks on demand

```text
check_erc document=<sheet> [baseline=<run>]
```

`check_erc` (#2199, decision
https://github.com/alplabai/signex/issues/2199#issuecomment-6798772157) runs
ERC when an agent asks for it, not after every edit. It checks the project of
the named sheet (a loose sheet on its own, with the sheets its sheet symbols
place) the way Run ERC does: the same rules, the project's stored rule set or
else this machine's, the pin matrix, the project's `erc.dsl`, its waivers and
the project-wide passes. Open sheets are checked as they are in memory, unsaved
edits included; closed ones are read from disk off the UI thread, and the
answer comes when the run lands. It is read-only: the check itself writes
nothing to the ERC panel, the ERC results or Messages, and `run_erc` (F8, the
panel) is unchanged. Two lines that are not the check's can still appear in
Messages: over the agent socket, the `Agent request (pid N): ... -> answered`
line every request gets ([AGENT_SOCKET.md](AGENT_SOCKET.md), "Security",
Visibility), and the repair of a duplicated document id that every query runs
first (`reconcile_document_ids`).
Unlike Run ERC it does not check loose tabs outside the project. It is a query
(no edit journal line) and is on the agent channel; a palette or key call is
refused, as for every query.

```bash
signex-cli run demo.snxprj check_erc --arg document=main.snxsch
```

The answer (`data` of the `answered` line) is a check report, the shape every
on-demand check shares (`signex_types::check`; `check_drc` below uses it
too):

```json
{"check": "erc", "run": 2,
 "findings": [{"rule": "builtin::unused_pin", "severity": "warning",
   "message": "...", "evidence": {"rule_name": "Unused pin", "space": "sheet",
   "at": [63.5, 50.8], "pin": "1", "waiver_key": {"rule_id": "...",
   "objects": ["..."], "fingerprint": "..."}},
   "items": ["<uuid>"], "sheet": "/abs/main.snxsch", "key": "<32 hex>"}],
 "waived": [{"finding": {...}, "waiver": {"kind": "finding", "rule_id": "...",
   "objects": ["..."], "fingerprint": "...", "reason": "...", "author": "...",
   "date": "...", "message": "..."}}],
 "lapsed": [{"kind": "finding", "rule_id": "...", "objects": ["..."], "...": "..."}],
 "warnings": ["ERC: sheet /abs/x.snxsch could not be read (<error>)",
   "ERC: sheet /abs/../y.snxsch is not read: /abs/../y.snxsch is outside the project folder (or a link out of it); a headless run does not read it"],
 "rule_set": {"source": "project", "profile": "...", "sha256": "<64 hex>"},
 "baseline": {"run": 1, "new": ["<key>"], "gone": ["<key>"]}}
```

- `rule`, `severity` (`error`, `warning`, `info`), `message`, `items` (the
  uuids of the objects the finding is about) and `sheet` (the path of the
  sheet it is filed under) are what every check has. ERC's own facts are in
  `evidence`: the rule's name, `space` (`sheet`, `child_sheet`, `net` or
  `unplaced`), `at` (the point in that sheet's own coordinates, mm, when it
  has one), `child_sheet`, `net` (`name` and `pins` as `R1.1`), `pin`, and the
  `waiver_key` a waiver of this finding would record.
- `key` names the same finding in every run. It is a hash of the sheet path,
  the rule id and the objects of the finding's waiver identity (its objects,
  pin, tag or net, `docs/ERC_RULES.md`, "Waivers"), so moving or editing the
  object keeps it; a finding that names no object is keyed by its message.
  Keys are unique within `findings` and within `waived` (not across the
  two): a repeat gets `#2`, `#3`, in report order.
- Order: by sheet path, then errors, warnings and info, then rule, message,
  key and evidence. The same project gives the same `findings`, `waived`
  and `lapsed`, byte for byte (`run` and `baseline` differ from run to run).
- `waived`: the findings a per-finding waiver (`"kind": "finding"`) or a net
  waiver (`"kind": "net"`, with `rule_id` and `net`) hides, each with that
  waiver; they are not in `findings`. `lapsed`: the per-finding waivers whose
  finding changed since it was waived, so the finding is in `findings` again,
  or in `waived` when a net waiver hides it.
- `warnings`: what Run ERC would say as a warning or error in Messages, in
  its words, said nowhere else: an unreadable sheet, an `erc.dsl` that does
  not compile, a broken hierarchy, a child sheet reference that leaves its
  folder (each once), a stored rule set this build cannot read (this
  machine's rules are used) and a rule entry it does not recognise. Run ERC's
  info lines (the totals, "no saved rule set") are not repeated; `rule_set`
  says which rules ran. Run ERC's rule-profile drift check ("Cannot check
  this project's rule profile for changes: ...") is not part of `check_erc`.
- `rule_set` (a field every check may fill, its content the check's own):
  for ERC, `source` (`project`: the project's stored rule set; `machine`:
  this machine's ERC rules, used when the project has none or one this build
  cannot read), `profile` (the profile it was saved from, or `null`) and
  `sha256` (the rule set's hash, as the ERC report prints it).
- For a sheet of a project, in a headless run or for an agent, the check
  reads from disk only what a query may read (`docs/AGENT_SOCKET.md`,
  "Security"): a page, the root, a child sheet or `erc.dsl` outside the
  project folder (or a link out of it), not a regular file (a FIFO) or over
  the size cap is not read, and a warning names it. Open sheets are read
  from memory. A loose sheet is not limited this way (see the known limits),
  and neither is Run ERC.
- `run`: this run's number. The app keeps the last 8 runs' keys in memory
  (`KEPT_CHECK_RUNS`). With `baseline=<run>` the answer also has `baseline`:
  `new` lists the keys of this run that run did not have, `gone` the keys only
  it had. A baseline that is not kept (never made, or older than the last 8)
  or that checked another project is refused with the reason, and the refused
  run is not numbered or kept. The runs are kept by the process: a headless
  `run` keeps them for its own command lines, a running Signex for every
  agent request until it quits.

Known limits:

- Findings that share a key (one rule firing twice on the same identity
  objects) are numbered `#2`, `#3` by message, then by evidence (which holds
  the position), so a new repeat whose message sorts first takes the bare key
  and the old finding shows as `new`, and moving one of several repeats can
  swap their numbers. This is rare.
- A baseline's scope is the project file. In a project with no
  `schematic_root` the netlist root is the named sheet, so two runs made with
  different `document=` sheets can differ in root-dependent findings, which
  then show as `new` and `gone`. Compare runs made with the same `document=`.
- A loose sheet (in no project) checked in a headless run is not confined:
  there is no project folder to confine it to, so the files its sheet symbols
  refer to are read as Run ERC reads them (a reference leaving the sheet's
  folder is still refused). The agent channel cannot reach a loose sheet.
- A file can change between the check that allows it and its read (a link
  put in its place). The headless queries have the same gap; using it needs
  write access to the project folder.

### `check_drc`

```text
check_drc document=<board> [baseline=<run>]
```

`check_drc` (#2199, decisions
https://github.com/alplabai/signex/issues/2199#issuecomment-6817821301 and
https://github.com/alplabai/signex/issues/2199#issuecomment-6818011610) runs
DRC on the one `.snxpcb` named, with Run DRC's checks and its choice of rule
layers (the project's `rules.drc` when the project stores a rule set, else
this machine's User Default layer, the rule text compiled for the run). The
project is the board's own: the one that lists it as its `pcb_file`. A board
in no open project is checked with its own setup rules and this machine's
layer, and `warnings` says so. Run DRC takes the same project, whichever
project is active (#2332). A closed board is read
off the UI thread, within the same headless read limits as `check_erc`, and
the answer comes when the run lands; a board open in the app is checked as it
is in memory (in the app's own process, in a headless run and over the agent
channel, #2331). A board that cannot be read is refused with the reason. It
writes nothing to the DRC panel, the DRC results or Messages; Run DRC is
unchanged. It is a query on the agent channel, like `check_erc`; a board no
open project names is refused there, as a loose sheet is.

```bash
signex-cli run demo.snxprj check_drc --arg document=demo.snxpcb
```

The answer is a check report (above), `"check": "drc"`:

```json
{"check": "drc", "run": 1,
 "findings": [{"rule": "builtin::clearance", "severity": "error",
   "message": "clearance 0.15 mm, needs 0.2 mm, GND / VCC",
   "evidence": {"nets": ["GND", "VCC"], "layers": ["TopCopper"],
     "at": [0.0, 0.2], "measured_nm": 150000, "required_nm": 200000,
     "waiver_key": "<32 hex>", "rule_source": "board_setup"},
   "items": ["<track uuid>", "<track uuid>"], "sheet": "/abs/demo.snxpcb",
   "key": "<32 hex>"}],
 "waived": [], "lapsed": [],
 "warnings": ["DRC: No clearance rule set on this board; checked with 0.2 mm. Set it in Board Setup."],
 "rule_set": {"source": "project", "profile": "...", "sha256": "<64 hex>",
   "clearance_nm": 200000, "from_default": true}}
```

- `rule`: `builtin::<kind>` (`short`, `clearance`, `unconnected`, `keepout`,
  `track_width`, `via_diameter`, `via_drill`), or `user::<name>` for a
  finding of a written rule. `severity` is the rule's resolved severity:
  built-in defaults, then the profile layer, then the project layer, and a
  written rule's own `then` severity. A rule switched `off` reports nothing.
- `sheet` is the board file's path: for DRC it is the document the finding
  is filed under, not a schematic sheet. `items` are the uuids of the
  tracks, vias and footprints involved, a pad as `<footprint uuid>/<pad uuid>`
  (a pad's own uuid repeats across placements of one footprint).
- `evidence`: `nets` (two names; the same net twice for an unconnected link
  or a keepout, `net <n>` for a net with no name), `layers` (copper layer
  names), `at` (mm, board coordinates), `measured_nm` and `required_nm` (as
  Run DRC's panel shows them), `waiver_key` (`signex_drc::waiver_key`, the
  id `waive_drc_violation` and the DRC panel name the finding by), `keepout`
  (the keepout's uuid) and `rule_source` (`written`, `net_class`,
  `board_setup` or `default`; for a clearance or a written limit).
- `key`: a hash of the board path, the rule id (and, for a written rule,
  the kind of limit broken), the items and the keepout, so moving an item
  keeps it. The board path is the document entry's own: the tab's path when
  the board is open, the project folder joined with `pcb_file` when it is
  closed; another spelling of `document=` resolves to the same entry. A
  finding that names no item (an unconnected link with no pad at either end)
  is keyed by its message, as ERC keys one. An unconnected link with one pad
  that joins two islands can give two findings on the same items; those are
  told apart by report order (`#2`).
- `warnings`: a board in no open project (checked with its own setup rules
  and this machine's DRC rules, no net classes), then what Run DRC says about
  its rules in Messages, in its words: unrecognised severity entries, the
  rule text's notes and remarks, rule text or rules not applied, a clearance
  taken from the default; then one line per keepout and per item the run
  could not check (#2300).
- `rule_set`: `source` (`project` or `machine`), `profile` (as for ERC),
  `sha256` (the hash of the `drc` section the run used), `clearance_nm` and
  `from_default` (no clearance set in Board Setup).
- `waived` and `lapsed` come from the board's own waivers, the ones the DRC
  panel and Run DRC read (`[board_options] drc_waivers` in the `.snxpcb`,
  #2307). A waived finding is in `waived` with its waiver, not in
  `findings`. A waiver whose objects changed since it was waived (moved,
  resized, on another layer or net) has lapsed: the finding is back in
  `findings` and the waiver is in `lapsed`. A waiver object is
  `{"kind": "finding", "rule_id", "key", "fingerprint", "reason"}`; `key` is
  the finding's `evidence.waiver_key`. A finding of a rule switched `off` is
  not listed and its waiver is in neither list. To waive a finding, run
  `waive_drc_violation document=<board> id=<evidence.waiver_key>
  text=<note> [author=<name>]` after a Run DRC of the board; it is refused when the board
  changed since that run (`docs/architecture/pcb-drc.md`, "Waivers").
- The baseline is scoped to the board file: a run of another board, or an
  ERC run, is refused as a baseline.

Known limits:

- The minimum track width and via sizes of Board Setup are not enforced;
  only a written rule sets them (`docs/architecture/pcb-drc.md`).
- The DRC panel does not show severities yet; the report does.
- The key uses the document entry's path, which is the tab's path while the
  board is open and the project folder joined with `pcb_file` while it is
  closed. A `pcb_file` spelled differently from the tab path (case,
  `sub/../`) can change the keys between an open and a closed check, and a
  baseline made in the other state is then refused.

## Requirement checks

```text
check_requirements document=<project, or one of its sheets or its board> [baseline=<run>]
```

A project keeps a list of requirements, its own targets (#2339). Each one is
a deterministic check that passes or fails with evidence, and
`check_requirements` runs the whole list on demand and answers with a check
report in the shape `check_erc` and `check_drc` use (above). It is a Community
feature, read-only like the other checks: it writes nothing to a panel, the
ERC or DRC results or Messages, and it is a query on the agent channel and in
`signex-cli`. The list is edited in the Project tab's Requirements section
or with the `project_*_requirement` commands ([PROJECT_TAB.md](PROJECT_TAB.md)).

```bash
signex-cli run demo.snxprj check_requirements --arg document=demo.snxprj
```

The list is the top-level `requirements` key of the `.snxprj`, beside
`rules`, not inside it: it is not part of the rule set's hash, a rule profile
or Update from profile. Each entry is an object with an `id` (unique in the
list), a `kind`, an optional `severity` (`error`, the default, `warning` or
`info`; the severity of the findings it raises), an optional `enabled`
(`true`, the default; a requirement with `false` is kept, not checked, never
in `passed`, and named in the report's `warnings`) and the kind's fields. A
project save writes every entry back as it was read: defaults left out stay
left out and `100` stays `100` (keys come out sorted, as in the rest of the
file).

| `kind` | Fields | Passes when |
|---|---|---|
| `erc_clean` | `allow_warnings` (default `false`) | the project's ERC run (as `check_erc` runs it from the root sheet) has no error finding, and no warning finding unless allowed |
| `drc_clean` | `allow_warnings` (default `false`) | the DRC run of the project's board (as `check_drc` runs it) has no error finding, and no warning finding unless allowed |
| `board_size` | `max_width_mm`, `max_height_mm` | the bounding box of the board outline is at most that wide and high |
| `parts_state` | `states` (default `["released"]`; of `draft`, `in_review`, `released`, `deprecated`, `obsolete`) | every part the BOM buys was placed from a library row, of an open library, in one of the states |
| `net_class` | `nets` (a `*`/`?` pattern), `class`, `min_trace_width_mm`, `min_clearance_mm` (all but `nets` optional, at least one given) | every board net matching `nets` is in `class`, and the class of each sets at least the given track width and clearance |
| `bom_cost` | `quantity`, `currency`, `max_unit_cost` | the BOM costs at most `max_unit_cost` per board in `currency` when `quantity` boards are built |

```json
"requirements": [
  {"id": "erc", "kind": "erc_clean"},
  {"id": "size", "kind": "board_size", "max_width_mm": 100, "max_height_mm": 80},
  {"id": "usb", "kind": "net_class", "nets": "USB_*", "class": "USB",
   "min_trace_width_mm": 0.2, "severity": "warning"},
  {"id": "cost", "kind": "bom_cost", "quantity": 100, "currency": "USD",
   "max_unit_cost": 12.5}
]
```

The report:

- `check` is `requirements`; the run is numbered and kept like the other
  checks, and `baseline=` compares with an earlier run of the same project
  (any document of the project names it). A run of another check is refused
  as a baseline.
- A finding's `rule` is `req::<id>`, its `severity` the requirement's.
  `evidence` holds the `kind` and what was measured: the board's
  `width_mm`/`height_mm` and the limits; a part's `designator`, `library_row`
  and `state`, with `count`, how many parts its BOM line holds, and `parts`,
  how many parts were checked in all; a net's `net` and `class`, or a
  class's set and minimum values; the cost's `unit_cost`, `total_cost`, `quantity` and `currency`;
  for `erc_clean`/`drc_clean` one finding with the counts (`errors`,
  `warnings`, `info`, `waived`, `run_warnings`) and the run's `rule_set`, not
  a copy of every ERC or DRC finding.
- `sheet` is the board file for `board_size`, `net_class` and `drc_clean`,
  the project file otherwise.
- `key` is a hash of the project file, the rule and what the finding is
  about (a part's designator, a net, a class and its value), so it holds
  while the measured values change.
- `passed` lists the requirements met (`req::<id>`), in list order. It is an
  addition to the shared shape: ERC and DRC leave it out. An id used by more
  than one entry, readable or not, is never in `passed`: the first entry is
  checked, every later one is a finding.
- `warnings`: what the ERC and DRC runs could not do (prefixed `ERC:` and
  `DRC:`), the sheets the BOM could not read, and `The project lists no
  requirements` for an empty list.
- `rule_set`: `source` (`project`), `requirements` (how many) and `sha256`
  (the hash of the list as stored, keys sorted), so two runs over different
  lists can be told apart.

A requirement that cannot be evaluated is a finding, never a pass: an entry
this build cannot read (an unknown kind, a missing or mistyped field, a field
it does not know; it is kept in the file as written), an id used twice, a
limit that is no positive length, no board or an unreadable one, a pattern no
board net matches, a class the project does not have, an ERC or DRC run that
did not finish, a part with no library row or one in no open library, and a
BOM line with no cached price in the currency.

How each kind is measured:

- `board_size` takes the outline's axis-aligned bounding box; a rotated board
  is not turned to fit.
- `parts_state` checks the parts the BOM buys (fitted, in the BOM and on the
  board, the active variant applied), each placement of a child sheet once.
  The states come from the libraries open in the app.
- `net_class` reads the nets of the project's board; a class's `0` (unset)
  value fails a minimum.
- `bom_cost` prices the BOM as the BOM export groups it, from the prices the
  open libraries cached; it never fetches a price. Each line buys its fitted
  quantity times `quantity`, at least the distributor's minimum order, at
  that quantity's price break (the BOM export's own pricing); the line costs
  are rounded to cents and the total is divided by `quantity`. A price in another currency is not
  converted; the cost is then not known and the requirement fails.

Known limits:

- A project lists one board (`pcb_file`), so `drc_clean`, `board_size` and
  `net_class` check that board.
- Rail budgets (current and voltage per rail) are not a kind yet: the parts,
  nets and classes do not carry that data.
- Over the agent channel `document=` names one of the project's sheets or its
  board: the project file itself is refused at the agent gate, as for every
  command, because it is not a document of a project.
- The ERC and DRC runs a requirement uses are not numbered or kept; run
  `check_erc` or `check_drc` for their findings.

## Board queries

Three queries read one open board (#2403), through the same domain code as
Measure Distance, the Net Inspector and Find Similar Objects on a board. Each
answers with data, changes nothing, and is on the agent channel. A sheet or a
board that is not loaded is refused with the reason.

```text
measure_board document=<board> start=x,y end=x,y
measure_board document=<board> ids=<uuid>,<uuid>
query_board_nets document=<board> [net=<name>]
find_similar_items document=<board> id=<uuid> [fields=kind,value,...]
```

- `measure_board` answers `kind` (`point_to_point`, or `edge_to_edge` for
  `ids`: the closest approach of the two items' copper, footprint body or
  area outline), `distance_mm`, `dx_mm`, `dy_mm`, and `from` / `to` in mm.
  `start`, `end`, `from` and `to` count from the board origin with Y up, and
  `dy_mm` is positive when `to` is above `from`.
- `query_board_nets` answers `count` and `nets`, by name: `net`, `name`,
  `routed_length_mm` (the tracks' centre lines), `tracks`, `vias`, `pads`,
  `unrouted` (connections still to route) and `routed`.
- `find_similar_items` answers the `seed`, the `criteria` (each property
  and the seed's value) and the matching `ids`. `fields` names the
  properties that must match, `kind` when left out: `kind`, `value`,
  `footprint`, `designator_prefix`, `net_name`, `layer`, `width`, `size`,
  `drill`, `shape`. A property the seed does not have is refused.

Find by text on a board is `find_items` (designator and value).

## Working through a running Signex

When a person has the project open in Signex with Preferences > Editing >
"Allow agent connections" on, `run` sends each command to that process
instead of booting its own (#1924; the app's side, the registry and the
security rules are in [AGENT_SOCKET.md](AGENT_SOCKET.md)). The command runs
inside the app's update loop like a click, so the person sees the change at
once and keeps their tab, selection, tool, panels and focus; a drag they are
in the middle of continues.

- **Finding it.** The CLI reads the registry entry of the project's canonical
  path in the user's runtime folder. A live entry (the process runs, and the
  endpoint its boot id names answers) is attached to. No entry, or a stale one
  (dead process, a socket or boot id the app would not have written, another
  project), means headless; a stale entry is removed, but only if it is the
  user's own plain file (never a link). The CLI connects to the endpoint the
  boot id gives and to nothing an entry names.
- **Token.** The session token is read from the file beside the socket. It
  must be a plain file of this user with no group or other permission; a link,
  a wide mode, a file of another user or one that is not a token ends the run
  with exit 2. It does not fall back to headless: the app has the project.
- **Same result lines.** Each reply line has the headless shape, with `"mode":
  "attached"`; `messages` is always empty (see below) and `changed` is the
  documents whose content or unsaved state differ. Replies are matched to
  requests by order, so after any failed request (a timeout above all: its reply
  may still arrive) the connection is closed for good: every later command of
  the run fails with "the connection ... failed earlier" and the run never falls
  back to headless.
- **Placements go through `apply_edits`.** The 17 placement commands are not
  on the app's agent channel: sent on their own, with or without `document`,
  the CLI refuses them ("reaches a running Signex only through
  `apply_edits`") and sends nothing; send them as edits of an `apply_edits`
  call.
- **Only this run's folders are shown.** The app answers a query across every
  project it has open, so the CLI filters each answer before it prints it:
  a document whose `path` lies outside the folders this run may use (the
  project folder and `--allow-path`) is dropped, and so is an element whose
  `document` is outside. Only the keys that name documents are judged (the
  answer's `document`, `element` and `project`, and the lists `documents`,
  `sheets`, `elements` and `not_loaded`); the rest of an answer, such as a
  `read_document`'s instances and libraries, comes back whole, because a `path`
  inside it is not a file. `count` is recomputed.
  `resolve` of an id outside answers `{"found": false, "what": "unknown"}`,
  the same as for an id that does not exist. `changed` keeps only files
  inside. The app's log lines (`messages`) are shown for no command, queries
  or edits: they are free text that may name any open project's files (the
  app's own Messages panel keeps them all). The filter fails
  closed: a document is shown only if it can be proven this run's, by its own
  `path` or, for a sheet never saved (a null `path`), by its owning
  `project.path`; a listed document or element that says neither is hidden.
  So a never-saved sheet of this project shows in `list_documents`, but its
  elements (their `document` names no project) are hidden from `find_items`
  and `resolve` until the sheet is saved.
- **Policy first, the app's allowlist on top.** Capabilities, `--save`,
  `--allow-path` and the input limits are applied to every command, and to
  every edit inside `apply_edits`, before anything is sent. The app then
  applies its agent allowlist and refuses what an agent may not do, and its
  reason comes back as the result line's `reason` (`undo` and `redo`, which
  are the person's; a command with a file path; anything that does not name
  its document; a close of the person's active tab or of a document with a
  pending draft). A command that acts on a document (it has a
  `document` operand and is not a query) and is sent without one is refused
  here with "name the document": the app would use the person's active
  document, which is never checked against the run's folders. Every `document` operand, an id or a path, in
  the command and in each edit of `apply_edits`, is first resolved by the app
  (a `document_info` query on the same connection) to the file it holds, and
  that file is confined like a path operand: inside the project folder or
  an `--allow-path`, no link out. A document of another project the same app has
  open is refused, as in a headless run; an id the app does not know is refused
  with the app's reason. `close_active_document` and `assign_net_class` name
  their document and are confined the same way. `save_document` also needs
  `--save`, and its named file is checked as a write target as in a headless
  run. A save of a project file writes libraries besides it (the headless run
  lists them with the app's `save_side_writes` and confines each); the running
  app tells them in the `save_writes` of its `document_info` answer, and the
  save is sent only when the project file and every file in `save_writes` are
  inside the run's folders. Otherwise it is refused without naming the file
  (it may belong to another project; `--allow-path` adds a folder); an answer
  without the list is refused too. `save_writes` is never printed: the CLI
  removes it from the `document_info` answer it shows (a headless run shows
  its own project's, the pending libraries of the project it holds). A sheet's
  plain save writes nothing else. The resolve, the `document_info` that lists
  the writes, and the command are separate requests, so a document that is
  re-pointed or a project that gets another pending library between them (a
  person saving as, closing and reopening, adding a library) is the same
  residual race as above: what the save writes is what the person's app holds
  when it runs.
- **Without `--save`.** Nothing is written to disk by the CLI when
  attached. What a command changes stays in the person's open app as unsaved
  changes; the person saves, or undoes it. With `--save`, a `save_document` of a
  sheet asks the app to save that sheet, which writes the person's unsaved work
  on it too.
- **Audit.** The app lists every request, with the CLI's process id, in its
  Messages panel. The CLI itself logs nothing when attached.
- **`--attach`** fails with exit 2 and no output if no running Signex has the
  project open. **`--headless`** fails with exit 2 if one does: the CLI does not
  edit a file that is open in a window. Without either flag: attach if a
  running Signex has it, else headless.

### Holding the project

A headless run holds its project for the run: it takes an exclusive lock on
the project's lock file in the registry folder and writes an entry of kind
`headless` for the project (no endpoint). It removes the entry when the run
ends (an error, a panic, a refused command); the empty lock file stays, as
deleting a locked file would let a second run lock the unlinked one. A run
killed by a signal cannot clean
up, but the kernel drops its lock with the process, so its entry is found stale
at once, whatever process reuses its id; the next run or Signex removes it.
A Signex that opens the project meanwhile refuses with a line in Messages and
an Open Failed notice that names the entry file, and changes nothing; it
opens once the run has ended.

A headless run (default or `--headless`) that finds another headless run
holding the project is refused with "the project is held by another
signex-cli run (pid N)" and exit 2. A run never goes without its hold: if it
cannot read the registry (a folder others can enter, a newer entry format), or
cannot write its entry, it fails with exit 2. If a running Signex registered
the project between the look and the hold, a default run attaches to it; a
forced `--headless` fails.

### A project open in Signex that does not listen

A Signex window holds a shared lock on every project it has open, from the
moment it opens it until it closes it (many windows can have one project open;
a headless run holds the lock exclusively, so it cannot take a project any
window has), and lists it in the registry, even while "Allow agent
connections" is off; its entry then names no socket. A run that finds the
project's lock held (and no entry it can attach to) does not run headless on
the same files (two copies of the document would diverge). On any route
(`--attach` too) it stops with exit 2 and no output:

```
<project> is open in Signex (pid N). To let an agent work on it, turn on Preferences > Allow agent connections.
```

(With the lock held and no entry yet, the message says the project is open in
Signex or held by a run that is just starting, without a pid.) Once the person
turns the setting on, the same entry names the socket and the run attaches. A
Signex that crashed leaves its entry, but its lock is gone, so the entry is
stale and the run goes on. An entry is never stale while its lock is held.

The lock and the entries live in one runtime folder per user, and the window
and the CLI must name the same one: `$XDG_RUNTIME_DIR/signex-agent`; on Linux,
with that variable unset (a shell over ssh, cron), `/run/user/<uid>/signex-agent`
when that folder of the user exists (a real folder of theirs, not a link), so a
desktop Signex and such a shell agree; else `<user temp>/signex-agent-<uid>`.
On macOS the user temp is the system's per-user folder (`confstr`,
`_CS_DARWIN_USER_TEMP_DIR`), not `TMPDIR`, so a Finder launch and a shell
without `TMPDIR` name the same one (if `confstr` fails, `TMPDIR`/`/tmp` is
used). Windows uses the local application data. Both sides also look in the
user's other possible folders (the `TMPDIR` one, `/tmp/signex-agent-<uid>`, the
`/run/user/<uid>` one, `$XDG_RUNTIME_DIR`), through the same trust rule as the
primary (a real folder of the user that nobody else can enter), and count a
lock held there: a CLI `lookup` or `hold` sees a window or a run that keeps its
folder in another place, and the window refuses to open a project a headless
run holds exclusively in another one. Known limit: a lock is only looked at in
those other folders, not taken, so two apps that name different folders and
start at the same moment can still both pass.

## Security

The input may come from an agent that read untrusted text, so every rule below
is enforced before the app sees a command.

- **No writes without `--save`.** `save_document`, `save_document_as` and
  `new_document` need the `save` capability, which only `--save` grants.
  Nothing is saved at exit. In a run without `--save`, the result lines and
  the end-of-run log say what would change; no project file is written, and
  no agent journal (`.signex/`, #2253). A headless run with `--save` journals
  its commands and its start and end; an attached run is journaled by the
  running Signex, never by the CLI.
- **Path confinement.** Every path argument is resolved first (symlinks
  followed, `..` removed) and must then sit inside the project folder, unless
  its folder is named with `--allow-path`. The folder is the one that holds
  the project file, fixed when the run starts; it may not be a filesystem
  root (`/`, `C:\`, `\\?\C:\`, a UNC share root, or a link to one), the home
  folder or one above it (`run ~/x.snxsch` is refused). What the project file lists
  (libraries, sheets) never adds a folder. A dangling link is refused. The
  resolved path is what the app receives, and it is resolved and compared
  again right before the command is handed over. A write target is never a
  symbolic link, and a command that writes (`save`, `export`,
  `library-write`) is checked for the files it writes without being told (the
  active document, documents with unsaved changes, the project file) before
  it runs.
- **What is not confined.** Only the paths a command names are checked and
  size-limited. Files the project itself refers to are read by the app's own
  loader without any check or size limit: a sheet entry with `../` or an
  absolute path, hierarchical child sheets, library files. Reading them is
  possible; writing to one that is outside the project folder or is a link is
  refused (the implicit write-target check above).
- **Residual race.** The checks run before the command and again just before
  it is handed to the app, but the app then opens or writes the path itself,
  and a save is asynchronous: the window is the whole time the save takes, and
  a path that did not exist at the check can be created as a link before the
  write. A process of the same user racing symlinks inside the project folder
  is out of scope.
- **Git.** A headless run never opens, creates or commits to a repository,
  whatever `enable_git` says and wherever `.git` points (a `gitdir:` file, a
  link, config includes). A save logs "commit skipped in a headless run";
  commit yourself after the run. An attached save is different: it runs in the
  person's Signex, as their own Save does, so it commits with the person's git
  identity exactly as their save would, including `git init` and the first
  commit of a pending library that asks for `enable_git`. Who made the change
  is not recorded in the commit (provenance is tracked in #2253).
- **Capabilities.** Each catalog command needs exactly one group: `read`,
  `edit`, `save`, `export`, `delete` or `library-write`. A run holds
  `read,edit`; `--allow` adds groups. A refused command names the one it needs. `edit` is any command
  the catalog marks `mutates_doc`; the writers and exporters are pinned by a
  test. `delete` has no command yet: it exists so the first file-removing
  command cannot run unnamed.
- **Honest results.** A command whose only effect was a window the headless
  driver dropped is `refused` ("needs a window"; the catalog already refuses
  the known ones, see `gui_only` below), and so is a dialog that could not be answered. Closing a
  document with unsaved changes closes its tab and parks the edits in memory,
  as in the GUI; the end-of-run log lists the sheet as unsaved.
- **No code execution, no network.** The run uses a throwaway config root,
  never the user's. Launching a browser, viewer or file manager is refused by
  the app's headless ports, and no command that reaches a library server is
  runnable headless (every `library` command is `gui_only`).
- **Same enablement as the GUI.** `dispatch_command`'s enablement gate
  applies; a command the GUI would grey out is `refused` with its reason.
- **`gui_only` commands** (their whole effect is window UI, or they need a
  pointer and have no operand form) are `refused` with "`<id>` only works in
  a window"; `commands --json` lists them as `"headless": "gui_only"` with
  that reason. Every catalog command is one or the other (#2170): window
  only, or it runs (a document change, operands, or the `runs_headless`
  flag); `every_command_is_classified_for_headless_use` fails otherwise.
- **Input limits.** A line over `--max-line-bytes` (default 64 KiB), more than
  `--max-commands` (10000) lines, or a path argument to a file over
  `--max-file-bytes` (64 MiB) is refused. Malformed JSON, an unknown field or
  text that is not UTF-8 is a refused line, never a panic.
- **Audit.** Every command line, its status and its reason go to the Messages
  log and to standard error, tagged with the run id. (Attached, the app's
  Messages panel keeps the audit: see above.)
