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

A command that asks a GUI dialog (open, new) takes the dialog's answer as
`path` and `overwrite`. A dialog that cannot be answered (an existing file
without `overwrite: true`) is `refused`, never `ran`. `save_document_as` takes
`document`, `path` and `overwrite`: an existing target without
`overwrite: true` is `refused`. An open or new that makes nothing, with an
error notice, is `failed` with that notice.

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
  driver dropped (`report_manager_bom`, `annotate_schematic`) is `refused`
  ("needs a window"), and so is a dialog that could not be answered. Closing a
  document with unsaved changes closes its tab and parks the edits in memory,
  as in the GUI; the end-of-run log lists the sheet as unsaved.
- **No code execution, no network.** The run uses a throwaway config root,
  never the user's. Launching a browser, viewer or file manager is refused by
  the app's headless ports, and no command that reaches a library server is
  runnable headless (every `library` command is `gui_only`).
- **Same enablement as the GUI.** `dispatch_command`'s enablement gate
  applies; a command the GUI would grey out is `refused` with its reason.
- **`gui_only` commands** (no operand form yet) are `refused` with the
  catalog's reason; `commands --json` lists them as `"headless": "gui_only"`.
- **Input limits.** A line over `--max-line-bytes` (default 64 KiB), more than
  `--max-commands` (10000) lines, or a path argument to a file over
  `--max-file-bytes` (64 MiB) is refused. Malformed JSON, an unknown field or
  text that is not UTF-8 is a refused line, never a panic.
- **Audit.** Every command line, its status and its reason go to the Messages
  log and to standard error, tagged with the run id.
