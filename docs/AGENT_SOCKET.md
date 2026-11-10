# Agent socket (#1924)

A person keeps Signex open; an agent works on the same project through the CLI.
The agent is a component of Signex, not a user of its window: it names its
document, uses the engine channel and never changes the person's active tab,
focus, panels, selection or tool. This page is the design of the running app's
side: the listener, its security and the registry. The CLI side (connect when a
live entry exists, otherwise run headless and hold the project; a GUI refusing
a project a headless run holds) is "Client" below and in [CLI.md](CLI.md).

Code: `crates/signex-app/src/app/agent/`; the client half is `client.rs`,
which `signex-cli` uses through `signex_app::headless::agent_client`.

## Off by default

Preferences > Editing > "Allow agent connections" (`agent_connections_enabled`
in `prefs.json`, default `false`). While it is off no socket and no token
exist and nothing listens or accepts a connection. The app still lists each
open project in the registry (below) and holds its lock, so that a headless
`signex-cli` run does not edit a file open in a window; those entries name no
socket. Turning the setting on at Save starts the listener and the entries
name its socket; turning it off at Save ends it: the socket is closed, the
connections are dropped, the socket and token files are removed, and the
entries lose their socket. A headless run never listens, whatever its config
root says, and lists nothing.

## Transport

The listener is an iced `Subscription` (`listener.rs`). One future owns the
socket, every connection and the files, so ending the subscription ends all of
them. A connection only frames lines; each request line becomes
`Message::Agent(AgentMsg::Request { .. })` (its own root family, routed to
`dispatch_agent_message`) and runs inside `Signex::update`,
like a click. That is one sequential queue: a person's edit and an agent's
request never interleave inside an edit. The reply goes back to the connection
when `update` has run the request. No window is created and nothing in
`iced::window` is called.

- macOS and Linux: a Unix socket `<runtime dir>/<boot id>.sock`, mode 0600.
- Windows: the named pipe `\\.\pipe\signex-agent-<boot id>`.
- No TCP port, no network.
- At most 8 connections at once; one that sits idle for 120 s is closed, and
  so is one that does not read a reply within 30 s.
- The listener's reports (`ListenerStarted`, `ListenerFailed`) carry its boot
  id; a report of a listener the app has since replaced is ignored.
- On Windows a client that connects and closes at once breaks the waiting pipe
  instance; it is replaced with a fresh one and is not counted as a failure.

## Wire format

JSON Lines: one request object per line, one reply object per line, in order.

```
{"token": "<session token>", "id": "apply_edits", "args": {"document": "<id or absolute path>", ...}}
{"id": "apply_edits", "status": "answered", "data": {...}, "messages": [...], "changed": ["/abs/path.snxsch"]}
```

`args` take the forms `signex-cli` sends (a string, a number, a boolean,
`[x, y]` for a point, the edit list of `apply_edits`). The reply is the headless
runner's envelope without its `run` and `seq`: `status` is `answered`, `ran`,
`refused` or `failed`, `reason` and `data` appear when there is one, `messages`
are the lines the command wrote to the Messages panel, `changed` the documents
whose content or unsaved state differ. `id` is `null` until the token has
proved the request is from a client that holds it. The statuses and the reading
of a command's outcome are `signex_app::headless::{Status, outcome_verdict}`,
shared with `signex-cli`: one mapping, never a fake `ran`.

## Allowlist

Derived from the command catalog (`allow.rs`), not listed by hand:

- the #2172 queries: `list_documents`, `document_info`, `resolve`, `find_items`,
  `read_document`, `query_pins`;
- a command that names its document with the `document` operand and takes no
  file path: today `apply_edits`, `revert`, `color_net`,
  `clear_all_net_colors`, `assign_net_class`, `save_document` and
  `close_active_document`. A test pins this set, so a catalog row that joins or
  leaves it is a visible decision.

The last three are open to agents by the maintainer's decision (#1924):

