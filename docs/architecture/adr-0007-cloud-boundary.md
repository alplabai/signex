# ADR-0007 — The cloud platform sits outside the application core, in the Pro layer

- **Status:** Proposed
- **Scope:** where the Signex cloud platform meets the desktop:
  - accounts, entitlements and AI credits
  - cloud storage of projects and libraries
  - the hosted AI agent
  - the cloud service's API contract
  - desktop sign-in
  - browser preview
  - what phase 1 must leave in place for real-time co-editing later
- **Builds on:** [ADR-0006](adr-0006-application-core.md) (one application core, driving and driven ports; accepted, #2327), [ADR-0003](adr-0003-deferred-work-and-consistent-history.md) (owed work, late results, undo snapshots), [ADR-0001](adr-0001-app-structure-and-boundaries.md) A3.2 (tool ports) and D8 (the agent channel)
- **Amends:** [ADR-0006](adr-0006-application-core.md): the `wasm32` gate bullet under "Enforcement", the domain-crate lint timing under "Enforcement" and slice S-IO, and "A web front end (epic D10)". See "Changes to ADR-0006"; #2327 itself is not edited.
- **Relates to:** #1715 (`signex-vcs`), #1707 (libraries from a git URL), #2245 (Agent mode is Pro), #2429 (object ids minted by the caller), #2431 (no disk, clock or randomness in DOMAIN crates), epic #2326
- **Tracking:** the Pro Release gate (`docs/ROADMAP.md`)
- **Code change:** none. This record only.

## Context

The editions are fixed (`docs/ROADMAP.md` §3, decision of 2026-11-08):
- **Community** is Apache-2.0 and offline forever. Nothing in it is locked, and it shows no upgrade prompts.
- **Pro** is a subscription with included AI credits, served by Signex's hosted service.
- **Team** adds collaboration.
- **Enterprise** adds SSO, a self-hosted AI gateway and data agreements.
- Pro is built from a separate private workspace, not from a Cargo feature.
- `docs/DECISIONS.md` (2026-11-09, #2245): "Agent mode is Pro only and is built in the separate Pro repository; the Community repository holds no AI code".

Pro needs a cloud platform, sold by Alp Lab's Swedish company to customers worldwide (EU, US and Asia first). It provides:
- accounts, organisations and seats
- metered AI and agent use
- cloud storage for projects and libraries
- a browser preview with sharing and comments
- later, real-time co-editing

Without a recorded boundary, each of these could add a port, a type or a check to the code every edition shares.

ADR-0006 already says where outside concerns belong:
- **Driven side:** "The core defines the port types: `Effect`, `EffectResult`, and the storage, version-control, network and clock capabilities they name. A *host* implements them" (Driven ports). "Swapping a host changes where documents are stored without touching the core or the front end."
- **Front-end state:** "What stays with the front end. Preferences, file dialogs and fonts. The core never reads `prefs.json`; it receives `CoreSettings` as values (epic D7)."
- **Agents:** "In a running window the agent socket does not own or call the core. Its requests enter the GUI loop through the ADR-0001 D8 channel → `Subscription` bridge. The GUI calls `execute` with an agent origin" (Events are a return value, not a bus).
- **Owning the core:** "Only a process without a GUI (the CLI, a server) lets another driving adapter own the core."
- **Web front end:** "not now; native first; a browser host only if a web front end is scheduled" (epic D10).

This record applies those clauses to the cloud.

## Decisions

### D1. Account, entitlement and credits live outside `AppCore`, in the Pro layer

Sign-in state, the organisation and seat, the plan, its entitlements and the AI credit balance are **Pro front-end state**:
- They are held by Pro driving adapters and Pro GUI panels.
- They are not core state, not `Effect`s and not `CoreSettings`.
- `AppCore` names no account, plan, licence or credit type.

This is ADR-0006 epic D7 applied to a new kind of setting: the core is given values and never reads where they come from.

### D2. Community holds no cloud port, no licence check and no upgrade prompt

- The Community repository defines no cloud-specific port, trait or `Effect` variant.
- It contains no entitlement or licence check, and no code path that changes behaviour by plan.
- What Community may contain is generic: git remotes over HTTPS, a credential source in the OS keychain (the `keyring` setup of #976), and the domain renderers.
- Each of these is useful without an account.
- Every entitlement check is enforced by the cloud service, never by the client. The client is open source and can be changed.

### D3. Cloud storage is git over HTTPS with LFS, hosted in Sweden

- A project or library in the cloud is a git repository on a Signex-hosted remote, with large files in LFS on object storage.
- The desktop reaches it through the version-control capability of ADR-0006 (`signex-vcs`, #1715) and the git-URL work of #1707.
- The remote is just a remote: the same code pushes to any git host.
- A Pro host adds only the access token.
- **Export is `git clone`.** That satisfies the cloud-switching duty of the EU Data Act without a separate export tool.
- Customer content and personal data are hosted in Sweden (EU).
- Version history is the git history. No second versioning model is introduced.

### D4. The hosted AI agent is a driving adapter; metering is our own ledger

- The agent loop runs in the hosted service. On the desktop, a Pro adapter receives its tool calls and enters the GUI loop exactly as the agent socket does: `execute` and `query` with an agent origin, through the ADR-0001 D8 channel.
- The core gains no AI type. The allowlist of commands an agent may run stays policy, as in ADR-0006.
- AI usage is metered in the cloud service's own Postgres credit ledger. Each call follows **reserve → call → settle**: reserve an estimated maximum, then settle the actual cost.
- The ledger is the source of truth for credits. It does not depend on the payment provider or on the model provider.
- Requests over the allowance are refused unless the organisation has set an overage budget.

### D5. The cloud services are Rust, with a committed OpenAPI contract and a generated client

- **Services:** the desktop talks to two cloud services:
  - **Alp Lab Account** (`account.alplab.ai`): identity, organisations, entitlements, credits, and the AI gateway
  - **Signex Cloud**: git hosting, sharing, comments and agent sessions

  Both are modular Rust services (axum, sqlx, Postgres), the pattern `signex-library-server` already uses. Their
  internal design is in internal architecture notes and is not part of this record.
- **Contract:** each REST API has a committed OpenAPI spec that is reviewed as the contract. A Rust client crate is
  generated from it for the Pro desktop and `signex-cli`.
- **Conventions:**
  - `/v1/` path versioning, additive within a version
  - errors as RFC 9457 `application/problem+json`, with a stable `type` code that maps to a `MessageId` on the desktop
  - an `Idempotency-Key` header on every request that creates or charges
  - cursor pagination
  - `ETag` / `If-Match` on mutable resources
- **Server-side domain crates:** preview and validation workers may call the DOMAIN crates directly, or own a core as a process without a GUI (ADR-0006).

### D6. Desktop sign-in: authorization code + PKCE through the system browser

- **Flow:** OAuth 2.1 authorization code with PKCE (S256), in the system browser, with a loopback redirect on an ephemeral port (RFC 8252). Embedded web views are not used.
- **Tokens:**
  - The refresh token is stored in the OS keychain through the existing `keyring` setup.
  - The access token stays in memory.
- **Offline:** the service issues a signed entitlement token with an expiry and a grace period. The Pro UI uses it to decide what to offer offline. Calls that need the service are online anyway.
- **Headless CLI:** uses organisation-scoped API tokens. The device-code grant is off by default.
- **Identity provider: Zitadel**, self-hosted (maintainer decision, 2026-11-16).
  - The core runs **unmodified, as a separate service**. It is AGPL-3.0-only, and is extended only through its APIs and Actions webhooks.
  - The Login v2 app (MIT) is forked for a white-label login.
  - Evidence: hands-on spikes of Zitadel v4.20.3 and Keycloak 26.8.2, kept in internal architecture notes.
  - **Authorisation is not Zitadel's job.** Workspaces, roles, guests and project grants live in the account service's
    own tables, and the desktop only sees their effect through API responses.
  - The desktop speaks only OIDC, so the provider stays replaceable (alternative (e)).
- **Client-side rules the spike makes ours:**
  - **The desktop always sends an S256 PKCE challenge.** Zitadel does not enforce PKCE on the server: an exchange without a verifier succeeded.
  - **Exactly one loopback redirect is registered** (`http://127.0.0.1/callback`, any port). Zitadel also accepts `localhost` and `[::1]` for it. That is within RFC 8252, so the client must not rely on a strict host match.
  - **One external IdP per customer organisation.** Domain discovery auto-redirects only when an organisation has exactly one IdP; with two, login fails with "User not found".
  - **Token lifetimes are instance-wide.** The desktop, the CLI and the portal share them; per-client lifetimes are not available.
  - **SCIM v2 is preview and covers Users only.** It is scoped per organisation. Group provisioning is not offered to customers until it lands.

### D7. Browser preview is a read-only WASM viewer; the server renders only static thumbnails

- **Viewer:** the browser preview is a read-only viewer compiled to `wasm32-unknown-unknown` from the DOMAIN crates (file parsing, `signex-types`, the render-to-primitives path of `signex-output`). It fetches the project files from the cloud remote and draws them in the browser: pan, zoom, layer and sheet switching, net highlight, search, cross-probe between schematic and PCB.
- **Why in the browser:** rendering cost moves to the viewer's machine. The server stores and serves files; it does no per-view rendering and needs no render workers that scale with viewers. Interaction (net highlight, layer toggles) needs no round trip.
- **Server-side rendering is kept only for static outputs:** a thumbnail per commit for link previews, e-mails and listings, made once per push by `signex-output` and cached on the CDN.
- **PCB:** both paths need a GPU-free PCB render-to-primitives path in a domain crate. The current PCB path draws through `signex-gfx` (wgpu).
- **Comments** anchor to object ids, with document coordinates in nm and the commit they were made on.
- **Delivery:** the viewer is built in this repository's CI as a versioned artefact, and embedded by the closed
  Signex Cloud web app (React). Its source stays here, in the DOMAIN crates.
- **This narrowly reopens ADR-0006 epic D10** for a read-only viewer only: no editing web front end and no browser host for `AppCore`.
  - The viewer uses the DOMAIN crates directly, without `AppCore`.
  - It adds a `wasm32` check gate on the DOMAIN crates the viewer links. A spike found they compile (open question 8).
  - **#2431** keeps them that way: no disk, clock or randomness in DOMAIN crates, enforced by `disallowed-methods` and the `wasm32` check gate.
- **Storage access from the browser is coarse-grained.** The viewer asks for "the project at commit X" in one request: a pack or archive of that commit. It does not make per-file reads, so a 50-sheet project is one round trip, not 50.
- **The same rule binds a future browser host.** If a browser host for `AppCore` is ever scheduled, its storage effect is that same project fetch.
  - ADR-0006's `Effect` / `EffectResult` + `complete` model is already asynchronous, so such a host would fit without core changes.
  - This record schedules no such host.

### D8. Real-time co-editing is not in phase 1; phase 1 keeps it possible

Phase 1 ships git storage only (D3). Co-editing is planned for later. When it comes, it is expected to be a server-ordered relay of core commands rather than a CRDT. That is not decided here.

These points are recorded now because they are cheap to get right during the ADR-0006 slices and expensive to retrofit:

1. **Object ids are minted by the caller.** Every new object's id is chosen by the front end and carried as an operand, so replaying a command gives the same ids. Ids are persisted in every file row and survive save, undo, paste and a wire split (one half keeps the original id).
   - **#2429:** new ids come from the caller through an `IdSource`, and the engine never mints one.
   - **Partly implemented:** #2429 part 1 merged in #2442 (`77a809cbe`). Part 2 is open.
   - The #2429 audit found 27 production sites that mint ids today, 8 of them inside `execute`.
2. **The command vocabulary is deterministic and versioned.** This is `signex-commands` (ADR-0006 epic D11, "Its types are versioned"), with targets named in the arguments (epic D1). The same vocabulary serves the CLI, the agent and later co-editing.
3. **`CoreOutput` carries per-object deltas** (before and after, by object id), next to the `DocumentPatch` redraw flags. Those flags say which collections changed, not which objects.
4. **A remote origin** (`Remote { peer, session }`) is reserved beside the agent origin when slice C defines `CoreContext`.
5. **Undo becomes origin-scoped inverse deltas** instead of whole-document snapshots. Restoring a snapshot in a shared document would undo other people's work. This changes ADR-0003 D3, so **it is a separate ADR**; this record does not amend ADR-0003.

## Changes to ADR-0006

This record changes three items of ADR-0006. The PR that lands this record also adds an "Amended by ADR-0007" pointer
under each item in ADR-0006. It updates the two ADR-0006 diagram labels that called S-IO "not scheduled" (the DOMAIN
subgraph label and the S-IO slice-graph node) to say that the DOMAIN-crate part is scheduled by #2431.

1. **The `wasm32` gate.**
   - ADR-0006, "Enforcement", says: "**No `wasm32-unknown-unknown` gate now** (epic D10: native first)."
   - **New rule:** a `wasm32` check gate covers the DOMAIN crates the read-only viewer links (D7, #2431). It does not cover `AppCore`, `signex-workspace` or `signex-app`.
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
   - **New rule:** D10 is reopened narrowly, for a read-only viewer only (D7). No editing web front end and no `AppCore` browser host are scheduled.

## Open questions (not decided here)

1. **Legal sign-off** on running the AGPL-3.0 Zitadel core unmodified as a separate service (D6). It is one line, owed before production.
2. **Billing provider.** Decided by the maintainers from a comparison kept in internal architecture notes. D4 holds
   whichever is chosen.
3. **Hosting provider in Sweden.** The services run on Kubernetes with automatic scaling, self-hosted first and
   managed later. The provider choice is internal.
4. **LLM routing and EU residency.** Whether inference is EU-pinned by default, with global routing as an organisation opt-in, and which model providers are approved sub-processors.
5. **Offline grace period** for the entitlement token, and behaviour when a subscription lapses (read-only cloud data is the working assumption).
6. **Remote sync as core effects.** Whether push and pull become core `Effect`s of the version-control capability, or stay host-side and front-end commands. This is settled with #1715.
7. **Markets.** Mainland China is out of scope for now. That is a working assumption, not a decision.
8. **WASM viewer: where project resolution lives.** A spike on 2026-11-16 (Rust 1.97.0) found the DOMAIN crates the viewer needs already compile for `wasm32-unknown-unknown`; only `uuid` needs its `js` feature on wasm. A schematic and four boards loaded from bytes in Node; the bundle is about 630 KB gzip. Open: multi-sheet hierarchy resolution, the project netlist and the export context are built inside `signex-app` from file paths, so they must move to a domain crate that works on in-memory files. Where that crate sits is the open decision; it overlaps the ADR-0006 slices that move project loading into the core. Also open: the rendering surface (schematic as Canvas 2D or SVG; PCB through wgpu on WebGPU with a WebGL2 fallback).

## Alternatives considered

| Alternative | What it offers | Why not |
|---|---|---|
| (a) Cloud ports inside `AppCore` (`IdentityPort`, `EntitlementPort`, `AiAgentPort` as core-defined capabilities) | One place for every outside concern | It puts account and AI vocabulary into the Community core, against D2 and the "Community holds no AI code" decision. The core would have to know about plans, which ADR-0006 epic D7 keeps out. |
| (b) A custom sync API (content-addressed blobs + a version table) instead of git | Fine-grained control over files, previews and permissions | It adds a second versioning model next to `signex-vcs`. It needs its own export tool. Community users could not use any git host. |
| (d) Server-rendered preview only (SVG/PDF per view, thin JS viewer) | No `wasm32` gate; works before the DOMAIN crates are wasm-clean | Render cost grows with viewers; every interaction (layer toggle, net highlight) is a round trip or a pre-rendered variant. Kept only for static thumbnails (D7). |
| (c) CRDT-first storage with real-time co-editing in phase 1 | Live multi-user editing from the start | It puts engine identity, delta and undo rework on the critical path to the first paid release. A CRDT merge can produce states that break EDA rules (duplicate designators, unintended connections). D8 keeps the option open at lower cost. |
| (e) Keycloak 26.8 as the identity provider (Apache-2.0) | Enforces PKCE per client. Strict loopback host. Per-client token lifetimes. Each domain can route to its own IdP. SCIM supported, with Groups. No licence review. | **Its tenancy is realm-wide:** roles and SCIM span the realm, not the customer organisation (an organisation's SCIM client saw another organisation's users). Branding is set per realm or per client, not per organisation. Per-customer tenancy would be ours to build. About 930 MB idle (JVM about 818 MB plus Postgres) against about 310 MB for Zitadel. **We would switch if** customers need several IdPs per organisation, compliance requires server-enforced PKCE, or the server stack must be Apache-only. |

## Consequences

- **The licence boundary is the process boundary.**
  - The Community repository stays Apache-2.0 and complete offline. It never links cloud code.
  - The cloud service, the Pro adapters and the Pro panels are closed and live in the private workspace.
  - Community users can still use git remotes on any host, including Signex's, with a token.
- `AppCore` stays free of account, plan and AI types. The ADR-0006 guard needs no new rule for this record.
- Entitlement is enforced only by the server. A modified client gains nothing the service does not grant.
- Customer data leaves the service as a git clone. The storage design needs no separate export or migration tool.
- The hosted agent and later co-editing reuse the command vocabulary and the agent path. Neither adds an event bus (ADR-0006 epic D12).
- **What phase 1 must carry:**
  - the D8 points 1–4 in the ADR-0006 slices (1, C, 2)
  - a follow-up ADR for origin-scoped undo
  - a GPU-free PCB render-to-primitives path before PCB preview
  - a `wasm32` check gate on the DOMAIN crates the viewer links (D7)
- Per-object deltas (D8.3) add detail to `CoreOutput`. The event-granularity benchmark that ADR-0006 requires before slice 2 must cover them.

## References

- ADR-0006: sections "Driving ports", "Driven ports", "Events are a return value, not a bus", "How DDD (ADR-0001 A2) maps onto it" (the shared command crate), "A web front end (epic D10)"
- ADR-0003 D1–D5; ADR-0001 A3.2 and D8
- `docs/DECISIONS.md`, 2026-11-09 (#2245, Agent mode is Pro; the Community repository holds no AI code)
- `docs/ROADMAP.md` §3 (editions) and the Pro Release gate
- RFC 8252 (OAuth 2.0 for Native Apps); RFC 7636 (PKCE); RFC 9457 (Problem Details for HTTP APIs)
- #1715, #1707, #976, #2326, #2429 (part 1: #2442), #2431
- Internal architecture notes (cloud services, identity spikes, billing comparison) are kept outside this repository
