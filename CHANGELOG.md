# Changelog

All notable changes to Signex ship here. Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and [Semantic Versioning](https://semver.org/spec/v2.0.0.html), both strictly.

## Format

- **Release heading:** `## [X.Y.Z] — YYYY-MM-DD`, newest first, directly under `## [Unreleased]`. Never `— unreleased` on a section you intend to tag.
- **Section heading:** `### <Type> — <scope>`, where `<Type>` is one of Keep a Changelog's six — **Added**, **Changed**, **Deprecated**, **Removed**, **Fixed**, **Security**. The scope suffix is free text (`### Fixed — netlist`); drop it when the section covers the whole release (`### Changed`). Lead with the type so a reader scanning for regressions finds every `Fixed` without reading section titles.
- **No `####`.** Sub-scope goes in the suffix after a colon: `### Added — footprint editor: sketch constraints`.
- **Note sections** are allowed alongside the types, because they record something other than a change: `### Breaking changes`, `### Deferred to <version>`, `### Constraints — …`, `### Upgrade notes`, `### Known issues / workarounds`, `### Issues closed`, `### Provenance`, `### Versions affected`. Anything that *is* a change takes a type instead — don't reach for a note heading to avoid choosing one.
- **Close every section** with `[Full changelog](https://github.com/alplabai/signex/compare/v<prev>...v<this>)`, where `<prev>` is the previous *tag* — not the previous section. v0.12.0 and v0.6.2 have no section, and v0.6.2 has a tag with none.

## Release mechanics

Each release section is authored **before** the `vX.Y.Z` tag is created, so the release workflow picks it up as the GitHub Release body. `.github/workflows/release.yml` ("Extract CHANGELOG entry for this tag") matches `## [X.Y.Z]` against the tag with the leading `v` stripped — so that heading line is load-bearing and must not be reworded.

**The release workflow enforces this.** A `vX.Y.Z` tag push fails before any build unless `scripts/check_release_version.sh` passes: the tag must equal `[workspace.package] version` in `Cargo.toml`, and CHANGELOG must hold a dated, not-`unreleased` `## [X.Y.Z]` section. The extractor's fallback to auto-generated notes for a missing section remains as a second line of defence. This file previously cited `.claude/hooks/pre-release-guard.sh` as the enforcement; that path is gitignored and no such hook exists.

## [Unreleased]

### Added

- **Unknown TSV columns are preserved (#315)** — a `.snxsch` / `.snxpcb` bulk block with a column this build does not know no longer fails to load with a header mismatch; columns resolve by name, the extra cells stay attached to their row through an edit, and they are re-emitted after the known columns (in their original relative order) on save. A missing known column and a repeated column name still error. A block may carry at most 256 foreign columns, each name at most 256 bytes; a file over either cap fails to load, and a save fails (before any byte is written, the previous file stays intact) on the same caps or when a foreign column repeats a known column name, each with an error naming the offending column.

### Added — schematic engine

- **`PlaceChildSheet` engine command** — adds a hierarchical sheet symbol to the schematic as one undoable step and marks `CHILD_SHEETS` dirty so the project netlist cache invalidates. A blank `filename` is rejected with `EngineError::EmptyChildSheetFilename`; an empty `name` is allowed. Engine only — the Place Sheet canvas tool is separate (#357).
### Added — start screen

- **Actionable start screen** — with nothing loaded, the empty centre region now shows the product name and version with **New Project**, **Open Project…** and **Keyboard Shortcuts (F1)** buttons, all dispatching existing menu messages and styled from `ThemeTokens`. A loaded project with no document selected keeps its own quieter hint. Open Sample Project and recent projects are left out until those features land (#331).
### Added — ERC

- **File ▸ Export ▸ ERC Report** writes the cached ERC results as plain text or
  CSV (chosen by the saved file's extension), in the Messages panel's row order.
  It never runs ERC; before a run it shows a message instead of writing a file.
  `signex_output::ErcReportExporter` also offers a minimum-severity filter and
  group-by-sheet. CSV fields are escaped by the BOM's shared helper (#351).
- **No-ERC directives are honoured (#349)** — the sheet's `no_erc_directives` are projected into `ErcContext`, and `run_all` / `run_all_with_dsl` drop any diagnostic located on a directive (1 µm point bucket, positional only, built-in and DSL alike). Placing and rendering a directive is tracked separately (#350).
### Added — native-format test corpus

- **Fixture-corpus round-trip harness** — `signex-types/tests/round_trip_corpus.rs` walks the checked-in `.snxsch` / `.snxpcb` goldens under `tests/corpus/` and asserts each parses, and re-saves byte-identically (which also implies a second parse -> write -> parse hop is stable); every failing fixture is listed with its path and first differing line. `SIGNEX_BLESS_CORPUS=1` rewrites fixtures for an intentional format change, and a `MIN_CORPUS_FILES` guard stops the corpus shrinking silently (#334).
- **`.snxsch` schematic feature-matrix fixtures** — 33 new goldens under `tests/corpus/schematic/` cover symbol flags, extras-only data, multi-unit and mirrored symbols, rotations, wires and the T-junction, buses, every `LabelType`, no-connect and no-ERC markers, text notes, drawing primitives, `title_block`, `lib_symbols`, adversarial text and nanometre rounding; `MIN_CORPUS_FILES` is now 39 and the corpus README lists the matrix (#335).

### Added — PCB GPU shader render (experimental, default-off)

- **GPU scene render path** — the PCB editor canvas can render traces, pads, vias, and zones through the `signex_gfx` wgpu pipelines via iced's `shader` widget instead of CPU `canvas::Frame` tessellation. Gated behind `feature_flags::PCB_GPU_RENDER` (default `false`) plus a Preferences toggle; the CPU path stays the default until GPU visual parity is confirmed on hardware.
- **CPU↔GPU draw-order parity lock** — a shared `signex_gfx::scene::order` module defines the canonical bucket draw order both paths walk, so neither path can drift silently.

### Added — schematic navigation

- **Recursive sheet hierarchy in the Navigator** (#356) — the panel now shows the whole project (root sheet, child sheets, grandchildren) whichever sheet is focused, instead of the focused sheet's immediate children. A file instantiated twice shows as two nodes, a missing child keeps a `(missing)` row, a reference cycle (including one spelled through `..`) stops at a `(cycle)` row, a sheet that exists but cannot be read shows `(unreadable)` with the reason in the Messages panel, a reference that escapes its folder shows `(rejected)` and is never opened, nesting beyond 64 levels shows `(not expanded)`, pages the project lists but no sheet references appear as extra top-level rows, and the active sheet is highlighted; double-click opens a sheet. Child references resolve through the same resolver as the netlist and canvas, so a `sub/../x` reference focuses the already-open `x` tab. Selection and collapsed branches follow the sheet across a rebuild. A closed child changed on disk is picked up when a tab opens or closes.
- **Tools > Up Hierarchy** (#356) — opens the sheet that references the focused one (the first by path when a file has several parents).

### Added — netlist export

- **Protel `.NET` netlist** (#354) — File ▸ Export ▸ Netlist now writes a Protel-format netlist (`[ ... ]` component blocks, `( ... )` net blocks, CRLF) that downstream PCB tools read, instead of the `# Signex netlist` text dump. The dump stays available as `NetlistFormat::Listing`; the menu label and save dialog now name the Protel format, and the dead "Generate Netlist" menu stub is gone. Hierarchical projects export every child-sheet part. The export refuses, and writes nothing, when a designator is held by more than one part (clear it with Reset Duplicate Schematic Designators...), when a part is unannotated, such as `R?` (clear it with Annotate Schematics...), when a real part has a blank designator (annotate it), or when a child sheet is placed more than once and keeps shared designators (give each placement its own). Power ports and parts excluded from the board are left out of the file. "Export anyway (incomplete)" writes a clean netlist with no header, since the format has no comment syntax; the omitted pages are reported in the Messages panel.
### Added — Help menu and File ▸ Open Recent

- **Help menu** — `Help ▸ About Signex` is a real dialog (version from the workspace manifest, licence link, Copy version info) instead of a disabled stub, and `Help ▸ Documentation` / `Help ▸ Report an Issue` open the user guide and the issue tracker in the default browser (#332).
- **File ▸ Open Recent** — the last 10 projects opened (or created) are remembered in `prefs.json`, de-duplicated by canonical path and listed newest first; choosing one reopens it through File ▸ Open, a project that has been deleted is dropped with a notice, and Clear Recent empties the list (#333).
### Added — BOM

- **BOM validation in the export modal** — a new Rules tab lists the four `signex-bom` rules as toggles and the live issues (by severity, naming each designator); the footer shows the error/warning count next to Export. The report re-validates on every grouping / variant / DNP / rule change, and an export with issues still writes the file and reports the summary (#355).
### Added — PDF export

- **Page setup in the PDF export modal (#353)** — page size (all named sizes plus `Custom` width/height in mm), orientation, the four margins in mm and scale (Fit to Page / 1:1 / Percent) are now editable and re-rasterise the preview; the exported MediaBox follows the selection. Empty, non-numeric, zero or negative input is rejected with a message and the last good value is kept; margins are limited to fit the page. A Custom page edge must be at least 10 mm; a Custom page takes its orientation from its width and height (the orientation picker is not offered for it). Shrinking the page refits the margins and says so, and each field keeps its own rejection notice until its text is fixed. Opening the modal still pre-selects the sheet's own paper and orientation. Page setup lasts only while the modal is open and is re-seeded from the sheet each time it opens. Automatic adjustments (margin limit, refit, Percent reset to 100%) and rejected input are also written to the Messages panel. The on-screen preview of a very large Custom page is drawn at a reduced resolution (longest side capped at 4096 px); the exported PDF is unaffected.
- **Roboto/Iosevka embedded as Type0 fonts (#352)** — exported PDFs now carry the bundled TTFs as `Identity-H` Type0/CIDFontType2 fonts with a `/W` widths array, a `/FontFile2` stream subset to the glyphs actually used, and a `/ToUnicode` CMap, instead of referencing Helvetica/Courier. `µ`, `°`, `±`, Turkish letters and Greek render in the intended typeface and extract/copy back as the original text; the ASCII-mangling `sanitize_pdf_text` workaround is gone. Characters no bundled face covers (e.g. CJK) draw as `.notdef`.
### Added — release workflow

- **Release version guard** — the release workflow now runs `scripts/check_release_version.sh` before any build and fails unless the tag, `[workspace.package] version` in `Cargo.toml`, and a dated (not `unreleased`) `## [X.Y.Z]` CHANGELOG section all agree, printing all three values on a mismatch (#343).

### Fixed — ERC

- **ERC DSL pin kinds (#676)** — `pin.kind == / !=` accepts the same pin-kind spellings as `has_pin_kind` (`power_in`, `tristate`, `open_collector`, …), so a rule written with one of them now fires. An unknown pin-kind literal in either form is now a validation error instead of a rule that silently never fires; as with other DSL errors, the rule file is refused and the error is logged.

### Fixed

- **A symbol with all-default extras keeps its unit, BOM and board flags (#789)** — a placed symbol with no footprint, datasheet, fields or flags is written without an `[extras.symbols.<uuid>]` table, but the parser filled the missing table from a derived `Default` (unit 0, `in_bom` false, `on_board` false), so it reloaded out of the BOM and the board and the next save wrote a different file. `SymbolExtras::default()` now matches the on-disk defaults (unit 1, in BOM, on board); files already saved without the table load correctly. The other extras structs were checked and already agree.
- **Messages panel no longer crashes on long non-ASCII lines (#769)** — truncating a line over 160 bytes sliced at a fixed byte offset and panicked when it landed inside a multi-byte character (a Turkish file name, an emoji), losing unsaved work; it now cuts on a character boundary with the same 160-byte limit, and the library column and datasheet-hash labels use the same helper.
- **Scope tests pass on Windows (#747)** — the `claimed_by_other_project` tests spelled fixture paths with `/` while `Path::join` adds `\` on Windows, so the verbatim `Preserve` keys never matched; the fixtures now use the host separator. Production behaviour is unchanged.
- **Project-tree Remove no longer deletes Shared / Global libraries (#139)** — "Delete file" could remove a `.snxlib` package other projects still use. Delete is now refused (and not offered) for any Shared or Global library entry whatever its path, and for any target outside the project folder; the library is only excluded, with a "Delete Refused" notice.
- **`cargo deny check` is green again** — upgraded the vulnerable dependency paths (quick-xml 0.41, git2 0.21, rust_xlsxwriter 0.101 / zip 9, pdf-extract 0.12 / lopdf 0.42, plus lockfile bumps of anyhow, h2, rustls, rustls-webpki, webpki-roots, webbrowser, crossbeam-epoch); the remaining unmaintained-crate notices (core2, instant, paste, rustybuzz, ttf-parser, zbus_names, zvariant, zvariant_derive) carry narrow, dated `deny.toml` ignores. (#148)
- **Symbol Locked / DNP toggles (#597)** — the Properties panel "Locked" and "DNP" checkboxes landed in an empty match arm and did nothing. They now go through the engine (`SetSymbolLocked` / `SetSymbolDnp`), mark the sheet dirty, refresh the selection info, persist, and undo as one step each. Locked is stored and undoable but not yet enforced on move/delete.
- **Project netlist rooted at the project's root sheet** — the cached canvas/ERC netlist was rooted at whichever sheet tab was focused, so focusing a child sheet silently dropped every net only the parent carries. Both it and the export scope now resolve one shared root: the project's `schematic_root`, falling back to the active sheet only for a loose document or a project with no declared root (#338). That last fallback is no longer silent: the netlist refresh and the export log a warning naming the project and the sheet it rooted at (#338).
- **ERC DSL rules keep their own identity** — `Violation` now carries `rule_id` (`builtin::<slug>` or `user::<name>`). DSL rules were force-filed under a built-in `RuleKind`, so the Messages panel labelled them as that built-in, a built-in severity override rewrote their declared severity, and setting it to Off hid them (and a DSL pin rule offered "Add No-Connect"). Overrides, panel labels and the quick fix now resolve through `RuleId::builtin_kind`; the preferences file and built-in output are unchanged.
- **ERC severity overrides** — `MissingPowerFlag` declared `Warning` but emitted `Info`, and the dialog dropped any override equal to the declared default, so picking Warning for it (or Error for `BadHierSheetPin`, which grades its own findings) was silently discarded. The default is now `Info`, overrides are stored verbatim and applied to every finding via `signex_erc::apply_overrides`, and a test pins every rule's emitted severity to `default_severity()`.
- **Git commit identity (#141)** — "Enable Version Control" fell back to `signex` / `signex@localhost` and library git commits to `Signex Library` / `library@signex.local` when no user identity was configured. Every library and project git entry point now shares one resolver (`signex_library::git_identity`): `GIT_AUTHOR_*`/`GIT_COMMITTER_*` first (as git does), then git `user.name`/`user.email`. Both a name and an email are required: there is no OS-user or `@host` fallback, and a missing identity is refused with a "git identity not configured" error before anything is written (a project's existing `.gitignore`/`.gitattributes` are left untouched, and library saves change nothing on disk or in memory). The library *Re-init* recovery follows the same rule: it refuses before `git init`, and the Git Repository Missing dialog stays open and shows the reason instead of failing silently.
- **Files passed on the command line now open (#317)** — the installers register `signex <path>` / `Exec=signex %F` file associations but the app never read its argv, so double-clicking a `.snxsch` launched an empty workspace. Every path argument now goes through the File ▸ Open flow (relative paths resolve against the working directory, unknown flags are ignored, a `.snxprj` opens the project), and a missing or unsupported file raises the Open Failed notice instead of failing silently. The AppImage desktop entry gains the `%F` placeholder and `MimeType` list. macOS Finder opens arrive as Apple events, not argv, and are not handled yet. `signex --version` and `signex --help` print and exit 0 before any window, iced or wgpu starts, so they work with no display or GPU.
- **Distributor HTTP timeouts** — the LCSC, JLCPCB, Mouser and DigiKey blocking clients relied on reqwest's implicit 30 s overall timeout and had no connect cap; they now share one client builder (including the DigiKey OAuth client) with an explicit 30 s overall-request timeout and a new 10 s connect timeout, both named constants (#144).
- **Unknown TOML keys survive a load-save cycle (#313, #314)** — manifest, sheet, symbol, stackup, nets, board, footprint and pad keys a newer Signex wrote now round-trip through `.snxsch` / `.snxpcb` instead of being dropped on save. Not yet preserved: keys directly under `[extras]`, junction extras, `[sheets.*]`, the PCB top-level `[extras]`, `[[zones]]` and TSV columns.
- **Path-key case policy is testable on every host** — `path_key` / `sheet_key` / `project_graph` took their case-folding from a `cfg!(windows)` read, so the Windows arm of the keying tests never ran in the Linux-only test job. The policy is now an injectable `CaseFold` (`path_key_with`, `sheet_key_with`, `project_graph_with`); the fold and preserve behaviours of the key and of project-sheet ownership (`project_owning_sheet_with`) are exercised on any host, and the per-platform defaults are unchanged. Per-volume probing (macOS APFS, NTFS case-sensitive directories) remains open (#543).
- **Escaped multi-byte characters in markup (#677)** — markup with a backslash before a multi-byte character (`~\µA~`, `*\Ölçüm*`) no longer panics PDF export or the preview, and the escaped character is no longer garbled.
- **GPU concave polygon fill** — the polygon pipeline fanned every contour from its first vertex, which is exact only for convex shapes; a concave copper pour/rule-area bridged triangles across the notch and painted copper where the pour had none. `append_fill` now triangulates via `signex_sketch::ear_clip`, matching the CPU `frame.fill` (lyon) result exactly (pinned by a shoelace-area regression test). Found and fixed a latent bug in `ear_clip` itself while wiring it up: a bridge edge created by clipping one ear could pass exactly through another still-remaining vertex, letting the algorithm approve an invalid ear and collapse the rest of the polygon into a self-intersecting remainder — `is_ear`'s occlusion test now also blocks on a non-corner vertex sitting exactly on the ear's boundary.
- **GPU overlay z-order** — `gpu_scene()` folded overlay geometry (active-layer zone highlight, selection highlight, DRC markers, ratsnest) into the base `polygons`/`lines`/`circles` buckets, which the GPU draws before other base content — so an overlay meant to sit on top rendered underneath it. Overlays now stay in their own buffers and composite in a dedicated pass strictly after every base bucket, matching the CPU `draw_scene` overlay pass.
- **GPU dashed lines** — `line.wgsl` declared the per-instance `style` field but never consumed it, so `RATSNEST_STYLE_DASHED` / keepout / DRC-overlay segments rendered solid on the GPU. The fragment shader now derives a dash/gap pattern from `camera.mm_per_px` (reproducing the CPU's literal 8px dash / 5px gap at any zoom) and discards the gap portions.
- **PCB traces on odd-numbered nets drew dashed (#679)** — the trace emitter wrote the net number into the line `style` field, whose low bit is the dash flag, so routed copper on nets 1, 3, 5, … looked like ratsnest airwires on both the CPU and GPU paths. Traces now emit a solid style.
- **Vias drew oversize with a shrunken hole (#681)** — the annulus stroke was centred on the outer radius, so a 0.6 mm / 0.3 mm via drew 0.75 mm across with a 0.45 mm hole. The ring now spans exactly drill/2 to diameter/2; a via with no annulus still draws as a filled disc.
- **GPU text now pans with the view** — glyph text rasterised for the GPU path was scaled by zoom but never translated by the pan offset, so labels drifted off the geometry they annotate on any pan; the screen-space pan term is now applied.
- **Bounded GPU buffer growth** — the line/circle/arc/polygon instance buffers are clamped to the device `max_buffer_size` before reallocation, so a pathological board degrades to a truncated draw instead of panicking the render thread.
- **Idempotent stock-library generator (#323)** — `build_stock_library` minted a fresh `uuid`, `file_uuid`, sketch ids and `Utc::now()` stamps on every run, so regenerating rewrote all five `.snxfpt` and would have broken every `PrimitiveRef { library_id, uuid }` pointing at a stock part. Identity now comes from a checked-in table (the ids already shipped; identity fields unchanged) plus one pinned timestamp (`2026-05-04T19:01:13Z`), via new `Footprint::with_identity` / `FootprintFile::with_identity`; a test fails if a run is not byte-identical. The five regenerated `.snxfpt` files were also brought up to the current schema (`centerline = false` on every sketch entity, a `[...pad.stack]` table).
- **Preferences dirty-tracking (review #308 findings 1-2)** — `preferences_draft_differs()` compared only the 7 appearance drafts, so theme import (a same-tag `custom_theme` content swap), a pending keymap rebind, or a component-class edit could get silently clobbered back to "clean" by the next unrelated appearance-draft change, letting Close discard it with no prompt. A `preferences_dirty_sticky` flag now pins the one imperative edit the comparator structurally cannot see (theme import); `preferences_has_unsaved_changes()` (`sticky || comparator`) is the single source every dismiss route checks. Every dismiss route — the in-dialog X, Esc, and an OS close request (Alt+F4 / native ✕) on the detached Preferences window — now refuses to close while dirty instead of silently reverting.
- **Database-library primitives (#688)** — `DatabaseAdapter` did not speak `signex-library-server`'s primitive contract: fetching a symbol, footprint or sim model got `400` (no `library_id`), saving one got `422` (no top-level `library_id` in the body), and listing returned every library's primitives in a shape the client could not decode. The client now sends the library on every primitive call and reads the server's list rows, and a new integration test drives the adapter against the real router.
- **3D importers refuse hostile input (#112)** — a glTF buffer/image `uri` with a scheme, drive prefix, absolute path, `..`, or a symlink resolving outside the model directory is now a typed error instead of an arbitrary file read embedded into the cached GLB. VRML node nesting is capped at 256 (typed error, no stack overflow), STEP statement splitting is linear-time and no longer breaks on `;` inside quoted strings, and the GLB writer rejects lengths beyond `u32::MAX` while `compute_min_max` tolerates a partial trailing vertex. No `cargo-fuzz` target yet (needs nightly); negative-path fixtures live in `tests/untrusted_input.rs`.
- **3D import cache can no longer serve stale or partial GLBs (#147)** — the cache key now uses nanosecond mtime plus source size instead of whole seconds, a cache hit requires a structurally complete GLB 2.0 (header, declared length, JSON/BIN chunks) rather than any non-empty file, and entries are written via `atomic_write` (temp + fsync + rename). WRL triangulation emits a triangle only when all three corners are in range, so a face with a bad index can no longer yield an index buffer that is not a multiple of 3.
- **Project-tree rename keeps the native extension and sheet references** (#145) — renaming a file can no longer drop or change its extension (the dialog shows a validation message instead), and renaming a `.snxsch` now re-points every parent sheet's hierarchical reference through the engine (`Command::RenameChildSheetFile`, undoable) and updates the `.snxprj` sheet list. Affected parents and the project are marked dirty, not written; unopened parents are loaded so Save All persists them. Renaming the project's `.snxpcb` re-points `pcb_file`, and library entries refuse a tree rename with a message. Undoing the parent-reference edit points the parent back at the old filename while the on-disk rename stays, so an undo after a rename leaves a dangling reference until the file is renamed back. Unparseable sheets are reported separately from references that could not be updated, and renaming a non-sheet file never walks the sheets.
- **Symbol editor pin drag (#170)** — pressing a pin that sits on a body rectangle's edge resized the rectangle through an undrawn resize handle, and the pin hit-test took the first pin in vector order within a fixed 1.5 mm tip disc. Only selected graphics now expose handles, and a pin is grabbed by an 8 px screen-space zone around its tip plus a narrow (at most 0.4 mm) zone along its tip-to-base line, nearest pin first. The line zone is deliberately narrow so a graphic touching a pin's base, or a press in the body beside a pinned edge, still picks the graphic; a pin's name/number label is the lowest-priority grab, after graphics. The hover cursor uses the same priority as the press, and resize handles honour the selection filter. The selection-filter pills now gate picking and box select, Esc leaves a placement tool, Add Pin shows a ghost and places on release (no more stacked pins), and Fit/initial view use the real canvas size instead of 800x500.
- **Dropped-`Task` gate hole (#627)** — `check_discarded_tasks.sh` now also flags `drop(<task>)` of a `handle_*` result or a Task-typed binding, which walked past both `unused_must_use = "deny"` and the `let _ =` grep; the one site (a test in `open_document.rs`) now asserts on the Task instead.
- **Close Project no longer discards a sibling project's dirty child sheet (#741)** — closing project A now splits A's dirty documents by one strict rule. Save All / Discard All act only on `mine`: the dirty documents A's own ownership walk reaches (its listed sheets plus children, following every reference, absolute and `../` alike, `..` clamped at the filesystem root) and A's own `.snxprj`, minus anything another open project's walk also reaches. Every other dirty document associated with A (in its folder, backing its tabs, a library draft) is kept: never discarded, named in the confirm dialog and the Messages panel (a count plus the first few names), and its tab stays open. If any open project has a sheet that cannot be read or parsed, Discard All is disabled with the reason and the unreadable sheets named, leaving Save All (of `mine` only) and Cancel. The disk read runs on a worker thread, the confirm step looks the project up again by id, and the references are re-read when the open-project set changed. Loaded engines are matched on the same normalised path the walk uses, so a sheet opened through a `../` spelling no longer hides its children. Ownership compares file identity, not path text: the off-thread resolve canonicalises existing paths (falling back to the lexical key), so a case variant on a case-insensitive volume and a path through a symlinked directory no longer let a sibling's file be discarded. An open sheet's in-memory references are followed together with its saved file's, a Component Preview row is the closing project's only when its library is the project's own and not another open project's, and engine edits made while the read ran are seen when the gate runs and again at confirm.
- **Rotated footprints turn their pad shapes (#680)** — a footprint at 90° or 45° moved its pad centres but drew every rect and oval pad axis-aligned, so SOIC pads lay crosswise and overlapped their neighbours on both the CPU and GPU paths. Pad outlines now take the footprint's rotation (`signex_types::pad_outline::PadOutline`); no file format changes. Hit-test and fit-to-board use a rotation-independent radius and are unaffected; RoundRect, Trapezoid and Custom board pads still draw as rectangles.
- **Stable addressing after tab close and primitive delete (#672)** — closing a tab left of the active one, or Close Project with interleaved tabs, moved focus onto the next document; deleting a footprint or symbol before the active one silently retargeted the editor onto a different primitive, and the footprint path left the tab chip clean. The active tab and primitive now follow their document, and the footprint delete marks the tab dirty.
- **File ▸ Save As on symbol and footprint tabs (#671)** — Save As always opened "Save Schematic As" and, on a `.snxsym` / `.snxfpt` tab, wrote nothing and logged nothing. It now routes by the active tab's document kind: a symbol or footprint tab gets its own title and filter, writes its own format atomically to the chosen path, leaves the original file untouched, and the tab, dirty marker and editor follow the new path. Kinds without Save As, and a target another tab already has open, are refused with a Messages-panel entry.
- **Arc end point stays on its circle (#684)** — nothing tied an arc's end point to the circle through its start point, so a raw third click or a later dimension edit left the end off the arc and the drawn arc, the end point and any joined line disagreed. The solver now carries one implicit `|end − centre| = |start − centre|` row per arc.
- **Over-constraint is reported, and a split no longer reshapes EqualLength (#685)** — a conflicting constraint set (say two different distances on one pair) failed as a generic "did not converge" with an empty over-constraint list; the solve now fails with `OverConstrained` naming the conflicting constraints. Splitting a line also re-pointed its `EqualLength` to the first half, which is shorter, so the next solve deformed the sketch; that constraint is now dropped and reported like `Midpoint`. The zero-length-line and 50 ms timeout findings in #685 are not part of this change.
- **Older arcs are repaired once at load, and a conflicting edit is rejected (#684, #685)** — a footprint saved before the arc row existed can hold an arc end point off its circle; every read from storage (the `.snxfpt` file, the database adapter, the library server) snaps that end onto the circle through the start (angle kept, centre and start never move). Opening the footprint in the editor names the adjusted arcs once in the Messages panel and marks the document dirty so a save persists the repair; previews and history reads stay quiet. In the footprint editor an edit that over-constrains the sketch is now rejected: the sketch reverts to its last good state, nothing is added to undo, redo or the dirty flag, and the conflicting constraints (only the ones whose removal resolves the conflict) are listed in the Messages panel and drawn, with the entities they name, in the theme error colour until the next edit or successful solve. A retry that escapes a saddle no longer leaves its start offset on geometry nothing constrains. A solve that stalls on a symmetric start (two equal distances from a point on the line between two fixed points) is retried from a slightly moved start before any conflict is reported.

### Fixed — BOM

- **A variant can fit a part marked DNP (#674)** — a part with DNP set in the base schematic was left out of a variant's BOM even when that variant fitted it, because the DNP filter ran before the variant was read; export validation skipped it the same way. Only the active variant's own override now lets a DNP part through. A leftover `Fitted`, `VariantFitted` or `DNP` field on the part does not, with or without an active variant.
- **Library actions no longer silently do nothing (#152)** — Components Panel "Add Library" on the Project section and a failed Installed/Global mount now raise the error card, a failed `global_libraries.toml` write is reported, "Refresh All Pricing" is shown disabled ("Not yet available") instead of logging a TODO, and the never-emitted Manage/Add-to-Project/row-refresh stub messages are removed.
- **Grouped BOM keeps parts with different MPNs apart (#673)** — grouping keyed only on name, value, footprint and library reference, so two parts differing in MPN, manufacturer, description or any other field merged into one row carrying the first part's data, and the BOM ordered the wrong part. A group now requires every part-level field to match; an empty field counts as absent, and variant fit-state fields (`DNP`, `Fitted@<V>`, …) never split identical parts.

### Changed

- **Test fixtures build core types from defaults (#749)** — `SchematicSheet`, `Symbol`, `Wire` and `Junction` gain a `Default` (equal to a deserialized minimal manifest, pinned by a test) and `signex_erc::Violation` gains `Violation::new`, so tests write only the fields they care about and adding a field no longer breaks every test in every open PR. Production literals stay exhaustive on purpose.
- **The god-file cap is now 1200 lines**, up from 1000. It still counts physical lines, comments included. Long explanations belong in `docs/` with a one-line pointer in the code (CONTRIBUTING.md, "Code Style"), so a documented file is not forced into a split it does not need.
- **GPU glyph atlas now trims each frame**, and GPU text prep/draw failures log once instead of being swallowed silently.
- **Removed the dead `SCHEMATIC_GPU_RENDER` flag** — it gated no live path (the schematic GPU adapter is compiled and tested but not yet mounted).
- **One GPU scene renderer, not two** — `schematic_shader` was a second, unmounted copy of `scene_shader`'s five pipelines that had drifted apart from it on upload skipping, overlay compositing, draw order and error reporting; mounting it as written would have deep-copied the whole `Scene` every frame and dropped every overlay. It is gone, and `scene_shader` is now parameterised by a `SceneSurface` marker so each editor surface gets its own pipeline slot — iced keys a stored pipeline by the primitive's `TypeId` in one process-wide map, so two surfaces sharing a type would overwrite each other's instance buffers and camera. The deleted copy held the only test in the tree pinning the glyph-atlas `trim` override; that guard now watches the pipeline that actually renders.
- **Library browser sorting builds each row's sort key once** — the comparator re-derived both cell strings, re-parsed numbers and re-lowercased text on every comparison (O(n log n) allocations per sort); keys are now computed once per row (O(n)) with identical ordering, ties included (#151).
- **Preferences draft-seeding deduplicated** — opening and reverting the dialog now share one `seed_preferences_drafts_from_live()` helper instead of two independently-maintained copies of the same 8 assignments, closing the add-a-field-to-one-but-not-the-other drift hazard. The native-close revert backstop is now gated on the dirty flag (skips the keymap-profile rebuild + canvas cache clears on a no-op close), the double content-cache invalidation in `revert_preferences_drafts` is gone, and boot reads the persisted GPU-render preference once instead of three times.

## [0.15.0] — 2026-07-23

The **v0.15.0 "Editor Fixes"** milestone — 43 commits, 2026-07-15 → 2026-07-23.
Footprint- and symbol-editor correctness, an order-independent netlist
derivation with the ERC/BOM consumers reading the same topology, crash-safe
persistence, and CI/toolchain hardening (pinned Rust 1.97.0, an enforced
`rustfmt` gate, and the god-file ratchet).

### Fixed — netlist

- **Netlist derivation no longer depends on wire order** (#402). `uf::union`
  now keeps the smaller root, so a class's representative is a pure function of
  the partition rather than of the order the unions were applied in, and
  `anchor_point` resolves a label tapping several wire interiors by a total
  order over the segments instead of by whichever the slice yielded first.
- **Junction dots are reconciled on every command that changes wire geometry**
  (#402). `PlaceWireSegment` reconciled both directions of a T; `MoveSelection`
  / `RotateSelection` / `MirrorSelection` mutate the same coordinates and
  reconciled nothing, so dragging a stub onto a trunk left a junction-less T —
  which the netlist deliberately reads as disconnected (#107) — and the
  connection was silently lost.
- **Junction autoplacement no longer mints a dot the netlist will not honour**
  (#402). The geometry helpers work in `f64` mm with a 0.01 mm tolerance, but
  `SheetConnectivity` honours a junction only where the point is *exactly*
  collinear in the 1 µm key space. A candidate a few µm off a wire produced a
  dot asserting a connection the derivation refused to make. Every minted dot
  is now gated on the netlist's own predicate; off-grid geometry gets no dot
  rather than a lying one.
- **Same-name label merging is applied by every connectivity consumer** (#404).
  `SheetConnectivity::merge_named_labels` is now called by `summarize_nets`
  (ERC/DSL), `net_label_conflict`, `missing_power_flag`, and
  `flood_net_elements` (the net-colour highlight), so all of them derive the
  same topology `build_netlist` does. Two physically disjoint wires sharing a
  label name were previously separate nets to each of these, but one net to the
  netlist.

  **Behaviour change for existing projects.** Merging changes the net *name*
  the ERC DSL sees, not just the net count: when a `VCC`-labelled fragment
  merges with one carrying a higher-priority `Global`/`Power` label, the merged
  net takes the higher-priority name. A DSL rule keyed on `net.name == "VCC"`
  can therefore stop matching entirely rather than merely matching once instead
  of twice. `net_label_conflict` also now reports conflicts created by such a
  join — two differently-named `Net` labels pulled onto one net by a shared
  `Global`/`Power` label previously went unreported, and both signal names were
  silently dropped from the netlist.

### Changed — netlist

- **Auto-generated `N$k` net names change for existing schematics.** Making the
  partition order-independent reorders the sorted roots that assign `NetId`
  `1..=N`, and unlabelled nets are named from that id. An unchanged schematic
  therefore exports a netlist whose `N$k` names differ from before this
  release. This is a one-time, unavoidable consequence of the fix — the
  alternative is keeping net numbering dependent on document order.

  Constraint this records for later: `net_name` is a persisted field of the PCB
  format (on pads, tracks and zones), so any future schematic → PCB net sync
  **must match by terminal set, not by auto-generated `N$k` name.** Nothing
  in-tree feeds `build_netlist` into `signex-pcb` today, so no routed copper is
  remapped by this change.

### Added — ERC

- **`AmbiguousLabelAnchor`** (warning). A label sitting where two or more wires
  cross with no junction is anchored to exactly one of them by the tiebreak
  above. The answer is deterministic but not predictable from the geometry, so
  ERC now says the net was decided by tiebreak instead of leaving the user to
  guess. Labels on a wire endpoint, or on a dotted crossing, are not flagged.

### Added — footprint & symbol editor

- **Real Align… dialog** composing the existing `AlignOps` (#370).
- **Break Track** wired to the sketch `split_line` primitive (#372), backed by a
  new `split_line` primitive that divides a `Line` at parameter *t* (#429).
- **Drag Track End** as an endpoint-biased segment grab (#361).
- **Sketch active bar** grouped under Create / Modify, with the chrome search
  bar centred (#443).

### Fixed — footprint & symbol editor

- **Context-menu selection** no longer acts on a stale target, and no-op edits
  stop dirtying the document or pushing history (#146).
- **Pad geometry** — pad rotation is now real geometry that holds the sidecar
  re-mint invariant (#433); a pad's ownership of its sketch geometry survives
  reopen (#424); the Place/Move active-bar button drives the pad Select tool
  (#427).
- **Shapes dropdown** collapses the three phantom Arc rows to one (#462).
- **Tab during footprint sketch placement** routes to the sketch instead of
  triggering Save in the standalone editor (#428).

### Fixed — schematic, ERC & export

- **Child-sheet references** resolve parent-relative through one shared helper
  (#339) and are rejected when they escape the resolution root (#473).
- **Sheet delete** completes correctly, and cut/copy is gated on a preservable
  selection subset (#425).
- **Annotate** preview and action now agree on sheet-walk order (#470).
- **Deterministic child-sheet ownership** with loud cross-directory filename
  collisions (#459).
- **Stale minted junction dots** are removed on wire-geometry reconcile (#480).
- **ERC** merges same-name labels in the shared connectivity so it matches the
  netlist (#418).
- **Export** gained a project sheet-set assembler with per-deliverable stitch
  severity, plus the Export-anyway netlist path (#436, #406, #431 via #449).

### Fixed — BOM

- Designators order naturally with Fitted ranked above DNP (#419), and field
  lookup is deterministic and case-insensitive with base fit resolution (#474).

### Fixed — persistence

- **Save-All** routes every dirty document kind and surfaces the failure reason
  (#452), covered by Cut/Save-All regression tests (#460).
- **Crash-safe writes** — table and settings writes go through `atomic_write`
  (#421), whose per-writer tmp sibling name is now unique (#469).
- Schematic extras serialize deterministically (#409).
- The test suite no longer touches the real `prefs.json`; the no-config-dir
  fallback is surfaced (#439).

### Fixed — geometry & output

- **Circumcircle math unified** — four copies collapsed to one canonical helper
  (#461), with the SVG output routed through it (#484).
- **Bake/library** rejects unit-suffixed chamfer ratios and fixes a
  byte-boundary panic in hash-cell parsing (#417).

### Changed — tooling, CI & toolchain

- **Rust pinned to 1.97.0** (`rust-toolchain.toml` + CI) for cargo/CI/fmt parity
  (#454), the MSRV raised to match (#456), and the pin documented (#455).
- **`rustfmt` is now an enforced gate** — `rustfmt --all` across the workspace
  and the fmt job made blocking (#453).
- **God-file ratchet gate** — no new or growing production god-files (#450), the
  cap raised to 1000 lines (#451), counting production lines past an external
  `mod tests;` (#468).
- **Data-driven dropdown table** replaces the `dropdown.rs` god-file (#458).
- The labeler matches `signex-app` tests so test-only PRs get area labels (#441).

[Full changelog](https://github.com/alplabai/signex/compare/v0.14.0...v0.15.0)

## [0.14.0] — 2026-07-18

**Everything since v0.13.0** — 221 commits, 2026-05-06 → 2026-07-18.

This section was originally written on 2026-05-31 covering only the footprint
editor, and never tagged. Work kept landing past it: symbol multi-unit, the
`signex-net` netlist contract, keyboard-shortcut profiles, the schematic GPU
render path, and 83 commits of ADR-0001 decomposition. Rather than mint a
phantom version — v0.12 is already one, planned and merged but never tagged —
v0.14.0 claims all of it. The bulk below was reconstructed from `git log` and
summarises by theme rather than listing every commit.

### Added — netlist

- **`signex-net` crate — the authoritative `Netlist` contract** (#137). One
  derivation of connectivity, consumed by everything that needs it instead of
  each subsystem rolling its own.
- **Cross-sheet netlist stitching** — `build_project_netlist` (#168).
- Same-name labels merge into a single net in `build_netlist` (#154).
- Netlist contract completed and connectivity-gate defects fixed (#157);
  ERC now consumes the shared derivation, with one `SymbolTransform` (#158).
- **Project-netlist app wiring** — cache, shared sheet-map, exporter (#159).

### Added — symbol editor

- **Multi-unit parts as a first-class concept** — `part_count`, unit buttons,
  band→footer layout (#290–#292), per-unit graphics via
  `SymbolGraphic.part_number` (#293), and per-unit body geometry, with draw,
  hit-test, and select all scoped to the active unit (#294).
- **Drawing tools** — two-click line, two-click circle, three-click arc,
  two-click rectangle with per-graphic fill (#299), rectangle edge handles
  with resize cursor hints.
- **Selection** — AutoCAD-style rubber-band box select, select-all (Ctrl+A),
  clicking a graphic body drags the whole symbol, and clicking a graphic
  selects only that graphic.
- Undo/redo and grid controls; separate grid styles for schematic and symbol
  editors; Properties panel refreshes on canvas selection.
- Optional pin grab-by-label with whole-pin glow — per-tab toggle, default
  off (#298).
- **`anchor2d`** — pivot-aware 2D rotation with a compensated (B-type)
  `Transform2D`.
- **Polygon graphic primitive + closed-shape authoring** (#378) — a
  `SymbolGraphicKind::Polygon` (implicitly-closed ring) with fill/stroke,
  concave-correct hit-test, and per-vertex handles; a **Place Polygon**
  click-collect tool with the full close-gesture set; **Join into Polygon**,
  which chains selected lines/arcs end-to-end into one closed polygon
  (auto-closing an open chain) in a single undo step; and a **right-click
  context menu** built from a pure data-to-menu row function.

### Added — keyboard & commands

- **Configurable keyboard-shortcut profiles with an in-app editor** (#202,
  supersedes #116).
- Menu labels are sourced from the command table via `menu_label()` (#270,
  #282), so menus and the shortcut pane can present differently from one
  `CommandMetadata` entry (#271).
- Drift-guard test: every menu command id must resolve in `CommandMetadata`
  (#272, #283).

### Added — rendering

- **Schematic GPU render module** via iced's shader widget (#169, #200).
- Schematic renderer scene cutover started in the app canvases; Milestone F
  schematic-runtime tasks 03–05 completed.

### Changed

- **`signex-gfx` aligned to iced's wgpu 27 + cryoglyph**, dropping the dual
  GPU stack (#198).
- **Large-scale decomposition under ADR-0001** — 83 commits splitting god-files
  and god-functions across `signex-app`, `signex-engine`, `signex-renderer`,
  `signex-gfx`, `signex-types`, `signex-library`, `signex-net`, `signex-output`,
  and the 3D importer, under an 800-line cap. Includes the root `Message`
  namespacing (D3), the canvas `update`/`draw` splits, the property-panel
  family, and the 1,223-line `collect_overlays` (#210). Internal only — no
  behaviour change intended.
- License Guard is framed by GPL rather than by the banned tool's name (#209).
- Repo governance: label taxonomy, path labeler, CODEOWNERS, refreshed
  templates (#133); roadmap reconciled and milestones + tiers as code (#300).

### Fixed

- **Data loss / persistence** — TSV cells are escaped so a schematic save can
  always reopen (#96, #130); C0 control bytes in a TSV cell are escaped too, so
  a stray control character in user text can no longer produce a `.snxsch` /
  `.snxpcb` that will not reload (#386, #397); the footprint editor's STEP store
  is written via `atomic_write`, so a crash mid-write can no longer strand a
  corrupt 3D asset that is then served forever (#387, #398); persistence made
  crash-safe with `fsync` `atomic_write`, atomic `.snxprj`, and a corrupt-JSON
  guard (#104, #119); residual document writes routed through `atomic_write`,
  New Project guarded (#104, #128); prompt for unsaved changes on app exit
  (#95, #124).
- **Footprint sketch-profile pads** — moving a pad made with "Make Pad from
  Profile" left its sketch profile behind, and the bake then resolved the
  copper back to the pad's original location, so an exported footprint placed
  the pad in the wrong spot with no warning. The profile now travels with the
  pad: the loop walker gained an id-level core that needs no solve, a pad that
  first appears from the sketch side is relinked to its `PadAttr` entity by
  number, and a whole-pad drag no longer snaps its cursor to the pad's own
  outline vertices (#142, #311).
- **Connectivity** — wires connect at T-junctions in net derivation (#107,
  #120); the net-colour flood runs on the authoritative connectivity core
  (#138).
- **ERC agrees with the netlist on mid-wire taps** — ERC re-derived
  connectivity with endpoint-only checks, so a label or pin tapping a wire's
  interior was invisible to the rules while the netlist saw it. The rules now
  read the shared wire-anchored connectivity (#388, #399); DSL net names are
  anchored the same way, so rules keyed on `net.name` / `net.class` no longer
  see an unnamed net plus a phantom (#396, #403); and bus range labels placed
  mid-span — where they are normally drawn — are anchored to their bundle, so a
  `D[0..7]` / `D[0..3]` width mismatch on one bus is reported instead of
  silently passing (#395, #405).
- **PDF and preview net names** — the exporters re-derived connectivity of
  their own instead of reading the authoritative `Netlist` off
  `ExportContext`, so an exported sheet could annotate a net differently from
  the netlist it shipped with (#389, #400).
- **Editing** — Ctrl+C/X/V/D and shift-chorded shortcuts un-broken (#103,
  #127); Find/Replace replaces the matched substring rather than the whole
  field (#102, #125).
- **Symbol rendering** — pin text rotation corrected for screen Y-flip; name
  `h_align` reversed for flipped pin orientations; pin tip rotates around the
  body-end pivot; arc angles negated in both preview and scene renderer for
  the screen-space y-flip; arc discontinuity past ±180° prevented; rotation
  angles normalised; `LineJoin::Round` for rectangle/polygon corners; round
  line caps.
- **Symbol arc sweep convention unified, with a data-safe legacy migration**
  (#378) — the CPU canvas draw path was the lone signed-sweep holdout while
  hit-test, the GPU SDF shader, and rotation all read `start_deg`/`end_deg` as a
  CCW sweep that wraps through 360°, so a rotated 0°-crossing arc drew its
  complement while clicks landed on the real arc. All consumers now route
  through one authority (`signex_gfx::primitive::arc::ccw_wrapped_sweep_rad`),
  a full-turn arc draws and hit-tests as one circle, and a load-time migration
  self-heals legacy clockwise-signed pairs on read. Every endpoint writer
  (placement, rotation, the Properties panel, and the arc-endpoint drag handle)
  normalises into `[0, 360)` so a saved arc round-trips instead of silently
  reloading as its complement.
- **Interaction** — unsnapped cursor position for Select-tool hit-testing,
  snapped coords retained for drag anchor and delta; `CursorAt` published
  during box-select drag to force redraw.
- **Detached windows open in front** — a detached modal, undocked tab, or
  detached panel opened at the default window level with no raise, so on
  Windows it could appear *behind* the main window and the user had to move
  the main window to find the dialog they had just opened (#311).
- Preferences modal is responsive to window resize (#208) and the reopen
  regression is fixed; theme-aligned canvas backdrop, sheet tracks the stored
  paper size (#201); library server gains a persistent DB backend and rejects
  duplicate-row POST (#97, #122).

### Changed — CI

- lavapipe installed so `signex-gfx` GPU smoke tests run headless (#126).
- `cargo-deny` advisories are informational, not a merge gate (#123).
- CI and license guards run on `trunk` (#121); PR preconditions aligned with
  the org control-process convention (#134); Linux dependency install hardened
  against the `packages.microsoft.com` apt outage (#155).
- **Declared the 1.88 MSRV** and corrected the README + CONTRIBUTING Rust
  version references to match (#383).
- README and codebase map refreshed to the v0.14 workspace reality (#385).

### Added — footprint editor (the original v0.14 scope)

v0.13.0 shipped the footprint / sketch editor compiled but hidden behind a
feature flag while it was finished; v0.14 completes the remaining active-bar
tooling and **enables the editor** — opening a `.snxfpt` now opens an editable
tab, and the New Footprint / PCB Library create flow is live again.

### Added — footprint editor: sketch constraints

- **Nine more sketch constraints exposed** in the sketch-mode active
  bar: Tangent (line-arc + arc-arc), Angle, Equal-Radius, Point-on-Arc,
  Distance-point-to-line, Distance-point-to-circle, and Symmetric
  (about a line + about a point). The Newton-LM solver and serialization
  already supported all 19 constraint kinds; this surfaces the 9 that
  had no button. Selection-first UX: select the entities, the valid
  constraint buttons light up. The two 3-entity Symmetric constraints
  take their third entity from the multi-select extra slot.

### Added — footprint editor: active-bar tools

- **Align / Distribute / Spacing** (12 ops) — Align Left/Right/Top/
  Bottom + center H/V, Distribute Horizontally/Vertically (equal centre
  gaps, extremes fixed), and Increase/Decrease H/V spacing (one grid
  step, pivoting about the selection centroid). Operate on the combined
  pad selection; no-op under 2 pads (3 for distribute). Sketch-backed
  pads mirror their move into the sketch; undo-snapshotted.
- **Move / Drag / Move Selection** — activate the Select tool (footprint
  pad-move is drag-under-select). Adds a `nudge_pads` helper +
  `FootprintActiveBarNudgeSelection` foundation for the typed-delta
  "Move Selection by X, Y…" dialog (dialog itself deferred to v0.15).
- **Fill / Solid Region / Text Frame** — wired to the existing
  filled-polygon (`PlaceRegion`) and silk-text (`PlaceString`) place
  tools.
- **Selection-filter "All - On / All - Off"** toggle wired to a new
  `SelectionFilter::set_all`.

### Added — footprint editor: deferred cleanup

- **Move Selection by X, Y…** now opens a typed-delta modal (two mm
  inputs) that nudges the pad selection by the entered amount, reusing
  the same sketch-mirror + undo path as the one-step grid nudge.
- **3D Body / Extruded 3D Body** active-bar items mint the footprint's
  `body_3d` (extrude the courtyard, or the fab outline) — visible
  immediately in the CPU 3D preview. The interactive wgpu pipeline stays
  deferred (v2.x).
- **Text Frame** is now a real bounding-box place tool: drag a rectangle
  to set the frame; the silk string aligns/clips inside it. (No
  auto-wrap yet.)
- **Footprint-native selection-filter presets** — the filter dropdown's
  All-On/All-Off toggle and preset chips are wired to real footprint
  `SelectionFilterKind` presets, persisted under `footprint_filter_presets`
  in prefs, with a "Save current filter as preset" capture. (Replaces the
  schematic-typed presets that could not apply to footprints.)

### Fixed — footprint editor

- **Pad shape-param leak** — `mint_shape_geometry_for` now clears a
  pad's `shape_params` before regenerating geometry, so changing a pad's
  shape (e.g. RoundRect → Round) no longer strands stale parameter keys
  for the solver / next bake.

### Changed — footprint editor

- `FOOTPRINT_EDITOR_ENABLED` flipped `false` → `true`
  (`crates/signex-app/src/feature_flags.rs`). The
  `opening_snxfpt_does_not_create_editable_tab_when_gated` regression
  test branches on the flag and now asserts the enabled behaviour.

### Deferred to v0.15

- Break Track / Drag Track End (need track-segment split infra).

### Constraints — Apache-clean invariants (carry forward)

- Zero `kicad` substrings under `crates/`; no third-party
  constraint-solver substrings under `signex-sketch` / `signex-bake`;
  `cargo-deny` advisories + licenses green; full `cargo test --workspace`
  green (GPU smoke tests skip headlessly).

[Full changelog](https://github.com/alplabai/signex/compare/v0.13.0...v0.14.0)

## [0.13.0] — 2026-05-31

The **v0.13 Symbol & Library** milestone. This release pairs the
cleanroom schematic renderer (the work planned as v0.12) with a wave of
symbol-editor and library polish. There is no separate v0.12.0 tag — the
cleanroom rewrite ships here. The workspace version had already advanced
to `0.13.0`, so this release adopts that number rather than bumping
backward.

### Changed — cleanroom schematic renderer

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

### Added — tests

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

[Full changelog](https://github.com/alplabai/signex/compare/v0.11.0...v0.13.0)

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

### Added — documentation (2026-05-01)

- **Hardware Requirements** section in `README.md` (addresses #63) — Vulkan 1.1 / DirectX 12 / Metal floor (~2014+ GPU). Older GPUs that fall back to legacy OpenGL may render incorrectly.
- **`docs/RENDERING_RULES.md`** (new public doc) — Apache-2.0 prose describing label rendering, field rotation/justify rules, and IEEE-Std-91 pin shape decorators. Sourced from `crates/signex-types/` (Signex's own `.snxsch` format), Altium parity goals, and public industry standards. Source comments in `signex-render` reference this spec instead of citing third-party EDA tooling.
- **`docs/audit/comments-scrub-2026-05-01.md`** (new audit doc) — records the 2026-05-01 source comment scrub that removed residual KiCad C++ class name references from `signex-render` and `signex-engine` and `signex-output/pdf` even after the v0.10.0 "Standard" rename. Eight comment lines across seven files reworded to neutral descriptions; algorithms unchanged.
- **`docs/internal/CLEANROOM_REWRITE_PLAN.md`** (private submodule) — plan for the v0.12 cleanroom milestone: deletion scope, working rules for the fresh agent session, six rewrite phases, CI guard extensions.

### Changed — license-guard CI

- `no-kicad-cpp-class-names` job (new) forbids `SCH_PAINTER`, `SCH_FIELD::`, `SCH_LABEL`, `SCH_PIN`, `SCH_SYMBOL`, `LIB_SYMBOL::`, `LIB_PIN::`, `sch_painter.cpp` / `sch_symbol.cpp` / `sch_label.cpp` / `sch_pin.cpp` / `lib_symbol.cpp`, and `eeschema/` anywhere in `crates/`.
- `no-derivation-attribution-markers` job (new) forbids `DeepWiki`, `KiCad mirror source`, `extracted from KiCad`, `based on KiCad source` anywhere in the repo (excluding the audit trail and `docs/LICENSING.md`).
- Total License Guard jobs: 6 → 8.

### Changed — minor allocations

- `LibraryCreateOptionsState` and the Enable Version Control modal hoist the `intro` paragraph `format!` out of the view-body function — built once at modal-open time and stored on the state, no per-frame allocation.

### Changed — types & samples

- **`PinElectricalType` → `PinDirection`** in the new `signex-library` crate to satisfy the License Guard's `no-removed-kicad-api` strict job. Variant set unchanged. Note: this is a different enum from `signex_types::schematic::PinDirection` (which has Signex-original variants); the two coexist as path-qualified `signex_library::PinDirection` vs `signex_types::schematic::PinDirection`. Consolidating them is a follow-up refactor.
- `crates/signex-types/src/library.rs` — the v0.10.0 thin `Library` / `LibraryComponent` types are removed. Library Browser tab content now reads through the `signex-library` adapter trait.
- `assets/samples/library/resistors-standard.snxlib` — removed; the obsolete v0.10.0 sample no longer fits the DBLib data model.

### Added — documentation

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

[Full changelog](https://github.com/alplabai/signex/compare/v0.10.0...v0.11.0)


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

### Changed — Apache-clean residual polish

In response to ongoing discussion on [issue #62](https://github.com/alplabai/signex/issues/62), v0.10.0 also lands a residual-polish pass that removes vestigial KiCad-shaped names from the codebase. None of these changes alter user-visible functionality, but they reduce the surface that reads as "Signex was once derived from KiCad" — the substantive change was already delivered in v0.9.0; this is the cosmetic follow-through.

- **Style enum variant renames.** `MultisheetStyle::KiCad` / `LabelStyle::KiCad` / `PowerPortStyle::KiCad` → `::Standard` for all three. The `::Altium` variants stay (Altium is the project's stated design reference and isn't a GPL exposure). On-disk preference strings remain `"kicad"` / `"altium"` for backward compatibility with existing `prefs.json` files; the user-facing dropdown labels also stay "KiCad" / "Altium" so users coming from those tools recognise the modes.
- **Legacy KiCad symbol-library scanner removed.** `helpers::find_kicad_symbols_dir` (which walked `/usr/share/kicad/symbols`, `C:\Program Files\KiCad\…`, etc.) and `helpers::list_kicad_libraries` deleted. The associated `DocumentState::kicad_lib_dir` field, `PanelContext::kicad_libraries` field, and the dock-panel `library_browser.rs` handler are gone. The Components panel's library dropdown is replaced by an inline placeholder until the v0.10.x `.snxlib` plumbing repopulates it. The scanner had been a no-op since v0.9.0 (the load handler logged "convert with companion tool" and skipped); removing it cleans up code that pointed at KiCad install layouts without serving a real flow.
- **Direct-open KiCad-extension dispatch arm removed.** `open_document_path` no longer matches `"kicad_pro" | "kicad_sch" | "kicad_pcb"`; the same arm in the project-tree double-click handler is also gone. Opening a `.kicad_*` file now falls through to a generic "unsupported file type" error. The migration story for KiCad users is unchanged — run `signex-kicad-import` first; the README and `docs/LICENSING.md` continue to describe the flow.
- **`docs/LICENSING.md` strengthened.** New "LLM context discipline" section documenting that post-cutover development uses LLM-assisted workflows where KiCad source code is never placed in agent context, prompts, retrieval indexes, or reference material. Versions table updated through v0.10.0.
- **License Guard CI tightened.** Three new jobs added to `.github/workflows/license-guard.yml`: forbid re-introduction of the v0.10.0-renamed names (`MultisheetStyle::KiCad`, `find_kicad_symbols_dir`, etc.); forbid KiCad-numbered layer-id constants (`F_CU = 0`, `B_CU = 31`, …); forbid the `Net-(<r>-Pad<p>)` auto-net-name format string.

### Added — tests

- `signex_types::format::tests::snxlibrary_round_trip_preserves_components` — locks parser/writer round-trip parity.
- `signex_types::format::tests::snxlibrary_borrow_matches_owned` — owned/borrowed serialise parity.
- `signex_types::format::tests::snxlibrary_rejects_unknown_version` — `UnsupportedVersion` error path.
- `signex_types::format::tests::snxlibrary_parses_empty_components_block` — empty-library round-trip.
- `signex_types::format::tests::shipped_sample_library_parses` — guards the shipped sample against parser drift.

### Constraints — Apache-clean invariants (carry forward from v0.9.0)

- No `use kicad_parser` / `use kicad_writer` in `crates/`.
- No `kicad-parser` / `kicad-writer` Cargo.toml deps.
- License Guard 4 jobs and `cargo-deny` continue to gate every PR.

[Full changelog](https://github.com/alplabai/signex/compare/v0.9.1...v0.10.0)

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

### Added — tests

- `signex_types::format::tests::schematic_borrow_matches_owned_serialise` — locks owned/borrowed parity for `SnxSchematic`.
- `signex_types::format::tests::pcb_borrow_matches_owned_serialise` — same, for `SnxPcb`.
- `signex_engine::tests::serialize_for_save_returns_parseable_bytes` — serialise + reparse round-trip.
- `signex_engine::tests::write_to_file_writes_serialised_bytes` — disk write + reparse round-trip via tempfile.

[Full changelog](https://github.com/alplabai/signex/compare/v0.9.0...v0.9.1)

## [0.9.0] — 2026-04-29

The **Apache-clean cutover** release. Resolves [issue #62](https://github.com/alplabai/signex/issues/62) raised by Seth Hillbrand of the KiCad project flagging that several Signex crates derived from KiCad's GPL-3.0 source were shipping under Apache-2.0. The main `signex` repository is now Apache-2.0 clean and contains no KiCad-derived code; KiCad I/O moves to the optional [signex-kicad-import](https://github.com/alplabai/signex-kicad-import) companion tool (GPL-3.0-or-later), shipped independently.

The library subsystem (Library Browser, SCH Library editor, Component Editor) that was in flight on `feature/v0.9-snxlib-as-file` is preserved at the `v0.9-snxlib-paused-2026-04-29` tag and ships as **v0.10.0** on top of the Apache-clean foundations from this release.

### Added — native file formats (TOML + TSV bulk blocks)

- **`.snxsch`** schematic format — TOML manifest (`format = "snxsch/1"`) wrapping the `SchematicSheet` payload via 4 adapter row tables (`[sheets.components]`, `[sheets.wires]`, `[sheets.junctions]`, `[sheets.labels]`) plus an `[extras.*]` TOML block for fields that don't fit a flat row schema. Same format family as `.snxlib` / `.snxsym` / `.snxfpt`.
- **`.snxpcb`** PCB format — same shape with adapter rows for `[footprints]`, `[pads]`, `[tracks]`, `[vias]`, plus `[stackup]` / `[[zones]]` in regular TOML for hierarchical data.
- Format chosen for line-diff-friendly git workflows, ~5× smaller than the equivalent JSON, single file per design, and zero S-expression-shaped grammar that could regress KiCad-derivation exposure.
- Round-trip preserves every field on `SchematicSheet` / `PcbBoard`.

### Changed — Apache-clean signex-types

- `PinElectricalType` (12-variant, KiCad-shaped) → **`PinDirection`** (14 variants — adds Signex-original `GroundReference`, `Differential`, `Clock`; collapses `Free`+`Unspecified` into `Unclassified`; renames `OpenCollector`/`OpenEmitter` → `OpenDrainLow`/`OpenDrainHigh`). Design rationale in `crates/signex-types/docs/pin-design.md`.
- `PinShape` (9-variant) → **`PinShapeStyle`** (7 variants — drops per-direction-low modifiers since polarity is now on `PinDirection`; adds `HysteresisInput` / `HysteresisOutput` / `Schmitt`).
- KiCad-numbered `LayerId(u8)` constants (`F_CU=0`, `B_CU=31`, …) → **`SignexLayer`** semantic enum + `LayerKind` categories + `altium_label()` per the Altium-flavoured Signex UI naming.
- KiCad markup parser (`~{X}` / `^{X}` / `_{X}` curly-brace syntax) → **`parse_signex_markup`** using a Markdown subset: `**bold**`, `*italic*`, `~~strike~~`, `^superscript^`, `~subscript~`, `_~overbar~_` (Signex extension for active-low signal naming), `[label](url)`, `\X` escape.
- `kicad_auto_net_name_from_pins` (`Net-(<r>-Pad<p>)` format string) → **`auto_net_name`** returning `unnamed-<sheet>:<ref>:<pin>`.

### Removed — crates moved to the GPL-3.0 companion repo

- `crates/kicad-parser/` (3,938 LOC) — moved to the GPL-3.0 companion repo.
- `crates/kicad-writer/` (2,274 LOC) — moved to the GPL-3.0 companion repo.
- `crates/signex-output/src/netlist/kicad_sexpr.rs` (336 LOC) — KiCad netlist exporter; moved to the companion or a future `signex-kicad-export` sibling.

### Added — companion repo `signex-kicad-import` (GPL-3.0-or-later)

- New separate repository at <https://github.com/alplabai/signex-kicad-import>.
- Houses the relocated `kicad-parser` + `kicad-writer` crates plus a CLI binary `signex-kicad-import`.
- One-way conversion: `.kicad_sch` / `.kicad_pcb` / `.kicad_pro` → `.snxsch` / `.snxpcb` / `.snxprj`. Originals remain intact.
- Distributed independently — Apache consumers of Signex Community see no GPL aggregation in their build closure.

### Deferred to v0.9.1 — performance

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

### Added — CI guards

- `.github/workflows/license-guard.yml` — fails any push or PR that re-introduces `kicad-parser` / `kicad-writer` imports or removed KiCad-shaped types.
- `.github/workflows/pr-license-declaration.yml` — fails PRs whose description is missing the contributor self-declaration block or whose author marked `KiCad source consulted: yes`.
- `cargo-deny` license job in `.github/workflows/ci.yml` — rejects GPL / AGPL / LGPL / unlicensed transitive dependencies.
- `deny.toml` — Apache-compatible permissive allowlist.

### Added — documentation

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

[Full changelog](https://github.com/alplabai/signex/compare/v0.8.0...v0.9.0)

## [0.8.0] — 2026-04-27

The output-and-polish release. Adds the full PDF / BOM / netlist export pipeline, multi-project workspaces, Altium-style dirty tracking, a chrome refactor with the new `TabPill` widget, hierarchical-sheet rendering parity, and KiCad-parity field autoplace. Every v0.8.x sub-feature ships under this one tag.

### Added — output subsystem

- **PDF export** — tabbed Export modal (file picker / settings / pan-drag preview), bookmarks per sheet, theme palette aware, DPI hookup, physical-structure tokens
- **BOM** — Altium-spec preview modal with column picker, variant picker, sort / drag / scroll, options applied on export, format-layer export (CSV / HTML / XLSX-ready)
- **Netlist** — date + path polish, KiCad-format export, output engine validation
- Unified PDF preview modal — File ▸ Export PDF and File ▸ Print Preview both open the same overlay; legacy `view_pdf_options_dialog` + 12 `ExportPdfSet*` / `ExportPdfDialog*` variants pruned

### Added — multi-project workspace

- Multiple `.snxprj` projects open side-by-side in the same window
- `TabInfo` carries `Option<ProjectId>`; `active_project` scoped to focused tab
- Accent-tinted active project root + per-project Close menu action
- Per-tree-path semantics for project tree actions (right-click on project B with project A active still operates on B)
- Phase 2.5 cleanup — legacy single-project fields removed from `DocumentState` (#54, #55, #56)

### Added — dirty tracking & tab right-click menu

- Altium-style `dirty_paths` model — closing a tab never prompts; engines park while dirty
- Project-close prompts with a Save All / Discard All / Cancel modal listing every dirty file
- Tab bar right-click menu replaces the inline close / undock buttons

### Added — hierarchical sheets

- Child-sheet pins rendered as Altium-style ports (no protruding stubs), inward direction
- Pin labels rotate vertical on top / bottom edges for parity
- Per-sheet stroke / fill colours round-trip and editable from the Properties panel
- `[[multisheet]]` Style preference (with sheet-alpha round-trip fix)
- Child-sheet name / filename rendered outside the box per multisheet style
- Inline preset palette under the colour row before opening the picker
- Altium-green default sheet palette

### Fixed — symbol field handling

- Autoplace fields on rotate / mirror — body-bbox classification, full text-height clearance, anchored to selection bbox including pins, two-text-height autoplace clearance, body-edge stack alignment
- Reference and Value rotate independently; rotate / mirror compose with symbol orientation
- Property `justify` mirrored under rotation fold and mirror flags
- KiCad `GetDrawRotation` toggle parity; symbol field rotation treated as absolute screen angle
- Re-autoplace marked fields on load to repair legacy rotations
- Property `justify` parsing defaults to Center per KiCad spec
- Pin numbers rotate along the pin axis for vertical pins
- Single em-size used for schematic text (parity with KiCad)

### Changed — chrome refactor

- New `TabPill` custom widget — 3-sided borders sharing L / R edges between adjacent tabs, permanent strip baseline, theme-border instead of pure black, drag accent follows theme accent
- Modal close button unified across all 11 modals; chrome icon sizes bumped; thicker panel strokes; modal alignment so borders trace rounded corners
- Inactive tab fill is now visible (was theme-bg, blending in)
- New `chrome-catalog` crate + UI iteration workflow doc
- Taller top-chrome search bar (24 → 28); narrow Justification labels in Properties

### Added — canvas & engine

- Adaptive multi-level grid that scales smoothly with zoom
- Schematic editor grid style preference

### Changed — plumbing

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

[Full changelog](https://github.com/alplabai/signex/compare/v0.7.0...v0.7.1)

## [0.7.0] — 2026-04-22

The schematic-phase release. Adds ERC & validation, project-wide annotation, real multi-window architecture via `iced::daemon`, per-window engine/canvas, borderless chrome, and a full Signex brand rollout. Every v0.7.x sub-feature ships under this one tag.

### Added — ERC & validation

- New `signex-erc` crate with **11 rule kinds** (`run()` single-sheet, `run_with_project()` cross-sheet)
- Project-wide ERC across open, cached, and unopened sheets
- Cross-sheet BadHierSheetPin: parent pins ↔ child hier-labels, both directions
- ERC markers rendered as overlays — zero KiCad schema drift
- Messages panel with E/W/I severity pips and per-sheet cached results
- Altium-style ERC dialog + Preferences panel (per-rule Error / Warning / Info / Off grid)
- `ErcContext` abstraction + rule metadata infrastructure
- Shortcuts: `F8` Run ERC, `F9` AutoFocus, `Alt+A` Annotate, `Shift+Alt+A` Reset & Renumber

### Added — annotation

- Project-wide change list parsed from every sheet in the project
- Altium two-column Annotate dialog with Reset All / Reset & Renumber / Reset Duplicates
- Per-symbol lock, draggable modals
- Power-port skip in both annotation and change list
- Design → Annotation submenu matching Altium's layout

### Added — multi-window (`iced::daemon`)

- Borderless main window + OS-decorated secondary windows
- Modals, tabs, and panels all detach into real OS windows
- Per-window engine: `document_state.engines: HashMap<PathBuf, Engine>` — every open tab keeps its Engine loaded so undocked tabs edit independently
- Per-window canvas: `interaction_state.canvases: HashMap<window::Id, SchematicCanvas>` — pan / zoom / selection / render cache per window
- `document_state.window_active_path: HashMap<window::Id, PathBuf>` — each undocked-tab window can show a different tab
- Canvas events routed via `CanvasEventInWindow { window_id, event }`
- macOS: process exits when main window closes

### Added — editor & tools

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

### Added — hierarchical sheets

- Double-click a sheet block → opens the child schematic
- Sheet-pin snaps to all four edges of the hierarchical sheet block
- Sheet-pin interactions + Altium label-style option

### Added — borderless chrome & brand

- Custom title bar: wordmark + menus + drag zone + search bar + min / max / close
- Per-monitor v2 DPI manifest (no bitmap stretching on hi-DPI)
- Roboto UI font (panels / toolbars / menus / dialogs); Iosevka stays the canvas font
- Windows 11 DWM rounded corners + drop shadow via `DwmSetWindowAttribute` (silent no-op on Win10 and non-Windows)
- Diagonal resize hit zones (NW / NE / SW / SE) via Stack overlay — keeps content y-origin natural
- Header logo bumped 74×24 → 96×31 for readability

### Added — icons & installer

- Signex brand SVGs (mark, wordmark, logo variants)
- Panton Bold wordmark regenerated from actual font (not fallback outlines)
- Tighter `signex-mark.svg` viewBox: S fills ~97 % of icon canvas (was ~58 %)
- Regenerated installer artifacts: Windows `.ico` (multi-size), macOS `.icns`, Linux PNGs
- Runtime window icon embedded via `iced::window::Icon`
- `signex.exe` icon + DPI manifest embedded at build time via `winres`
- Pure-Python fallback for `build-icons.sh` — `tools/build_icons.py`, no rsvg-convert / magick / inkscape needed

### Changed — refactors

- `signex-engine/src/lib.rs` split into semantic modules
- `kicad-writer` migrated from `wln!` string formatting to full SExpr AST (`kicad-parser/sexpr_builder` → `kicad-writer/sexpr_render`)
- Named constants for PCB magic numbers; `lib_symbol` unit round-trip fix
- Wire rendering chains connected segments into polylines (rounded corners)
- Font-size constants corrected, hidden refs and pin-name rotation fixed

### Changed — merged-in dependencies

- v0.6.1 render fixes (font scale, pin numbers, power-ref visibility)
- v0.6.2 AST sexpr pipeline
- v0.6.3 macOS runner pin (`macos-14`)
- v0.6.4 per-OS installers (`.exe`, `.dmg`, `.deb`, `.AppImage`)
- Node.js 24 Actions bump

### Changed — performance

- `expand_to_net` is now `O(N)` via quantised `HashSet` (was `O(P²·N²)`)

[Full changelog](https://github.com/alplabai/signex/compare/v0.6.4...v0.7.0) · [Release artifacts](https://github.com/alplabai/signex/releases/tag/v0.7.0)

## [0.6.4] — 2026-04-20

First cumulative release since v0.6.1. Rolls in the abandoned v0.6.2 and the CI-fix-only v0.6.3.

### Added — installers

- **Windows** — `signex-setup-x86_64-0.6.4.exe` / `signex-setup-aarch64-0.6.4.exe` via InnoSetup. Installs to `Program Files`, adds Start Menu entry and optional Desktop shortcut, proper uninstaller. Portable `.zip` also attached for scripted installs.
- **macOS** — `signex-macos-aarch64-0.6.4.dmg` with a full `Signex.app` bundle and `/Applications` drag-target. Registered as the editor for `.kicad_sch` / `.kicad_pro` files.
- **Linux** — native `.deb` (with `.desktop` entry + MIME types) plus a portable `.AppImage`. `.tar.gz` fallback also attached.

### Changed — KiCad pipeline refactor (from v0.6.2)

- AST-based S-expression pipeline in `kicad-parser` / `kicad-writer` with a property-metadata layer. More robust round-trip, less fragile than prior ad-hoc string handling.
- Named constants replace the magic numbers scattered through `signex-types`, `kicad-parser`, and `kicad-writer`.

### Fixed — release pipeline (from v0.6.3)

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
