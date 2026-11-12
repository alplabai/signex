# Signex — Repository and Codebase

> **Status:** Repository structure and implementation map.
> **Audience:** Engineers, contributors, and maintainers working in this repo.
> **Companion to:** `ROADMAP.md` and `docs/architecture/`. The product plan
> (thesis, editions) lives in the private `signex-internal` repo
> (`docs/internal`, not in this checkout).

This document explains how the Signex repository is organized today, how the
codebase is expected to evolve, and how the single repository supports both the
Community and Pro product lines.

It is intentionally practical. The product plan describes the product thesis.
The ADRs in `docs/architecture/` describe the target system architecture. This document
describes where the code lives, what each part is responsible for, and how the
repository should be kept coherent as the project grows.

---

## 1. Repository Purpose

This repository contains the Signex desktop editor codebase.

The repository exists to build one product family from one shared foundation:

- **Signex Community** — the open desktop EDA editor
- **Signex Pro** — the commercial edition that adds hosted Signex AI and the
  Generator (Team adds collaboration; Enterprise adds SSO, a self-hosted AI
  gateway and data agreements)

The core rule is simple:

**There is one editor core, and it lives here. Pro is built from a separate
private workspace that depends on this core; it is not a fork of it.**

This matters for three reasons:

- Native `.snx*` parser/writer logic must not fork
- The editing engine must not diverge between editions
- Community and Pro must open the same projects without format drift or feature
  fragmentation in the core editor

---

## 2. Repository Scope

This repository is responsible for:

- The desktop application shell
- Native `.snx*` file parsing and writing (`.snxsch`, `.snxpcb`, `.snxlib`,
  `.snxsym`, `.snxfpt`, `.snxprj`, etc.)
- Domain types and editor-facing data structures
- Rendering and hit-testing
- Reusable widgets used by the editor UI
- Product documentation and planning documents

Import from GPL-based EDA tools is **not** part of this repository. It lives in the separate
GPL-3.0 import companion repository, which provides one-way
conversion to Signex only. See the v0.9 Apache-clean cutover and
issue #62 for the rationale, plus `docs/audit/cleanroom-rewrite-2026-05-01.md`
for the cleanroom-rewrite audit trail.

This repository is not the right place for:

- Deployment infrastructure for hosted Pro services
- Billing systems
- External web marketing sites
- Internal company operations tooling

Those can live in separate repositories when they become necessary. The editor
itself belongs here.

---

## 3. Edition Model in One Codebase

Signex is developed as a single codebase with a shared core.

### 3.1. Community

Signex Community is the main editor foundation made available as open source.

The intended licensing direction for Community is:

- **Apache License 2.0**

That means the Community code should be structured so it can stand on its own as
an open, fully usable editor core without depending on proprietary runtime
services for normal editing workflows.

### 3.2. Pro

Signex Pro is built on top of the same core and adds capabilities that require
commercial infrastructure or proprietary integration, especially:

- Signex AI and the Generator, run through Signex's hosted service (Pro)
- Live collaboration (Team)
- SSO, a self-hosted AI gateway and data agreements (Enterprise)

The plugin system and the headless CLI are Community, not Pro. Customer
designs are never used for AI training unless the customer opts in.

Pro-specific integrations should remain separable from the editor core so that:

- the Community build stays clean and understandable
- the editor engine does not become coupled to hosted services
- edition boundaries remain explicit instead of leaking through the whole tree

### 3.3. Hard rule

If a subsystem is required for basic local editing of native `.snx*` projects,
it belongs in the shared core, not in a Pro-only layer.

If a subsystem depends on paid infrastructure, account-backed multi-user state,
or managed AI usage, it can live behind the Pro boundary.

---

## 4. Current Top-Level Layout

Today, the repository is organized around a Rust workspace:

```text
signex/
├── Cargo.toml
├── README.md
├── LICENSE
├── crates/
│   ├── signex-app/                 # desktop application shell (apex)
│   ├── signex-cli/                 # headless command runner (JSON Lines, see CLI.md)
│   ├── signex-types/               # shared domain types (foundation)
│   ├── signex-engine/              # command / patch / undo engine
│   ├── signex-net/                 # authoritative netlist + connectivity
│   ├── signex-history/             # undo/redo history with a save point (leaf)
│   ├── signex-erc/                 # ERC rule engine (+ signex-erc-dsl)
│   ├── signex-sketch/              # Newton-LM constraint solver + sketch schema
│   ├── signex-bake/                # sketch → footprint bake pipeline
│   ├── signex-output/              # PDF / netlist / BOM export (+ signex-bom)
│   ├── signex-renderer/            # domain types → render primitives (+ signex-gfx)
│   ├── signex-library/             # .snxlib library model (+ signex-library-server)
│   ├── pcb-geom/                   # integer-nm board geometry: R-tree, hit-test, MST (reads signex-types)
│   ├── signex-drc/                 # PCB design rule check: clearance, shorts, unconnected, keepouts, width, vias (reads pcb-geom)
│   ├── signex-drc-dsl/             # DRC rule language: rule text → signex-drc rules
│   ├── signex-widgets/             # reusable iced widgets
│   ├── signex-i18n/                # UI message catalogs and locale-aware formatting
│   └── signex-3d-model-importer/   # STEP / WRL importer
└── docs/
```

