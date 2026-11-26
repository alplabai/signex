# ADR-0006 — One application core, many front ends (ports and adapters)

- **Status:** Accepted 2026-11-16 (#2327, merged by Hakan; Caner's review decisions of 2026-11-15 applied). Amended 2026-11-17 (#2443, Hakan): the contract grows with the slices, slice C is the safety net, slice 1b moves the catalog.
- **Decides on merge:** epic #2326 decisions D0 (this record and the ADR-0001 notes), D1, D2, D3, D5, D7 and D11. Already decided: D10 (web, [maintainer decision 2026-11-15](https://github.com/alplabai/signex/issues/2326#issuecomment-6884693348)) and D12 (no event bus, 2026-11-13). The i18n placement below is Caner's decision of 2026-11-15 (`docs/DECISIONS.md`).
- **Amended (proposed):** [ADR-0007](adr-0007-cloud-boundary.md), not yet accepted, would amend the `wasm32` gate and domain-crate lint bullets under "Enforcement", slice S-IO and "A web front end (epic D10)". Each place carries a "proposed" pointer; they are removed if ADR-0007 is not accepted.
- **Scope:** an iced-free application core shared by the GUI, `signex-cli` and later front ends; the crates `signex-commands`, `signex-i18n-ids` and `signex-workspace` (`signex-workspace` planned)
- **Amends:** [ADR-0001](adr-0001-app-structure-and-boundaries.md) A1, A2, A3.2, D1, D5, D8 and three Part C bullets, each of which carries a short note pointing here
- **Builds on:** [ADR-0003](adr-0003-deferred-work-and-consistent-history.md) (owed work, the boundary flush, late results)
- **Tracking:** epic #2326, milestone Application Core (`docs/ROADMAP.md`); the slices ship across releases. Sources and the full measurement commands: [the research note](research/hexagonal-ddd.md)

## Context

The GUI is not the only way into signex: `signex-cli` runs catalog commands for scripts and agents, and it must have the same effect as the GUI for the same command. Today that parity is bought by booting the whole GUI model without a window (`boot_headless`). Attach mode is not part of the motivation: its requests already enter the GUI's `update` loop, and this record keeps that.

```mermaid
flowchart LR
  subgraph app["signex-app (iced)"]
    model["Signex model<br/>UiState · DocumentState · InteractionState"]
    update["Signex::update(Message)"]
    bridge["app/command/bridge.rs<br/>id → Message"]
    headless["headless.rs<br/>boot_headless · dispatch_command_headless<br/>gui_only refusal"]
    driver["task_driver.rs: TaskDriver<br/>runs iced Tasks"]
  end
  gui["iced window"] --> update
  update <--> model
  bridge --> update
  cli["signex-cli"] -->|"boots the whole model"| headless
  headless --> model
  headless --> bridge
  cli --> driver
  cli -.->|"attach.rs imports headless::agent_client"| app
  model --> domain["Domain crates<br/>engine · erc · drc · net · library · output"]
```

### The problem, measured

macOS arm64. Each row names the trunk commit it was measured at. The full commands are in the research note, §12.

| Claim | Numbers | Command |
|---|---|---|
| The CLI builds the GUI | 518 distinct crates for `signex-cli`, 115 for the domain crates it needs (`1bf5750dd`) | `cargo tree -p signex-cli -e normal --prefix none \| awk 'NF{print $1" "$2}' \| sort -u \| wc -l` |
| | GUI/GPU crates are about 26% of the summed compile-unit time of a cold release build, about 32% with `signex-app`; the binary is 47 MB (`1bf5750dd`) | `cargo build --release -p signex-cli --timings` |
| The CLI boots the GUI | 1.29 s for a run with no commands (median of 5 warm runs), 0.026 s for `--help`; the split inside the boot is not measured (`1bf5750dd`) | `time signex-cli run <project>.snxprj` |
| | The run log shows `dropped an iced action a headless run does not perform kind="window"` three times | the same run |
| Parity costs every new command | 34 ids have an operand handler; 20 of them also have a GUI arm in `bridge.rs` (`f3d4efc75`) | research note §12 |
| | 250 commands in `tests/golden/commands.json`; `gui_only` on 184 lines in 6 files of `signex-app`, 166 of them catalog golden rows (`f3d4efc75`) | research note §12 |
| IO runs on the update path | 156 production `std::fs::` call sites under `crates/signex-app/src/app` (`1bf5750dd`) | research note §12 |
| | In a hand-read sample of 15, about half run synchronously in a `&mut self` handler: `canonicalize` in `handlers/document_identity.rs`, `load_sheet` in the Sync Sheet Pins menu action, the agent registry and journal. A sample estimate. | — |

**What a new command costs today.** A command with a headless form is handled twice: a GUI arm in `app/command/bridge.rs` (id → `Message`) and an operand handler in `app/command/operand_handlers.rs`. It carries a `gui_only` flag. Headless runs are fenced by it through `headless_refusal`, which the agent allowlist `allow::refusal` also reads. A required-operand check (`missing_headless_operand`) and, in the CLI, a status correction for dropped windows and refused dialogs (`honest_status`) follow. The command also gets golden rows in `catalog.golden.tsv`, `tests/golden/commands.json` and, when bound, the three `builtin_profiles*.golden.tsv`.

Under this record a command is one catalog row (the golden rows stay) and one core handler, or one GUI-only table entry. `gui_only` goes in slice 6 and `honest_status` in slice 9; the agent allowlist stays as policy. Headless-parity work so far, some of which this record reuses (#1934, #2172): #1926, #1927, #1929, #1931, #1932, #1934, #2170, #2172, #2243, and #2155 in part.

### The cost of waiting

The surface this record has to move grows with every feature. Counted in `crates/signex-app/src` on the first-parent history of trunk:

| Trunk on | `ui_state.` uses | `document_state.` uses | `spawn_blocking` / `run_blocking` sites | `.rs` files |
|---|---|---|---|---|
| 2026-10-23 (`f901a48ab`) | 766 | 1022 | 34 | 496 |
| 2026-11-04 (`8985a1c02`) | 2085 | 4029 | 148 | 1102 |
| 2026-11-15 (`fee721ea3`) | 2893 | 5507 | 179 | 1566 |
| 2026-11-15 (`f3d4efc75`) | 3067 | 5627 | 183 | 1671 |

Commands, for a commit `<c>`:

```sh
git grep -h -o 'ui_state\.' <c> -- crates/signex-app/src | wc -l
git grep -h -o 'document_state\.' <c> -- crates/signex-app/src | wc -l
git grep -h -o -E 'spawn_blocking|run_blocking' <c> -- crates/signex-app/src | wc -l
git ls-tree -r --name-only <c> -- crates/signex-app/src | rg -c '\.rs$'
git rev-list --count --first-parent f901a48ab..fee721ea3                              # 749
git rev-list --count --first-parent f901a48ab..fee721ea3 -- crates/signex-app/src     # 596
```

From `f901a48ab` to `fee721ea3` (23 days, 749 first-parent commits, 596 of them touching `crates/signex-app/src`):
- `ui_state.` uses grew 3.8×, `document_state.` uses 5.4×, 4.7× combined.
- Background-job sites grew 5.3×; each becomes an `Effect` plus a `complete`.
- `.rs` files grew from 496 to 1566.

Not every access changes when its state moves, but each one is a site a slice has to read and review. Slices 0 and 1 cost the same now or later; slices 2, 3 and 5 get dearer every week.

**#2325 is a symptom, not the reason.** The Windows stack overflow sits on the CLI path, but boxing `Signex` or running the session on a thread with a bigger stack fixes it. It does not by itself justify this record.

## Decision

One iced-free application core, `AppCore`, is the application layer over the engines. Every front end calls it through the same ports.

```mermaid
flowchart TB
  subgraph domain["UI-free domain crates: no iced; native IO moves out (S-IO, not scheduled · proposed by ADR-0007: DOMAIN-crate part by issue 2431)"]
    types["signex-types"] --- engine["signex-engine"]
    engine --- erc["signex-erc / signex-erc-dsl"]
    engine --- drc["signex-drc / signex-drc-dsl"]
    engine --- net["signex-net"]
    output["signex-output / signex-bom"] --- library["signex-library"]
    history["signex-history"] --- ids["signex-i18n-ids"]
  end
  commands["signex-commands<br/>catalog · operands · outcome · wire types"]
  core["AppCore (planned)<br/>app/core/ now, signex-workspace after slice 9<br/>execute · complete · query → events + effects"]
  domain --> core
  commands --> core
  subgraph hosts["Driven: hosts, outside the core"]
    native["Native host<br/>files · git · network · clock"]
    testhost["Test host<br/>in memory"]
  end
  core -.->|"Effect"| native
  core -.->|"Effect"| testhost
  subgraph adapters["Driving adapters"]
    gui["GUI adapter (signex-app, iced)<br/>translator · projector · GUI-only table"]
    cli["CLI adapter (signex-cli)<br/>Box of AppCore · inline host"]
    sock["Agent socket<br/>enters through the GUI loop"]
  end
  gui --> core
  cli --> core
  sock --> gui
  gui --> ui["signex-i18n (renders) · signex-widgets · signex-renderer · signex-gfx"]
```

Names marked *planned* do not exist on trunk yet.

```mermaid
flowchart LR
  subgraph driving["Driving adapters (outside)"]
    gui["GUI (iced)"]
    cli["signex-cli"]
    sock["Agent socket<br/>(enters through the GUI loop)"]
    web["Web / plugin host (later)"]
  end
  subgraph hex["Inside the hexagon"]
    core["AppCore — application layer<br/>thin handlers · CoreOutput { events, effects }"]
    subgraph domain["Domain"]
      docs["Documents (aggregates)<br/>engines: Command → DocumentPatch"]
      svc["Domain crates<br/>net · erc · drc · sketch · bake · bom · output"]
    end
    core --> docs
    core --> svc
  end
  subgraph driven["Driven adapters (outside): hosts"]
    native["Native host<br/>files · git · network · clock"]
    browser["Browser host (later)<br/>browser storage / remote"]
    test["Test host<br/>in memory"]
  end
  gui -->|"execute / complete / query"| core
  cli -->|"execute / complete / query"| core
  sock -.->|"via GUI"| gui
  web -.-> core
  core -->|"Effect"| native
  core -.->|"Effect"| browser
  core -.->|"Effect"| test
  native -->|"complete(EffectResult)"| core
```

Arrows into the core are calls. The core returns events and effects to its caller; it publishes nothing. A host is chosen per platform. The core depends on no host and names only the `Effect` / `EffectResult` types it defines.

### Inside the hexagon

- The domain crates of ADR-0001 A2.
- `AppCore`, the application core. It is iced-free; it grows in `signex-app/src/app/core/` and is lifted into its own crate, `signex-workspace`, in slice 9 (epic D2). Until then the guard under "Enforcement" keeps it apart.

`AppCore` names no `iced` type, window id, dialog, canvas, selection or clipboard. It holds no file handle, thread or runtime.

### Driving ports

- `execute(command id, args, origin) -> (outcome, CoreOutput)`;
- `complete(effect id, result) -> CoreOutput`, for finished effects;
- `query` for read-only answers, with a typed error.

Every core command names its document and its targets in its arguments (epic D1). A front end fills in what a person leaves implicit before it calls `execute`: the active document, the selection, the pre-placement form. A front end that must protect a person's in-progress drag or text edit passes that as plain data in the arguments, never as a callback into the GUI.

Commands that create objects carry the new ids as operands. Ids are minted outside the core by the caller through an `IdSource`, like the clock. The rule is that the engine mints no ids, so a command replays to the same result; the core refuses a command whose id already exists. The tracked exception is eight engine sites, in five areas, that still mint their own ids: the rubber band, junction autoplace, the component pin fan-out, automatic sheet entries and the copy fragment. They are on a shrink-only allowlist, `scripts/engine-ids-allowlist.txt`, checked by `scripts/check_engine_ids.sh` and tracked by #2429. Each entry also carries how many production places name that helper: a caller mints through it, so a new caller fails the gate as a new site would, and the count may only fall. Ids the engine itself derives during a command (automatic pin junction dots) come from the host's `IdSource`, set with `Engine::set_id_source`.

`execute` routes by command id through a thin table, like the footprint editor's `crates/signex-app/src/library/editor/footprint/updates/mod.rs`. The handling lives in area modules (documents, placement, hierarchy, files, agent edits), never in the router.

A core command never opens a dialog (epic D3): "Open…" gathers the path in the GUI, then calls `open_document path=`.

### Driven ports

IO is a set of pluggable driven adapters, the mirror of the driving side (epic D5).
- **Who defines what.** The core defines the port types: `Effect`, `EffectResult`, and the storage, version-control, network and clock capabilities they name. A *host* implements them, owns the adapters and runs `perform(Effect) -> EffectResult`.
- **Where the host lives.** The host is outside the core and depends on it, never the reverse. `perform` takes no `&AppCore`.
- **Swappable per platform.** Native (files, git) and test (in memory) now; a browser host (later) only if a web front end is ever scheduled. Swapping a host changes where documents are stored without touching the core or the front end.
- **How hosts are run.** The GUI runs the host off the UI thread as a `Task`; the CLI runs it inline.
- **ADR-0001 A3.2 tool ports** (`LibraryAdapter`, `DistributorAdapter`, a future `Simulator`) are called by the host, never by the core.
- **What is not an effect.** Pure CPU work (netlist building, ERC, DRC) does no IO, so it is not an effect.
  - A short pure call runs inside `execute`.
  - A long pure job (a full ERC or DRC run) must not block the UI thread. Phase 1 keeps such a job on its existing `Task`.
  - How the core hands such a job out later is still open. One candidate is a compute job (an input snapshot and a pure function) that the front end runs off the UI thread and feeds back through `complete`. The research note, §7.3, argues this is compute, not an effect.
- **What stays with the front end.** Preferences, file dialogs and fonts. The core never reads `prefs.json`; it receives `CoreSettings` as values (epic D7).

### Effect ordering and completion (ADR-0003)

- Effects returned together may run concurrently, except that two writes to one path never run out of order. An effect that must follow another (a git commit after its write) is returned only by the `complete` of the one it follows.
- Owed and in-flight work is core state (ADR-0003 D1). That includes the writes in flight and their per-path order: the #997 ledger, under which an older write never lands after a newer one.
- `complete` applies a result only to the same document instance and generation it was issued for (ADR-0003 D4).
- Every effect gets exactly one `complete`, a failure included, so a lost result frees its slot and reaches the person (ADR-0003 D5).
- A boundary command (save, undo, switch, file move) first finishes or settles its owed work (`finish_pending`, ADR-0003 D2).
  - Writes wait, as `SaveBarrier` does today before a synchronous write or a file move.
  - Compute is settled inline, as `settle_pending_inline` does for an owed footprint solve.
  - For a schematic, undo, redo, switch and async Save need no other flush: the revision stamp covers them (ADR-0003 D2, D4).

### Events are a return value, not a bus

- `CoreOutput { events, effects }` goes back to the one caller, in order. These are application events (what changed: a document, its dirty state, its results), not domain events published to subscribers.
- The GUI projects the events of its own calls into GUI-only state (ADR-0001 A1).
- In a running window the agent socket does not own or call the core. Its requests enter the GUI loop through the ADR-0001 D8 channel → `Subscription` bridge. The GUI calls `execute` with an agent origin, projects the events like its own, and hands the reply and the events back to the socket.
- Only a process without a GUI (the CLI, a server) lets another driving adapter own the core. An adapter serving out-of-process clients forwards the events it received over its own transport.

### Driving adapters

- The iced GUI (`signex-app`): a translator from `Message` to core commands, a projector from core events to GUI-only state, and the GUI-only commands (windows, dialogs, tools, view, the undo-target router).
- `signex-cli`.
- The agent socket.
- Later, a web front end or a plugin host.

### Command flow

Effects come back as data; the caller's host runs them and reports each one through `complete`.

```mermaid
sequenceDiagram
  participant P as Person
  participant G as GUI adapter
  participant A as Agent socket
  participant L as CLI adapter
  participant C as AppCore
  participant H as Host
  P->>G: key, menu or palette
  A->>G: request through the D8 channel and Subscription
  G->>G: fill implicit operands: document, targets, form defaults, person-busy data
  G->>C: execute(id, args, origin)
  Note over C: a boundary command first finishes or settles owed work (ADR-0003 D2)
  C-->>G: outcome + events + effects
  G->>G: project events into GUI-only state
  G-->>A: reply + events
  G->>H: run effects off the UI thread as a Task
  H-->>G: EffectResult
  G->>C: complete(effect id, result)
  Note over C: applied only to the same document instance and generation (ADR-0003 D4)
  C-->>G: events + follow-up effects, e.g. a git commit after its write
  L->>C: execute(id, args, origin)
  C-->>L: outcome + events + effects
  L->>H: run effects inline
  H-->>L: EffectResult
  L->>C: complete(effect id, result)
```

Two writes to one path never run out of order, whichever caller runs them (the #997 ledger is core state).

### Messages the core reports: `signex-i18n-ids`

Caner's decision of 2026-11-15 (`docs/DECISIONS.md`):
- **What it holds.** A small UI-free crate, `signex-i18n-ids`, holds `MessageId`, the id constants (`ids::*`, `command_ids::*`) and `Translatable` (an id plus its arguments). It has no Fluent, ICU or locale dependency.
- **The guard.** It joins `DOMAIN` in `scripts/check_arch_guard.sh`.
- **`signex-i18n` does not move.** It keeps the Fluent bundles, ICU formatting and locale negotiation, and stays in `UI_STACK`. It re-exports the ids at their old paths, so existing call sites keep working. The one change for callers: `render_en_us()` and `localized()` become methods of the `signex_i18n::RenderTranslatable` trait, so a file calling them imports the trait. The 2026-11-07 row that no domain crate may depend on `signex-i18n` stays true.
- **Who renders.** The core and `signex-commands` carry `Translatable` values (notices, refusal reasons, typed errors such as `QueryError`) and never render them. Front ends render them in `view()`, which keeps the `(MessageId, args)` label rule.

Splitting this crate is the first step of slice 1, because the catalog that slice 1b moves imports `signex_i18n::{I18n, MessageId, command_ids, t}` today.

### How DDD (ADR-0001 A2) maps onto it

ADR-0001 A2 stays. This record names the layer above the engines and makes the engine's role precise.

| DDD element | In signex |
|---|---|
| Aggregate (root and its commands) and consistency boundary | A document and its `signex-engine` engine: `Command` → `DocumentPatch`, history, revision. `AppCore` never pokes document fields. This holds for schematics today; symbol, footprint and Component rows join as #661's `History<S>` lands. Until then, their app-local history moves with them. |
| Domain services | `signex-net`, `signex-erc`/`-dsl`, `signex-drc`/`-dsl`, `signex-req`, `signex-sketch`, `signex-bake`, `signex-bom`, `signex-output`, `pcb-geom`, and the domain part of `signex-library`. Every UI-free domain crate is listed in `DOMAIN` in `scripts/check_arch_guard.sh`. |
| Value objects | `coord`, `rotation2d`, `anchor2d`, … (unchanged) |
| Common domain types | `signex-types`, the whole model's data shared inside one team. "Shared kernel" is kept for the one real context seam, the schematic-to-PCB `Netlist` contract of ADR-0001 A3.1. The crate keeps its name. |
| Application layer (use cases) | the `AppCore` command handlers: load, run a command, save, run ERC/DRC, journal an agent edit. A command that touches several documents (ECO, back-annotation) records one undo group. |
| Ports | driving: `execute` / `complete` / `query`; driven: `Effect` / `EffectResult` and the capabilities they name, defined by the core |
| Adapters | driving: GUI, CLI, agent socket; driven: the host (native and test now, browser later) and the A3.2 tool adapters it calls |

**`signex-types` keeps its name** (Hakan, 2026-11-15). A rename would churn hundreds of imports for no gain. Its contexts grow as modules inside it: the ADR-0001 A3.1 schematic-to-PCB contract is already one (`signex_types::net`, `Netlist`), and a new contract follows the same pattern, a module rather than a new crate, unless a later ADR says otherwise.

**Handlers stay thin.** An `AppCore` handler does three things:
1. resolves the target document;
2. builds one engine `Command` or calls a domain service;
3. turns the result into events and effects.

A rule about document content belongs in the engine or a domain crate, never in a handler. Examples: the next free designator, where a wire splits, what a paste renames. A handler that grows such a rule is the ADR-0001 D4 leak one level down, and review treats it the same way. This keeps the core from becoming a transaction script.

**The shared command crate (epic D11).** The command catalog, its arguments and outcomes live in `signex-commands`, a small crate that the core and every front end depend on. It is the driving port's vocabulary, so it gets the core's checks. Agent wire framing kept there is only a serialization of those types; the core never names it. Its types are versioned.

## Enforcement

The application core and the crates it uses get the treatment ADR-0001 D5 gives the domain crates, using allowlists rather than denylists.

- **Arch guard.** Each crate's edges go in `scripts/arch-edges.txt`, and the crate joins `DOMAIN` in `scripts/check_arch_guard.sh` when it is created. `DOMAIN`, not the edges file, is what fails on a UI/GPU dependency.
  - `signex-i18n-ids` and `signex-commands` join at slice 1. The scan of the `signex-commands` source for `std::fs`, `std::thread`, `std::process` and runtimes comes with the clippy lint after the lift; until then review keeps it free of IO.
  - `signex-workspace` joins at slice 9.
- **While the core grows inside `signex-app`.** A guard fails when `app/core/` imports any `crate::` path outside `app/core/`, or names `iced`, `tokio`, `spawn_blocking` / `run_blocking`, `std::fs`, `std::thread` or `std::process`. It lands with slice 1.
- **Core-bound state stays UI-free before its slice** (Hakan, 2026-11-17). The `app/core/` guard sees only code already moved; the defects that would surprise a slice sit in the state still waiting to move. A struct whose data the slice table moves into the core (documents, drafts, dirty state, histories, owed work, ledgers, check results, the agent journal) gains only fields whose types come from an allowlist: `std`, the domain crates in `DOMAIN`, `signex-commands`, `signex-i18n-ids`, `app/core/`, named third-party data crates (such as `uuid`), and the `signex-app` modules listed in the rule file as UI-free (the targets of the "move to a UI-free module" PRs). UI-free is closed: outside `#[cfg(test)]` items, a listed module imports only from `std`, `DOMAIN`, the named data crates, `app/core/` and other listed modules. Almost no `signex-app` module meets that today (`state/dirty.rs` reaches `state/mod.rs`, which imports iced, through `super`; `document_read` uses `persist::DiskRead`). `state/check_runs.rs` does meet it; listing it is a maintainer's call like any other addition, so the list starts empty and fills as the pre-slice PRs land. The ratchet checks that definition for every listed module on each run, so a listed module that gains such an import fails. An addition to the list needs a maintainer's approval (Caner or Hakan), like the ratchet's baseline. Fields that exist when the ratchet lands are in its baseline; a field leaves the count when its type's module joins the list. A new field whose type is an unlisted `signex-app` type is counted, so the PR that adds it first moves that type to a listed module. Anything else (`iced`, a render or widget type, a GUI module of `signex-app` such as `canvas`, `panels`, `schematic_runtime`, `library::editor`) is counted. An ast-grep ratchet, `scripts/check_uifree_state.sh`, scans the structs and enums named in its own rule file, `scripts/uifree-state.txt`, records the count when it lands (`scripts/uifree-state-baseline.txt`), and the count may only go down. A document rule may not read `UiState` or a canvas render snapshot: such a value reaches the rule as an operand or as `CoreSettings`, and review treats a new read as the ADR-0001 D4 leak. The ratchet lands before slice 2, with the pre-slice PRs listed under "Safety net before the high-risk slices".
- **After the lift.** Clippy `disallowed-methods` / `disallowed-types` keep the same calls out of the core. They are modelled on the `io_ports` precedent in `clippy.toml`.
  - The lint reaches the domain crates only after slice S-IO, because `signex-types` and `signex-engine` still call `std::fs` today.
    - *Amended by [ADR-0007](adr-0007-cloud-boundary.md) (proposed): #2431 schedules the DOMAIN-crate part of S-IO now.*
  - Clippy reads the nearest `clippy.toml` and does not merge it with the workspace file, so a per-crate file must repeat the workspace bans.
- **No `wasm32-unknown-unknown` gate now** (epic D10: native first).
  - *Amended by [ADR-0007](adr-0007-cloud-boundary.md) (proposed): a `wasm32` check gate on the `DOMAIN` set in `scripts/check_arch_guard.sh` (#2431).*
  - If a web front end is ever scheduled, a wasm32 build of the core and the crates below it would keep OS-bound dependencies (git2, tokio's IO and multi-thread runtime) out.
  - That build alone would not catch `std::fs` or `std::thread`: they compile there and fail only at run time. The lint above is what keeps them out either way.
- **Stack frames (recommendation, open).** Run Clippy `large_stack_frames` on the core, against the stack overflow of #2325. This comes from the research note (§7.6), not from an epic decision.

## Alternatives considered

| Alternative | What it fixes | Why it stops short |
|---|---|---|
| (a) Status quo, plus a lazy `UiState` and boxing the large fields | #2325; part of the headless boot cost | The CLI keeps all 518 crates, the parity heuristics (`gui_only`, `headless_refusal`, `honest_status`) and the IO in `update`. Rejected; slice 0 still lands its first part, so a pause after slice 0 is never worse than today. |
| (b) Extract only a command-catalog crate | The CLI and agents get the catalog, operands and outcomes without `signex-app` types | Running a command still needs `Signex`, so the CLI still builds and boots the GUI. |
| (c) Turn only persistence and git IO into effects | The data-loss-prone IO leaves `update`; ADR-0003's invariants get one home | The handlers still live in the GUI model, so the CLI still boots it and parity stays heuristic. |

This record is (b) and (c) together, plus moving the handlers into the core; each slice below is a stopping point.

## Slices

Numbering follows epic #2326; one PR at a time, in the order 0, 1, 1b, C, 2, 3, 4, 5, 7, 6, 8, 9 (S-A after 1, S-L after 5, both before 9). Each is reverted by reverting its PR, so the revert column names only what a revert must also undo.

Every slice has the same stop rule: stop and come back to the maintainers if its exit check fails, or if it needs a facade on `Signex` that outlives the slice (except the declared exceptions under "Consequences"). The stop-if column adds what is specific to the slice.

**The contract grows with the slices** (Hakan, 2026-11-17). `CoreEvent`, `Effect`, `EffectResult` and `CoreContext` are not designed ahead of the code they describe: each variant is added by the slice that moves that behaviour, derived from the trunk type it replaces (for example `DocumentChanged` from `signex_engine::DocumentPatch`, the write effect from `DocumentSaveJob`), and the PR names that trunk type. Three drafts that froze the whole vocabulary in slice C each lost a trunk guarantee (the disk-changed check of #1826, the case-folded write order of #1112, the restore claim). What keeps a move pure is the safety net: the same test-adapter tests pass before and after it.

| # | What ships | Exit check | Stop if | Revert path |
|---|---|---|---|---|
| 0 | #2325: box `Signex` / run the CLI session on a thread with an explicit stack (the first part of alternative (a)) | The debug `signex-cli run` passes on Windows CI. | — | revert the PR |
| 1 | First, split `signex-i18n-ids` out of `signex-i18n`. Then extract `signex-commands`: `Modifiers` / `ShortcutContext` out of `binding.rs` (`from_iced` stays in the app), `DocumentTarget` and `PcbTargets`. The `app/core/` guard script lands. Low risk. | Both crates are in `DOMAIN` and `check_arch_guard.sh` passes. `cargo tree -p signex-commands` shows no `iced*`, `wgpu*`, `fluent*` or `icu*`. Catalog JSON and `catalog.golden.tsv` are byte-equal. | a golden file changes | revert; the golden files are byte-equal, so nothing else moves |
| 1b | The command catalog into `signex-commands`: `AppCommandId`, the catalog tables and their metadata; labels name `signex-i18n-ids` ids. `CommandOutcome` waits for slice 7 (see there). | Catalog JSON and `catalog.golden.tsv` are byte-equal; `check_arch_guard.sh` passes. | the catalog needs a UI-stack crate | revert; the golden files are byte-equal |
| C | The safety net, not the vocabulary: the (id, form) parity table, the test adapter, the three safety-net tests below, the hostile-operand test (every catalog id with junk for each `OperandKind`, run through the adapter against today's dispatch; no panic, a panic it finds goes on a shrink-only `known_panics` list with an issue) and the drag benchmark baseline, all run against trunk behaviour. A form is each distinct route by which a command id reaches its handler or its implicit target, for example the window-routed commands, the in-window rewrites, the undo routers, the enablement and operand refusals, a bridge arm, the Active Bar table, an operand handler, a query and the headless path; each row records its owner and its implicit inputs. The parity golden has a one-command regenerate step, documented in CONTRIBUTING. The adapter's test-facing API is the frozen part. | The parity table lists every (id, form) pair on trunk; the three safety-net tests and the baseline pass on trunk through the adapter. | an (id, form) has implicit inputs (document, targets, window, a person's in-progress work) that cannot be named as operands | revert; no behaviour moved |
| 2 | Event seam: the schematic edit tails split into a core half that returns events, plus a projector. PCB edit tails move later in the same pattern, with the schematic as the reference, once the PCB editor settles; the maintainers schedule that PR. | Every removed inline call has a projector arm and a test. The safety-net tests below pass through the test adapter. The edit-tail wrappers are the only callers of the core halves (an ast-grep guard over the functions of `app/state/sheet_change/` visible outside the module: `pub` / `pub(..)`, not `pub(self)`), and the module names no GUI state (decision M6) and declares no traits; it joins `app/core/` at 4a. | the event-granularity bench shows a drag-time regression beyond its noise band, or a safety-net test cannot be expressed through the test adapter | revert restores the inline tails; the removed-call list in the PR says what comes back |
| 3 | `load_gateway` split | Load and restore tests pass through the test adapter, and a test proves that a panicking install leaves no engine missing from the map. The load wrappers are the only callers of the load halves (an ast-grep guard over `app/state/document_load/`), and the module names no GUI state (decision M6) and declares no traits; it joins `app/core/` at 4a. | any restore is dropped in a test | revert both PRs together |
| 4 | State partition (4a documents, 4b ERC/DRC/profiles, 4c settings) | The moved field groups are private to the core; `rg 'ui_state\.(io\|erc_\|profile_)'` has 0 hits for the moved groups. | a group cannot move without a facade | the ast-grep renames are reversible by the inverse rule |
| 5 | Effects as data and the native host: 5a save/journal, 5b load, 5c git, 5d export, 5e the rest | For each group, `app/core/` has 0 `spawn_blocking` / `run_blocking` / `std::fs` hits, and the save-order and quit-barrier tests pass. At the end of 5b and of 5c, the load wrappers that end in that sub-slice are gone from their guard list. | the save-order or quit-barrier test fails | revert brings back the `Task` path for that group |
| 7 | `Needs` split into Core and Ui. `CommandOutcome` moves into `signex-commands` here, as `Disabled { reason }` becomes a `Translatable`, so the versioned crate never ships the `String` form (Hakan, 2026-11-17). | Enablement of every core command is computed without `UiState`. | — | revert |
| 6 | Handler table into the core per area, the `Targets` operand, `gui_only` deleted. Updates the CONTRIBUTING "Commands" bullet and `docs/COMMANDS.md`, and records the supersession under "Superseded decisions". Before the first area moves, an inventory PR maps every production call site of the edit-tail and load wrappers to a catalog command (adding ids where none exists) or to a GUI-only table entry that calls `execute`; each area then moves its mapped sites with its handlers. | Per area, the area's ids are no longer flagged `gui_only` and have no bridge arm. At the end, `git grep -c gui_only -- crates` gives 0. At the end, `scripts/sheet-change-wrappers.txt` and the load-wrapper list are empty and #2704 is closed. | an area needs a dual path past its slice | revert per area |
| 8 | Queries on `core.query` | Every query answers through `core.query` in a test-adapter test. | — | revert |
| S-A | Agent wire protocol and `OperandKind` in `signex-commands` | `signex-cli` has no `signex_app::headless::agent_client` import. | — | revert |
| S-L | Library, pricing, editors and 3D import onto the core | The moved areas have no IO in `app/core/` (the guard passes). | — | revert per sub-area |
| 9 | Lift `app/core/` into `signex-workspace`; the CLI drops `signex-app` | Four checks: `cargo tree -p signex-cli -e normal` shows no `iced*`, `wgpu*` or `winit`; a headless run with no commands boots in about `--help` time; `git grep -E 'gui_only\|honest_status\|boot_headless' -- crates` gives 0; #2704 is closed. | — | revert the move; the module stays in place |
| S-IO, S-P | Domain IO out for wasm32; the plugin host | — | not scheduled: S-IO waits on a future web front end being scheduled, S-P on #1707. *Amended by [ADR-0007](adr-0007-cloud-boundary.md) (proposed): #2431 schedules the DOMAIN-crate part of S-IO now.* | — |

```mermaid
flowchart LR
  S0["0 · CLI stack fix, issue 2325"]
  S1["1 · i18n-ids split, signex-commands extraction<br/>+ guard script"]
  S1b["1b · command catalog<br/>into signex-commands"]
  SC["C · safety net<br/>parity · adapter · tests · baseline"]
  S2["2 · event seam<br/>high risk"]
  S3["3 · load_gateway split<br/>high risk"]
  S4["4 · state partition<br/>4a · 4b · 4c"]
  S5["5 · effects as data<br/>high risk"]
  S7["7 · Needs Core/Ui split"]
  S6["6 · handler table into the core<br/>+ Targets operand"]
  S8["8 · queries"]
  SA["S-A · agent protocol<br/>into signex-commands"]
  SL["S-L · library · editors · 3D"]
  S9["9 · lift the crate<br/>CLI drops signex-app"]
  SIO["S-IO · domain IO out<br/>not scheduled · proposed by ADR-0007:<br/>DOMAIN part by issue 2431"]
  SP["S-P · plugin host<br/>not scheduled"]
  S0 --> S1 --> S1b --> SC --> S2 --> S3 --> S4 --> S5 --> S7 --> S6 --> S8 --> S9
  S1 --> SA --> S9
  S5 --> SL --> S9
  SC -.-> SIO
  S9 -.-> SP
```

### Safety net before the high-risk slices

The epic rates slices 2, 3 and 5 (and S-L) high or medium-high. The research note (§10, item 1) names the reason: the persistence invariants become state machines across `complete`. Before slice 2 starts, these tests run against the test adapter, not only against the ledger:
- **Per-path write order (#997):** an older write never lands after a newer one to the same path. Today it is pinned at the ledger level (`persist/document_save.rs`, `an_older_snapshot_scheduled_last_never_overwrites_a_newer_one`).
- **The `SaveBarrier` wait:** a synchronous write or file move waits for every write in flight (`the_barrier_waits_for_an_unfinished_job`, same file).
- **The `complete` generation check (ADR-0003 D4):** a result for a closed, reopened or re-keyed document is dropped. Slice C pins it as a behaviour test on today's completions (the save revision check in `handlers/document_files/save_async.rs` and `finish_claim` in `app/document_read.rs`); the slice that adds `complete` keeps that test green. One trunk detail the tests keep: a restore cancelled by Close All that meets a clean reopened tab of the same file is applied, not dropped; one that meets a reopened tab with edits is dropped.

Slice C landed these as `crates/signex-app/tests/regression/core_safety_net.rs`, driven through the test adapter (`crates/signex-app/src/test_adapter/`): `an_older_save_delivered_last_never_overwrites_a_newer_one`, `a_restore_waits_for_every_write_in_flight`, `a_save_completion_for_a_document_that_moved_on_keeps_it_dirty`, `a_tree_read_finishing_after_close_all_is_dropped`, `a_restore_cancelled_by_close_all_shows_its_file_in_a_clean_reopened_sheet` and `a_restore_cancelled_by_close_all_keeps_the_edits_of_a_reopened_sheet`. The hostile-operand test is `test_adapter/hostile_tests.rs`; its `known_panics` list started empty. When a slice moves the click-equivalence tests onto the adapter, their assertions do not change; only their setup moves to the adapter.

The event-granularity benchmark of the research note (§10, item 4) also lands before slice 2: a drag must not over-invalidate the projector.

Also before slice 2 (done, #2445), a behaviour-neutral PR moved "what changed" out of the rendering type. The schematic edit tails and the agent's owed refresh (`agent_edits.owed`) decided on `RenderInvalidation`, which lives in `schematic_runtime` beside iced and the renderer, although the fact is the engine's `DocumentPatch` and `RenderInvalidation` is only a lossy projection of it. The tails and `owed` now take `DocumentPatch`; the projection is `From<DocumentPatch> for RenderInvalidation` in `schematic_runtime`, applied once where the GUI refreshes; the "display-only" rule (formerly `== NET_COLORS`) is `DocumentPatch::is_display_only` in `signex-engine`. Slice 2's event is then `DocumentChanged { path, patch, cause }`, with `cause` an edit or a history step (the library drift check needs it).

A structural sweep (2026-11-17) found more core facts held in UI types. Each gets a behaviour-neutral PR before the slice that moves it:
- **Before 4a: the PCB board engine lives in its tab** (`TabInfo.cached_document`, parked only when a dirty tab closes). It becomes `DocumentState::boards` keyed by path, like `engines` (done, #2449). The #2149 tests (clean close drops, dirty close keeps, reopen restores) pass with their assertions unchanged; their setup reaches the board through the `pcb_board` accessors since #2960.
- **Before 4a: the symbol draft** splits into a draft (file, baseline, dirty, history with its field coalescing key and the open move run of a drag (#2326), `save_generation`, the owed `cascade_owed`, and the active part, body and index) and a GUI editor view (canvas cache, viewport, camera, tool, selection) (done, #2974). The draft is `SymbolDraft` in `app/symbol_draft.rs`, held with the `SymbolEditorView` (the `gui` field) inside `SymbolDocument`; undo and redo stay on the container, because a step clears the selection and resets the view. The footprint draft waits for a decision, because its undo restores the selection today.
- **Before 4a: the project General history** moves from `ProjectTabState` into `DocumentState`, next to `rules_history` (done, #2975): it is `DocumentState::general_history`, keyed by `ProjectId` like `rules_history`.
- **Before 5a and 6: in-progress edits** (`PanelDrafts`, title-block text a person has typed) are committed by one GUI step before every boundary command, which keeps each boundary's scope from the 2026-11-04 decision (#2027: Save and Save As commit that document's sheets, Close Project that project's drafts, an export its document's project; another project's drafts stay pending). The existing `title_block_persist_tests` pin the scopes and must pass unchanged.
- **Before 4b:** `ErcRunOutcome`, `WaivedFinding` and `PageFrames` move to a UI-free module.
- **Before slice C needs them (done, #2456):** `Origin`, `ResolvedPlacement`, `ProjectId`, `CaseFold`, `DocumentKey` and `NewLibraryDefaults` moved to the application core module `app/core/`; `persist::project_save` imports `CaseFold` and `NewLibraryDefaults` from there. The gateway's `CommandOutcome` is now `MutationOutcome`, so it no longer shares a name with the command one.

Two items wait for a maintainer decision. One: component row drafts (unsaved rows, staged datasheet bytes) live in the GUI library state, so either 5a covers files only and rows stay a GUI-side gate until S-L, or the row draft moves in 4a. Two: Run ERC takes the active sheet from the canvas render cache, which the core does not have; it moves to the engine document only after a test proves the two are equal at rest.

## A web front end (epic D10)

[Maintainer decision 2026-11-15](https://github.com/alplabai/signex/issues/2326#issuecomment-6884693348): not now; native first; a browser host only if a web front end is scheduled. Keeping the core free of iced and of native IO keeps a web front end possible later. That is an option, not a commitment: there is no `wasm32` build gate now, and S-IO is not scheduled.

*Amended by [ADR-0007](adr-0007-cloud-boundary.md) (proposed): D10 is reopened narrowly for a read-only browser viewer only. No editing web front end and no `AppCore` browser host are scheduled.*

## Superseded decisions

- **The 2026-11-02 `docs/DECISIONS.md` row ("an `AppCommandId` behaves the same on every surface").** Its operand clause says operands are filled only by a CLI and no GUI caller sets one. Under this record a front end fills in the document, the targets and the form defaults as operands before `execute`. The rest of the row stays. The supersession takes effect at slice 6, which updates the CONTRIBUTING "Commands" bullet and `docs/COMMANDS.md`. It is recorded in `docs/DECISIONS.md` (2026-11-15).
- **The 2026-11-07 row "one crate owns internationalisation"** stays true for the rendering crate. `signex-i18n-ids` holds ids and arguments only and imports no Fluent or ICU4X.

## Consequences

- The CLI stops building and booting the GUI once slice 9 lands. Until then each slice is a valid stopping point.
- Every command gets one handler; the parity heuristics go in slices 6 and 9.
- The persistence invariants move into the core as state machines across `complete`. This is the main risk, and the safety net above is the guard against it.
- Long-lived dual paths are not allowed. A facade on `Signex` that delegates to the core is removed in the slice that adds it. There are three declared exceptions, and no other:
  - the test adapter (slice C);
  - the edit-tail wrappers of slice 2, the nine functions that call the core halves of `app/state/sheet_change/` directly: `apply_engine_commands_reporting`, `apply_engine_command_recording`, `finish_schematic_mutation`, `apply_engine_step` and `mark_active_document_dirty` (`app/mutation_gateway.rs`); `execute_on_sheet`, `step_sheet_on` and `after_sheet_change` (`app/handlers/editing_commands/sheet_on.rs`); `refuse_if_read_only_at` (`app/read_only_refusal.rs`). The public forms that only delegate to them go with them: `apply_engine_command`, `apply_engine_command_checked`, `apply_engine_command_reporting`, `apply_engine_command_unrecorded`, `apply_engine_command_noticing` (#2552), `apply_engine_commands`, `apply_engine_commands_checked`, `apply_engine_undo`, `apply_engine_redo` and `refuse_if_active_read_only`. Each wrapper runs the core half and then projects the events it returns (`mark_active_document_dirty` returns none), inside the same call, so the GUI keeps today's call sites until slice 6 moves the handlers that call them. They are removed area by area in slice 6, and the list is empty when slice 6 ends. A CI guard (`scripts/check_sheet_change_seam.sh`, list `scripts/sheet-change-wrappers.txt`, its line count and names' checksum pinned in the script) keeps every other function from calling the core halves.
  - the load wrappers of slice 3 (3.B), seven functions. `open_schematic_tab` (including the ERC-navigation open in `handlers/erc/erc_run.rs`) and `restore_loaded` are removed in slice 5b. `install_reloaded_schematic` is removed in slice 5c, because its caller `apply_reloaded_reporting` also serves History restore, a git job. Four are removed in slice 6: `apply_loaded_schematic`, whose callers include `attach_parked_schematic_tab` and `sync_visible_document_from_active_tab`; `attach_parked_schematic_tab`; `sync_visible_document_from_active_tab`, which goes when the active document becomes a front-end operand; and `clear_schematic_ui_state`, whose core half is `active_path = None`; its callers `apply_loaded_pcb_document` and `apply_loaded_empty_document` therefore stay outside `app/projector/` until it is removed. The same shape and the same kind of guard, over `app/state/document_load/`.

  The two wrapper exceptions are closed: #2704 tracks every wrapper until it is gone, and each guard's list only shrinks. A raised cap or a new name needs both maintainers' approval (CODEOWNERS on both list files and both guard scripts). A guard stays, following its module into `app/core/` at 4a, until its list is empty. New call sites of the wrappers are allowed until the slice 6 inventory counts them; from then on the count may only fall (the inventory PR adds the call-site count ratchet). Wrapper call sites do not count as dual paths under slice 6's stop-if until the inventory PR has mapped them. Wrappers are called from GUI handlers only: `app/projector/` calls no wrapper and writes no core state (the engines map, `active_path`, dirty and saved marks, histories; library state joins at S-L); a guard checks it from 3.A. Slice 4's stop-if ("a group cannot move without a facade") is not relaxed by them.
