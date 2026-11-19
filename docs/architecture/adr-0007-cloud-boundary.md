# ADR-0007 — The application core carries no cloud vocabulary; what a hosted service may rely on

- **Status:** Proposed
- **Scope:** what the shared code, and the application core of ADR-0006 in particular, may and may not contain so that a hosted service built outside this repository can use it:
  - no account, plan, licence or AI type in the core
  - git as the only remote-storage path
  - a hosted agent entering through the agent path
  - a read-only browser viewer built from the DOMAIN crates
  - what the core must keep possible for real-time co-editing later
- **Builds on:** [ADR-0006](adr-0006-application-core.md) (one application core, driving and driven ports; accepted, #2327), [ADR-0003](adr-0003-deferred-work-and-consistent-history.md) (owed work, late results, undo snapshots), [ADR-0001](adr-0001-app-structure-and-boundaries.md) A3.2 (tool ports) and D8 (the agent channel)
- **Amends:** [ADR-0006](adr-0006-application-core.md): the `wasm32` gate bullet under "Enforcement", the domain-crate lint timing under "Enforcement" and slice S-IO, and "A web front end (epic D10)". See "Changes to ADR-0006". The amended places in ADR-0006 carry a "proposed" pointer here until this record is accepted.
- **Relates to:** #1715 (`signex-vcs`), #1707 (libraries from a git URL), #2245 (Agent mode is built outside this repository), #2429 (object ids minted by the caller), #2431 (no disk, clock or randomness in DOMAIN crates), #2456 (`Origin` in `app/core/`)
- **Tracking:** epic #2326
- **Code change:** none. This record only.

## Context

Signex Community (FSL-1.1-ALv2) is offline forever: nothing is locked and there are no upgrade prompts (`docs/ROADMAP.md` §3). Signex Pro is a separate, private product built on Community (`docs/ROADMAP.md` §3, "Signex Pro"). Its plans and its service design are kept in the private Pro repository and are not part of this record.

`docs/REPOSITORY_AND_CODEBASE.md` §3.3 already states the hard rule: "Hosted services, paid infrastructure and account-backed multi-user state are not part of this repository, and the editor engine must not depend on them."

A hosted service will still build on the code this repository ships: the command vocabulary, the agent path, the version-control capability and the DOMAIN crates. Without a recorded boundary, each feature such a service needs could add a port, a type or a check to the code every user runs.

ADR-0006 already says where outside concerns belong:
- **Driven side:** "The core defines the port types: `Effect`, `EffectResult`, and the storage, version-control, network and clock capabilities they name. A *host* implements them" (Driven ports).
- **Front-end state:** "Preferences, file dialogs and fonts. The core never reads `prefs.json`; it receives `CoreSettings` as values (epic D7)."
- **Agents:** "In a running window the agent socket does not own or call the core. Its requests enter the GUI loop through the ADR-0001 D8 channel → `Subscription` bridge. The GUI calls `execute` with an agent origin" (Events are a return value, not a bus).
- **Web front end:** "not now; native first; a browser host only if a web front end is scheduled" (epic D10).

This record applies those clauses to a hosted service.

## Decisions

### D1. The core names no account, plan, licence or AI type

- Sign-in state, organisations, seats, plans, entitlements and AI credits are not core state, not `Effect`s and not `CoreSettings`.
- A front end built outside this repository holds them, and passes the core only values the core already understands. This is ADR-0006 epic D7 applied to a new kind of setting.

### D2. This repository holds no cloud port, no licence check and no upgrade prompt

- No cloud-specific port, trait or `Effect` variant is defined here.
- There is no entitlement or licence check, and no code path that changes behaviour by plan.
- What this repository may contain is generic and useful without an account:
  - git remotes over HTTPS;
  - a credential source in the OS keychain (the `keyring` setup of #976);
  - the domain renderers.
- An entitlement can only be enforced by the service that grants it. The client's source is public and can be changed, so a check in the client would protect nothing.

### D3. Remote storage reaches the desktop as git

- A project or library stored by a hosted service is a git repository on a remote. The desktop reaches it through the version-control capability of ADR-0006 (`signex-vcs`, #1715) and the git-URL work of #1707.
- The remote is just a remote: the same code pushes to any git host. A host adds only an access token.
- No second versioning model is introduced. Version history is the git history.

### D4. A hosted agent is a driving adapter, with no privileges a person lacks

- A hosted agent's tool calls enter the GUI loop exactly as the agent socket's do: `execute` and `query` with an agent origin, through the ADR-0001 D8 channel.
- The core gains no AI type. The allowlist of commands an agent may run stays policy, as in ADR-0006.
- **Provenance.** Every command carries its origin: a person, an agent or, once the plugin host exists, an extension. The origin also names the acting user, the person on whose behalf the command runs. Agent work is attributed to that user:
  - a commit made from agent changes uses the user's git identity, never a bot identity;
  - the origin records that an agent made the change.

  Today `Origin` (`app/core/origin.rs`, #2456) is `Person` or `Agent { pid, client }`. The acting user and the extension origin are added by the slice that first needs them, as ADR-0006 adds every `CoreContext` field ("The contract grows with the slices"). The acting user is a plain value, such as the git author, not an account type (D1).
- **Leases.** Operations that need exclusive access to a region of a board (routing, zone edits, push-and-shove, zone fill) may later be guarded by advisory leases. An agent acquires a lease the same way a person does, with no bypass, and that includes a hosted agent's tool calls.

### D5. The browser preview is a read-only WASM viewer built from the DOMAIN crates

- **Viewer:** a read-only viewer compiled to `wasm32-unknown-unknown` from the DOMAIN crates:
  - file parsing and `signex-types`;
  - the render-to-primitives path of `signex-output`.

  It draws a project in the browser: pan, zoom, layer and sheet switching, net highlight, search, cross-probe between schematic and PCB.
- **Why in the browser:** rendering cost moves to the viewer's machine, and interaction (net highlight, layer toggles) needs no round trip.
- **PCB:** the viewer needs a GPU-free PCB render-to-primitives path in a domain crate. The interactive canvas draws through `signex-gfx` (wgpu). GPU-free board drawing already exists in `signex-output`: the board print in `pdf/board/` (#2402) and `fab/draw.rs`. The viewer's path is split out of that code, not built from scratch.
- **Delivery:** the viewer is built in this repository's CI as a versioned artefact. Its source stays here, in the DOMAIN crates. A hosted web app may embed it.
- **This narrowly reopens ADR-0006 epic D10**, for a read-only viewer only. There is no editing web front end and no browser host for `AppCore`.
  - The viewer uses the DOMAIN crates directly, without `AppCore`.
  - A `wasm32` check gate covers the `DOMAIN` set in `scripts/check_arch_guard.sh`, so a crate that joins the set joins the gate. The `check` job already builds `signex-mcad` for `wasm32-unknown-unknown`. A spike found the other DOMAIN crates the viewer needs compile too (open question 2).
  - **#2431** keeps them that way: no disk, clock or randomness in DOMAIN crates, enforced by `disallowed-methods` and the `wasm32` check gate.
- **Storage access from the browser is coarse-grained.** The viewer asks for "the project at commit X" in one request, as a pack or archive of that commit, not as per-file reads.
- **The same rule binds a future browser host.** If a browser host for `AppCore` is ever scheduled, its storage effect is that same project fetch. ADR-0006's `Effect` / `EffectResult` + `complete` model is already asynchronous, so such a host would fit without core changes. This record schedules no such host.

### D6. Real-time co-editing is not designed here; the core keeps it possible

How co-editing works, and who builds it, is outside this repository. These points are recorded because they are cheap to get right during the ADR-0006 slices and expensive to retrofit. They hold for any co-editing design:

1. **Object ids are minted by the caller.** Every new object's id is chosen by the front end and carried as an operand, so replaying a command gives the same ids. Ids are persisted in every file row and survive save, undo, paste and a wire split (one half keeps the original id).
   - **#2429:** new ids come from the caller through an `IdSource`, and the engine never mints one.
   - **Partly implemented:** #2429 part 1 merged in #2442 (`77a809cbe`). Part 2 is open.
   - The #2429 audit found 27 production sites that mint ids today, 8 of them inside `execute`.
2. **The command vocabulary is deterministic and versioned.** This is `signex-commands` (ADR-0006 epic D11, "Its types are versioned"), with targets named in the arguments (epic D1). The same vocabulary serves the CLI, an agent and co-editing.
3. **`CoreOutput` carries per-object deltas** (before and after, by object id), next to the `DocumentPatch` redraw flags. Those flags say which collections changed, not which objects.
4. **A remote origin can be added.** When co-editing is built, `Origin` gains a variant for a command that came from another session, beside the agent origin. It is not added now.
   - **Why it is not a cloud port (D2):** it is a value that names where a command came from, as the agent origin does. It names no service, transport or account, carries only opaque peer and session ids, and any relay can fill it in, including a test or a LAN one. The core still never calls out.
5. **Leases and provenance** as D4 states them hold for every peer, person or agent.
6. **Undo becomes origin-scoped inverse deltas** instead of whole-document snapshots. Restoring a snapshot in a shared document would undo other people's work. This changes ADR-0003 D3, so **it is a separate ADR**; this record does not amend ADR-0003.

## Changes to ADR-0006

This record changes three items of ADR-0006. The PR that lands it adds a "proposed" pointer to this record:
- under each of the three items;
- on the S-IO row of the slice table;
- on the two diagram labels that called S-IO "not scheduled" (the DOMAIN subgraph label and the S-IO slice-graph node).

The PR also adds an "Amended" header line to ADR-0006. If this record is not accepted, the pointers and the header line are removed.

1. **The `wasm32` gate.**
   - ADR-0006, "Enforcement", says: "**No `wasm32-unknown-unknown` gate now** (epic D10: native first)."
   - **New rule:** a `wasm32` check gate covers the `DOMAIN` set in `scripts/check_arch_guard.sh` (D5, #2431). It does not cover `AppCore`, `signex-workspace` or `signex-app`.
   - `docs/ROADMAP.md`, Application Core, "Not in this plan", is amended to match.
2. **When the I/O lint reaches the domain crates.**
   - ADR-0006, "Enforcement", says: "The lint reaches the domain crates only after slice S-IO, because `signex-types` and `signex-engine` still call `std::fs` today."
   - Its slice table says S-IO is "not scheduled: S-IO waits on a future web front end being scheduled".
   - **New rule:** #2431 schedules the DOMAIN-crate part now, as a `disallowed-methods` gate covering:
     - `std::fs`
     - `Path` probes that touch the disk
     - `Instant` and `SystemTime`
     - `uuid` minting outside the #2429 `IdSource` allowlist
   - The rest of S-IO stays as ADR-0006 orders it.
3. **Epic D10.**
   - ADR-0006, "A web front end (epic D10)", says: "not now; native first; a browser host only if a web front end is scheduled … there is no `wasm32` build gate now, and S-IO is not scheduled."
   - **New rule:** D10 is reopened narrowly, for a read-only viewer only (D5). No editing web front end and no `AppCore` browser host are scheduled.

## Open questions (not decided here)

1. **Remote sync as core effects.** Whether push and pull become core `Effect`s of the version-control capability, or stay host-side and front-end commands. This is settled with #1715.
2. **WASM viewer: where project resolution lives.**
   - A spike on 2026-11-16 (Rust 1.97.0) found that the DOMAIN crates the viewer needs already compile for `wasm32-unknown-unknown`. Only `uuid` needs its `js` feature on wasm.
   - A schematic and four boards loaded from bytes in Node, and the bundle was about 630 KB gzip.
   - Open: multi-sheet hierarchy resolution, the project netlist and the export context are built inside `signex-app` from file paths. They must move to a domain crate that works on in-memory files. Where that crate sits is the open decision; it overlaps the ADR-0006 slices that move project loading into the core.
   - Also open: the rendering surface (schematic as Canvas 2D or SVG; PCB through wgpu on WebGPU with a WebGL2 fallback).

## Alternatives considered

| Alternative | What it offers | Why not |
|---|---|---|
| (a) Cloud ports inside `AppCore` (`IdentityPort`, `EntitlementPort`, `AiAgentPort` as core-defined capabilities) | One place for every outside concern | It puts account and AI vocabulary into the shared core, against D2 and the decision that this repository holds no AI code (#2245). The core would have to know about plans, which ADR-0006 epic D7 keeps out. |
| (b) A custom sync API (content-addressed blobs + a version table) instead of git | Fine-grained control over files, previews and permissions | It adds a second versioning model next to `signex-vcs`. It needs its own export tool. Community users could not use any git host. |
| (c) Server-rendered preview only (SVG/PDF per view, thin JS viewer) | No `wasm32` gate; works before the DOMAIN crates are wasm-clean | Render cost grows with viewers, and every interaction (layer toggle, net highlight) is a round trip or a pre-rendered variant. |

## Consequences

- **The licence boundary is the process boundary.**
  - This repository stays FSL-1.1-ALv2 and complete offline, and it never links code from a hosted service.
  - Community users can use git remotes on any host with a token.
- `AppCore` stays free of account, plan and AI types. The ADR-0006 guard needs no new rule for this record.
- A modified client gains nothing, because only the service that grants an entitlement can enforce it.
- Agents, hosted or local, reuse the command vocabulary and the agent path, with the same leases and the same attribution as a person. Neither adds an event bus (ADR-0006 epic D12).
- **What the shared code must carry:**
  - the D6 points 1–4, added by the ADR-0006 slices that first need them;
  - a follow-up ADR for origin-scoped undo;
  - a GPU-free PCB render-to-primitives path, split out of `signex-output`, before a PCB preview;
  - a `wasm32` check gate on the `DOMAIN` set (D5).
- Per-object deltas (D6.3) add detail to `CoreOutput`. The event-granularity benchmark that ADR-0006 requires before slice 2 must cover them.

## References

- ADR-0006: sections "Driving ports", "Driven ports", "Events are a return value, not a bus", "How DDD (ADR-0001 A2) maps onto it" (the shared command crate), "A web front end (epic D10)"
- ADR-0003 D1–D5; ADR-0001 A3.2 and D8
- `docs/ROADMAP.md` §3 (Community, "Signex Pro"); `docs/REPOSITORY_AND_CODEBASE.md` §3.3
- `docs/DECISIONS.md`, 2026-11-09 (#2245)
- #1715, #1707, #976, #2326, #2402, #2429 (part 1: #2442), #2431, #2456