> **v0.9 cutover note:** earlier revisions of this document listed
> two external-format parser and writer crates here. Both crates were
> removed from the main workspace in v0.9 (Apr 2026) as part of the
> Apache-clean cutover for issue #62, and one-way import from GPL-based EDA tools
> now lives in the separate GPL-3.0 import companion repository.

This is the correct direction for the current phase of the project: small,
focused crates with clear boundaries.

---

## 5. Crate Responsibilities

### 5.1. `signex-app`

The desktop application crate.

Responsibilities:

- Iced application state
- message/update/view orchestration
- panel layout and docking behavior
- menus, toolbars, dialogs, tabs
- editor interaction flow
- UI-only transient state

This crate should not become the home for parser internals, raw file mutation,
or renderer-specific geometry logic.

### 5.1.1. `signex-app` internal app layout

Inside `crates/signex-app/src/app/`, the app shell is now split by responsibility:

- `state/` owns grouped application state for UI, document/session, and interaction concerns.
- `view/` owns `Element` construction and overlay composition.
- `handlers/` owns message-family handlers that are called from the main Iced update loop.
- `dispatch/` is a thin routing layer that keeps the top-level `update()` entry point readable, with document and overlay dispatch families split into dedicated files.
- `actions.rs` owns higher-level editor operations.
- `runtime/` owns derived-state synchronization such as panel context and theme propagation.

This split exists to keep the Iced `update()` entry point aligned with MVU responsibilities without turning `app.rs` back into a catch-all file.

### 5.2. `signex-types`

The shared domain types crate.

Responsibilities:

- schematic and PCB domain structs
- coordinates, units, layers, themes, identifiers
- data definitions shared across parser, writer, renderer, and app layers

This crate must stay lightweight and should not depend on rendering or UI
frameworks.

### 5.3. `signex-renderer`

The rendering crate (with `signex-gfx` for the wgpu pipeline layer).

Responsibilities:

- conversion from semantic/domain objects to renderable primitives
- cached drawing logic
- hit-testing support
- canvas-facing rendering rules

This crate should derive from editor semantics. It should not become a second
owner of document semantics.

### 5.4. `signex-widgets`

Reusable UI widgets.

Responsibilities:

- custom tree views
- icon and toolbar widgets
- reusable UI building blocks used across the application shell

This crate exists to keep `signex-app` from collapsing into a monolith of local
widget implementations.

### 5.4a. `signex-i18n`

