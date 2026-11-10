# Agent journals (#2253)

A project keeps a persistent record of what agents did in it, so a person can
see later which changes an agent made, under which rights and when, and replay
them. Two files under `.signex/` in the project folder, both JSON Lines (one
event per line, never YAML: an append that is cut short leaves at most one
partial last line, and YAML's implicit typing reads `NO` as `false`).

| File | What |
|---|---|
| `agent-edits.jsonl` | every agent command that changes a document, or writes or closes one (a `mutates_doc` command, `save_document`, `save_document_as`, `new_document`, `new_project_with_profile`, `close_active_document`), whatever came of it |
| `agent-access.jsonl` | a connection accepted, refused (with the reason) or closed; the rights a client reported; a headless run's start and end |

Read queries are not logged one by one: the access record covers them.
Journals are not documents: they never mark one dirty and are not part of Save
All. They are written when the event happens.

## Lines

Every line carries `"v": 1` first. Readers skip a line with a newer `v`, say
so, and never rewrite it.

An edit line: `ts` (UTC, to the millisecond, taken when the command ran),
`pid`, `client`, `document` (the persistent `DocumentId`), `path` (at the time
of the event), `command`, `operands` (by name, as the wire carries them),
`revision_before`, `revision_after`, `group` (the undo group of the step, when
it made one), `outcome` (`ran`, `refused` or `failed`), `reason`, `truncated`
and `replayable`.

An access line: `ts`, `event` (`accepted`, `refused`, `closed`, `grants`,
`started`, `ended`), `mode` (`socket` or `headless`), `pid`, `client`, `reason`
and `capabilities` (`read`, `edit`, `save`, ..., `allow-path:<dir>`).

A line is at most 64 KiB. An edit over that keeps its `summary` and drops its
other operands, with `"truncated": true, "replayable": false`.

Replay: `EditEntry::replay_request(current_revision)` returns the command and
operands to send again, and refuses unless the document is at
`revision_before` (the `expect_revision` rule of `apply_edits`, #2178), the
entry ran, and its operands were not dropped.

A session token never reaches a journal: a request's token is not an operand,
and an operand that contains it is replaced by `[redacted]`.

## Append

`append_line` (`app/agent/journal/append.rs`):

1. `.signex/` is created with `create_dir`, never `create_dir_all`, and a link or
   a file of that name is refused.
2. The sibling `.signex/agent-journal.lock` is opened, and an exclusive advisory
   lock is taken on it (`File::lock`: `flock` on Unix, `LockFileEx` on
   Windows). It is a sibling because rotation renames the journal, which would
   orphan a lock on the old file. Both journals share it. It is in the managed
   `.gitignore` block always.
3. The journal is opened `O_NOFOLLOW` (Windows: `FILE_FLAG_OPEN_REPARSE_POINT`
   and a check that it is not a link), created with mode 0600, and refused
   unless it is a regular file. The refusal is reported.
4. If the file ends without a newline (a crash cut its last line), the write
   starts with one, so the new line is never fused with the cut one.
5. The whole line and its newline are written by one `write`.
6. Still under the lock, the journal is rotated if it is over the cap.

Two processes (a window and a headless run) append safely: the lock is the
operating system's, so it holds across processes, and the #1924 project hold
already allows one writer per project.

## Rotation

The cap is the project setting (default 10,000 lines). A file of `n` bytes holds
at most `n / 64` lines (no entry is shorter), so the lines are only counted
above that. Over the cap, the newest `max_lines` lines are written to a
temporary file (`atomic_write`) that is renamed over the journal: a reader sees
the old file or the new one. Lines a reader does not understand are kept as the
bytes they are while they are within the newest lines. Lines dropped stay in git
history while the journals are kept in git; with git off they are discarded.

## Reading

`read_journal` skips a trailing partial line, a line of a newer `v` and a whole
line that is not an entry, and reports each (`Report::problems`). It opens the
file the same safe way. `signex-cli agent-log <project> [--access]` prints the
table and the problems on standard error; it writes nothing.

## Who writes

- **The window** journals every agent request it serves (the socket) and every
  connection it sees. The CLI never writes journals in an attached run: it sends
  its name and rights (`client` and `grants` on each request, see
  `docs/AGENT_SOCKET.md`) and the app records them.
- **A headless run** journals only with `--save`. A run without it writes
  nothing in the project folder: no `.signex/`, no journals. That is the
  existing promise of `docs/CLI.md`, "No writes without `--save`". A run with
  `--save` writes an access line at start and end, and an edit line per command.
- An access event concerns every project the app has open (a connection can
  reach all of them), so it is written to each of their access journals.

## Writing without blocking

`update` only puts a line on an ordered per-project queue
(`journal/queue.rs`). One task per project at a time appends what waits, on a
worker, in the order the events happened; the time of a line is taken in
`update`, when the event happens, not by the task. A failure (a read-only
folder, a refused link, a lock error) is said in Messages as "Agent journal:
...", and on standard error for the CLI. It never stops the command, which
answers as it would without a journal.

## Settings

Stored in the `.snxprj` as one optional object `agent_journal`, skipped while
untouched, so an old project re-saves byte-identical and the format token
(`snxprj/1`) does not change. An older Signex keeps the key in `unknown`.

| Setting | Default | Meaning |
|---|---|---|
| `keep_in_git` | on | off adds `.signex/agent-*.jsonl` to the managed `.gitignore` block |
| `summarise_in_commit` | off | stored only; the commit body is a later change |
| `max_lines` | 10,000 | lines per journal, 100 to 1,000,000 |

The `.gitignore` block follows `keep_in_git` when the project is saved, not when
the box is ticked: the setting is not saved until then, and the person may
discard it. It applies where the project has a `.git`. Turning it off does not
untrack journals already committed: Signex never runs `git rm`. Do that
yourself if you want them gone from the index.

## Not covered

A person with write access to the project folder can edit or delete a journal:
it is a record, not a tamper-proof log. The `O_NOFOLLOW` open stops a link at
the journal's own name; a link planted in a parent folder between the check
and the open is out of scope, as in `docs/CLI.md`, "Residual race".
