# Signex — Repository and Codebase

> **Status:** Repository structure and implementation map.
> **Audience:** Engineers, contributors, and maintainers working in this repo.
> **Companion to:** `MASTER_PLAN.md`, `ARCHITECTURE.md`, `PRODUCT_AND_EDITIONS.md`,
> `ROADMAP.md`.

This document explains how the Signex repository is organized today, how the
codebase is expected to evolve, and how the single repository supports both the
Community and Pro product lines.

It is intentionally practical. `MASTER_PLAN.md` describes the product thesis.
`ARCHITECTURE.md` describes the target system architecture. This document
describes where the code lives, what each part is responsible for, and how the
repository should be kept coherent as the project grows.

---

## 1. Repository Purpose

This repository contains the Signex desktop editor codebase.

The repository exists to build one product family from one shared foundation:

- **Signex Community** — the open desktop EDA editor
- **Signex Pro** — the commercial edition that adds Signal AI and live
  collaboration

The core rule is simple:

**There is one editor core and one primary codebase. Community and Pro are not
separate products implemented in separate repositories.**

This matters for three reasons:

- KiCad compatibility logic must not fork
- The editing engine must not diverge between editions
- Community and Pro must open the same projects without format drift or feature
  fragmentation in the core editor

---

## 2. Repository Scope

This repository is responsible for:

- The desktop application shell
- KiCad file parsing and writing
- Domain types and editor-facing data structures
- Rendering and hit-testing
- Reusable widgets used by the editor UI
- Product documentation and planning documents

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

- Signal AI
- Live collaboration
- Team-oriented cloud workflows

Pro-specific integrations should remain separable from the editor core so that:

- the Community build stays clean and understandable
- the editor engine does not become coupled to hosted services
- edition boundaries remain explicit instead of leaking through the whole tree

### 3.3. Hard rule

If a subsystem is required for basic local editing of KiCad-compatible projects,
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
│   ├── kicad-parser/
│   ├── kicad-writer/
│   ├── signex-app/
│   ├── signex-render/
│   ├── signex-types/
│   └── signex-widgets/
└── docs/
```

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

- `state.rs` owns grouped application state for UI, document/session, and interaction concerns.
- `view/` owns `Element` construction and overlay composition.
- `handlers/` owns message-family handlers that are called from the main Iced update loop.
- `dispatch/` is a thin routing layer that keeps the top-level `update()` entry point readable, with document and overlay dispatch families split into dedicated files.
- `actions.rs` owns higher-level editor operations.
- `runtime.rs` owns derived-state synchronization such as panel context and theme propagation.

This split exists to keep the Iced `update()` entry point aligned with MVU responsibilities without turning `app.rs` back into a catch-all file.

### 5.2. `signex-types`

The shared domain types crate.

Responsibilities:

- schematic and PCB domain structs
- coordinates, units, layers, themes, identifiers
- data definitions shared across parser, writer, renderer, and app layers

This crate must stay lightweight and should not depend on rendering or UI
frameworks.

### 5.3. `signex-render`

The rendering crate.

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

### 5.5. `kicad-parser`

The KiCad file parsing layer.

Responsibilities:

- reading KiCad S-expression-based formats
- producing typed data that the rest of the editor can consume
- compatibility preservation at the parse boundary

### 5.6. `kicad-writer`

The KiCad file serialization layer.

Responsibilities:

- writing supported Signex in-memory structures back to KiCad-compatible files
- minimizing output churn where possible
- protecting round-trip compatibility

---

## 6. Documentation Layout

The `docs/` folder is not an afterthought. It is part of the product and
engineering system.

The current intent of the core documents is:

- `MASTER_PLAN.md` — product thesis, scope, and priorities
- `ARCHITECTURE.md` — target architectural rules and layering model
- `PRODUCT_AND_EDITIONS.md` — edition split, packaging, pricing logic
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
`ARCHITECTURE.md`.

In particular:

- the current app crate still owns more editing flow than the long-term design
  should allow
- parser/writer responsibilities exist, but the future raw-document and engine
  split is still ahead
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
six-crate layout.

The most likely additions are:

- `signex-engine` — command execution, patching, undo/redo orchestration
- `signex-model` — semantic model layer
- `kicad-document` — raw KiCad document representation with node identity and
  preservation of unknown constructs

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
If architecture direction changes, `ARCHITECTURE.md` must be updated first.

### 9.4. Preserve KiCad compatibility as a repository-wide concern

Compatibility is not just the parser's problem. It affects parser, writer,
domain types, rendering assumptions, and editing behavior. Repository layout
should continue to reflect that shared constraint.

---

## 10. Build and Validation Expectations

The repository should always support straightforward local validation.

Core commands:

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
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

This repository is the shared engineering home of Signex: a KiCad-compatible
editor core with a clean path to both an Apache-2.0 Community edition and a
commercial Pro edition from the same codebase.
