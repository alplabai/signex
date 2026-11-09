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
usage error or a project that cannot be opened.

| Option | Meaning |
|---|---|
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
{"run": "<uuid>", "seq": 2, "id": "color_net", "status": "ran",
 "messages": [{"level": "INFO", "message": "..."}], "changed": ["<path>"]}
```

| Field | Meaning |
|---|---|
| `run`, `seq` | the run id (also in the audit log) and the line's 1-based place in the run |
| `id` | the command id, `null` when the line was not a command |
| `status` | `ran`, `answered` (a query; the answer is in `data`), `refused` (nothing ran; `reason` says why) or `failed` (it ran and did not succeed; `reason`) |
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
`apply_edits` only.

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
4. **Short**: both ends are on named nets and the two nets carry different
   sets of names (every label text and power-port value of each net, not only
   the best name: `{DATA, P}` and `{DATA, Q}` are two nets that share a name);
   refused unless `allow_merge=true` ([Why `allow_merge`](#why-allow_merge)).
5. **Open end**: an end with no pin tip, wire, junction, label, sheet pin or
   bus entry (either end of it) under it; refused unless `allow_open=true`.
6. The two ends must be on one net afterwards; a run that is not is refused
   whatever the flags (it cannot happen for a connected run, so it signals a
   bug rather than a choice).

`dry_run=true` (`--dry-run`) answers the report and changes nothing: no undo
step, no dirty flag, no Messages line. A run that lands answers
`{"dry_run": false, "ends": [{"ref": "R1.2", "at": [x, y]}, {"at": [x, y]}],
"segments": [[x1,y1,x2,y2],...], "net": {"name": "N$3", "pins":
["C1.1","R1.2"]} | null, "merged": [names]}`; `net` is `null` when no pin is on
the run's net. `ends` records both ends as given: the pin reference (`ref`,
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

## Labels and power ports

`place_net_label`, `place_local_net_label`, `place_global_label`,
`place_hierarchical_label` and `place_power_symbol` from operands, at a
coordinate or a pin reference, take every name the net already at `at` carries
(label texts and power-port values: `signex_net::net_names_at`). If the net has
names and the new label text or power `net_name` is not one of them, the
placement is refused as a short ("`at` 16.51,12.7 is on net VCC, X; a GND label
there would short VCC, X and GND; pass allow_merge=true to merge on purpose").
A name the net already carries, an unnamed net, or nothing at `at` is allowed.
`allow_merge=true` places it anyway ([Why `allow_merge`](#why-allow_merge)).
There is no `dry_run` for these commands. A power flag (`power:PWR_FLAG`) never
names a net, so it is not checked for a short.

`place_junction` has the same check at its point, by coordinate or pin
reference: it compares the name sets of every net the junction would join (the
wires under the point) and refuses when two named nets carry different names
("a junction at 12.7,25.4 would join the nets {VCC} and {GND}"), unless
`allow_merge=true`. Unnamed nets are what a junction is for and stay allowed.

Two more refusals apply to a pin reference only (a coordinate is unchanged),
and have no escape. The pin-reference placements that are join-checked are the
wire, the label, the power port, the junction and the no-connect:

- A label, power port, junction or no-connect whose tip lies under a wire the
  pin is not on (no end or junction there), or on which another pin is butted,
  is refused ("the label on R1.2 also touches pin C1.1 ... would join them to
  the net"): it would anchor the tip and join that net. This is rule 3 of
  "Wire intent", applied to the tip.
- A power port on a pin whose tip has no wire end, junction or label is refused
  ("would touch the pin without connecting"): pin to pin is no connection, so
  draw a wire to it first or place a label. A no-connect flag does not count as
  a connection.

## Bus geometry

`place_bus` and `draw_bus` from operands follow the geometry rules of a wire
(orthogonal segments, no body crossing, no pin references) and refuse a
dangling end: an end must sit on an existing bus (an end or its interior), a
bus entry (either end) or a bus-name label, unless `allow_open=true`.

They are intent-checked too, with the one bus analysis (`analyse_buses`) applied
to the sheet before and after the run (`signex_net::bus_intent`):

- **Extra connection** (never allowed): another bus run touched by its end or
  interior along the path or at a bend, a bus entry, or a bus-name label that
  ends up on the new run and was not on a run an end attaches to ("the bus also
  touches bus D[0..7] at 12.7,38.1 ..."). What an end attaches to is the intent.
- **Merge**: the runs the two ends attach to carry different bus names;
  refused unless `allow_merge=true` ([Why `allow_merge`](#why-allow_merge)).
  One name and one unnamed run is allowed.

There is no net diff: bus connectivity is the bus analysis, not the netlist.
`dry_run` works as for a wire; the answer is `{"dry_run", "segments", "bus":
[names of the run], "merged": [names]}`.

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
- **No git.** The CLI does not commit to git: a headless run never opens,
  creates or commits to a repository, whatever `enable_git` says and wherever
  `.git` points (a `gitdir:` file, a link, config includes). A save logs
  "commit skipped in a headless run"; commit yourself after the run. Vetted
  git support is a follow-up.
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
  log and to standard error, tagged with the run id.
