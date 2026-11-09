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
`find_items` and `read_document` are queries: their result line is `answered`
with the answer in `data`. `open_document` and `new_document` answer with the
new document's `{id, path, kind}`.

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
  "attached"`; `messages` are what the app logged during the command and
  `changed` the documents whose content or unsaved state differ.
- **Policy first, the app's allowlist on top.** Capabilities, `--save`,
  `--allow-path` and the input limits are applied to every command, and to
  every edit inside `apply_edits`, before anything is sent. The app then
  applies its agent allowlist and refuses what an agent may not do, and its
  reason comes back as the result line's `reason` (`undo` and `redo`, which
  are the person's; `save_document`, `close_active_document` and
  `assign_net_class`, held back; a command with a file path; anything that
  does not name its document). A command that acts on a document (it has a
  `document` operand and is not a query) and is sent without one is refused
  here with "name the document": the app would use the person's active
  document, which is never checked against the run's folders. Every `document` operand, an id or a path, in
  the command and in each edit of `apply_edits`, is first resolved by the app
  (a `document_info` query on the same connection) to the file it holds, and
  that file is confined like a path operand: inside the project folder or
  an `--allow-path`, no link out. A document of another project the same app has
  open is refused, as in a headless run; an id the app does not know is refused
  with the app's reason. The side writes of a save are not checked here: the app
  refuses every file-writing command for an agent. The resolve and the command
  are two requests, so a document that is re-pointed between them (a person
  saving as, closing and reopening) is the same residual race as above.
- **Dry run.** Nothing is written to disk by the CLI when attached. What a
  command changes stays in the person's open app as unsaved changes; the
  person saves, or undoes it. (`--save` lets a `save_document` through the CLI's
  own policy, but the app refuses it for an agent today.)
- **Audit.** The app lists every request, with the CLI's process id, in its
  Messages panel. The CLI itself logs nothing when attached.
- **`--attach`** fails with exit 2 and no output if no running Signex has the
  project open. **`--headless`** fails with exit 2 if one does: the CLI does not
  edit a file that is open in a window. Without either flag: attach if a
  running Signex has it, else headless.

### Holding the project

A headless run holds its project for the run: it writes an entry of kind
`headless` for the project (no endpoint) in the same registry, and removes it
when the run ends, however it ends (an error, a panic, a refused command).
A Signex that opens the project meanwhile refuses with a line in Messages and
an Open Failed notice, and changes nothing; it opens once the run has ended.
A crashed run leaves an entry that is stale once its process is gone. A
headless run that finds another headless run holding the project goes on as
before and leaves that run's entry alone.

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
  log and to standard error, tagged with the run id. (Attached, the app's
  Messages panel keeps the audit: see above.)
