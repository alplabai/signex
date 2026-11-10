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
`session`, `pid`, `client`, `document` (the persistent `DocumentId`), `path` (at
the time of the event), `command`, `operands` (by name, as the wire carries
them), `revision_before`, `revision_after`, `group` (the undo group of the step,
when it made one), `outcome` (`ran`, `refused` or `failed`), `reason`,
`truncated` and `replayable`.

An access line: `ts`, `event` (`accepted`, `refused`, `closed`, `grants`,
`started`, `ended`), `mode` (`socket` or `headless`), `pid`, `client`, `reason`,
`capabilities` (`read`, `edit`, `save`, ..., `allow-path:<dir>`) and, when
something was cut, `truncated`.

A line is at most 64 KiB. An edit over that keeps its `summary` and drops its
other operands, with `"truncated": true, "replayable": false`. An access line
over it keeps the first 64 rights, each at most 1,024 characters, and drops them
all if it is still too long, with `"truncated": true`.

**Replay and sessions.** The engine's revision counter starts again in every
process, so a revision means something only within the session that recorded
it. Each edit line carries a `session` (an id minted when the app, or a headless
run, starts), and `EditEntry::replay_request(session, current_revision)` returns
the command and operands to send again only if the line is of that session,
the document is at `revision_before` (the `expect_revision` rule of
`apply_edits`, #2178), the entry ran, and its operands were not dropped. A line
of another session is refused with that reason, never accepted at a number that
happens to match. A line with no session (an older build) and a `revert` line
(its undo group exists only in its session) are never replayable. The
document's content is not compared: a line of an earlier session can only be
replayed by reading the document again, not blindly. A content hash that would
let a replay cross sessions is a later change.

A headless `--save` run journals its edits as `ran` when they ran in memory,
even if the run never saves them: the journal records what the agent did, not
what reached the disk (the run's end-of-run log names the unsaved documents).

A session token never reaches a journal: it is replaced by `[redacted]` in every
text of an entry before the free fields are cut to length (so a token cut at the
boundary cannot survive as a prefix), and again in the encoded line before it is
queued, so no field of either journal can carry it, whether an echoed operand, a
client name or a right.

## Append

`append_line` (`app/agent/journal/append.rs`):

1. `.signex/` is created with `create_dir`, never `create_dir_all`, and opened
   once. On Unix that is `O_DIRECTORY | O_NOFOLLOW`, so a link or a file of that
   name is refused by the open itself; every file below is then opened with
   `openat` relative to that handle, and the rotation's rename is a `renameat`
   on it. A folder swapped for a link after it was opened therefore changes
   nothing: the files are reached through the handle, not the path. On Windows
   the folder is opened with the reparse-point flag, without
   `FILE_SHARE_DELETE` (so it cannot be renamed or replaced while the handle is
   held) and its final path (`GetFinalPathNameByHandleW`) is kept; each file,
   and the rotation's temporary file before anything is written to it, must
   resolve under it. The rename goes by path: Windows has no `renameat`, and the
   held handle with those checks gives what a rename by handle would. This is
   verified by CI only (it cannot be run where it was written).
2. The sibling `.signex/agent-journal.lock` is opened, and an exclusive advisory
   lock is taken on it (`File::lock`: `flock` on Unix, `LockFileEx` on
   Windows). It is a sibling because rotation renames the journal, which would
   orphan a lock on the old file. Both journals share it. It is in the managed
   `.gitignore` block always.
3. The journal is opened `O_NOFOLLOW` (Windows: `FILE_FLAG_OPEN_REPARSE_POINT`),
   created with mode 0600, and refused unless it is a regular file with exactly
   one link (`nlink`; Windows `nNumberOfLinks`), so a hard link to a file that
   lives elsewhere is not written through. The same check holds for the lock
   file and for the rotation's and the reader's open. The refusal is reported.
   On Unix a new file is made by an exclusive create (`O_CREAT | O_EXCL`, an
   existing file is fine) and then opened *without* `O_CREAT`: on macOS a plain
   create-open raced by another creator of the same new name can fail with
   ENOENT (reproduced with six threads appending to a fresh project), and this
   form has no such race. On Windows the folder handle is also checked for
   `FILE_ATTRIBUTE_DIRECTORY` and not `FILE_ATTRIBUTE_REPARSE_POINT` before any
   file is created.
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
above that. Over the cap, the journal is rotated down to 90 % of it (so a
journal that was just rotated is not rewritten by the next append, only once it
is past the cap again): two passes over the file in 64 KiB chunks, the first
counting its lines, the second copying all but the oldest into a private
temporary file (`agent-*.jsonl.<pid>-rot.tmp`, covered by the managed `*.tmp`
rule) that is renamed over the journal, and the folder is synced after the
rename (as `atomic_write` does). Memory stays constant, whatever the
size, and there is no size at which rotation gives up. A reader sees the old
file or the new one. Lines a reader does not understand are kept as the bytes
they are while they are within the newest lines. Lines dropped stay in git
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
- **Which project an access event goes to.** A project's journal never names
  another project's place:
  - `accepted` and `closed` carry only a pid and go to every project the app has
    open when the connection opens; `closed` goes only to those of them that
    are still open. A project opened while the connection lasts has no
    `accepted`, so it gets no `closed`; one closed meanwhile simply has no
    journal to write it to.
  - `grants` (the rights a client reports) go only to the project the request
    names (its `document`), and only the `allow-path:` roots inside that
    project's folder are kept; the others are replaced by a count
    (`allow-path:(2 outside this project not recorded)`).
  - A request turned away goes to the project it names; one that names none (a
    bad token, a malformed line) goes to every open project, and a reason that
    contains another open project's folder has it replaced by `[another
    project]`. The same holds for a connection refused at connect.
  - A headless run's `started` line lists only the roots inside the project.

## Writing without blocking

`update` only puts a line on an ordered per-project queue
(`journal/queue.rs`). One task per project at a time appends what waits, on a
worker, in the order the events happened; the time of a line is taken in
`update`, when the event happens, not by the task. A failure (a read-only
folder, a refused link, a lock error) is said in Messages as "Agent journal:
...", and on standard error for the CLI. It never stops the command, which
answers as it would without a journal.

At quit the queue is flushed like the preference writes (`wait_for_journal_then`
in `flush_quit_writes_then`): the lines that had not started are taken, the
writes in flight are waited for (at most 10 s, said if it ran out), and the
taken lines are written, all before the process exits. A connection still open
at quit gets its `closed` line (reason `app quit`) first.

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
discard it. It applies where the project is under a git repository: git reads a
`.gitignore` in every folder and the rule is anchored, so the block goes into
the project folder's own `.gitignore` whether the repository's root is that
folder or any folder above it (a project in a subfolder of a repository, or
under a dotfiles repository in `$HOME`, is handled the same way, with no
warning). A project under no repository is not touched. Enable Version Control
writes the same block from the setting. Turning it off does not untrack
journals already committed: Signex never runs `git rm`. Do that yourself if you
want them gone from the index.

The keys of `agent_journal` are read strictly (`max_lines` must be a whole
number, in range or clamped into 100 to 1,000,000). A stored value this build
cannot read, a wrong type such as `"max_lines": "5000"` included, is not
silently reset: the defaults apply, Messages says so, and the raw value is
written back untouched until the settings are edited. Keys a newer build stored
beside the known ones are kept across a re-save.

## Not covered

A person with write access to the project folder can edit or delete a journal:
it is a record, not a tamper-proof log. A link planted in a parent of the
project folder is out of scope, as in `docs/CLI.md`, "Residual race"; the
`.signex/` folder itself and what is in it are checked and opened through one
handle, as above.