- `save_document` writes the named document only, as a person's Save of that
  document does; its side writes stay confined to what the headless run allows
  (`save_side_writes`). It never saves another document. For a project
  document, `document_info` answers `save_writes`: the absolute paths a save
  would write besides the project file (its pending libraries), so a client
  that confines writes can check them before it asks; absent for a sheet.
  The save runs in the person's app as their own Save does, so it commits to
  git with the person's identity like their save, including `git init` and the
  first commit of a pending library that asks for `enable_git`; the commit does
  not say an agent asked for it (provenance is tracked in #2253).
- `close_active_document` closes the named document's tab as its close button
  does; a document with unsaved edits is parked, never discarded. It is refused
  when the document is the person's active tab, or is shown in a window of its
  own, since closing it would move their tab or close that window, and when the
  person has a pending title-block or paper-edge draft for that sheet, since
  closing would drop their unconfirmed typing.
- `assign_net_class` runs by document as in the headless run: the project's
  class table changes, and the pick is one step on that sheet's undo timeline
  (#2171), after the person's own steps.

Refused with a reason: `undo` and `redo` (`PERSON_ONLY`, the one hand-kept
constant, checked against the catalog by a test; the agent takes back its own
step with `revert`), a command with a file path operand (`save_document_as`,
`open_document`, `new_document`), a window-only command (`headless_refusal`,
#2170), and every tool, selection or view command, which names no document.

`open_document` and `new_document` are refused on this channel: they take a
path, and opening a tab changes what the person sees. A document that is not
part of a project open in this app (a loose sheet) is refused too, and while
an agent's request runs `document_entries` lists only project-owned documents,
so `list_documents`, `find_items` and `resolve` never show a loose sheet
either. A person's own view and the headless run are not filtered.

## Security

1. **No network.** A Unix socket in a per-user directory (0700, socket 0600),
   or a Windows pipe whose security descriptor allows only the current user's
   SID (`D:P(A;;GA;;;<SID>)`) and which rejects remote clients.
2. **Peer check.** The client's user id comes from the kernel (`getpeereid` on
   macOS, `SO_PEERCRED` on Linux, through tokio's `peer_cred`) and must be the
   user running the app: `transport::unix::peer_allowed`, one decision, tested
   alone. On Windows the client's process is read from the pipe and its token's
   user must equal ours (`EqualSid`); a process that cannot be opened is
   refused. That fails closed: an elevated client of the same user connecting to
   a Signex that is not elevated cannot be opened, so it is refused; run both
   at the same level. A refused peer is logged (`PeerRefused`) and nothing is
   read. The socket itself is made with the process's umask and `chmod`ed 0600
   straight after; a `umask` call would be process-wide and race other threads'
   file creation, and until the `chmod` the socket is unreachable to others
   because its folder is ours and 0700.
3. **Session token.** 256 random bits from the OS, made at boot and again each
   time the setting goes off (so an old token never works again and a listener
   being dropped never removes the files of the next one), written to
   `<runtime dir>/<boot id>.token` (0600). Every request carries it; it is
   compared in constant time (`token.rs`). It is never logged: `AgentCall`'s
   `Debug` shows only the size of a request.
4. **Strict requests.** A line over 1 MiB is refused and the connection
   closed (the rest is read and dropped briefly so the reply is not lost to a
   reset). The request schema refuses unknown fields. Only the allowlist is
   accepted. A `document` resolves, through the #2172 resolution, to a document
   of an open project; no other path is read or written. A project that names
   a file outside its own folder is not read for an agent's query
   (`may_read_referenced`), as in a headless run.
5. **Visibility.** Every request is a line in Messages:
   `Agent request (pid N): `apply_edits` -> answered` (a refusal is a warning
   with its reason), and a refused peer is another. The log does not open the
   panel or take focus.
6. **Hygiene.** The runtime directory is created 0700; the app refuses to start
   if it is not a plain folder, is owned by another user or has any group or
   other permission. A file is removed (stale socket, temporary file, entry)
   only if it is the user's own plain file or socket, never a link or a
   folder; a link where a file goes is not followed or replaced and stops the
   start. The socket, the token and the registry entries of a boot are removed
   when the listener ends, and again on the quit path before the process exits.
7. **Off by default** (above).
8. **Local only.** Nothing here crosses machines.

The Windows directory is `%LOCALAPPDATA%\Signex\agent`, private to the user by
the profile's ACL; there is no mode or owner check there.

## Files

The runtime directory is `$XDG_RUNTIME_DIR/signex-agent`; on Linux, with the
variable unset (ssh, cron), `/run/user/<uid>/signex-agent` when `/run/user/<uid>`
is the user's own real folder, so a desktop app and such a shell name one
folder; else `<temp dir>/signex-agent-<uid>` (macOS: the per-user temp dir);
on Windows `%LOCALAPPDATA%\Signex\agent`. A client (`lookup`, `hold`) also
looks at the user's other possible folders and counts a lock held there.

```
<runtime dir>/                       0700
  <boot id>.sock                     0600   the socket (Unix)
  <boot id>.token                    0600   the session token
  registry/                          0700
    <digest>.json                    0600   one per open project
```

`<boot id>` is 16 random hex characters, new at every app start. The headless
config-root override does not move this directory.

## Registry

One entry per open project, written whenever a project is open (listening or
not), rewritten when the set of open projects or the listener changes, and
removed when its project closes and when the app exits. The app holds the
project's lock file (below) for as long as the project is open and releases
it when it closes; the lock file itself is never deleted. It is written for
the canonical path of the project file; the file name is the first 32 hex
characters of the SHA-256 of that path, so two spellings of one path share an
entry and the name holds no path text.

```json
{
  "format": 3,
  "kind": "gui",
  "project": "/canonical/path/Demo.snxprj",
  "pid": 4242,
  "socket": "/run/user/1000/signex-agent/3f9c0d1e5a7b2c44.sock",
  "boot_id": "3f9c0d1e5a7b2c44"
}
```

`kind` is `gui` (a running app) or `headless` (a `signex-cli` run that holds
the project and has no endpoint: `socket` is empty and `boot_id` is only its
identity). A `gui` entry with an empty `socket` is an app that has the project
open and does not listen (agent connections off): nothing to attach to, and
the project is not to be edited behind it. Format 2 added `kind`; an entry
without one is a format 1 entry and reads as `gui`. Format 3 added the
`gui` entry without a socket, which a format 2 reader would misjudge as
stale, so it reads a format 3 entry as one it does not know.

**The lock decides who has the project open.** Every window that has a project
open holds a SHARED lock on `registry/<digest>.lock`, beside the entry, from
the moment it opens the project (in the update that opens it, with one small
file open and a non-blocking lock) until it closes it; a headless run holds
the lock EXCLUSIVELY. So many windows can have one project open, a headless
run cannot take it while any window does, and a window cannot open a project a
run holds (it refuses, as above). A closed project keeps its lock while any
document write is still running (it goes with the update that reports the last
one done), and on quit the locks go with the cleanup task, after the document,
autosave and library writes, so a run never reads a file a save is still
writing. A lock that cannot be made is said once in Messages and tried again at
the registry retry tick (5 s), not at every update; a run that took the project
after that is said once too, with its process id. A stale entry is removed only
if the project is still unlocked when it is. The kernel drops a lock with its process, so
a process killed by a signal is found stale at once and a reused process id
means nothing. A reader asks for the lock exclusively without waiting: it
fails while anyone holds it, and if it succeeds every holder is gone.

A client finds its project's entry by the digest of the canonical path and
reads the token from `<boot_id>.token` in the runtime directory, and judges it
by the lock first. With nobody holding the lock the project is free, and an
entry that remains is stale (removed after the same owner and link check the
app makes). With the lock held an entry is never stale: if it names a live app
whose boot id has the right form, whose `socket` is the endpoint that boot id
names, whose process runs and whose endpoint answers, the client attaches;
if it is a `headless` entry (no socket) it is a headless run; anything else
(no socket, no entry yet, an entry that does not check out) is a project open
where there is nothing to attach to. A client never connects to anything an
entry names: it derives the endpoint from the boot id. The registry write
makes the runtime folder first (with agent connections off no listener does).
Registry writes run on worker threads and can reach the lock out of order, so
each carries a number and one older than the last applied is dropped.

Every reader opens an entry without following a link and without blocking (a
FIFO put in its place is not opened for reading), and checks the descriptor it
opened: a plain file of this user, no larger than 64 KiB. The GUI also checks
the runtime folder first (a real folder of this user that nobody else can
enter); one that is not holds nothing. An entry whose `format` is higher than
the reader knows is not the reader's: it is neither live nor stale, and is
never removed or replaced (`signex-cli` stops with a message; the app leaves it
and logs).

Two instances with the same project: an app removes only entries that carry
its own `pid` and `boot_id`, and writes over an entry only if there is none,
it is its own (same `pid` or same `boot_id`) or the entry is stale: its process
is gone, or its socket refuses a connection (a reused process id, or a
listener that has gone). An entry file is data another process wrote, so
only the endpoint its boot id gives is probed: the boot id must be 16
lowercase hex characters and `socket` must equal the endpoint that boot id
names in the runtime folder, or the entry is stale and nothing it names is
touched. The probe does not block, connects and closes at once without
sending a byte (the descriptor is close-on-exec), so the other instance logs
nothing. Where the connect would block, Linux answers `EAGAIN`, which counts
as there. macOS answers `ECONNREFUSED` for a full backlog, so there a stalled
peer whose backlog has filled may be taken for a dead one; the only effect is
that the registry then points at this instance. On Windows the pipe is opened
for identification only (`SECURITY_IDENTIFICATION`), so a server that squats a
stale entry's name cannot impersonate the user probing it. The entry of another
live instance is kept and Messages says the project is also open there;
closing the project in one instance never unregisters the other. An entry
that is already ours and unchanged is not rewritten. The registry is written
and removed under one lock, so those never interleave. A write that runs after
its listener's cleanup can still land, so the quit path cleans every boot id
this run reported (the last 32), not only the current one.

A write that fails, or leaves a project to another instance, is tried again:
while a listener is up and a project is unregistered, a timer asks for
another try every 5 s (no timer otherwise, and no write at every update).
Messages says so once per run of failures; the result of a write of a
listener that is gone is ignored, and a tick waits while a write is still
running. So when the other instance quits, this one registers the project
within seconds.

A listener that fails (it could not bind, or gave up accepting) is restarted
after 2, 4, 8, 16 and 32 s with a new boot id and token, which gives iced a
new subscription; after those five restarts it stays off and Messages says to
turn the setting off and on in Preferences. Only a listener that has served a
connection earns the count back (a listener that binds and then fails again
does not), and turning the setting on resets it. The boot ids of listeners
that have ended stay in the quit path's list, which keeps the last 32.

## Client

`signex-cli run` (PR B of #1924, behaviour in [CLI.md](CLI.md)) uses
`client.rs`:

- `lookup` reads the entry of a project (the directory must pass the same
  owner and mode check as the app's) and answers `Free`, `Stale`, `Gui` or
  `Headless`. `StaleEntry::remove` re-reads the entry and removes it only if it
  is unchanged and the user's own plain file.
- `GuiPeer::connect` reads the token file, which must be a plain file of this
  user with no group or other permission, opened without following a link
  (`O_NOFOLLOW`); it checks the endpoint is a socket of this user and connects
  to the endpoint the boot id names. On Windows the pipe is opened for
  identification only.
- `Connection::ask` sends one request line and reads one reply line, in the
  wire format above. A reply that is not the documented shape is an error.
  Replies are read by a thread of their own and awaited for at most 120 s on
  every platform; a connect is bounded to 5 s (a full backlog blocks it).
  Replies are matched to requests by order: after any failure, a timeout above
  all, the connection is broken for good (a late reply must never be taken for
  the next answer) and every later `ask` fails; a reply that names another
  command than the one asked is a failure too.
- `hold` takes the project's lock first (a few attempts, 5 x 20 ms: a reader's
  momentary lock must not fail a starting run; and the path must still name the
  locked file), then writes the `headless` entry (one format, the app's file
  writer). It fails if another run holds the lock or a window has the project
  open (its shared lock; its entry, when one is written, names it); a dead
  run's entry is replaced. Dropping the `Hold`
  removes the entry (only if it is still the run's own). The lock file is never
  deleted: an empty 0600 file is harmless, and deleting a locked file would let
  a second run lock the unlinked one.

## The app refuses a held project

When the window opens a project (any way: Open, a recent file, the command
line) it takes the shared lock at once, and if a headless run holds it
exclusively the window refuses: a line in Messages and an Open Failed notice
(`<file> is being edited by a headless signex-cli run (process N), so it was
not opened`), and nothing in the app changes. This needs no setting: the lock
is taken whether or not agent connections are on. A project another window has
open opens as before (the shared locks coexist; the later entry write logs
that it is also open there). The lock is taken in the update that opens the
project, so a run cannot slip in between the open and the registry write. A
headless app (the CLI's own) and an app made for a test hold no lock and never
refuse: the run holds the project (a second run is refused before it starts,
by `signex-cli`).

Recovery: the notice names the entry file. A hold is live only while its run
holds the lock, so an entry that stays after its run was killed is stale and is
ignored; if the notice appears with no run alive (it should not), delete the
entry file it names.

## Known limits

- The lock keeps a window and a run apart (one or the other has it), but the
  entry is a separate write: two windows with one project both want to write
  its entry, the first keeps it and the other logs and retries every 5 s, so
  after the first closes the project there can be no entry for a few seconds;
  the lock still says it is open. A stale entry is removed after a re-read that
  shows it unchanged; an app that rewrites it between that read and the removal
  can lose its entry, which the app's retry timer restores.
- The lock is a file lock (`flock`, `LockFileEx`): on a runtime folder on a
  network file system whose locks are not honoured, a hold is not seen.
- A sheet opened loose, without its project, is not listed and not refused.
- On Windows the client has not been run against a real app here (it is
  compiled and linted for `x86_64-pc-windows-msvc`).
- The Windows code (`transport/windows.rs`) is compiled and linted for
  `x86_64-pc-windows-msvc` on its own, and has transport-level tests that run
  on the Windows CI job (a round trip, a hundred clients that connect and close
  at once, a second pipe of the same name refused). It was not run locally, and
  no test drives the whole app through the pipe.
- A crash leaves its token and socket behind (0600, a dead token). The next
  start of that boot id never happens, so they stay until a client or a later
  version sweeps the folder.
- A query on a project sheet that is not loaded reads it on the update thread,
  as the headless run does (#665 tracks synchronous reads).
- `save_document` answers `ran` when the save has started; its result reaches
  Messages when the write finishes. It writes that sheet's unsaved work,
  including edits the person made; the `--save` gate is the CLI's (PR B).