UI message catalogs and locale-aware formatting (epic #2053).

Responsibilities:

- Fluent catalogs embedded from `crates/signex-i18n/locales/`, with `en-US` as the source language
- locale negotiation with an `en-US` fallback, and the `en-XA` pseudolocale
- number formatting through ICU4X

The app holds the locale in `UiState::i18n`, saves it as `language` in the
preferences file and restores it at launch. `Signex::set_language` (reached by
`UiMsg::LanguageChanged`) switches and persists it at runtime. There is no
Language picker in Preferences yet; it comes with the UI redesign and will call
that entry.

It is a UI-layer crate and the only one that imports Fluent or ICU4X; domain
crates cannot depend on it (`scripts/check_arch_guard.sh`).

### 5.5. Native `.snx*` codec (`signex-types`)

The codec for the native canonical formats — `.snxsch`, `.snxpcb`, `.snxlib`,
`.snxsym`, `.snxfpt`, `.snxprj` — lives in `crates/signex-types/src/format/`
(TOML envelope plus TSV bulk blocks; `.snxprj` is JSON, see
`docs/architecture/snxprj-format.md`). `signex-engine` consumes it but does not
own it.

Responsibilities:

- reading the native TOML+TSV `.snx*` formats
- producing typed data that the rest of the editor can consume
- writing in-memory structures back to disk with minimal output churn and
  round-trip stability
- preserving unknown/forward-compatible nodes at the parse boundary

> **Pre-v0.9 history:** earlier revisions of this document listed
> an external-format parser and writer as Sections 5.5 and 5.6. Both crates
> were removed from this workspace in v0.9 as part of the Apache-clean
> cutover for issue #62. That I/O is now optional and lives in the
> separate GPL-3.0 import companion repository (one-way
> import only). License Guard CI enforces that
> `crates/` contains zero imports, dependencies, or shaped symbols from GPL-based EDA tools.

---

## 6. Documentation Layout

The `docs/` folder is not an afterthought. It is part of the product and
engineering system.

The current intent of the core documents is:

- `architecture/` — ADRs: target architectural rules and layering model
- `ROADMAP.md` — sequencing and delivery plan
- `UX_REFERENCE_ALTIUM.md` — UX benchmark and interaction reference
- `REPOSITORY_AND_CODEBASE.md` — repository map and implementation boundaries

Each document should answer a different question. Avoid duplicating whole
sections across files unless the duplication is essential for discoverability.

---

## 7. Current Codebase Reality vs Target Architecture

The repository is in an intermediate stage.

The current workspace already has healthy separation at the crate level, but it
has not yet reached the full target architecture described in
the ADRs under `docs/architecture/`.

In particular:

- the current app crate still owns more editing flow than the long-term design
  should allow
- the native `.snx*` codec lives in `signex-types`, but the future raw-document
  and engine split is still ahead
- some behavior is still organized around application update handlers rather
  than a dedicated engine crate

This is acceptable for the current phase, as long as the direction stays clear.

The repository should evolve toward a stricter structure where:

- the editor UI is only a client of editing commands
- the semantic model is explicit and stable
- document mutation is centralized
- rendering remains derived and cacheable

---

## 8. Desired Medium-Term Expansion

As the project matures, this repository is expected to grow beyond the current
crate layout.

The most likely additions are:

- `signex-engine` — command execution, patching, undo/redo orchestration
  *(landed)*
- `signex-model` — semantic model layer
- `signex-document` — raw native `.snx*` document representation with node
  identity and preservation of unknown constructs (would be extracted from
  `signex-types/src/format`)
- `signex-erc` — schematic rule check engine *(landed in v0.7)*
- `signex-sketch` — 2D parametric sketch mode for footprint editor + PCB
  outline (v0.13+; see `docs/internal/SKETCH_MODE_PLAN.md`)
- `signex-drc` — *(landed, #2307)* PCB design rule check: clearance,
  shorts, unconnected nets and keepouts, measured with `pcb-geom`
  (`docs/architecture/pcb-drc.md`); its rule language is `signex-drc-dsl`
  (`docs/DRC_RULES.md`)
- `pcb-geom` — *(landed, #2174)* integer-nm board geometry: R-tree, hit-test,
  union-find, spanning tree, pad world transform. Polygon offset, Delaunay and
  boolean ops are not part of it yet.
- `pcb-router` — interactive router (walkaround, shove, diff pair,
  tuning, pour). Added with PCB Routing. See `internal/docs/PCB_ROUTER_PLAN.md`.

These should be added only when their responsibility is real and clear, not as
speculative abstraction.

The goal is not to maximize crate count. The goal is to keep ownership
boundaries explicit.

---

## 9. Repository Rules

### 9.1. Keep the core shared

Anything required for local editing, rendering, persistence, or project opening
must remain in the shared codebase and must not become Pro-only by accident.

### 9.2. Do not fork behavior by edition in random places

Edition differences should be introduced behind deliberate seams, not by ad hoc
`if pro_enabled` branches scattered through unrelated modules.

### 9.3. Keep docs aligned with code

If the repository structure changes meaningfully, this document must be updated.
If architecture direction changes, the relevant ADR in `docs/architecture/` must be updated first.

### 9.4. Preserve native `.snx*` round-trip stability as a repository-wide concern

Round-trip stability is not just the parser's problem. It affects parser,
writer, domain types, rendering assumptions, and editing behavior. Repository
layout should continue to reflect that shared constraint.

> **Pre-v0.9 framing:** earlier revisions of this section read "Preserve
> compatibility with a GPL-based EDA tool as a repository-wide concern". After the v0.9
> Apache-clean cutover (issue #62), Signex's canonical formats are the
> native `.snx*` family; external-format I/O is optional and one-way via
> the import companion repository, so the repo-wide constraint is now native
> round-trip stability rather than external-tool compatibility.

---

## 10. Build and Validation Expectations

The repository should always support straightforward local validation.

Core commands:

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Contributors should be able to understand the codebase by moving from docs to
crate boundaries to implementation modules without needing hidden tribal
knowledge.

That is the standard this repository should hold.

---

## 11. Licensing Statement for Planning

For product and repository planning purposes, the intended licensing model is:

- **Signex Community:** Apache License 2.0
- **Signex Pro:** commercial / proprietary terms for Pro-only additions

This document records the intended direction so the repository can be organized
around a clean open-core boundary.

If legal or release files elsewhere in the repository still reflect an older
license position, those files should be updated deliberately in a separate
change rather than by silent drift.

---

## 12. One-Sentence Summary

This repository is the shared engineering home of Signex: an Apache-clean
EDA editor core with native `.snx*` formats and a clean path to both an
Apache-2.0 Community edition and a commercial Pro edition from the same
codebase. Optional one-way import from GPL-based EDA tools lives in the separate GPL-3.0
import companion repository.
