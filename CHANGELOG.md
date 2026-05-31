# Changelog

All notable changes to Signex ship here. Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) loosely and [Semantic Versioning](https://semver.org/spec/v2.0.0.html) strictly.

Each release section is authored **before** the `vX.Y.Z` tag is created, so the release workflow picks it up as the GitHub Release body. See `.claude/hooks/pre-release-guard.sh` for the enforcement.

## [Unreleased]

## [0.13.0] — 2026-05-31

The **v0.13 Symbol & Library** milestone. This release pairs the
cleanroom schematic renderer (the work planned as v0.12) with a wave of
symbol-editor and library polish. There is no separate v0.12.0 tag — the
cleanroom rewrite ships here. The workspace version had already advanced
to `0.13.0`, so this release adopts that number rather than bumping
backward.

### Headline — cleanroom schematic renderer

- **Clean-room reimplementation** of `crates/signex-renderer/src/schematic.rs`
  (label / symbol / field-style rendering) and the field-autoplace
  heuristic, executed against Signex-only specifications
  (`docs/RENDERING_RULES.md`, Altium parity goals, IEEE-Std-91) rather
  than any third-party EDA source. Schematic rendering output changes
  subtly versus v0.11 — label placement, field rotation/justification,
  and IEEE-Std-91 pin decorators are now driven by the documented rules.
  This is the milestone tracked as "v0.12" in prior READMEs.

### Added — symbol editor

- **Unified active-bar widget.** The `.snxsym` editor adopts the generic
  `signex_widgets::active_bar` in a single-call form, so the symbol
  editor's floating toolbar matches the schematic editor byte-for-byte
  (root highlight, dropdown panels, right-click, chevron). New
  `active_bar_dropdowns` module backs the per-tool dropdown overlays;
  dropdown panels position relative to the bar's `y_offset` so they open
  directly under their trigger button.
- **`.snxsym` TOML+TSV envelope.** Standalone symbol files serialise to
  the same TOML-header + TSV-bulk envelope as the rest of the Signex
  format family (`SymbolFile::to_toml_string` / `from_bytes`), so pin
  tables are line-diffable in git. Legacy JSON `.snxsym` files still load
  (auto-detected on open).

### Added — library

- Library-subsystem polish across the Library Browser, Component
  Preview, and standalone primitive tabs: inline Pick Symbol / Pick
  Footprint binding with read-only cells, project-tree listing of
  `.snxsym` / `.snxfpt` files (rather than individual primitives),
  Save-As flow for standalone primitive libraries, and assorted binding
  / refresh fixes.

### Added — per-file history

- **Per-file Git history right-dock panel** refinements — the History
  panel follows the active tab and renders the file's recent commits via
  `signex_widgets::history_pane`, async-loaded with a generation counter
  to drop stale results on tab switch.

### Changed — footprint editor hidden for this release

- **The footprint / sketch editor is gated off in v0.13.0.** It is
  feature-incomplete and was under heavy daily iteration; rather than
  ship an unfinished editor, its user-facing entry points are disabled
  behind a compile-time flag (`signex_app::feature_flags::FOOTPRINT_EDITOR_ENABLED`).
  Opening a `.snxfpt` no longer pushes an editable Footprint Editor tab,
  and the "New Footprint / PCB Library" create flow is removed from the
  command palette and project-tree menus.
- **Footprints remain first-class data.** Read-only footprint preview in
  the Component Preview tab, Pick Footprint binding of existing `.snxfpt`
  files into component rows, the footprint column in the Library Browser,
  and the bake / library backend are all unchanged. Only the *editor*
  surface is hidden. The full editor returns in a later release by
  flipping the flag.

### Tests

- New regression coverage pinning the footprint gate
  (`opening_snxfpt_does_not_create_editable_tab_when_gated`) plus a
  positive control proving the symbol editor still opens
  (`opening_snxsym_still_creates_editable_tab`). Full `cargo test
  --workspace` green.

### Constraints — Apache-clean invariants (carry forward from v0.9.0)

- Zero `kicad`/`KiCad`/`KICAD` substrings under `crates/`; no
  `kicad-parser` / `kicad-writer` deps or imports; no removed-API
  surface re-introduced; `cargo-deny check licenses` green. The
  cleanroom renderer was authored against Signex-only specs with no
  third-party EDA source in context.

## [0.11.0] — 2026-05-01

The **v0.11 Library & Polish** milestone. Restores the full library subsystem implementation that was developed on the v0.9 branch (2026-04-26 → 2026-04-29) and parked when the Apache-clean cutover took priority. The original 169-commit chain is preserved on the `v0.9-snxlib-paused-2026-04-29` and `v0.10-library-cherry-pick-2026-04-29` tags; this release squashes the `crates/` tree of that chain onto the post-cutover dev as a single restoration unit, with all Apache-clean invariants enforced.

The library subsystem follows the **DBLib model** specified in the v0.9 plan series (`docs/internal/docs/v0.9-library-plan.md` → `v0.9-library-refactor-plan.md` → `v0.9-refactor-2-plan.md`): components live as **rows in TSV tables** (`tables/<category>.tsv`) rather than per-component files; symbols, footprints, and sim models stay as standalone editable primitive files (`.snxsym`, `.snxfpt`, `.snxsim`) addressed by UUID; component rows reference primitives by `(library_id, uuid)` tuples. The same column schema serialises to TSV (LocalGit) or JSONB rows (Database) — one wire format, two backends.

### Added — `signex-library` crate (data model + adapters + diff + search)

- **Component model** — `ComponentRow` carries `row_id` (stable Uuid v7), `internal_pn`, `class`, `datasheet`, `state` (lifecycle), `symbol_ref` / `footprint_ref` / `sim_ref` (`PrimitiveRef { library_id, uuid }`), `pin_map_overrides`, `primary_mpn` + `alternates`, `supply` (distributor listings), `parameters` (template-validated `ParamMap`), and PLM-reserved inert fields for forward compatibility with Signex 365.
- **Lifecycle states** — `Draft / InReview / Released / Deprecated / Obsolete` per the LIBRARY_PLAN §4 contract; placement gating + `state` field changes recorded in git history (LocalGit) or `updated_at` (Database).
- **`LibraryAdapter` trait** — object-safe trait covering table CRUD (`list_tables`, `read_table`, `iter_rows`), row CRUD (`read_row`, `read_row_by_pn`, `insert_row`, `update_row`, `delete_row`), and primitive CRUD (`get_symbol`, `save_symbol`, `list_symbols`, etc.).
- **`LocalGitAdapter`** — `*.snxlib/` directory format with `library.toml`, `tables/<category>.tsv`, `symbols/<uuid>.snxsym`, `footprints/<uuid>.snxfpt`, `sims/<uuid>.snxsim`, `step/<sha256>.step`. Every write commits via libgit2 with a supplied message; reads stream from the on-disk TSV.
- **`DatabaseAdapter`** — generic `component_rows (library_id, table_name, row_id, payload jsonb)` schema; same column shape as the LocalGit TSV, JSONB payload preserves struct fidelity. Sync detection via `content_hash`.
- **`DistributorAdapter` trait** with **DigiKey** (OAuth2 + PKCE), **Mouser** (API key), **LCSC** (anonymous), **JLCPCB** (anonymous) implementations. Per-provider 24h cache; OS-keyring storage for user-supplied keys (macOS Keychain, Windows Credential Manager, libsecret on Linux). Settings → Library → Distributor APIs UI for connecting providers + ordering preference.
- **AI-stub from datasheet** — heuristic table extraction (PDF → text → pin-name guess) gated behind `pdf-extract`. Handed back as a `SymbolPinPreview` the user reviews before committing.
- **Where-Used reverse index** — `WhereUsedIndex::primitive_to_rows` rebuilt from `iter_rows()`; click-to-jump from the Component Preview tab.
- **Tantivy search index** — full-text index over `internal_pn / mpn / manufacturer / description / parameters`; rebuilt on commit.
- **Diff API** — `RowDiff` with per-column-group flags drives lifecycle auto-bump heuristic.
- **Manifest schema** — `library.toml` carries `[[tables]]` config (category → table name + class allowlist), `[users]` (per-email role table), `[workflow]` (review_required, reviewers_required, auto_lifecycle_promote).

### Added — `signex-library-server` crate (axum HTTP+WS skeleton)

- **REST API** — `GET /tables`, `GET /tables/:name`, `POST /tables/:name/rows`, `GET /tables/:name/rows/:row_id`, `PUT /tables/:name/rows/:row_id`, `DELETE /tables/:name/rows/:row_id`. Bearer-token gated.
- **Lock service** — advisory locks per (table, row_id, field-set) with idle TTL + WS notification on release.
- **Migrations** — sqlx-managed schema; `0001_initial.sql` through `0005_tabular_components.sql` covering both Postgres and SQLite via the same column DDL.
- **Lifecycle transitions** with optional review workflow (per-library setting): `state = Draft` → `InReview` → `Released` (with reviewer approval), or direct `Draft` → `Released` when `review_required = false`.

### Added — `signex-app` library UI

- **SCH Library editor** — opens a `.snxsym` as a main-window tab (`TabKind::SymbolEditor(PathBuf)`). Multi-symbol container, per-pin Properties panel (name / number / direction / shape / position / length), drawing tools (Rectangle / Line / Circle), per-graphic Properties surface, drag-to-resize, multi-part component support via `SymbolPin.part_number`, `signex_widgets::active_bar` migration. Save uses the v0.9.1 borrow-based pattern.
- **Footprint editor** — `.snxfpt` opens as `TabKind::FootprintEditor(PathBuf)`. Pad placement canvas, Body3D pane with STEP attach + 3D preview, layer toolbar, courtyard/silk/fab/paste-mask layers.
- **Library Browser tab** — table on the left with clickable column headers (numeric-aware sort), Rev column showing the bound primitive's `version + released` indicator, side preview pane on the right rendering the bound symbol + footprint via `signex-render`. Substring filter across name / value / footprint / description.
- **Component Preview tab** — 5 read-only tabs (Preview / Parameters / Supply / Datasheet / Simulation) per the v0.9-refactor-2 plan §11. Right-click the symbol render → "Open Symbol Editor" opens the standalone primitive tab.
- **Library left-dock panel** — flat list of mounted libraries with single-click `[Open]` button. Filter input narrows the visible library list. Inline category-tree-with-row-grid is intentionally not in this panel — the canonical surface is the Library Browser tab (real libraries have thousands of components).
- **Components panel** — Project / Installed / Global mount sources; renders the active Library Browser tab's row set as a placement palette.
- **New Component modal** — picks library + table + class, mints sentinel UUIDs for symbol/footprint, writes the new row + primitive files atomically. Inline cell editing in the Library Browser grid; Edit Component Details modal on row double-click.
- **Pick Symbol / Pick Footprint picker** — modal scoped to currently-mounted libraries; binds an existing primitive into a row's `symbol_ref` / `footprint_ref`. Never auto-mints empty primitive files; sentinel-`nil` UUIDs flow through until the user picks.
- **Filesystem auto-mount on picker miss** — when the picker can't find a binding in mounted libraries, scans the active project tree for `.snxsym` / `.snxfpt` files and offers them as auto-mount candidates. Selecting one mounts its parent library transparently before binding.
- **Library Updates dialog** — detects primitive version drift across mounted libraries, applies updates row-by-row.
- **Cascade engine** — primitive saves bump bound rows so the Library Browser's Rev column refreshes without an explicit re-scan.
- **History pane scaffold** — `LibraryAdapter::history` API for surfacing per-row commit / migration history.
- **Recovery dialogs** — "missing snxlib", "git directory missing", "binding broken" prompts that route the user back to a working state.
- **Save-As flow** for new symbols / footprints / libraries.
- **Distributor-API settings UI** — Settings → Library → Distributor APIs renders the connect / test / order-preference grid.

### Added — workspace plumbing

- New deps in `Cargo.toml`: `chrono`, `sha2`, `git2` (vendored libgit2), `keyring`, `tantivy`, `oauth2`, `reqwest`, `axum`, `tokio`, `sqlx`, `pdf-extract`, `tower-http`, `tracing`, `tracing-subscriber`. All Apache-2.0 / MIT permissive — `cargo-deny` clean.
- `CDLA-Permissive-2.0` added to the `deny.toml` allowlist for `webpki-roots` (transitive via `reqwest` + `sqlx`).
- `.gitattributes` adds binary patterns for `.pdf`, `.step`, `.wrl`, `.png`.

### Added — UX polish (v0.11 over-Altium wins)

- **Command palette** (UX_IMPROVEMENTS_OVER_ALTIUM §4.1) — `Ctrl+Shift+P` focuses the chrome-strip search bar and opens a fuzzy-match dropdown over every menu action, every panel-open command, every placed designator, and every project file. Sublime-text-style scoring (word-boundary + contiguous + literal-substring bonuses, length penalty); arrow keys navigate, Enter executes, Esc / click-outside dismisses. Caps the result list at 10 with a "More results — refine query" footer. Replaces the old static placeholder. Export PDF stays reachable via File ▸ Export ▸ PDF…
- **Hover tooltip on placed symbols** (UX_IMPROVEMENTS_OVER_ALTIUM §3.4) — dwelling the cursor on a placed schematic symbol for 250 ms now opens a small floating card showing designator + value + footprint + library id. Card tracks the cursor with a 16 px bottom-right offset (auto-flips at the right/bottom edges); vanishes when the cursor leaves the symbol. Click-through is preserved — the tooltip never blocks the next click. Symbol-only by design; wires/labels carry no library metadata worth surfacing.
- **Persistent search across reopens** (UX_IMPROVEMENTS_OVER_ALTIUM §1.1) — the Components-panel filter and the Library Browser tab's search query are now written through to the prefs file on every keystroke and rehydrated on the next launch. Browser queries are scoped per `.snxlib` path so two open libraries don't share state. The primitive picker stays session-only by design (pickers exist to find _new_ primitives — preserved query feels stale).
- **ERC violations grow a Quick Fix chip** (UX_IMPROVEMENTS_OVER_ALTIUM §4.4) — every ERC diagnostic row in the Messages panel now carries a small chip next to the message. `Unused pin` violations get an `Add No-Connect` chip that places a `NoConnect` marker at the offending pin and re-runs ERC so the row disappears immediately; every other rule's chip is a `Show on Canvas` alias for the row click that gives the user a one-click target even when the row's text is long. Click on the row body still does the default zoom-and-select.

### Added — Version control + history (2026-05-01)

- **Per-item tracking-scope picker on Enable Version Control** — the modal opened from the project root context menu now shows a checkbox list of each `.snxsch` / `.snxpcb` / `.snxlib` in the project. Unchecked rows are written to a generated `.gitignore` at confirm time so users can scope the initial commit without manual editing. Library API (`signex_library::enable_project_version_control`) takes the gitignore body as `Option<&str>` and writes both the gitignore and `.gitattributes` atomically alongside `git init`, with rollback on failure — disk state never lands half-applied.
- **Library-level Enable Version Control** — right-click an existing plain-files `.snxlib` node → `Enable Version Control...` opens the same modal scoped to the library directory. Surfaces `library.toml`, `components.tsv`, and any `classes/` / `symbols/` / `footprints/` / `sims/` / `models/` subdirectories that exist on disk as separately tickable rows. `TrackItem` shape refactored to `{ absolute, relative, label, is_directory, tracked }` so library-scope items plug into the same picker without enum bloat. Confirm branches on `VersionControlScope { Project, Library }` so the post-init refresh updates the right tree node.
- **Per-file Git history right-dock panel** — new `PanelKind::History` follows the active tab and renders the file's last 50 commits via `signex_widgets::history_pane`. Wires on a new `signex_library::project_file_history(project_dir, rel_path)` helper that walks any `git2::Repository` (not just library-rooted ones). Async-loaded with a generation counter to drop stale results on tab switch. States: not-in-git / no-commits / dirty-only / normal. Working-tree pseudo-card on top when the active path is in `dirty_paths`.
- **Per-class filter in Library Browser sidebar** — clicking a class row in the master-detail layout's left sidebar filters the right-side component grid to rows whose `class` field matches. Clicking the active class clears the filter; `LibraryBrowserState.class_filter: Option<String>` applied alongside `lifecycle_filter`. Active class row renders with the same accent-tint background as the table sidebar's selected row for visual feedback.

### Documentation (2026-05-01)

- **Hardware Requirements** section in `README.md` (addresses #63) — Vulkan 1.1 / DirectX 12 / Metal floor (~2014+ GPU). Older GPUs that fall back to legacy OpenGL may render incorrectly.
- **`docs/RENDERING_RULES.md`** (new public doc) — Apache-2.0 prose describing label rendering, field rotation/justify rules, and IEEE-Std-91 pin shape decorators. Sourced from `crates/signex-types/` (Signex's own `.snxsch` format), Altium parity goals, and public industry standards. Source comments in `signex-render` reference this spec instead of citing third-party EDA tooling.
- **`docs/audit/comments-scrub-2026-05-01.md`** (new audit doc) — records the 2026-05-01 source comment scrub that removed residual KiCad C++ class name references from `signex-render` and `signex-engine` and `signex-output/pdf` even after the v0.10.0 "Standard" rename. Eight comment lines across seven files reworded to neutral descriptions; algorithms unchanged.
- **`docs/internal/CLEANROOM_REWRITE_PLAN.md`** (private submodule) — plan for the v0.12 cleanroom milestone: deletion scope, working rules for the fresh agent session, six rewrite phases, CI guard extensions.

### Changed — license-guard CI

- `no-kicad-cpp-class-names` job (new) forbids `SCH_PAINTER`, `SCH_FIELD::`, `SCH_LABEL`, `SCH_PIN`, `SCH_SYMBOL`, `LIB_SYMBOL::`, `LIB_PIN::`, `sch_painter.cpp` / `sch_symbol.cpp` / `sch_label.cpp` / `sch_pin.cpp` / `lib_symbol.cpp`, and `eeschema/` anywhere in `crates/`.
- `no-derivation-attribution-markers` job (new) forbids `DeepWiki`, `KiCad mirror source`, `extracted from KiCad`, `based on KiCad source` anywhere in the repo (excluding the audit trail and `docs/LICENSING.md`).
- Total License Guard jobs: 6 → 8.

### Changed — minor

- `LibraryCreateOptionsState` and the Enable Version Control modal hoist the `intro` paragraph `format!` out of the view-body function — built once at modal-open time and stored on the state, no per-frame allocation.

### Changed

- **`PinElectricalType` → `PinDirection`** in the new `signex-library` crate to satisfy the License Guard's `no-removed-kicad-api` strict job. Variant set unchanged. Note: this is a different enum from `signex_types::schematic::PinDirection` (which has Signex-original variants); the two coexist as path-qualified `signex_library::PinDirection` vs `signex_types::schematic::PinDirection`. Consolidating them is a follow-up refactor.
- `crates/signex-types/src/library.rs` — the v0.10.0 thin `Library` / `LibraryComponent` types are removed. Library Browser tab content now reads through the `signex-library` adapter trait.
- `assets/samples/library/resistors-standard.snxlib` — removed; the obsolete v0.10.0 sample no longer fits the DBLib data model.

### Documentation

- Three internal-docs plans landed in `docs/internal/docs/` via the private subrepo: `v0.9-library-plan.md` (foundation), `v0.9-library-refactor-plan.md` (primitive split / DBLib shape), `v0.9-refactor-2-plan.md` (table-row + UI layout spec). The older `LIBRARY_PLAN.md` design-intent doc is retired in favour of the concrete implementation plans.
- `docs/audit/history-rewrite-2026-04-29.md` — records the rationale, scope, and audit trail of the 2026-04-29 KiCad-name scrub history rewrite. Cosmetic, not a license remediation; full pre-rewrite chain preserved in the maintainer's backup repo.
- `docs/internal/docs/issue-62-execution-plan.md` and `issue-62-licensing-remediation.md` — strategy docs behind the v0.9.0 Apache-clean cutover, pulled into the canonical internal docs home.

### Provenance

Every commit on the original 169-commit chain is preserved both locally and on origin under:

- `refs/tags/v0.9-snxlib-paused-2026-04-29` (b9eac1f3) — library WIP as paused for the Apache-clean cutover.
- `refs/tags/v0.10-library-cherry-pick-2026-04-29` (012264fb) — fully reconciled post-cutover state, parent of this restore.
- `refs/tags/v0.11-pre-library-restore-2026-04-29` (e8b8e8f5) — branch tip before the squash, preserves the Phase A polish (filter + preview pane scaffolding) that was superseded by the orphan's richer Library Browser.

The squash here is a tree-only restoration; full per-commit authorship and history is reachable via those tags.

### Constraints — Apache-clean invariants (carry forward from v0.9.0)

- Zero `kicad`/`KiCad`/`KICAD` substrings under `crates/` (License Guard's `no-kicad-shaped-symbols` strict job: PASS).
- No `kicad-parser` / `kicad-writer` Cargo deps or imports.
- No removed-API surface re-introduced (`PinElectricalType`, numeric `LayerId` constants, `parse_markup`, `kicad_auto_net_name_from_pins`, `find_kicad_symbols_dir`, …).
- `cargo-deny check licenses` green — every transitive dep is permissive.
- Every PR description carries the self-declaration block (Source basis / LLM-assisted / KiCad source consulted).


## [0.10.0] — 2026-04-29

First slice of the **v0.10 Library & Polish** milestone — the Library Browser tab scaffold. Double-clicking a `.snxlib` file in the project tree now opens a dedicated tab that lists the components contained in the library package; the surface is intentionally read-only this release. v0.10.1 adds the side-by-side symbol preview pane on row click; v0.10.2 adds the filter / search bar above the table.

The library subsystem paused on 2026-04-29 alongside the v0.9.0 Apache-clean cutover (preserved at the `v0.9-snxlib-paused-2026-04-29` tag) is being re-landed file-by-file on top of the Apache-clean foundations rather than wholesale-rebased — every contributing change is verified against the issue #62 invariants before it ships.

### Added

- `signex-types::library::Library` and `LibraryComponent` — in-memory representation of a `.snxlib` package. `LibraryComponent` carries a sentinel-`nil` `symbol_uuid` / `footprint_uuid` for unbound rows; v0.10.8 wires the Pick Symbol/Footprint flow that fills them in.
- `signex-types::format::SnxLibrary` — TOML+TSV envelope for `.snxlib` files. `parse(&str)`, `write_string()`, and `write_string_borrowed(&str, &Library)` mirror the v0.9.1 borrow-based pattern from `SnxSchematic` / `SnxPcb`, so v0.10.6's async-save plumbing drops in unchanged.
- `signex-types::format::LibraryComponentRow` — TSV adapter row implementing `SnxTable` with columns `uuid name value footprint description symbol_uuid footprint_uuid`.
- `signex-app::TabDocument::Library(Library)` variant — Library Browser tabs cache the parsed library on the tab. New `as_library()` accessor; the existing `as_pcb()` is unchanged.
- `signex-app` open path — `open_document_path` and the project-tree handler both route `.snxlib` to a new `open_library_tab(path, title, library)` mirroring `open_pcb_tab`.
- `signex-app::view::view_library_browser` — read-only Library Browser tab body. Header strip (library name + component count + optional description), 4-column scrollable table (Name, Value, Footprint, Description), and an empty-state placeholder when the library has zero components.
- `assets/samples/library/resistors-standard.snxlib` — 3-component sample library shipped with the source tree for the smoke test.

### Changed

- **Project tree click semantics — single click highlights, double click opens.** Previously a single click on a leaf both highlighted and opened the file, which was easy to trigger accidentally while navigating. Now a single click only highlights the row (`panel_ctx.selected_tree_path`); a second click on the same row within 500 ms opens the file. The icon-gate (only schematic / PCB / `.snxprj` / `.snxlib` / `.snxsym` / `.snxfpt` / `.snxsim` leaves open) is unchanged. Right-click → Open in the context menu still opens immediately, bypassing the double-click latch. Folder rows still toggle expand/collapse on a single click. Implementation lives in `signex-app::handlers::dock::project_navigation::handle_dock_project_navigation_panel_message` and routes through a new `open_tree_path_if_document` helper shared with the right-click menu path.

### Apache-clean residual polish (bundled)

In response to ongoing discussion on [issue #62](https://github.com/alplabai/signex/issues/62), v0.10.0 also lands a residual-polish pass that removes vestigial KiCad-shaped names from the codebase. None of these changes alter user-visible functionality, but they reduce the surface that reads as "Signex was once derived from KiCad" — the substantive change was already delivered in v0.9.0; this is the cosmetic follow-through.

- **Style enum variant renames.** `MultisheetStyle::KiCad` / `LabelStyle::KiCad` / `PowerPortStyle::KiCad` → `::Standard` for all three. The `::Altium` variants stay (Altium is the project's stated design reference and isn't a GPL exposure). On-disk preference strings remain `"kicad"` / `"altium"` for backward compatibility with existing `prefs.json` files; the user-facing dropdown labels also stay "KiCad" / "Altium" so users coming from those tools recognise the modes.
- **Legacy KiCad symbol-library scanner removed.** `helpers::find_kicad_symbols_dir` (which walked `/usr/share/kicad/symbols`, `C:\Program Files\KiCad\…`, etc.) and `helpers::list_kicad_libraries` deleted. The associated `DocumentState::kicad_lib_dir` field, `PanelContext::kicad_libraries` field, and the dock-panel `library_browser.rs` handler are gone. The Components panel's library dropdown is replaced by an inline placeholder until the v0.10.x `.snxlib` plumbing repopulates it. The scanner had been a no-op since v0.9.0 (the load handler logged "convert with companion tool" and skipped); removing it cleans up code that pointed at KiCad install layouts without serving a real flow.
- **Direct-open KiCad-extension dispatch arm removed.** `open_document_path` no longer matches `"kicad_pro" | "kicad_sch" | "kicad_pcb"`; the same arm in the project-tree double-click handler is also gone. Opening a `.kicad_*` file now falls through to a generic "unsupported file type" error. The migration story for KiCad users is unchanged — run `signex-kicad-import` first; the README and `docs/LICENSING.md` continue to describe the flow.
- **`docs/LICENSING.md` strengthened.** New "LLM context discipline" section documenting that post-cutover development uses LLM-assisted workflows where KiCad source code is never placed in agent context, prompts, retrieval indexes, or reference material. Versions table updated through v0.10.0.
- **License Guard CI tightened.** Three new jobs added to `.github/workflows/license-guard.yml`: forbid re-introduction of the v0.10.0-renamed names (`MultisheetStyle::KiCad`, `find_kicad_symbols_dir`, etc.); forbid KiCad-numbered layer-id constants (`F_CU = 0`, `B_CU = 31`, …); forbid the `Net-(<r>-Pad<p>)` auto-net-name format string.

### Tests

- `signex_types::format::tests::snxlibrary_round_trip_preserves_components` — locks parser/writer round-trip parity.
- `signex_types::format::tests::snxlibrary_borrow_matches_owned` — owned/borrowed serialise parity.
- `signex_types::format::tests::snxlibrary_rejects_unknown_version` — `UnsupportedVersion` error path.
- `signex_types::format::tests::snxlibrary_parses_empty_components_block` — empty-library round-trip.
- `signex_types::format::tests::shipped_sample_library_parses` — guards the shipped sample against parser drift.

### Constraints — Apache-clean invariants (carry forward from v0.9.0)

- No `use kicad_parser` / `use kicad_writer` in `crates/`.
- No `kicad-parser` / `kicad-writer` Cargo.toml deps.
- License Guard 4 jobs and `cargo-deny` continue to gate every PR.

## [0.9.1] — 2026-04-29

The **async save + borrow-based serialise** patch deferred from v0.9.0. Schematic saves were already imperceptible; this release targets the huge-PCB Ctrl+S stutter (~1–2 s on ~500 K-track boards) by moving the disk write off the UI thread and skipping the full-document clone that the previous serialise required.

### Changed

- `signex-types::format::SnxSchematic::write_string_borrowed(&str, &SchematicSheet)` and the matching `SnxPcb::write_string_borrowed(&str, &PcbBoard)` — borrow-based serialise. The owned `write_string()` methods now delegate to these, so byte-for-byte output is unchanged. Skips the ~50–100 ms `self.sheet.clone()` / `self.board.clone()` that the engine previously paid before each serialise.
- `signex-engine::Engine::serialize_for_save(&self) -> Result<Vec<u8>, EngineError>` — pure, side-effect-free serialise using the borrow path. Cheap to call repeatedly; no path mutation.
- `signex-engine::Engine::write_to_file(path, bytes)` — stateless disk write half of the async-save pair. Pair with `serialize_for_save` to run the write off the UI thread.
- `signex-engine::Engine::record_saved_path(path)` — set the engine's path after an async save resolves.
- `signex-app` save handler — `Ctrl+S` and File → Save now serialise on the UI thread (cheap with the borrow-based path) and dispatch the disk write via `iced::Task::perform`. iced's tokio runtime runs the blocking `std::fs::write` on a worker thread, so the UI stays responsive even on huge boards.
- New `Message::SaveFileFinished(PathBuf, Result<(), String>)` completion arm.
- Status bar shows a small "Saving…" pill for the duration of the off-thread write; transient save errors surface as a 3-second pill before fading.

### Tests

- `signex_types::format::tests::schematic_borrow_matches_owned_serialise` — locks owned/borrowed parity for `SnxSchematic`.
- `signex_types::format::tests::pcb_borrow_matches_owned_serialise` — same, for `SnxPcb`.
- `signex_engine::tests::serialize_for_save_returns_parseable_bytes` — serialise + reparse round-trip.
- `signex_engine::tests::write_to_file_writes_serialised_bytes` — disk write + reparse round-trip via tempfile.

## [0.9.0] — 2026-04-29

The **Apache-clean cutover** release. Resolves [issue #62](https://github.com/alplabai/signex/issues/62) raised by Seth Hillbrand of the KiCad project flagging that several Signex crates derived from KiCad's GPL-3.0 source were shipping under Apache-2.0. The main `signex` repository is now Apache-2.0 clean and contains no KiCad-derived code; KiCad I/O moves to the optional [signex-kicad-import](https://github.com/alplabai/signex-kicad-import) companion tool (GPL-3.0-or-later), shipped independently.

The library subsystem (Library Browser, SCH Library editor, Component Editor) that was in flight on `feature/v0.9-snxlib-as-file` is preserved at the `v0.9-snxlib-paused-2026-04-29` tag and ships as **v0.10.0** on top of the Apache-clean foundations from this release.

### Native file formats — TOML + TSV bulk blocks

- **`.snxsch`** schematic format — TOML manifest (`format = "snxsch/1"`) wrapping the `SchematicSheet` payload via 4 adapter row tables (`[sheets.components]`, `[sheets.wires]`, `[sheets.junctions]`, `[sheets.labels]`) plus an `[extras.*]` TOML block for fields that don't fit a flat row schema. Same format family as `.snxlib` / `.snxsym` / `.snxfpt`.
- **`.snxpcb`** PCB format — same shape with adapter rows for `[footprints]`, `[pads]`, `[tracks]`, `[vias]`, plus `[stackup]` / `[[zones]]` in regular TOML for hierarchical data.
- Format chosen for line-diff-friendly git workflows, ~5× smaller than the equivalent JSON, single file per design, and zero S-expression-shaped grammar that could regress KiCad-derivation exposure.
- Round-trip preserves every field on `SchematicSheet` / `PcbBoard`.

### Apache-clean signex-types

- `PinElectricalType` (12-variant, KiCad-shaped) → **`PinDirection`** (14 variants — adds Signex-original `GroundReference`, `Differential`, `Clock`; collapses `Free`+`Unspecified` into `Unclassified`; renames `OpenCollector`/`OpenEmitter` → `OpenDrainLow`/`OpenDrainHigh`). Design rationale in `crates/signex-types/docs/pin-design.md`.
- `PinShape` (9-variant) → **`PinShapeStyle`** (7 variants — drops per-direction-low modifiers since polarity is now on `PinDirection`; adds `HysteresisInput` / `HysteresisOutput` / `Schmitt`).
- KiCad-numbered `LayerId(u8)` constants (`F_CU=0`, `B_CU=31`, …) → **`SignexLayer`** semantic enum + `LayerKind` categories + `altium_label()` per the Altium-flavoured Signex UI naming.
- KiCad markup parser (`~{X}` / `^{X}` / `_{X}` curly-brace syntax) → **`parse_signex_markup`** using a Markdown subset: `**bold**`, `*italic*`, `~~strike~~`, `^superscript^`, `~subscript~`, `_~overbar~_` (Signex extension for active-low signal naming), `[label](url)`, `\X` escape.
- `kicad_auto_net_name_from_pins` (`Net-(<r>-Pad<p>)` format string) → **`auto_net_name`** returning `unnamed-<sheet>:<ref>:<pin>`.

### Crates removed from main repo

- `crates/kicad-parser/` (3,938 LOC) — moved to the GPL-3.0 companion repo.
- `crates/kicad-writer/` (2,274 LOC) — moved to the GPL-3.0 companion repo.
- `crates/signex-output/src/netlist/kicad_sexpr.rs` (336 LOC) — KiCad netlist exporter; moved to the companion or a future `signex-kicad-export` sibling.

### Companion repo `signex-kicad-import` (GPL-3.0-or-later)

- New separate repository at <https://github.com/alplabai/signex-kicad-import>.
- Houses the relocated `kicad-parser` + `kicad-writer` crates plus a CLI binary `signex-kicad-import`.
- One-way conversion: `.kicad_sch` / `.kicad_pcb` / `.kicad_pro` → `.snxsch` / `.snxpcb` / `.snxprj`. Originals remain intact.
- Distributed independently — Apache consumers of Signex Community see no GPL aggregation in their build closure.

### Performance (deferred to v0.9.1)

**Async save** and **borrow-based serialise** were planned for v0.9.0 but
are deferred to **v0.9.1** to bound the v0.9.0 release scope. The
current `Engine::save_as` runs synchronously on the calling thread
with a full document-clone before serialisation; on huge PCBs (~500K
tracks) this blocks the UI for ~1–2 s on Ctrl+S. Tracked as a v0.9.1
follow-up:

- `Engine::save_as_async` returning an `iced::Task` that serialises +
  writes off the UI thread.
- Borrow-based `SnxSchematic::write_string_borrowed(&SchematicSheet)`
  to skip the document-clone (~50–100 ms saving on huge PCBs).

Schematics save instantly today; the perf work is targeted at the
high-end PCB case.

### CI guards

- `.github/workflows/license-guard.yml` — fails any push or PR that re-introduces `kicad-parser` / `kicad-writer` imports or removed KiCad-shaped types.
- `.github/workflows/pr-license-declaration.yml` — fails PRs whose description is missing the contributor self-declaration block or whose author marked `KiCad source consulted: yes`.
- `cargo-deny` license job in `.github/workflows/ci.yml` — rejects GPL / AGPL / LGPL / unlicensed transitive dependencies.
- `deny.toml` — Apache-compatible permissive allowlist.

### Documentation

- `docs/LICENSING.md` — canonical licensing rationale + audit-trail pointers + contributor guidance.
- `docs/audit/kicad-derivation.md` — file-by-file audit of every removed item plus the residual-mention catalog.
- `docs/audit/contributors-2026-04-29.md` — contributor consent record (audit trail).
- `docs/audit/third-party-kicad-parsers.md` — survey of clean-room third-party Rust KiCad parsers.
- `docs/audit/release-notes-remediation-v07-v08.md` — text to apply manually to v0.7.0 / v0.7.1 / v0.8.0 GitHub Release bodies (those releases shipped Apache-2.0 with KiCad-derived code in error; flagged superseded).
- `docs/audit/communication-drafts.md` — drafts of the issue #62 reply, signex.dev hero copy, GitHub Discussions sticky, Discord post.
- `CONTRIBUTING.md` — added License compliance section + PR self-declaration block.
- `crates/signex-types/docs/pin-design.md` — rationale for the curated `PinDirection` / `PinShapeStyle` variant sets.

### Breaking changes

- `.kicad_sch` / `.kicad_pcb` / `.kicad_pro` files no longer open directly in Signex. Users with existing KiCad projects install the [signex-kicad-import](https://github.com/alplabai/signex-kicad-import/releases) companion tool, run it once against their `.kicad_pro`, and open the resulting `.snxprj` from then on.
- `signex_types::schematic::PinElectricalType` and `PinShape` are gone — downstream code uses `PinDirection` / `PinShapeStyle`.
- `signex_types::layer::{F_CU, B_CU, F_SILKS, …}` constants are gone — downstream code uses `SignexLayer::*` variants.
- `signex_types::markup::parse_markup` and `kicad_auto_net_name_from_pins` are gone — downstream code uses `parse_signex_markup` and `auto_net_name`.
- `crates/kicad-parser` and `crates/kicad-writer` are gone from the workspace.

### Versions affected

The v0.7.0 / v0.7.1 / v0.8.0 release notes have been edited to flag those releases as superseded with the licensing notice. Binaries remain available for historical use; please prefer v0.9.0 (or later) for new installations.

## [0.8.0] — 2026-04-27

The output-and-polish release. Adds the full PDF / BOM / netlist export pipeline, multi-project workspaces, Altium-style dirty tracking, a chrome refactor with the new `TabPill` widget, hierarchical-sheet rendering parity, and KiCad-parity field autoplace. Every v0.8.x sub-feature ships under this one tag.

### Output subsystem

- **PDF export** — tabbed Export modal (file picker / settings / pan-drag preview), bookmarks per sheet, theme palette aware, DPI hookup, physical-structure tokens
- **BOM** — Altium-spec preview modal with column picker, variant picker, sort / drag / scroll, options applied on export, format-layer export (CSV / HTML / XLSX-ready)
- **Netlist** — date + path polish, KiCad-format export, output engine validation
- Unified PDF preview modal — File ▸ Export PDF and File ▸ Print Preview both open the same overlay; legacy `view_pdf_options_dialog` + 12 `ExportPdfSet*` / `ExportPdfDialog*` variants pruned

### Multi-project workspace

- Multiple `.snxprj` projects open side-by-side in the same window
- `TabInfo` carries `Option<ProjectId>`; `active_project` scoped to focused tab
- Accent-tinted active project root + per-project Close menu action
- Per-tree-path semantics for project tree actions (right-click on project B with project A active still operates on B)
- Phase 2.5 cleanup — legacy single-project fields removed from `DocumentState` (#54, #55, #56)

### Dirty tracking + tab right-click menu

- Altium-style `dirty_paths` model — closing a tab never prompts; engines park while dirty
- Project-close prompts with a Save All / Discard All / Cancel modal listing every dirty file
- Tab bar right-click menu replaces the inline close / undock buttons

### Hierarchical sheets

- Child-sheet pins rendered as Altium-style ports (no protruding stubs), inward direction
- Pin labels rotate vertical on top / bottom edges for parity
- Per-sheet stroke / fill colours round-trip and editable from the Properties panel
- `[[multisheet]]` Style preference (with sheet-alpha round-trip fix)
- Child-sheet name / filename rendered outside the box per multisheet style
- Inline preset palette under the colour row before opening the picker
- Altium-green default sheet palette

### Symbol field handling (KiCad parity)

- Autoplace fields on rotate / mirror — body-bbox classification, full text-height clearance, anchored to selection bbox including pins, two-text-height autoplace clearance, body-edge stack alignment
- Reference and Value rotate independently; rotate / mirror compose with symbol orientation
- Property `justify` mirrored under rotation fold and mirror flags
- KiCad `GetDrawRotation` toggle parity; symbol field rotation treated as absolute screen angle
- Re-autoplace marked fields on load to repair legacy rotations
- Property `justify` parsing defaults to Center per KiCad spec
- Pin numbers rotate along the pin axis for vertical pins
- Single em-size used for schematic text (parity with KiCad)

### Chrome refactor

- New `TabPill` custom widget — 3-sided borders sharing L / R edges between adjacent tabs, permanent strip baseline, theme-border instead of pure black, drag accent follows theme accent
- Modal close button unified across all 11 modals; chrome icon sizes bumped; thicker panel strokes; modal alignment so borders trace rounded corners
- Inactive tab fill is now visible (was theme-bg, blending in)
- New `chrome-catalog` crate + UI iteration workflow doc
- Taller top-chrome search bar (24 → 28); narrow Justification labels in Properties

### Canvas + engine

- Adaptive multi-level grid that scales smoothly with zoom
- Schematic editor grid style preference

### Plumbing

- `kicad-parser` / `kicad-writer` round-trip per-sheet stroke / fill colours and respect label style for defaults
- `signex-types::Label` gains `justify_v` to match renderer + parser contract
- Engine command surface expanded with multi-project routing

### Issues closed

- #54 multi-project workspace
- #55 multi-project + chrome polish + unified PDF preview
- #56 phase 2.5 cleanup of legacy single-project fields

[Full changelog](https://github.com/alplabai/signex/compare/v0.7.1...v0.8.0) · [Release artifacts](https://github.com/alplabai/signex/releases/tag/v0.8.0)

## [0.7.1] — 2026-04-24

Patch release addressing a macOS launch failure on Apple Silicon.

### Fixed

- **macOS (Apple Silicon) cannot launch the shipped `.app`** (#49). The
  DMG-packaged bundle was unsigned; arm64 macOS refuses to execute any
  binary without at least an ad-hoc signature, so users on M-series
  Macs saw "Signex is damaged and can't be opened" / "cannot be
  verified" immediately after dragging the app to Applications. The
  installer script now ad-hoc signs the bundle (`codesign --force
  --deep --sign -`) as part of DMG assembly. This is the minimum
  viable shipping state for arm64 until a Developer ID certificate
  and notarisation credentials are wired into CI.

### Known issues / workarounds

- The DMG still carries the downloaded-from-internet quarantine flag,
  so first-launch users will see a "cannot be verified" Gatekeeper
  prompt. Bypass it with **right-click → Open** on the app icon the
  first time, or run
  `xattr -dr com.apple.quarantine /Applications/Signex.app` in
  Terminal. Subsequent launches work without prompts.

## [0.7.0] — 2026-04-22

The schematic-phase release. Adds ERC & validation, project-wide annotation, real multi-window architecture via `iced::daemon`, per-window engine/canvas, borderless chrome, and a full Signex brand rollout. Every v0.7.x sub-feature ships under this one tag.

### ERC & validation

- New `signex-erc` crate with **11 rule kinds** (`run()` single-sheet, `run_with_project()` cross-sheet)
- Project-wide ERC across open, cached, and unopened sheets
- Cross-sheet BadHierSheetPin: parent pins ↔ child hier-labels, both directions
- ERC markers rendered as overlays — zero KiCad schema drift
- Messages panel with E/W/I severity pips and per-sheet cached results
- Altium-style ERC dialog + Preferences panel (per-rule Error / Warning / Info / Off grid)
- `ErcContext` abstraction + rule metadata infrastructure
- Shortcuts: `F8` Run ERC, `F9` AutoFocus, `Alt+A` Annotate, `Shift+Alt+A` Reset & Renumber

### Annotation

- Project-wide change list parsed from every sheet in the project
- Altium two-column Annotate dialog with Reset All / Reset & Renumber / Reset Duplicates
- Per-symbol lock, draggable modals
- Power-port skip in both annotation and change list
- Design → Annotation submenu matching Altium's layout

### Multi-window (`iced::daemon`)

- Borderless main window + OS-decorated secondary windows
- Modals, tabs, and panels all detach into real OS windows
- Per-window engine: `document_state.engines: HashMap<PathBuf, Engine>` — every open tab keeps its Engine loaded so undocked tabs edit independently
- Per-window canvas: `interaction_state.canvases: HashMap<window::Id, SchematicCanvas>` — pan / zoom / selection / render cache per window
- `document_state.window_active_path: HashMap<window::Id, PathBuf>` — each undocked-tab window can show a different tab
- Canvas events routed via `CanvasEventInWindow { window_id, event }`
- macOS: process exits when main window closes

### Editor & tools

- Lasso freehand select (Altium), bbox child-sheet hit, detached-field support
- Tab / panel drag-reorder with visual feedback
- Reorder picker polish — gray-X cursor while armed, Esc-cancel visible
- Unified gray-X placement cursor across every armed tool
- TAB-during-placement for Line / Rect / Circle / Arc / Polygon (width + fill pre-configured)
- Editable drawing Properties panel with live DrawingPreview canvas
- Stroke colour per drawing (round-trips in KiCad format)
- Erasable numeric `text_input` (per-field `String` buffer)
- Context-aware menus — Annotate / ERC / Save / Edit gated by `has_schematic` / selection
- Net-colour pen — strict-hit snap, union-find flood, render-only overrides, undo stack
- Move / z-order engine commands — `MoveSymbolAbsolute`, `ReorderObjects` (Front / Back / JustAbove / JustBelow)
- Active Bar — BringToFront / SendToBack + BringToFrontOf / SendToBackOf pickers
- Reset Duplicate Designators (project-wide, undoable for open tabs)
- Arc (3-click) + Polygon (click-by-click) placement tools

### Hierarchical sheets

- Double-click a sheet block → opens the child schematic
- Sheet-pin snaps to all four edges of the hierarchical sheet block
- Sheet-pin interactions + Altium label-style option

### Borderless chrome & brand

- Custom title bar: wordmark + menus + drag zone + search bar + min / max / close
- Per-monitor v2 DPI manifest (no bitmap stretching on hi-DPI)
- Roboto UI font (panels / toolbars / menus / dialogs); Iosevka stays the canvas font
- Windows 11 DWM rounded corners + drop shadow via `DwmSetWindowAttribute` (silent no-op on Win10 and non-Windows)
- Diagonal resize hit zones (NW / NE / SW / SE) via Stack overlay — keeps content y-origin natural
- Header logo bumped 74×24 → 96×31 for readability

### Icons & installer

- Signex brand SVGs (mark, wordmark, logo variants)
- Panton Bold wordmark regenerated from actual font (not fallback outlines)
- Tighter `signex-mark.svg` viewBox: S fills ~97 % of icon canvas (was ~58 %)
- Regenerated installer artifacts: Windows `.ico` (multi-size), macOS `.icns`, Linux PNGs
- Runtime window icon embedded via `iced::window::Icon`
- `signex.exe` icon + DPI manifest embedded at build time via `winres`
- Pure-Python fallback for `build-icons.sh` — `tools/build_icons.py`, no rsvg-convert / magick / inkscape needed

### Refactors

- `signex-engine/src/lib.rs` split into semantic modules
- `kicad-writer` migrated from `wln!` string formatting to full SExpr AST (`kicad-parser/sexpr_builder` → `kicad-writer/sexpr_render`)
- Named constants for PCB magic numbers; `lib_symbol` unit round-trip fix
- Wire rendering chains connected segments into polylines (rounded corners)
- Font-size constants corrected, hidden refs and pin-name rotation fixed

### Merged-in dependencies

- v0.6.1 render fixes (font scale, pin numbers, power-ref visibility)
- v0.6.2 AST sexpr pipeline
- v0.6.3 macOS runner pin (`macos-14`)
- v0.6.4 per-OS installers (`.exe`, `.dmg`, `.deb`, `.AppImage`)
- Node.js 24 Actions bump

### Performance

- `expand_to_net` is now `O(N)` via quantised `HashSet` (was `O(P²·N²)`)

[Full changelog](https://github.com/alplabai/signex/compare/v0.6.4...v0.7.0) · [Release artifacts](https://github.com/alplabai/signex/releases/tag/v0.7.0)

## [0.6.4] — 2026-04-20

First cumulative release since v0.6.1. Rolls in the abandoned v0.6.2 and the CI-fix-only v0.6.3.

### Installers (new)

- **Windows** — `signex-setup-x86_64-0.6.4.exe` / `signex-setup-aarch64-0.6.4.exe` via InnoSetup. Installs to `Program Files`, adds Start Menu entry and optional Desktop shortcut, proper uninstaller. Portable `.zip` also attached for scripted installs.
- **macOS** — `signex-macos-aarch64-0.6.4.dmg` with a full `Signex.app` bundle and `/Applications` drag-target. Registered as the editor for `.kicad_sch` / `.kicad_pro` files.
- **Linux** — native `.deb` (with `.desktop` entry + MIME types) plus a portable `.AppImage`. `.tar.gz` fallback also attached.

### KiCad pipeline refactor (from v0.6.2)

- AST-based S-expression pipeline in `kicad-parser` / `kicad-writer` with a property-metadata layer. More robust round-trip, less fragile than prior ad-hoc string handling.
- Named constants replace the magic numbers scattered through `signex-types`, `kicad-parser`, and `kicad-writer`.

### Release pipeline fixes (from v0.6.3)

- `aarch64-apple-darwin` pinned to `macos-14` so GitHub's `macos-latest` queue stalls don't take the whole release down (symptom that killed v0.6.2).

### Upgrade notes

No `.kicad_sch` / `.kicad_pcb` breaking changes — opening a v0.6.1 project in v0.6.4 is a clean round-trip. If you were on v0.6.2 or v0.6.3, nothing additional to migrate — v0.6.4 is a superset.

[Full changelog](https://github.com/alplabai/signex/compare/v0.6.1...v0.6.4)

## [0.6.3] — 2026-04-20 _(superseded by 0.6.4)_

- fix(ci): pin aarch64-apple-darwin to macos-14 (#34)

[Full changelog](https://github.com/alplabai/signex/compare/v0.6.2...v0.6.3)

## [0.6.1] — 2026-04-20

Render + KiCad round-trip fixes (font scale, pin numbers, power-ref visibility).

[Full changelog](https://github.com/alplabai/signex/compare/v0.6.0...v0.6.1)

## [0.6.0] — 2026-04-18

Full Schematic Editor — drag-move, properties editing, placement tools, iced_aw, Active Bar.

[Full changelog](https://github.com/alplabai/signex/commits/v0.6.0)
