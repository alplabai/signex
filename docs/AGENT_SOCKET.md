# Agent socket (#1924)

A person keeps Signex open; an agent works on the same project through the CLI.
The agent is a component of Signex, not a user of its window: it names its
document, uses the engine channel and never changes the person's active tab,
focus, panels, selection or tool. This page is the design of the running app's
side (PR A of #1924): the listener, its security and the registry. The CLI side
(connect when a live entry exists, otherwise run headless; a GUI refusing a
project a headless run holds) is PR B.

Code: `crates/signex-app/src/app/agent/`.

## Off by default

Preferences > Editing > "Allow agent connections" (`agent_connections_enabled`
in `prefs.json`, default `false`). While it is off no socket, token or registry
file exists, and the app behaves as before. Turning it on at Save starts the
listener; turning it off at Save ends it: the socket is closed, the connections
are dropped and every file below is removed. A headless run never listens,
whatever its config root says.

## Transport

The listener is an iced `Subscription` (`listener.rs`). One future owns the
socket, every connection and the files, so ending the subscription ends all of
them. A connection only frames lines; each request line becomes
`Message::Agent(AgentMsg::Request { .. })` and runs inside `Signex::update`,
like a click. That is one sequential queue: a person's edit and an agent's
request never interleave inside an edit. The reply goes back to the connection
when `update` has run the request. No window is created and nothing in
`iced::window` is called.

- macOS and Linux: a Unix socket `<runtime dir>/<boot id>.sock`, mode 0600.
- Windows: the named pipe `\\.\pipe\signex-agent-<boot id>`.
- No TCP port, no network.
- At most 8 connections at once; one that sits idle for 120 s is closed.

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
  `read_document`;
- a command that names its document with the `document` operand and takes no
  file path, except the three held back below: today `apply_edits`, `revert`,
  `color_net` and `clear_all_net_colors`. A test pins the final set, so a
  catalog row that joins or leaves it is a visible decision.

Held back (`NOT_YET_FOR_AGENTS`, each with the reason the agent is told). They
pass the derivation and are refused until the maintainer opens them:

- `save_document`: "saving the person's file is not available to agents yet".
  It could write the person's half-done unsaved work on that sheet. Candidate to
  open later behind PR B's `--save` gate.
- `close_active_document`: it changes the person's tab strip.
- `assign_net_class`: project-wide data, which #2178 also keeps out of
  `apply_edits`.

**Maintainer decision pending:** these three are the candidates to open later.

Refused with a reason: `undo` and `redo` (`PERSON_ONLY`, the one hand-kept
constant, checked against the catalog by a test; the agent takes back its own
step with `revert`), a command with a file path operand (`save_document_as`,
`open_document`, `new_document`), a window-only command (`headless_refusal`,
#2170), and every tool, selection or view command, which names no document.

`open_document` and `new_document` are refused on this channel: they take a
path, and opening a tab changes what the person sees. A document that is not
part of a project open in this app (a loose sheet) is refused too.

## Security

1. **No network.** A Unix socket in a per-user directory (0700, socket 0600),
   or a Windows pipe whose security descriptor allows only the current user's
   SID (`D:P(A;;GA;;;<SID>)`) and which rejects remote clients.
2. **Peer check.** The client's user id comes from the kernel (`getpeereid` on
   macOS, `SO_PEERCRED` on Linux, through tokio's `peer_cred`) and must be the
   user running the app: `transport::unix::peer_allowed`, one decision, tested
   alone. On Windows the client's process is read from the pipe and its token's
   user must equal ours (`EqualSid`); a process that cannot be opened is
   refused. A refused peer is logged (`PeerRefused`) and nothing is read.
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

The runtime directory is `$XDG_RUNTIME_DIR/signex-agent`, else
`<temp dir>/signex-agent-<uid>` (macOS: the per-user temp dir), else on Windows
`%LOCALAPPDATA%\Signex\agent`.

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

One entry per open project, written when the listener is up and a project is
open, rewritten when the set of open projects changes, removed when its
project closes, when the setting goes off and when the app exits. It is
written for the canonical path of the project file; the file name is the first
32 hex characters of the SHA-256 of that path, so two spellings of one path
share an entry and the name holds no path text.

```json
{
  "format": 1,
  "project": "/canonical/path/Demo.snxprj",
  "pid": 4242,
  "socket": "/run/user/1000/signex-agent/3f9c0d1e5a7b2c44.sock",
  "boot_id": "3f9c0d1e5a7b2c44"
}
```

A client finds its project's entry by the digest of the canonical path, opens
`socket` and reads the token from `<boot_id>.token` in the same folder as the
socket (the pipe's token is in the runtime directory). An entry whose `pid` is
dead or whose socket refuses is stale: the client removes it, after the same
owner and link check the app makes. An app removes only entries that carry its
own `pid` and `boot_id`; a later run that rewrites an entry takes it over.

## Known limits

- The client (PR B) is not here: nothing yet connects, and nothing checks that
  a headless run holds a project.
- The Windows code (`transport/windows.rs`) is compiled and linted for
  `x86_64-pc-windows-msvc` on its own but has not been run.
- A crash leaves its token and socket behind (0600, a dead token). The next
  start of that boot id never happens, so they stay until a client or a later
  version sweeps the folder.
- A query on a project sheet that is not loaded reads it on the update thread,
  as the headless run does (#665 tracks synchronous reads).
- An agent cannot save, close a tab or assign a net class yet (see "Held
  back"); a person saves.
