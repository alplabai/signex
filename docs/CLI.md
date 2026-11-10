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
```

`<project>` is a `.snxprj` (or a loose `.snxsch`). The exit code is `0` when
every command was ok, `1` when a command was refused or failed, `2` for a
usage error, a project that cannot be opened or a run that cannot be placed
(`--attach` or `--headless` below, an unsafe token file).

A run works in one of two places (#1924): in a running Signex that has the
project open, or headless, as described above. See "Working through a running
Signex".

| Option | Meaning |
|---|---|
| `--attach` | only work through a running Signex; fail (exit 2) if none has the project open |
| `--headless` | only run headless; fail (exit 2) if a running Signex has the project open |
| `--keep-going` | run the rest after a refused or failed command (default: stop at the first) |
| `--save` | allow commands that write files (capability `save`) |
| `--dry-run` | run every command with `dry_run=true`: a wire or bus run reports and changes nothing. A read-only query and `open_document` run as given; any other command without that operand is refused, and so is `--save` together with it |
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
`draw_graphic_line`, `draw_rectangle`) take an optional `document`, an id or a
path as for `apply_edits`. Given, the command runs on that sheet and never on
the active tab: pin references, the intent checks and their scratch copies, the
commit, undo and the dirty flag all go to it, and no tab or focus changes. An
unknown, unloaded, non-schematic or read-only document is refused with its
reason. Without it, the command acts on the sheet the edit targets (the
`apply_edits` document, else the session's own active sheet). Inside
`apply_edits` an edit may repeat the call's `document` but not name another
("an edit names another document than the list"). The optional `document` does
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
   touches R5.1 at 20.32,5.08"). "Touches" is what the netlist joins: a pin
   the run only passes over, with no wire end or junction at its tip, is not
   connected and not named. What an end intends is what is connected there
   before the run (wire ends, junctions, labels, connected pins, sheet pins) and
   what the caller named: a pin reference names only that pin (another pin on
   the same tip is extra), a coordinate names the pins at the point. A start
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
   labelled `VCC`, whose entry the parent wired to `GND`, is a short. So the
   same final design reached by a different order of edits can get a
   different verdict; the check judges the edit, not the design.
7. **Buses** follow the same rule with bus names (hierarchical bus labels are
   aliases), and a bus run ending on a sheet entry is connected, not open.

A short or rename is refused unless `allow_merge=true`. The rule is one
function (`signex_net::join_rule`), read by the per-sheet checks, the bus
checks and the project check. A run's answer lists the names the joined net
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
  (points 2 to 5), for nets and, with bus names, for bus runs: a hierarchical
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
- **Merge**: the runs the two ends attach to are a short by the name rule of
  "Wire intent" with bus names (hierarchical bus labels are aliases): two
  runs with different own bus names, or two alias-only runs that share no
  name; refused unless `allow_merge=true` ([Why `allow_merge`](#why-allow_merge)).
  One name and one unnamed run, or an own-named run and an alias-only run,
  is allowed.

There is no net diff: bus connectivity is the bus analysis, not the netlist.
`dry_run` works as for a wire; the answer is `{"dry_run", "ends", "segments",
"bus": [names of the run], "merged": [names], "aliases": [names]}`, `aliases`
as for a wire (the member nets' names that reach them only through a child's
hierarchical label).

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
- **Dry run.** Without `--save`, nothing is written to disk by the CLI when
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
  Nothing is saved at exit. A run without it is a dry run: the result lines
  and the end-of-run log say what would change; no project file is written.
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
