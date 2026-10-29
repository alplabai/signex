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

## [1.0.0-rc.2] — 2026-10-29

_v1.0.0-rc.1 was tagged but never published: its release run crashed in the test step before any build. rc.2 is the same release with the release workflow fixed._

_The first v1.0 release candidate: a schematic-only Community Preview. PCB and footprint editing are present but are not part of the v1.0 promise, and this build is a pre-release._

### Highlights

- **Typed component values** — a number field, an SI prefix picker and a fixed unit replace free text; typed values sort by magnitude and take a range filter (#1572).
- **Stock library with placeable parts** — passives, discretes, connectors and 74HC logic ship in every installer and mount on first run, and a library part follows the cursor until you click (#325, #929, #327, #328, #1252).
- **ERC rule profiles, waivers and reports** — a rule set per project with its own profile screens, waivers with a reason (per finding and per net), completeness rules, and reports as text, CSV, JSON, HTML and PDF (#1085, #1608, #1285, #1581, #1286, #1287).
- **Library integrity** — drift from the library is shown on placed parts, a stale save no longer overwrites a newer edit, two Signex windows no longer overwrite each other's library edits, library writes are crash-safe, and a failed git commit no longer fails a save: Messages says "Saved, not committed" and the library browser shows an uncommitted-changes banner with a Commit now button (#912, #1108, #892, #904, #104, #858).
- **Saves and edits off the UI thread** — saving, exporting, opening from the project tree, Annotate, ERC, netlist rebuilds, library edits and Preferences saves no longer freeze the window (#859, #1420, #893, #858, #1602).
- **Net navigation** — follow a net across sheets, a deterministic name for multi-named nets, buses across sheets and net classes (#1270, #1271, #1272, #1043, #1049, #1042).
- **Recovery and autosave** — optional autosave of recovery copies and restore of unsaved edits after a crash, for schematic, symbol, footprint and PCB editors (#104, #1164, #1185).
- **Keymap and menu unification** — shortcuts follow the menu's enabled state, context-menu rows read their label and shortcut from the command catalog, and Move and Move Selection are distinct (#907, #271).
- **Native dialogs and menus** — dialogs open as their own windows and the main menu lives in the macOS system menu bar (#1632).
- **Projects tree hierarchy** — the Projects tree shows the sheet hierarchy (#1618).
- **Schematic editing depth** — Break Wire, Find Similar Objects, project-wide Find and Replace, system-clipboard paste, repeat placement with auto-increment, and Place Sheet and Sheet Entry tools (#1254, #1257, #1268, #1263, #1253, #358, #1275).
- **Altium project import** — File > Import > Altium Project… converts an Altium project, and a running import can be cancelled (#1097, #1132).

### Install

Pick the package for your platform from the assets below. `SHA256SUMS.txt` lists the SHA-256 of every file.

| Platform | Asset |
|---|---|
| Windows (x86_64, ARM64) | `signex-setup-<arch>-1.0.0-rc.2.exe` installer, or `signex-windows-<arch>-1.0.0-rc.2.zip` portable |
| macOS (Apple silicon and Intel) | `signex-macos-universal-1.0.0-rc.2.dmg` |
| Linux (x86_64, ARM64) | `signex_1.0.0-rc.2_<arch>.deb`, `Signex-1.0.0-rc.2-<arch>.AppImage`, or `signex-linux-<arch>-1.0.0-rc.2.tar.gz` |
| All | `signex-1.0.0-rc.2.cdx.json` (CycloneDX SBOM) and `SHA256SUMS.txt` |

- **These builds are UNSIGNED.** On macOS, right-click Signex and choose **Open** on first launch (Gatekeeper); the app carries only an ad-hoc signature and is not notarized. On Windows, SmartScreen shows "Windows protected your PC": choose **More info**, then **Run anyway**. On Linux nothing is signed, and `SHA256SUMS.txt` is an integrity checksum, not a signature.
- **Linux needs glibc 2.35 or newer** (Ubuntu 22.04, Debian 12 and later); check with `ldd --version`.
- **This is a pre-release.** GitHub's `releases/latest` still points at v0.15.0; get this build from the Releases page.

### Known issues

- **Synchronous IO still in `update`** — a few paths still do IO on the UI thread (#859).
- **One Document model** — a `DocumentId`, an engine-owned history and a single dirty source for every document kind is not finished (#661).
- **User guide** — the guide is written, but the fresh-install walkthrough on a clean machine is still pending (#322).
- **Ruleset drift** — `.github/rulesets/*.json` have drifted from the live `trunk` and `main` rulesets (#1723).
- **README screenshots** — they predate the clean-room renderer and show legacy `.kicad_sch` file names (#1724).

### Detailed changes

Every change since v0.15.0, grouped by type, is in [CHANGELOG.md](https://github.com/alplabai/signex/blob/v1.0.0-rc.2/CHANGELOG.md#detailed-changes-in-100-rc2).

[Full changelog](https://github.com/alplabai/signex/compare/v0.15.0...v1.0.0-rc.2)

## Detailed changes in 1.0.0-rc.2

All entries folded from the per-PR fragments for 1.0.0-rc.2 (first prepared as 1.0.0-rc.1). They sit under their own heading because the release workflow publishes only the `## [1.0.0-rc.2]` section above as the Release body, and the full list is far over GitHub's 125,000-character limit.

### Added

- **Unknown TSV columns are preserved (#315)** — a `.snxsch` / `.snxpcb` bulk block with a column this build does not know no longer fails to load with a header mismatch; columns resolve by name, the extra cells stay attached to their row through an edit, and they are re-emitted after the known columns (in their original relative order) on save. A missing known column and a repeated column name still error. A block may carry at most 256 foreign columns, each name at most 256 bytes; a file over either cap fails to load, and a save fails (before any byte is written, the previous file stays intact) on the same caps or when a foreign column repeats a known column name, each with an error naming the offending column.
- **Find next / previous, previous grid, document tab cycling (#103)** — `find_next`/`find_previous` step the Find matches, `previous_grid`/`cycle_snap_grid_backward` step the grid back, and `next_document_tab`/`previous_document_tab` cycle every open tab instead of reporting "not available yet".
- **Optional autosave of recovery copies (#104)** — Preferences > Editing has a new "Autosave recovery copies" switch, off by default. When on, Signex writes a recovery copy of each schematic with unsaved changes about once a minute, into a `recovery` folder under the Signex config folder. Your own file is never touched by it. The copy is removed when you save the document, close it without keeping the changes, or quit with Discard All, even if you turn the switch off first. Restoring from a recovery copy after a crash is not in the app yet.
- **Library server setup and limits documented (#115)** — `crates/signex-library-server/README.md` lists every environment variable, the startup refusals on a public bind, TLS-behind-a-proxy deployment and the single shared token limit of the preview.
- **User guide walkthrough with screenshots (#322)** — The user guide now has a place, wire, annotate, ERC, save and PDF walkthrough with three screenshots rendered headlessly by an ignored test (`cargo test -p signex-app --test guide_screenshots -- --ignored`).
- **Bundled stock footprints are now a mountable library (#324)** — `assets/stock-library/Stock.snxlib` wraps the five stock footprints with a pinned library id, personal workflow, a seeded class registry and an empty component table, plus a `symbols/` directory. The footprint files are renamed to `<footprint uuid>.snxfpt` so the library lists and loads them. `build_stock_library` generates the manifest and a rerun leaves the tree byte-identical.
- **Start screen: Open Sample Project and recent projects (#331)** — the no-project start screen now has an Open Sample Project button and a short list of the five most recent projects (newest first, hidden when there are none). Each entry opens through File ▸ Open Recent, so a project that has since vanished is dropped from the list and reported the same way.
- **Pin type conflict ERC rule (#348)** — the ERC dialog's pin connection matrix now drives a real rule: two pins on one net whose types the matrix forbids (two outputs, an output on a power rail, a driven no-connect) raise a "Pin type conflict" in the Messages panel at that cell's severity, locatable by click. Setting a cell to Off removes the finding, Warning downgrades it, and the choice survives a restart; clicking a cell now updates its mirrored cell too. Tri-state, open-drain, clock and differential pins are treated as bidirectional, and passive / unclassified pins never conflict.
- **Generic No ERC places a real directive (#350)** — the active-bar entry, context menu and `place_no_erc` shortcut now arm a dedicated tool that drops a No-ERC directive (drawn as a diamond, distinct from the no-connect X) into the sheet's No-ERC directives instead of a no-connect. Directives undo/redo, can be selected and deleted, and round-trip through save. In the PDF export, "No-ERC Markers" now controls directives and a new "No-Connect Markers" toggle controls the no-connect X marks.
- **3D importer fuzz target (#708)** — a cargo-fuzz target now feeds arbitrary STEP, VRML, glTF and GLB bytes to `import_model`, and the fuzz CI job runs it for 60 s beside the schematic reader.
- **Sync Sheet Pins (#359)** — right-clicking a selected sheet symbol now offers "Sync Sheet Pins", which reconciles its pins with the child schematic's Hierarchical and Global labels (live tab if open, else the file on disk) through `Command::ReconcileChildSheetPins`, so it is one undo step and refreshes the netlist. A sync never moves an existing pin: matched pins keep their position and direction (so attached wires keep their nets), hand-added pins that match a label stay hand-added and in place, new pins take a slot free of other pins, of pins dropped in the same sync and of everything on the parent sheet a pin there would connect to (wires, labels, junctions, no-connects, power ports and other sheet symbols' pins), with their label's shape as direction (bidirectional if none), and the auto-generated pins of removed labels are dropped. The Messages panel reports which pins were added and removed, any port left without a pin because that sheet symbol's edge has no free slot, plus a missing, unparseable or port-less child. Port names are trimmed the same way by the sync, the project netlist stitcher and the ERC hierarchy rule.
- **Help ▸ Open Sample Project (#330)** — a small original 5 V to 3.3 V linear-regulator schematic ships inside the binary (source under `crates/signex-app/assets/sample-project/`, generic values, Apache-2.0). It is self-contained (symbols embedded, no library needed), passes ERC with zero violations, and is unpacked once into `<config_dir>/signex/samples/linear-regulator/` so it is editable and saveable; a copy that already exists is opened as-is and never overwritten. An unpack or open failure raises the Open Failed notice and is logged.
- **Edit a placed sheet symbol's Name and File (#794)** — the Name and File rows of a selected sheet symbol in Properties are now editable (Enter commits). The edit is one undoable step; an empty name, an empty or non-`.snxsch` file, a duplicate name or file among the sheet's symbols, or a file path leaving the sheet's folder is refused with a message and changes nothing. Re-pointing only changes the reference; no file is created or renamed.
- **Read-only library mounts (#329)** — `LibrarySet::mount_read_only` / `ReadOnlyAdapter` mark a mount read-only so every adapter write fails with the typed `LibraryError::ReadOnly`, whatever the install directory's permissions; the Library Browser disables Add Component, Delete, cell editing, table and class edits, and Pick Symbol / Pick Footprint for such a library and explains that it "ships with Signex and is not editable". Component Preview tabs for such a row render dimmed and inert, every refusal is also logged to the Messages panel, paths are compared in resolved form so symlink or `..` spellings cannot bypass the guard, and New Component skips read-only libraries. The stock library is not auto-mounted yet (#328), so `LibraryState::open_library_read_only` is the mount entry point it will use.
- **PCB corpus fixtures (#336)** — 30 generic `.snxpcb` goldens now cover every pad type, pad shape and via type in the round-trip corpus.
- **PCB zones can store holes (#818)** — the `.snxpcb` format now records cut-out polygons inside a zone, so a copper zone with a keep-out island round-trips instead of needing an overlapping second zone. Rendering still fills the outline only.
- **Round-trip corpus covers `.snxprj` and hierarchical sheets (#337)** — the fixture corpus now includes project files (including unknown keys and the format token) and a three-level child-sheet hierarchy, so a no-edit save of either is guarded in CI.
- **Place a library symbol onto the schematic (#364)** — placing a row from a mounted `.snxlib` now registers the symbol's `LibSymbol` on the active sheet and places a part that resolves against it, so per-unit bodies and pins (including Part Zero pins) reach the netlist. Registering and placing are one undo step; failures report to the Messages panel.
- **Board zones keep fields this build does not know (#894)** — a zone key written by a newer build (such as zone holes from #818) survives being opened and saved by an older one instead of being dropped.
- **PCB zones draw their holes (#898)** — cut-outs inside a copper zone are left unfilled and outlined on both the GPU and CPU render paths, instead of drawing as solid copper.
- **Italic canvas text (#654)** — schematic text marked italic now draws in a bundled italic Iosevka on both the CPU and GPU paths instead of upright, and bold italic keeps its weight.
- **Release artifacts are smoke-tested before upload (#342)** — each release build leg now installs or mounts the .deb, AppImage, Windows installer (x64) or DMG it just built, runs `signex --version` and fails the release unless the output matches the tag version.
- **Designator and value in the symbol editor (#170)** — the designator and value fields are drawn on the symbol canvas and can be clicked and dragged like pins (gated by the Designators / Values filter pills); a moved position is saved with the symbol, and untouched symbols save unchanged.
- **Move and Move Selection are distinct (#271)** — in the schematic Active Bar, Move picks the object under the next click and drops it on the click after; Move Selection does the same for the current selection from a clicked reference point; Esc, right-click, a tool change or a tab switch cancels. Altium's `M M` now triggers Move.
- **Zoom speed preference (#170)** — Preferences > Appearance > Navigation has a Zoom speed slider (5–50% per wheel notch, default 10%) that every canvas uses; trackpad scrolling now zooms proportionally instead of a full step per event, and the footprint editor's notch moves from 15% to the shared default.
- **Stock passive and discrete symbols (#325)** — `assets/stock-library/symbols/` now ships 20 `.snxsym` files: R, C, C_Polarized, L, Ferrite, Fuse, D, D_Schottky, D_Zener, LED, Q_NPN, Q_PNP, Q_NMOS, Q_PMOS, Crystal, the GND / VCC / +3V3 / +5V power ports and a generic 8-pin IC body. Every pin connects on the 1.27 mm grid, and `build_stock_library` regenerates them byte-identically.
- **Stock connector and 74HC logic symbols (#929)** — 11 connector and 7 74HC logic symbols in the stock library generator.
- **macOS release is now a universal `.dmg` (#320)** — one `signex-macos-universal-<version>.dmg` runs on both Apple silicon and Intel Macs; the release workflow cross-builds both slices on the macos-14 runner and joins them with `lipo`, failing if either architecture is missing.
- **Stock library ships in every installer (#327)** — the Windows installer, `.deb`, AppImage and macOS `.dmg` now bundle `Stock.snxlib` with its `footprints/` and `symbols/` folders next to the app, and a Windows uninstall removes them.
- **Bundled Stock library mounts on first run (#328)** — the Components Panel now has a Stock section, populated from the library shipped with the install (or `assets/stock-library` when running from a checkout) without any entry in `global_libraries.toml`. A missing or unmountable stock library logs a warning and leaves the section empty; the app still starts.
- **Library symbol drift (#912)** — when a library symbol changes after it was placed, the Properties panel of the placed part shows "Library: out of date" with an "Update from library" button, and the Messages panel logs how many symbols on a sheet differ. The check runs off the UI thread when a sheet opens, a library mounts, a primitive is saved, or after undo/redo, comparing a content hash of the embedded copy with the library symbol (no file-format change). Update replaces the sheet copy through `Command::ReplaceLibSymbol` as one undo step and is refused, with the affected pins named in the Messages panel, when a pin carrying a wire, junction, label or no-connect would vanish or move, when a new pin would land on an existing connection, or when a placed unit no longer exists; Update changes every instance of the part on that sheet, and power ports get the same Library row and button; "Keep" is simply not updating. Properties, the hover card and the BOM LibRef column now show `Library/Symbol` names instead of raw ids where the library is mounted.
- **Annotate undo is one project-level step (#829)** — Ctrl+Z after a project-wide annotate reverts every touched sheet together, with a Preferences > Schematic > Annotation option to keep the old per-sheet undo.
- **A child sheet placed twice gets its own designators per placement (#829)** — the netlist, Protel export, BOM and printed pages resolve each symbol's designator per sheet placement from its instance entries. A child placed more than once prints as one page copy per placement (titled with its sheet-symbol name, one picker row each), and a project that uses instance entries says so once on open.
- **Release builds can be code-signed (#318, #319)** — the release workflow signs the Windows exe and installer and signs and notarizes the macOS dmg once the signing secrets are configured; without them releases build unsigned as before. See docs/RELEASE_SIGNING.md.
- **Keep wires connected when moving (#963)** — moving symbols, child sheets and sheet pins (drag, Move, Move By, align) now drags the wires on their pins along, bending them to stay orthogonal. A net check after the move keeps this from changing connectivity beyond what a plain move would do; a pin that cannot be kept connected is moved as before and reported in one Messages notice. Preference on by default (Preferences, Schematic, Editing).
- **Text stores its font family (#510)** — labels, notes and symbol reference/value text can name a font family, with a sheet default and a bundled Iosevka fallback, and files survive a re-save by older builds with every family intact.
- **Buses carry the signals their label names (#1048)** — a bus now carries the signals its label names. One shared parser reads `D[0..7]`, `D[7..0]`, `D[7:0]` and `{SDA SCL}` groups and refuses malformed text. A bus run takes its members from its labels without ever merging them, and ERC gains five warnings: a bus-entry wire with no label, a label that is not a member of its bus (such as `D33`), a bus with no valid name, two names on one bus that carry different signals, and a bus-name label that sits on a wire instead of a bus. A bus-name label now counts as on its bus anywhere along it, for the orphan-label and hierarchical-port checks as well as the width check (#1048).
- **Buses across sheets, width conflicts (#1049)** - ERC now also reports one Port bus that appears at two widths in the project (`D[0..7]` against `D[0..3]`).
- **Bus entries snap onto a bus and can be rotated and mirrored (#1050)** — the bus end of a new entry snaps to the nearest bus within one grid step, and Space, Shift+Space, X and Y turn and flip it both while placing and on a selected entry, always about its bus end. The orientation is the sign of the entry's size, so no file format changes and old files load and re-save unchanged. Bus entries are now drawn on the schematic canvas in the wire colour. Dragging a bus together with its entries and wires is not part of this change yet.
- **New ERC rule "Net has multiple names" and a `net.names` field for custom ERC rules (#1043)** — a net that carries several names is listed with each name and its position, and says which name wins on that sheet (Info by default; the check is per sheet, cross-sheet is tracked in #1061). Custom ERC rules can match any name on a net with `net.names`, for example `net.names matches "^I2C_"`. See docs/NET_NAMING.md.
- **Net colours are saved with the schematic (#1041, #1065)** — colours set with the Active Bar net-colour pen are stored per sheet in the `.snxsch` and come back when the sheet is reopened. A colour change (pen, Clear All, undo, redo) marks the sheet as having unsaved changes; nothing is written until you save. Colours of wires and junctions that were deleted are dropped on save. Files with no colours are saved exactly as before.
- **ERC reports multiple net names across sheets (#1061)** — "Net has multiple names" now checks the whole project: a Global `I2C_SDA` on the root wire of a sheet pin and a Hierarchical `SDA` in the child are one finding listing every name with its sheet and position, and the name the netlist uses. See docs/NET_NAMING.md.
- **Cached library prices (#152)** — Refresh All Pricing fetches Mouser and DigiKey prices for each library row in the background, stores them in the `.snxlib`, and shows them in a Price column in the library browser.
- **arm64 Linux builds (#345)** — releases now include an arm64 `.deb`, AppImage and tarball, built natively on an arm64 runner.
- **Project parameters and special strings (#1080)** — a project keeps title-block values and custom name/value parameters in its Parameters tab, every sheet inherits them and may override one in its own Title Block, and `${...}` strings resolve the same way in any text note on the canvas, in the PDF and in the SVG page.
- **Sheet N of M follows the hierarchy (#1081)** — every placement of a sheet in a project gets one number, depth-first from the root, and `${SHEETNUMBER}` / `${SHEETCOUNT}` and the sheet cell print it in every export, whichever pages it includes. A manual number in a sheet symbol's placement entry overrides it; clashes, gaps and invalid numbers are reported in the Messages panel. A project's pages print in hierarchy order, and a page range in the print dialog selects sheet numbers.
- **Rotate and mirror any selection (#1039)** — Space, Shift+Space, X and Y now turn or flip a mixed selection of symbols, wires, labels and notes about its grid-snapped centre in one undo step; before, they only acted on a single selected symbol.
- **A project can carry its own ERC rule set (#1085)** — the `.snxprj` gains an optional `rules` section (rule severities and pin-matrix cells as stable string ids, in deterministic order). When a project has one, ERC reads it instead of this machine's preferences, so the same project gives the same result everywhere; a project without one behaves as before and is not rewritten. Unknown rule ids and sections written by a newer build are kept on save. Editing the rules, seeding from profiles and the drift note follow in later changes.
- **Software bill of materials (#362)** — every release now ships `signex-<version>.cdx.json`, a CycloneDX SBOM of the shipped binary's dependencies across all platforms, listed in `SHA256SUMS.txt`.
- **Import Altium projects (#1097)** — File › Import › Altium Project… converts a `.PrjPcb`, `.SchDoc`, `.PcbDoc`, `.SchLib` or `.PcbLib` into Signex files in a sibling `<name>-signex/` folder and opens them, using the optional [signex-altium-import](https://github.com/alplabai/signex-altium-import) companion. Existing files are never replaced. Everything the import had to approximate or skip is listed for review in the Messages panel and in `IMPORT_REPORT.txt` beside the converted files. A conversion that runs past five minutes is stopped and reported.
- **Keep wires connected when rotating or mirroring (#1115)** — rotating or mirroring a symbol, or a whole selection (Space, Shift+Space, X, Y, Active Bar), now drags the wires on the turned pins along, under the same net check as a move and as one undo step. A pin that cannot be kept connected is turned as before and reported in one Messages notice. Follows the "Keep wires connected when moving" preference.
- **New projects get a managed `.gitignore` (#1117)** — New Project and Enable Version Control keep a marked block of Signex-generated leftovers (atomic-write temp files, library write-lock files, OS clutter) at the top of the project `.gitignore` and, in project scope, a managed block in `.gitattributes` (LFS rules only when LFS is ticked). Your own lines sit below the block and keep overriding it; their permissions are kept. Links, read-only files, unbalanced markers and, on Unix, hard-linked files are refused before anything is written and logged to Messages. If the project folder is the root of an existing repository, the block lands in that repository's top-level `.gitignore`.
- **Net scope is fixed (#1074)** — a net label connects only on its own sheet, a Port connects by name across the project and a sheet entry through its sheet pin, with no setting to change it; a new ERC warning flags a local net label name used on two sheets, which usually means a Port was intended.
- **Cancel a running Altium import (#1132)** — While an import runs, the status bar shows "Importing <file>… ✕"; clicking it stops the converter at once.
- **Sheet frame and title block on the canvas (#1136)** — The schematic canvas draws the sheet border, zone markers (1, 2, 3… across, A, B, C… down) and the title block with the sheet's title, revision, date, company, file name and sheet number. One lowering feeds the canvas, PDF export and print preview; in the PDF the title block now sits in the bottom-right corner inside the frame instead of the top-right page corner.
- **Edit the title block (#1154)** — With nothing selected, Properties › Title Block edits the sheet's Title, Document No., Revision, Date, Company, Drawn By, Checked By and Approved By. Press Enter to apply a field; each change is one undo step. `${DOCUMENTNUMBER}`, `${DRAWNBY}`, `${CHECKEDBY}` and `${APPROVEDBY}` now resolve in exports.
- **Recover unsaved edits after a crash (#1164)** — when you open a schematic that has an autosave recovery copy that differs from the saved file, Signex asks whether to Restore it, Discard it or Decide later (Esc). Restore loads the copy as unsaved edits and writes nothing to your file until you save; a copy byte-identical to the file is removed without asking, and a differing copy is offered whatever its modification time. Quitting now waits for a recovery copy that is still being written, so a copy of edits you discarded no longer survives the quit. Annotate and Reset Duplicate Designators remove a recovery copy this run wrote for a sheet file they rewrite; an earlier run's copy of a sheet you have not opened is kept and offered when you open it. Only schematics are autosaved; symbol, footprint and PCB editors are not covered yet.
- **Buses across sheets (#1049)** — a sheet pin, a hierarchical label or a Port that names a bus (`D[0..7]`, `{SDA SCL}`) now joins its signals one by one: a sheet pin to the same bus on its child sheet, a Port to every same-named Port bus in the project, and a local net label bus stays on its sheet. ERC reports a bus sheet pin whose child port carries a different width (`D[0..7]` against `D[0..3]`), naming both sides, instead of calling the pair unmatched.
- **Net classes for the schematic, first slice (#1042)** — the `.snxprj` gains an optional `net_classes` table (name, member nets and `*`/`?` patterns, wire colour, width and line style), resolved to wires through the netlist. The Properties panel's General tab has a Net Classes section to add, rename, restyle, reorder and delete classes, each edit undoable and saved with the project. A project with no classes saves byte-identical to before. Drawing and exporting the class style comes next.
- **One sheet-size model (#1082, #1188)** — a typed `SheetSize` and orientation in `signex-types` now feeds the canvas frame, the PDF page and the Properties picker, replacing the separate paper tables. ANSI A-E sizes, `Custom <w>x<h>` pages and portrait (`A4 portrait`) are understood; every existing file keeps its `paper_size` text and re-saves byte-identical, and a size this build does not know keeps its text instead of being rewritten.
- **A project can store its own ERC rules (#1085)** — the ERC dialog has a "Store ERC rules in this project" button that copies this machine's severities and pin matrix into the `.snxprj`. After that the dialog edits the project's rules instead of the machine preferences, every edit can be undone and redone (Ctrl+Z / Ctrl+Y or the dialog's Undo and Redo), and ERC runs with the project's rules. Nothing is written until you save the project; projects without stored rules behave as before.
- **Net class styles drawn and exported (#1042)** — a net's class colour, wire width and line style (solid, dashed, dotted) now show on the schematic canvas (CPU and GPU paths) and in SVG, PDF and print preview, and wires are clicked at the width they are drawn. A per-net colour set with the net-colour tool wins over the class colour on wires and on a class-coloured junction (a junction with no class colour keeps the theme colour). A selected net label's Properties has a Net Class picker that assigns the net the project netlist puts the label on (a child sheet's local `LOCAL` is `/LOCAL`) to a class in the label's own project (one undo step); `(none)` is reserved and cannot name a class. Projects and sheets with no classes draw and save exactly as before.
- **Per-sheet frame settings (#1134)** — Page Options' Border and Zones section (formerly Margin and Zones) now sets, per sheet, whether the border and the title block are drawn and how many zones the border has. The canvas and the PDF export both follow it, and each change is undoable. Sheets that keep the default settings save exactly as before.
- **Sheet orientation and custom size in Page Options (#1196)** — the Properties panel now has an Orientation picker (Landscape or Portrait) and, in Custom mode, Width and Height boxes. Both are saved with the sheet and undo like any other edit, and a custom edge is held between 10 mm and 5080 mm, the range PDF page setup accepts. Picking another size keeps the sheet's orientation. A sheet whose paper size this build does not recognise is now named in the Messages panel when it opens; its stored value is left as written.
- **A child sheet shows the designators of the placement it was opened through (#1151)** — descending into a sheet placed more than once draws that placement's designators on the canvas, as the export prints them, and Properties, the hover card and Find & Replace show them too. Editing a designator there changes that placement only; the other placements keep theirs.
- **Project tab (#1202)** — Project Options in the project tree now opens the project as a document tab instead of a read-only window. Its General section shows the project name with a Rename… button (the same rename as the project tree), edits a new project description, and shows the project folder, root schematic, PCB file and library count. Description edits mark the project modified, Ctrl+Z / Ctrl+Y undo and redo them, Ctrl+S saves the project file without blocking the window, and closing the project or quitting asks about unsaved changes as for any other document. Projects without a description save exactly as before.
- **The BOM shows cached library prices (#1023)**: BOM lines placed from a library row, or matching its MPN, can show the cached unit price, the quantity bought (the line's quantity raised to the distributor's minimum order), the extended price, currency, stock, distributor and price age, without going online. Grouping is unchanged. Exports end with one total per currency that names the currency, and the preview's footer shows the totals and how many lines have no cached price.
- **Alternative symbol drawings on component rows (#1230)** — a library component can now carry alternative symbols (for example IEC and ANSI drawings) next to its default symbol, each with its own pinned version and label. Libraries without alternatives are written byte-for-byte as before and still open in older builds. The Component Preview lists them with add, rename, reorder and remove; adding one checks that its pin numbers match the default symbol and refuses a mismatch with a message in the editor and in the Messages panel. A placed symbol whose row has alternatives gets a Symbol pick list in Properties; switching is one undo step, and Library Updates treats a symbol placed from an alternative as bound to its row.
- **Several documents per library component (#1231)**: a component row can carry more than one document (a datasheet per manufacturer, an errata, an application note), each with an optional label and the first one as the primary. The Datasheet tab adds, removes, relabels and reorders them, and the Properties panel lists a placed part's documents. Rows with one unlabelled datasheet are stored byte for byte as before and keep their content hash. **Mixed versions:** once a row has a label or a second document, an older Signex cannot mount a `.snxlib` that holds it. Through a library server, the row schema rises to 3: the server refuses row writes from an older Signex, and Signex refuses to write rows to an older library server, so neither side drops the documents. Upgrade clients and the library server together.
- **Autosave recovery for symbol and footprint editors (#1185)** — with "Autosave recovery copies" on, unsaved symbol and footprint edits are copied to the recovery folder about once a minute, like schematics. Reopening the file offers Restore, Discard or Decide later, and saving removes the copy. The PCB editor is not covered yet.
- **Electrical grid snaps moves (#1246)** — with the electrical grid on (Shift+E), dragging or click-click moving a symbol, wire or label lands its pin tip or wire end exactly on a nearby pin, wire end or junction, and the move preview shows the same landing point.
- **Repeat placement with auto-increment (#1253)** — a repeated net label, port or text note counts up its trailing number (`D0`, `D1`, `D2`) by the Increment step in Preferences > Editing, and Repeat last item arms the last one again with the next value.
- **Break Wire tool and automatic merge of collinear wires (#1254)** — Break Wire (E W) splits the wire you click into two at that point, and wires that end up meeting end to end on one straight line after you draw or move them are merged into one in the same undo step, never where a junction, pin, label or other wire end sits; nets are unchanged.
- **Edit a part's designator and value in place (#1255)** — double-click either on the sheet, type, and press Enter (one undo step) or Esc to cancel; labels and text notes use the same editor, which now takes the keyboard focus when it opens.
- **Measure distance and zoom to selection (#1256)** — Ctrl+M arms a point-to-point Measure tool: click two points to read dx, dy and distance in the active unit on the canvas, in the status bar and in the Messages panel, without touching the document; `zoom_to_selection_area` frames the selection.
- **Find Similar Objects and drag-direction selection (#1257)** — Shift+F or the canvas menu row opens a dialog listing the seed object's properties, each Same or Any, scoped to this sheet or the whole project; OK selects every match on the sheet (matches on other sheets are counted in the Messages panel). A selection box dragged left to right now takes what it contains and right to left what it touches, switchable in Preferences > Editing.
- **Properties edits a multi-selection (#1258)** — with several parts, labels or wires selected, the shared fields (part value, footprint, DNP, In BOM, On board and parameters; label size, justification and rotation; wire width) are editable at once, differing values read `<mixed>`, and one change is one undo step. Label text is not edited in bulk, as that would merge nets. Wire width is a new edit; a net class width still wins over it.
- **Paste through the system clipboard and re-designate pasted parts (#1263)** — Copy and Cut also put the selection on the OS clipboard so another Signex process can paste it, and the Designators on paste preference keeps, resets or renumbers pasted parts.
- **Paste array and rubber-stamp copy (#1264)** — `rubber_stamp_copy` keeps the copy on the cursor until Esc, and the `paste_array` dialog places counted, spaced copies as one undo step with stepped label numbers.
- **Drag with or without wires per gesture (#1266)** — hold Alt through a drag to flip "Keep wires connected when moving" for that drag only, and `drag_keep_connections` (G in the Classic profile) drags the picked object with its wires attached whatever the preference says; the status bar shows which behaviour is active during the drag.
- **Create the child sheet from a sheet symbol, and a real Sheet Entry tool (#1275)** — placing a sheet symbol now opens a Create Sheet dialog (in its own window) with an editable file path, a Browse button and Create / Later; the same dialog opens when you descend into a symbol whose file is missing. Create writes the child beside the parent (sub-folders allowed, never outside the project, never over an existing file), with one port per entry, lists it in the project and sets the symbol's File field (undoable). The Sheet Entry tool places an entry on the symbol border, snapped to the nearest edge (undoable).
- **Waive one ERC finding with a reason (#1285)** — every ERC panel entry that names objects gets a "Waive" action with a required reason and author. Waivers are stored in the project (`rules.erc_waivers`), keyed by rule id and the involved objects' UUIDs, lapse (the finding shows again) when any of those objects changes, stay listed in the panel with their status and a Restore button, and are undoable. See docs/ERC_RULES.md, "Waivers".
- **Completeness ERC rules (#1286)** — ten new built-in rules: part without footprint, value or MPN, unannotated designator, single-pin net, net with no driver, power input not driven, multi-unit part with units not placed (judged across every sheet), off-grid pins and wires, and nets that carry only labels. Each has a default severity chosen not to flood existing projects (see docs/ERC_RULES.md) and can be overridden per project and in rule profiles.
- **ERC report as JSON, HTML and PDF (#1287)** — File > Export > ERC Report also writes a JSON report with a documented, versioned schema for tooling (docs/ERC_REPORT.md), a self-contained HTML page and a PDF, each with summary counts, findings grouped by severity then sheet with their location, and the project's waivers with reason, author and status.
- **Designator prefix follows a project standard (#1232)** — a project can pick IEEE 315, DIN 40719-2 or EN IEC 81346-2 in Project Options, and newly placed parts take their reference letters from the component class (a diode is `V` under DIN 40719-2); the default stays the library symbol's own letter. See docs/USER_GUIDE.md section 8.
- **De Morgan bodies work end to end (#1233)** — a placed symbol can show its library symbol's alternate body (Properties ▸ Body), and the netlist, ERC, exports and hit-testing follow the body shown; a switch that would disturb a wired pin is refused with the reason in the Messages panel. The symbol editor gets Normal / Alternate body buttons and "Both bodies" / "This body only" to author the second body. Files that use no alternate body are written byte-for-byte as before; `.snxsym` files with an alternate item are written as `snxsym/v3`, which builds without body-style support do not open (they accept only v1 and v2).
- **Alternate footprints per component row (#1234)** — a row can bind alternate footprints next to its default, each with its own version pin and pin map; rows without alternates keep their bytes and hash. Library row schema 5: a client or server that does not know alternate footprints is refused rather than allowed to drop them. Do not downgrade the library server once rows carry alternates.
- **Theme gains ERC/DRC marker colours (#1371)** — every theme (and exported custom themes) now carries `erc_error`, `erc_warning`, `drc_error` and `drc_warning` slots that the ERC and DRC markers draw from, and the project tree, passive calculator and active-bar dropdown take their colours and offsets from theme tokens and named constants.
- **Re-prefix placed parts to the project's designator standard, and a per-row prefix override (#1315)** — a **Re-prefix to standard** button in the Annotate dialog lists the changes first, then, on Confirm, re-issues the prefix of every part placed from a library row under the project's standard and renumbers it, as one undo step across every sheet. Sheets that are not open are loaded in memory and stay unsaved until you save; nothing is written behind your back, and one failing sheet undoes the whole run. Locked, hand-edited, unlinked, unmounted and read-only parts are left alone and listed in the Messages panel. A library row can pin its own letters with a `designator_prefix` text parameter, which both placement paths and Re-prefix honour.
- **Interface scale and dialogs that fit (#1227, #1228)** — Preferences → Appearance → Interface scale (80–200 %) scales every window on top of the OS display scale, and the ERC, Annotate, BOM and print-preview dialogs now fit the window with their buttons reachable on small or HiDPI screens.
- **Project-wide Find and Replace (#1268)** — Find and Replace has a Sheet / Project scope selector; Project searches every sheet, open or not, groups results by sheet, and Replace All across sheets is one undo step. Clicking a result opens its sheet and zooms to it, and F3 / Shift+F3 step through the results.
- **A library part follows the cursor until you click (#1252)** — placing a part from the Components panel or the Place Component picker now attaches it to the cursor instead of dropping it at the centre of the view. Space, X and Y turn it, Tab opens its value, designator and rotation, and every click places a copy with the next designator, one Undo step each, until Esc or a right-click.
- **Paste follows the cursor (#1252)** — Paste attaches the copied items to the cursor instead of dropping them at a fixed offset: Space, X and Y turn the group, one click places it as one Undo step, and the paste ends. Copy now carries the library symbols its parts use, so pasting into another sheet no longer leaves parts pointing at a symbol that sheet lacks. Paste Special keeps its fixed placement.
- **Search everywhere in the command palette (#1268)** — the palette lists designators from every sheet of the project (one row per placement), nets, and child sheets and each placement of a sheet placed more than once, and choosing a row opens that sheet and zooms to the item.
- **Component rows carry a description (#1474)** — a first-class `description` column on component rows in `.snxlib`, editable in Edit Details and shown and searched in the Place Component picker; the library format token is now `snxlib/2` and the row schema level 4, so older builds refuse these libraries.
- **PCB editor autosave (#1185)** — unsaved PCB edits get a recovery copy like schematics and library editors do, offered on reopen with Restore / Discard / Later.
- **ERC rule profiles (#1085)** — save a project's ERC rules as a reusable `.snxrules` profile, update a project from it after reviewing a diff, and get a note when the local profile has drifted.
- **Wires, labels, text notes and drawings can be locked (#1573)** — a locked object refuses move, rotate, mirror, delete and property edits like a locked symbol, shows a padlock on the canvas, and is locked or unlocked with the "Locked" checkbox in Properties; files that lock nothing save byte-identical.
- **Schematic drawings draw in file order, at the very back (#1579)** — lines, rectangles, circles, arcs and polylines now paint in the order they are stored, so a later drawing covers an earlier one, and all of them sit under wires, symbols, pins and text. A drawing circle is a smooth disc plus ring with its own fill and stroke colours instead of a 40-sided polygon. The GPU scene gains the ordered bucket that makes this possible; the PCB is unchanged.
- **Waive a net-level ERC finding per net (#1581)** — the ERC panel gets a "Waive net" action on net-level findings and a "N waived" list with Restore, stored in the project rule set by rule id + net name, undoable, and honoured by every ERC run.
- **Arrange drawings and place them above the circuit (#1583)** — Edit ▸ Arrange and the canvas right-click Arrange submenu (shown when a drawing is selected) bring the selected drawings to the front or back, or one place forward or backward, as one undo step that keeps their order among themselves; circuit elements in the selection are left alone and the rows are greyed without a drawing. The Properties panel has a Placement row that puts a drawing behind the circuit (the default) or above it, over symbols and wires but under text. Files that never choose a placement are unchanged.
- **Typed component values: shared quantity core (#1572)** — library measurement parameters now read and display through one quantity type: legacy values like `4.7 kΩ` and canonical base values read the same, units are checked across SI prefixes (a kΩ value satisfies an Ω slot), and values render without float noise in engineering style (`4.7 kΩ`, `100 nF`, `4700 µF`).

  Passive calculator results now come from the same formatter, with 4 significant digits: 4.7 mF shows as `4700 µF`, 0.5 F as `500000 µF`, and 999.96 kΩ as `1 MΩ`. Library template slots also reject a zero capacitance or inductance and a negative R, C or L value unless the slot sets `allow_zero` or `allow_negative`. Stored values, the browser cell edit and the content hashes of untouched rows are unchanged; the browser commit, the prefix picker and the component editor (#1606) move to the next phase.
- **Rule profile screens (#1608)** — new projects start from the chosen default profile, Preferences gains a Rule Profiles editor (list, pick the default, create, duplicate, rename, delete, edit with the ERC dialog's rule editor), Save as profile asks for a name and version and confirms before overwriting, the no-project ERC dialog now edits the `User Default` profile instead of `prefs.json`, and a project's profile drift is checked when it opens. A build from before this change keeps reading the old `prefs.json` rules, so it does not see edits made after the upgrade.
- **Projects tree shows the sheet hierarchy (#1618)** — child sheets nest under the parent whose sheet symbol references them, in every open project, and a symbol whose file does not exist shows as a `(missing)` row.
- **Typed values sort by magnitude (#1572)** — the component browser and the BOM Value column order 2 kΩ before 10 kΩ and 100 nF before 1 µF and keep each unit family together, and other browser text columns now sort in natural order (R2 before R10).
- **Main menu in the macOS system menu bar (#1632)** — on macOS the menus now live in the system menu bar, with an app menu (About, Preferences, Hide, Quit); Quit asks about unsaved changes instead of exiting at once. Windows and Linux keep the in-window menu.
- **Dialogs open as their own windows (#1632)** — confirms, warnings and most dialogs open in a native, draggable window; confirm and warning windows are sized to their content and lock the main window until answered, while tool windows such as ERC, BOM and Preferences stay usable side by side. Quitting with unsaved changes never discards anything the dialog did not list.
- **Dock panels no longer run under their scrollbar (#1632)** — panel content is laid out beside the vertical scrollbar.
- **Typed component values: number field, SI prefix picker, fixed unit (#1572)** — the Edit Component Details modal and the component editor's Parameters tab edit a measurement as a plain number, a unit picked from a list and an SI prefix picked from a list (`kΩ`, `µF`; hidden for `%`, `ppm/°C` and other units without prefixes). Units are never typed: a template slot fixes its unit, otherwise the editors offer a unit picker, and a unit string the editor does not know is shown as a read-only label and kept as written. Typing `4k7` or `4.7uF` into the number field is refused with a message as you type. Values are stored as the base value and the unit token (4.7 with `k` on an ohm slot is `{4700, "ohm"}`), and committing a number that did not change leaves the stored value, and so its content hash, as it was.

  The Library Browser's measurement cells open the Edit Component Details modal instead of taking raw text: a 90 to 110 px column has no room for a number field and a prefix picker. The Parameters tab now shows why a value breaks its template (zero capacitance, a negative R, C or L, wrong unit or kind) beside the slot. The library diff shows the raw stored values when two parameter values display alike, so a legacy `{4.7, "kΩ"}` becoming `{4700, "ohm"}` no longer reads as `4.7 kΩ → 4.7 kΩ`.

  A typed value that breaks its class-template slot is refused: zero where the slot does not allow it, a negative where it does not allow it, or a wrong unit is not stored, the field shows the reason, and Save is blocked until it is fixed. In the Edit Component Details modal a template slot fixes its unit (a label, no unit picker); other parameters keep the picker. A value that already breaks its slot stays as it is until it is edited, and pressing Enter on it without changing the number rewrites nothing. An edit past the 15th digit is stored, and a value that overflows with the picked prefix is refused instead of dropped on Save. Changing the unit of a free parameter keeps the number as shown and says the value is reinterpreted. The search index still reads raw values, so the numbers an edited legacy row indexes change until the range filter phase.
- **Sketch solver names redundant constraints (#1649)** — a constraint that only restates earlier ones (for example Horizontal applied twice) is listed under "Redundant" in the footprint editor's DOF / Last solve section, with click-to-select.
- **Open Datasheet, Edit Symbol and Change Component… on a placed part (#1301, #1262, #1260)** — right-click a placed part or use its Properties panel to open its datasheet (a link or a pinned PDF, with a choice when the library row lists several), to open its source symbol in the Symbol Editor (saving it then offers to update the placed copies), or to swap it for another library part; Change Component… keeps the designator and parameters, maps pins by number and then by name, lists connected pins that lose their wire before you confirm, and undoes in one step.
- **Placement shortcuts for power port, global label and hierarchical label (#1666)** — `P`, `Ctrl+L` and `H` (Classic profile) arm the same tools as the Place menu and Active Bar buttons. The junction shortcut stays unavailable: no junction tool exists yet.
- **Junction tool (#1666)** — a click places a junction at the point, undoable, from the Wiring group of the Active Bar, Place > Junction, or the `J` shortcut (Classic profile).
- **In-place edit shortcuts work (#1670)** — Edit value, Edit reference designator, Edit footprint field and In-place edit text open the inline editor on the object under the pointer, else the one selected part; the footprint editor opens at the part, and a command with no target says so in the Messages panel.
- **Follow a net across sheets (#1270, #1271, #1272)** — Go to net partner jumps from a port, global or hierarchical label or sheet entry to the next one on its net, Alt+Left and Alt+Right (Classic) step back and forward through the sheets visited and Up goes to the sheet symbol, the net highlight stays on across sheet tabs and steps through its pins, and resting the pointer on a wire, pin or label shows its net, class and pins (Preferences, Schematic, Editing turns it off). See docs/NET_NAVIGATION.md.
- **Range filter on typed values (#1572)** — the library browser and the Place Component picker filter a measurement column with an operator (≥, ≤, =, between), a number field and an SI-prefix picker in base units, so 4.7 kΩ and 4700 Ω match, and the filter says how many non-measurement rows it excluded.

### Added — schematic engine

- **`PlaceChildSheet` engine command** — adds a hierarchical sheet symbol to the schematic as one undoable step and marks `CHILD_SHEETS` dirty so the project netlist cache invalidates. A blank `filename` is rejected with `EngineError::EmptyChildSheetFilename`; an empty `name` is allowed. Engine only — the Place Sheet canvas tool is separate (#357).

### Added — schematic placement

- **Place Sheet tool** (#358) — toolbar **Sheet** (and the `draw_hierarchical_sheet` command, `S` in the classic preset, the Active Bar dropdown and the canvas context menu **Place > Sheet Symbol**) places a hierarchical sheet symbol with two clicks on opposite corners, with a rubber-band preview; Esc or `CancelDrawing` cancels and returns to Select, and an edge under 5.08 mm places nothing (a warning reaches the Messages panel). The sheet gets the first unused `sheetN` as both name and file (`sheetN.snxsch`, unused in the parent and the project); renaming it from Properties is not yet possible, and the child file is not created on disk. One undo step removes it.

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

- **Pre-release builds install and upgrade cleanly** — the `.deb` control `Version:` maps `X.Y.Z-rc.N` to `X.Y.Z~rc.N` so a release candidate upgrades to the final `X.Y.Z` (the file name keeps `X.Y.Z-rc.N`), and the macOS `Info.plist` `CFBundleShortVersionString` is the numeric `X.Y.Z` while `CFBundleVersion` keeps the full version string.
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
- **Unmapped keyboard shortcuts now act (#103)** — Zoom In / Zoom Out (`zoom_in_at_cursor`, `zoom_out_at_cursor`), Next / Previous Sheet and Unselect All now do something instead of swallowing the keypress. Single-key shortcuts no longer reach the canvas behind a blocking modal, and the Keyboard Shortcuts sheet no longer lists bindings that have no action (#103).
- **Dead shortcuts now explain themselves (#103)** — Edit properties, Leave sheet and Navigate up hierarchy now reach their existing handlers, and every other catalog command without a handler reports "<command> is not available yet" in the Messages panel instead of silently eating the keystroke.
- **Altium menu hints (#103)** — the Altium profile binds New Project (Ctrl+N), Save As (Ctrl+Shift+S) and Place Bus (P B), and its Ctrl+F and Ctrl+PageDown now name the Find and Fit All commands the menu shows, so those five rows show their shortcut again.
- **View menu panel check marks follow the panel (#103)** — Projects, Components, Navigator, Properties, ERC, Messages and Signal in the View menu, in the in-window bar and the macOS system menu, now show a check mark while the panel is open and clear it when it closes, including when a panel shortcut toggles it.
- **Document tabs show a dirty dot again (#110)** — a tab whose document has unsaved changes now carries a small dot in the theme error colour before its title, sized from the tab font. The Projects tree row keeps its own indicator.
- **Schematic symbols draw and hit-test their library body (#171)** — the canvas drew every symbol as a fixed 8 x 5.6 mm placeholder box and selected/hit-tested it by the same box, so tall ICs were mostly unselectable and pins were never drawn. Symbols now render their library graphics and pins, and bounds, hit-testing and drag previews share one symbol-geometry lowering in `signex-types` (also used by the SVG export). A symbol with no library geometry still falls back to the box.
- **GPU polygon outlines no longer drop out when the stroke is thicker than the edges (#199)** — a stroked polygon on the GPU path, a PCB zone or pad outline included, left gaps between its edge rectangles once the stroke was thicker than the edges were long (a tiny circle, an arc-lowered corner, anything the pixel floor thickens), and a translucent stroke blended twice at its joints. A corner now moves toward the miter point as the stroke thickens past the shorter of the two edges that meet there; where the stroke is at least as thick as the edge is long the neighbours meet exactly, with no gap and no overlap, and in between the notch only shrinks. A corner whose edges are both at least twice the half-width is drawn as before, and a non-finite vertex loses only its own edges. A circle smaller than its own stroke half-width still draws its inner edge crossed over the centre.
- **Sketch solver: zero-length lines and a deterministic, bounded solve (#685)** — A point-on-line or point-to-line constraint on a line whose endpoints coincide (only those two kinds; parallel, perpendicular, angle and a coincident constraint on the line's own endpoints are not covered and stay under #685) now reports "line … has zero length" (naming the line, in the Properties strip and the Messages panel), also when every point involved is fixed, instead of "solver internal error: initial residual: entity … not found". The 50 ms wall-clock limit is replaced by one work budget per solve (`Solver::work_budget`, default 125,000,000 work units, counting residual evaluations and the dense linear algebra), shared by the first run, the retry, the offset undo and conflict narrowing, so the same sketch solves, or fails with the same "exceeded its work budget" error, on every run of the same build and platform (`hypot`, `atan2`, `sin` and `cos` come from the platform's maths library, so two platforms can differ in the last bit). Measured in release on an Apple-silicon Mac, a solve that exhausts the budget takes roughly 90 to 150 ms; a 40-point chain (50 ms), a 16-rectangle grid (16 ms) and a small conflict (9 ms) solve as before. The analysis of a converged sketch is outside the budget, so an already-satisfied large sketch is never refused for its size (92 ms at n = 574 under-constrained, 400 to 520 ms at n = 510 fully constrained, as before). A sketch that still needs solver steps, one of which exceeds the budget (one step costs `(2n + 2)·m + m·n² + n³` work units, which passes the budget from n = 397 when m = n, or n = 437 when m = n / 2), is reported as "sketch too large for the solve budget" with the work it needs. When the budget runs out while the retry's start offset is being taken back out of the solution, the solve now fails instead of returning the offset geometry. A sketch with nothing to solve for (every point fixed) whose residual cannot be evaluated now fails as a solver internal error instead of returning `Ok` without evaluating anything.
- **A rejected sketch edit logs its conflict once (#685)** — making the same conflicting edit again no longer adds another Messages entry each time.
- **Project-tree row actions on a library are covered for two libraries sharing a file name (#731)** — regression tests pin that Remove, Rename, Exclude, Delete-file on disk and Enable Version Control act on exactly the clicked library when a project-local and a shared library are both called `Power.snxlib`, and that a row whose library has left the project is refused. Remove and Rename on a project-tree row that no longer resolves to a file now say so in the Messages panel instead of doing nothing. Enable Version Control on a library that is not on disk yet (a pending library) is now refused with a Messages line instead of opening a dialog that with nothing to track. The Remove dialog for a `.snxlib` shows the library's full path, so same-named libraries can be told apart.
- **A save refused for too many unknown TSV columns names the objects to delete (#768)** — when the objects of one block together carry more unrecognised columns than the limit allows, the refusal now lists the objects (uuid, and designator or kind where they have one) that hold the columns past the limit, each as its own Messages line (also from Save All on close and exit, and from Annotate), then "N more"; and a header that lacks a known column no longer echoes the whole untrusted header, only its column count and short prefixes of the first names.
- **Guard tests for TSV unknown-column saves (#768)** — an over-limit save through the engine is pinned to return the typed error without writing, and a file saved before unknown-column support re-saves byte for byte.
- **Close Project no longer discards another project's never-saved sheet (#771)** — a sheet that was created but never saved is now matched case-insensitively on a case-insensitive volume, so a sibling project referencing `n.snxsch` keeps `N.snxsch` out of Discard All. Case sensitivity is probed on an entry inside the directory (not the directory's own name, which measures the parent volume) and falls back to case-insensitive when no answer is possible. The ownership walk refuses network references outside the open project's own server and share, every device and non-disk verbatim path (a `\\?\C:\` verbatim-disk path is the same local disk and is followed), at most `max_sheets` listed refusals plus one `(+N more refused)` line, non-regular and oversized sheet files (opened without blocking), and walks past 10 000 sheets, treating each as unreadable (Discard All off); a project stored on a network share keeps Discard All, and a refused reference no longer makes Save All / Discard All re-prompt forever. The "re-read" log line now says what changed.
- **Undo after deleting a library footprint or symbol (#760)** — the delete recorded no history step, so Undo restored the snapshot of the previous edit: the deleted primitive came back and the earlier edit was silently reverted. Deleting is now an undo step of its own, and the Symbol editor's history snapshots the whole `.snxsym` container (not just the active symbol), so Undo can no longer write one symbol's edit into another after a symbol switch. Refused or out-of-range deletes now report to the Messages panel.
- **Exclude from Project on a dirty sheet asks first (#738)** — the Remove modal now moves to a Save / Discard / Cancel stage when the sheet has unsaved edits: Save writes it (a failed save keeps the prompt and logs to Messages) then excludes, Discard drops the edits (logged to Messages) then excludes, and Cancel / Esc change nothing. A clean sheet is excluded without a prompt, as before.
- **One unreadable `.snxsym` no longer blocks every symbol save (#690)** — a symbol container that fails to parse (hand-edited, truncated, or written by a newer format) is skipped and left byte-for-byte untouched, so other symbols in the library still save, load and list; it is reported once in the Messages panel with its path and a bounded parse error. A save that could duplicate a symbol held in the unreadable file is refused, and a lookup that finds nothing while a file was skipped returns an error rather than "not found". Nothing is migrated or rewritten automatically.
- **Renaming a project library keeps the project pointing at it (#739)** — a project-local `.snxlib` can now be renamed from the project tree. The file move and the re-point happen in one update, like a sheet rename: the `LibraryEntry` path follows (relative stays relative), and the mounted library, its open editors, browser tabs, pickers and dirty markers move to the new path, so no edit can recreate the old file or miss the renamed one. A saved project's library entry is patched atomically on disk, so a Discard, quit or crash cannot leave it listing a missing file, while the project's other unsaved edits stay dirty; a never-saved project is marked unsaved and a Messages-panel entry asks for a save. A library kept in its own repository records the rename as a commit. Shared and Global libraries are still refused (#657).
- **VRML importer no longer hangs, desyncs, or mis-places nested geometry (#689, #782)** — a `.wrl` containing form feed, vertical tab, NBSP, NEL or any other Unicode whitespace outside a comment no longer spins the lexer forever. An unknown node holding a `[ ... ]` list (such as `NavigationInfo { type [ "EXAMINE" ] }`) no longer swallows the rest of the file, a mismatched or stray closing bracket is warned about and skipped, and a node-valued field such as `normal Normal { ... }` or `coord DEF c Coordinate { ... }` no longer drops the mesh or its colour. `Transform` now composes per VRML97 (`T * C * R * SR * S * -SR * -C`), so `rotation`, `center` and `scaleOrientation` apply, a child translation is scaled and rotated by its parent, a mirroring scale keeps triangle winding, and a non-finite or overflowing transform drops the affected triangle with a warning instead of emitting NaN.
- **Column sort is a total order (#721)** — a library column mixing numbers and text (`9`, `10`, `10k`) or containing `NaN` no longer risks a `slice::sort_by` panic. Numeric cells now sort before text cells (numbers by `f64::total_cmp`, text case-insensitively); a literal `NaN` cell sorts as text.
- **Symbol Reload and footprint switching no longer leave stale state (#780)** — reloading a symbol from disk now clears its unsaved-changes marker like the footprint reload does, so Quit no longer asks to save a file that already matches disk; switching, adding or deleting footprints in a library file now ends the current undo coalescing run, so editing footprint A, visiting B and editing A again gives two undo steps instead of one.
- **Symbol Editor text and filled-circle hit boxes match what is drawn (#683)** — a Text graphic was selectable (by click and by box/marquee select) only inside a ±size/2 square round its anchor, although it draws right and down from it, so clicking the word missed and clicking empty space up-left selected it; click and marquee now share one estimated drawn extent (longest line by 0.6 em per glyph, lines by size, padded with the grab tolerance for clicks). A filled Circle also accepts clicks on its interior, as filled Polygons already did.
- **PCB grid honours Grid Toggle and Visible Grid Size (#682)** — the PCB canvas now follows the app-wide grid state, and a grid too dense at the current zoom steps up by whole multiples of the real pitch instead of drawing a fake 8 px pitch.
- **Schematic reference/value fields render from their stored text properties (#171)** — the canvas drew the reference and value at fixed offsets while hit-testing and drag used the stored `TextProp`, so dragging a field moved its hit box but not the text. Draw now uses the stored position, size and justification, with rotation and justification resolved relative to the symbol exactly as the SVG export does (one shared `field_effective_style`), and the field hit box follows the same justification and orientation (a hidden field is not drawn), falling back to the old offsets only when a symbol carries none, and the field hit box no longer re-applies the symbol transform to a world-space position. Labels and text notes are also culled by their extent rather than their anchor point, so long text no longer pops out while mostly on screen.
- **Deleting a sketch point no longer leaves dangling references** — the delete now cascades structurally: every line, arc or circle built on the point and every constraint or array (source or polar centre) naming a removed entity go with it, as one undo step, and the Messages panel reports how many dependents were removed. Deleting a pad in Pads mode takes the same path, so an array whose source was the pad no longer survives it. Constraints were previously matched by searching their `Debug` text for the deleted id, which missed `Horizontal` and `PointOnLine` and left the lines and arcs in place.
- **A sketch that names a missing entity fails the solve with a typed error** — `SolveError::DanglingReference` lists the missing ids instead of the solver reading entities that are gone; the failure reaches the Properties warnings and the Messages panel, the sketch stays editable, and deleting any entity heals it. Footprints already on disk with dangling references are repaired on load, and the editor that opens one reports the removal in the Messages panel and marks the document dirty.
- **Four dead schematic controls now work (#791)** — View ▸ Cycle Grid Size steps the snap grid (and the visible grid, when linked) through the grid-size presets; View ▸ Zoom In / Zoom Out zoom one wheel notch about the view centre on schematic and PCB; the Annotate dialog's All On / All Off unlock / lock every listed symbol instead of closing the dialog; "Update Changes List" is removed because the change list is already live.
- **BOM export no longer carries spreadsheet formulas or raw HTML headers (#109)** — a CSV cell (header row included) starting with `=`, `+`, `-`, `@`, TAB or CR is prefixed with `'` and quoted so Excel reads it as text, except a signed number with a unit such as `-5V` or `+3.3V`, which stays as is; the ERC CSV shares the rule. The HTML BOM escapes custom column headers, and the XLSX header row now gets its bold, grey, bordered format.
- **Canvas editing correctness leftovers (#102)** — switching placement tools (toolbar or Place menu) now drops the previous tool's in-flight wire, arc, polyline and shape-anchor buffers, while re-selecting the active tool keeps the gesture; switching to another schematic tab clears the selection and the Properties panel's copy of it; a double-click that commits a shape snaps to the grid like a single click, with one shared snap rule (no divide-by-zero at grid size 0); and the double-click distance is 5 screen pixels rather than 3 mm of world space, so it no longer scales with zoom.
- **STEP and glTF import hardening (#708)** — digits and `#` inside a STEP name string no longer move a vertex or abort the import; a STEP face with several bounds is built from its outline only (the marked outer bound, else the largest loop) and the skipped holes are reported once per model; STEP and glTF model reads, and all external glTF buffers and textures together, refuse a non-regular file or more than 256 MiB before allocating, and a `.gltf` JSON text is capped at 32 MiB; glTF URIs are percent-decoded before the traversal check (`my%20tex.png` resolves, `%2e%2e/` is rejected); a corrupt `.glb` source is rejected instead of cached. VRML reads are not covered here (#778), and STEP holes are still not cut.
- **Distributor cache and version-control init writes are crash-safe (#104)** — `DistributorCache::put` and `enable_project_version_control` (`.gitattributes` / `.gitignore`) now write via `atomic_write` instead of `fs::write`, so a crash mid-save cannot leave a truncated cache entry or truncate a user's existing `.gitignore`.
- **Failures are shown, unsaved drafts are kept (#110)** — a failed Ctrl+S, project create or Component Preview row save now raises the Save Failed card; Close Library > Save All keeps the library open when a row fails to save; closing a dirty symbol, footprint or Component Preview tab parks the draft instead of discarding it; History Restore over unsaved edits asks for confirmation first and reports failures; a Custom theme survives a panel refresh.
- **PDF export page setup follow-ups (#810)** — the preview now re-renders in the background, so changing page size, margins or scale no longer stalls the window, and a slow render of outdated settings can no longer overwrite a newer preview. Margin and refit notices show in the warning colour instead of the error colour, a tiny non-zero margin no longer reads as `0`, and the modal reopens with the page setup you last used for the same document (this session only).
- **Sync Sheet Pins follow-ups (#812)** — with several sheet symbols selected, the sync now reconciles every distinct selected child file instead of only the first, and reports each file separately in the Messages panel, as a single undo step (a missing or unparseable file no longer stops the others). A failed engine command now says so instead of also logging "pins already match its labels". A blank Hierarchical or Global label in the child, which names no port, is now reported in the Messages panel rather than silently ignored.
- **`.snxpcb` save no longer drops board setup when the stackup has no layers (#817)** — the `[stackup]` block is now written whenever `setup` is set, so grid, clearance and via defaults survive a save/load of a board with an empty layer list.
- **`.snxprj` keeps keys it does not know and carries a format version (#788)** — an older Signex saving a project written by a newer one no longer drops the newer keys (top level, per sheet, per library). Saves now write `"format": "snxprj/1"`; files without it still load, and a file from a newer format version is refused with a clear error instead of being opened and degraded.
- **External `.snxlib` row edits now show up (#531)** — a library edited by another window, a `git pull` or a text editor is noticed within a couple of seconds (a background `stat` check, logged to the Messages panel), so new rows and tables appear instead of staying hidden until restart. An in-app edit is applied on top of the external change instead of overwriting it, even when the external edit kept the file's size and timestamp, and an unreadable or missing file keeps the last good view, refuses writes and logs one warning rather than one per refresh.
- **ERC and the netlist now agree on power ports (#675)** — same-value power ports on separate wires join into one net (no more `GND` plus `GND_2`), a port joins same-name Global/Power labels, and ERC names the net the same way the netlist does. Pins of two different parts touching tip to tip stay unconnected, as before, but ERC now warns "pins touch but are not connected — add a wire" naming both pins instead of leaving it silent. A GND and a VCC port that merely touch no longer merge two rails into one net; ERC reports them as a power-port short at the pin tips, in the project netlist (including across sheets) as well as on a single sheet. A junction or wire at that point does connect them, so the short is then real and shows as one net.
- **Multi-unit parts are one part in annotation, ERC and the BOM (#691)** — units of one part (same designator and library symbol, different unit) now share one designator when annotated, are no longer flagged as a duplicate designator by ERC, and count as one BOM row with quantity 1. All three share one grouping rule in `signex-types`, and Annotate and Reset Duplicate Designators group across the whole project, so a part split over sheets stays one part. A lock on one unit pins its whole part. The BOM merges fields across a part's units (a DNP or MPN on any unit applies) and warns when units disagree on a field. ERC hints when units of one designator come from different library symbols. The Protel netlist export still treats units as duplicate designators and is tracked separately.
- **Non-finite and unbounded numeric input is refused (#678)** — `NaN`/`inf` in a file, an expression, a schematic property field or a footprint-editor field now fails with a clear message in the Messages panel instead of becoming NaN geometry, and saving a document that holds one fails before the file is touched, naming the block, row and field. Over-deep expressions (including `1^1^1…` chains) and oversized pad arrays are rejected rather than crashing or freezing the editor, and annotating past `R4294967295` no longer wraps to a duplicate designator.
- **Typed `NaN`/`inf` no longer reaches geometry or library files (#828)** — the remaining numeric text inputs, including both Move Selection dialogs, now keep the stored value when `nan`, `inf` or an overflowing number is typed, and saving a `.snxsym`, `.snxfpt`, `.snxsch` or `.snxpcb` that holds a non-finite number fails with an error naming the field instead of writing it. A footprint pad shape with a non-finite ratio or custom point is also refused on load.
- **Distributor cache read failures are logged (#806)** — a corrupt or unreadable cache entry in the LCSC, DigiKey, JLCPCB or Mouser adapters is now logged at `warn` and treated as a miss, instead of silently falling through to a live fetch.
- **VRML strings no longer break 3D model import (#781)** — a `#`, bracket or brace inside a quoted VRML string (for example `title "Part #1"`) is now part of the string, instead of starting a comment or a block and dropping the shapes that follow it.
- **Unchanged saves no longer add empty git history entries (#149)** — re-saving a project file whose content matches the last commit no longer creates a same-tree commit in version-controlled projects; the Messages log notes the file was unchanged instead. Library primitive saves (`.snxsym` / `.snxfpt` / `.snxsim`) still record a save timestamp, so they still commit every time.
- **Detached modals no longer float above other applications (#622)** — detached dialogs (Preferences, Print/BOM Preview, Parameter Manager, ...) use a normal window level and a native OS title bar instead of always-on-top and a custom header, so they stay out of the way of other apps, no longer cover the in-window shortcut sheet, tour and confirm dialogs, appear in the taskbar/Dock, can be moved, and can be raised again after the main window covers them; each window is titled with the dialog's name and its OS close button runs the dialog's own close path.
- **3D import reports VRML repairs (#805)** — when a VRML model is malformed and the importer recovers by skipping input, replacing a non-finite transform, or dropping triangles or unresolved `USE` nodes, the import result now carries a `GeometryAltered` warning with the source line, instead of only writing to the log.
- **New Project now returns the project-open Task (#841)** — creating a project hands both the project-open Task (which carries the library mounts) and the schematic-open Task back to the runtime instead of dropping the first through an underscore binding, so a project that opens with libraries can no longer leave them unmounted with nothing logged.
- **A failed encode or write no longer leaves a library edit in memory but not on disk (#831)** — an unencodable cell or a failed write is rejected and changes nothing; when only the git commit fails the saved edit is kept, "saved, but the git commit failed" is logged to the Messages panel, the git index keeps HEAD's `.snxlib`, and the next commit of the library file includes the edit.
- **DigiKey OAuth honours Cancel (#834)** — cancelling the Connect flow now stores no refresh token when the cancel lands before the exchange, and a token stored by a cancel that lost the race is removed again in the background; the cancellation is reported in the Messages panel. The callback connection is blocking with a 10 s timeout, so it no longer fails spuriously on Windows, and an idle speculative connection (one that times out or is closed without a request) is skipped instead of ending the flow.
- **ERC DSL rejects scoping it cannot enforce (#835)** — a rule using `apply_to`, a `scope` other than `sheet`, or `sheet.tagged(...)` now fails to load with an "is not supported yet" warning in the Messages panel, instead of silently running on every sheet.
- **Async completions no longer resolve their target late** — a PDF export exports the document it was started from even if you switch tabs while the Save dialog is open (and refuses if that document was closed), Add Existing / Add New Schematic act on the project you chose even if the project list changed meanwhile, and rapid saves commit to a project repository one at a time instead of racing on its lock (#833).
- **Dock layout saves no longer stall the UI (#692)** — hovering a dock tab or dragging a floating panel no longer rewrites `prefs.json` on every event; the layout is written only when it actually changed, once, 500 ms after the last change and off the UI thread. All `prefs.json` read-modify-write paths now share one lock, so a background dock write can no longer drop a setting saved at the same moment, an older layout can no longer overwrite a newer one, and quitting waits for a write already running as well as a pending one. A failed write is reported in the Messages panel.
- **Widget defects: tab dividers, project-tree row identity and symbol preview (#686)** — adjacent tab pills now share a single 1-px divider instead of stacking two borders, as `TabPillStyle::is_last` always promised. The Projects and Navigator trees identify rows by a stable key instead of their position, so the selection highlight, a pending double-click, an expand/collapse and a right-click menu keep pointing at the file you chose after a Refresh, Add New Schematic or Add Existing inserts a sibling before it, and an action aimed at a row that no longer exists does nothing instead of hitting its neighbour. The Components-panel symbol preview draws arcs as true circular arcs through their three points (a semicircle no longer renders half as deep), flattens Beziers into curves, and shows only unit 1 / body style 1 plus the common items instead of every unit stacked on top of each other.
- **signex-app rustdoc intra-doc links resolve again (#854)** — the 19 unresolved links (and two ambiguous `history_pane` links) now point at real items or are escaped, so `RUSTDOCFLAGS='-D rustdoc::broken_intra_doc_links' cargo doc -p signex-app --no-deps` passes.
- **A version-control commit now records the bytes that were saved or restored (#859)** — "Save X" and "Restore X" commits used to read the file when their worker ran, so an edit from another program, or a History restore, that landed in between was committed under the save's label. The save hands its bytes to the commit (the file's LFS pointer for a Git LFS path), and a restore commits the revision it wrote. If the file has changed again by then, git still reports it as modified rather than clean. A save made while another commit of the same file runs is queued behind it with its own bytes. A commit of an older save never replaces or follows a newer one. A restore is held back only by a commit of that same file that has not captured its bytes (a primitive-editor save, for example), not by any commit in the project. A restore of a file whose "Save" commit is still waiting in the queue gets its own commit after it, so the saved version is never replaced by the restored one.
- **Center View At Cursor (Home) recentres the view (#525)** — it now pans so the cursor position is at the centre and keeps the zoom, instead of zooming out to fit the whole sheet.
- **check_discarded_tasks scopes `drop()` tracking per fn (#732)** — a same-named binding in another function no longer trips the gate; real discarded Tasks are still flagged.
- **Protel netlist export groups units by the shared part rule (#824)** — duplicate-designator checking now uses the same part rule as annotation, ERC and the BOM, so a unit whose library id differs only by stray whitespace is no longer refused as a duplicate.
- **Project tree dialogs keep their target (#855)** — a Remove or Rename dialog left open while another project is opened or closed now acts on the project it was opened for (or is cancelled if that project closed), and creating the same pending library twice no longer adds a second, clashing tree row.
- **Library external-edit checks are quieter and no longer stop silently (#827)** — Signex no longer reports "changed on disk; reloaded" after its own edits (including class edits), and if checking a mounted library hangs (for example on an unreachable network drive) it warns once in the Messages panel instead of silently stopping.
- **Annotation edits are always undoable (#829)** — when a reset copies a locked unit's designator onto its sibling units, the change now gets an undo step and marks the sheet dirty; an annotate that leaves every designator as it was no longer adds an empty undo step.
- **3D import warnings survive the model cache (#871)** — a model loaded from the conversion cache now reports the same warnings (repaired VRML geometry, missing textures, unsupported STEP entities) as the import that converted it; cache entries from older versions are converted once more to recover them.
- **Place Sheet no longer reuses a `sheetN.snxsch` that exists on disk (#876)** — the default `sheetN` name now also skips any `.snxsch` already in the sheet's or the project's folder, even when no project lists it.
- **Reset Only annotation no longer rewrites unchanged sheets (#884)** — a reset over designators that are already reset adds no undo step and leaves unopened sheets unsaved.
- **VRML models keep their DEF/USE instances (#591)** — a `USE` now imports a copy of the node its `DEF` named, so a part modelled as one pin body plus forty `USE`s imports every pin instead of only the first.
- **A failing library check is reported once (#888)** — when the background check for external `.snxlib` edits fails, the Messages panel gets one warning instead of a new line every 2 s, and a note when checks resume.
- **Sketch solves no longer nudge partly constrained points (#890)** — after the solver retries a saddle start, a point held by a single distance (or a chain hanging off a fixed point) stays where it was along its free direction instead of sliding by about a thousandth of the sketch size; where the anchor itself moves, the leftover is cut by more than ten times.
- **Two Signex windows can no longer overwrite each other's library edits (#892)** — each `.snxlib` edit now holds a cross-process lock across its reload and write; an edit that cannot get the lock within 5 s is refused with a message instead of dropping the other process's change.
- **VRML prototype declarations no longer import as geometry (#895)** — a `PROTO` body is skipped instead of being drawn on its own, its `DEF` names stay inside the prototype, and an unexpanded prototype instance is reported once in the import warnings.
- **Library commits no longer fail or revert when another Signex process commits at the same time (#900)** — a commit that meets another process's `.git/index.lock` waits for it and then builds on that commit, a stale lock left by a crashed git is refused at once with a hint, and adapter-level primitive saves hold the library write lock.
- **Invalid zone holes draw the same on both render paths and are reported (#901)** — a hole that touches or crosses its zone outline, lies outside it or overlaps another hole is drawn as solid copper on both paths, and opening the board says how many.
- **Undocked PCB tabs keep their own view (#903)** — panning or zooming an undocked PCB tab no longer moves the main window, and the undocked window shows its own board rather than the main window's active one.
- **Two windows editing one symbol or footprint file keep both edits (#904)** — saving a symbol or footprint now writes only the parts that editor changed, under the library's write lock, so a save from another window to a different symbol or footprint of the same file is no longer overwritten. When both changed the same one, the later save wins and the Messages panel names it. A file that no longer reads on disk, or a save that would leave it empty, is refused and the file is left untouched.
- **Restoring a library file from history waits for other windows (#908)** — restoring a symbol or footprint file from the History panel now takes the library's write lock, so it no longer overwrites, or gets overwritten by, a save another Signex window is making to the same file at that moment.
- **Right-click leaves a symbol editor tool (#170)** — in a placement tool with nothing in progress, a right-click now returns to Select, the same as the second Esc, instead of opening the context menu.
- **ERC label, pin and designator checks (#836)** — `NetLabelConflict` now also compares Global and Power labels, `UnusedPin` no longer flags do-not-connect pins (unclassified pins are still flagged), and `DuplicateRefDesignator` flags a designator reused on a directly referenced child sheet (units of one multi-unit part are still one part; power and `#` flag symbols are not designators).
- **ERC cross-sheet designators follow the whole hierarchy (#915)** — `DuplicateRefDesignator` now compares the top sheet against every descendant sheet, not only its direct children, so a designator reused between a top symbol and a grandchild, or between two sibling subtrees, is reported. A file referenced more than once, or a reference cycle, is counted once.
- **Overlays paint on every tab (#114)** — the error card, the netlist-incomplete prompt and the print preview no longer stay hidden while the start screen or a non-editor tab is in front; a failed project close is now always shown.
- **Pin type conflict follow-ups (#919)** — the "Pin type conflict" row in the ERC dialog and Preferences is now on/off only, so it no longer flattens each matrix cell's Error/Warning/Info verdict (the row says so); power flags (`PWR_FLAG`) no longer name or merge nets by value (flags on different rails stay separate) and no longer raise PowerOut x PowerOut conflicts; and a net with several conflicting pins yields one finding (at the worst cell's severity, listing the other pairs) instead of one per pair. A No-ERC directive on a pin now silences only that pin's conflicts.
- **Symbol editor multi-unit moves and field right-click (#922)** — dragging one unit of a multi-unit symbol no longer drags the other units or the shared designator and value; right-clicking a designator or value selects it instead of opening the menu of the graphic under it.
- **Stale-target follow-ups** — a PDF/netlist/BOM export now names and uses the document it was started from, with its options captured at the Export click so overlapping exports and tab switches cannot mix them up (the "no netlist derived" warning no longer names the current tab); a new symbol/footprint library lands on the project you started it from; and Add Existing / Add New file copies and writes run off the UI thread, never overwrite an existing file (even when two batches overlap), and report failures to the Messages panel (#852).
- **Non-finite grid steps are reported (#925)** — typing `nan` or `inf` as a footprint editor grid step (Grid Properties or the Properties panel) now posts a refusal to the Messages panel instead of silently ignoring the edit.
- **A selection change cancels a pending Move (#271)** — Ctrl+A, Delete, paste or undo while a click-click Move / Move Selection is pending now cancels it, instead of the next click dropping whatever became selected.
- **Sheet names follow the volume's case rule (#694)** — each project now checks whether its folder ignores case, so on macOS a sheet referenced as `Top.snxsch` and `top.snxsch` is one sheet (no duplicate stitching), and in a case-sensitive Windows folder `A.snxsch` and `a.snxsch` stay two sheets (none drops out of the netlist); if the check cannot decide, the old per-OS rule is kept and the Messages panel says so.
- **A cancelled DigiKey Connect can no longer delete a newer connection's token (#856)** — Cancel followed by an immediate reconnect could let the cancelled flow's late "Connected" result remove the refresh token the second flow had just stored, leaving the UI on "Connected" but the next lookup failing with "no refresh token stored". Connect is now refused (with a "still winding down" status and a Messages-panel warning) until the cancelled attempt has finished and any token it stored has been removed, so only one flow can write the keyring at a time.
- **A power-port symbol placed from the library is now a power port (#928)** — a library symbol whose only pin is a power pin places with `is_power` set and its comment (or pin name) as the value, so it names its net, reaches the power ERC rules and stays out of the BOM. A power flag (`#FLG` designator) places as a power part with no net name, so two flags on different rails never merge them.
- **Footprint editor dialogs follow the theme (#114)** — the Move By and Align dialogs, STEP attach, pad-mode chips and the sketch inspector use theme colours, so hover and pressed states show on light themes too.
- **Distributor settings and property rows follow the theme (#114)** — the distributor settings use theme colours instead of fixed dark greys, and property-row values stay readable on light input backgrounds.
- **Zone cut-outs that touch the outline or each other draw correctly, and fill faster (#897)** — such holes (and some octagonal via clearances on decimal coordinates) no longer fall back to solid copper on the GPU path, and a faster ear clipper brings a 32x32 grid of cut-outs from about 1.6 s down to about 26–68 ms (bridging the holes is still quadratic in the hole count).
- **Library menu actions follow the library, not its row (#742)** — Remove and Enable Version Control on a library row now name the library by project and path, so a tree refresh between opening the menu and choosing cannot retarget another library; a library that has left the project is refused with a Messages-panel warning. Two libraries sharing a file name (`Power.snxlib`) no longer resolve to the first, and Remove drops only the one picked.
- **STEP faces with polygonal holes are cut (#708)** — 3D models imported from STEP now show straight-edged holes and cut-outs in faces instead of filling them. Circular holes are still filled and are listed as a warning, and VRML files are read with the same size cap as STEP.
- **The library server can now run on Postgres (#955)** — its migrations used SQLite-only `AUTOINCREMENT`, so a Postgres deployment failed at startup with a syntax error. Postgres now has its own migration set (`migrations/postgres/`, same versions and tables, identity columns); the SQLite files are untouched so existing databases keep their checksums.
- **ERC child-sheet findings land on the right sheet (#956)** — a finding located inside a child schematic (such as a hierarchical label the parent sheet symbol does not expose) used to draw its marker on the parent's canvas at the child's coordinates, and clicking it in the ERC panel panned to an empty spot. It is now filed under the child sheet: the marker appears on the child, and the panel entry opens the child before panning. If the child was not part of the run, the entry is still listed but nothing is drawn or panned to.
- **Close Project reference walk classifies the path it actually opens (#820)** — a sheet reference is now refused when its raw text, the normalised path it resolves to, or the join with the parent directory names a device or an untrusted network share, and a `..` that climbs above a UNC share root is refused instead of relying on the clamp (a legitimate `..` inside the share is still followed). Library and dirty paths from a `.snxprj` are no longer canonicalised when they name an untrusted share, so a `\attacker\share\lib.snxlib` entry no longer touches the network. Refusals are reported as before (Messages panel, Discard All off).
- **Global and hierarchical labels anchor at their connection point (#171)** — the flag and its text now start at the anchor and extend in the label's direction at every rotation instead of being centred on it, and the hit and selection boxes for labels and text notes follow the drawn geometry, rotation and justification. The selection, ERC and ghost-preview overlays now use the active canvas theme instead of a fixed one.
- **Pad shape and layer pickers no longer store something other than what they show (#964, #965)** — the footprint editor's pad Shape list stops offering Octagonal and Donut, which had no stored form and were silently saved as Round; the remaining shapes round-trip through save and reopen. Moving a pad from Multi-Layer to Top or Bottom now clears its drill and slot (previously a hidden hole still reached the saved file and the bake output), moving back to Multi-Layer gives a default 0.8 mm drill, and one undo reverses either step. Non-plated hole pads keep their hole.
- **Library rows bound to a missing symbol, footprint or sim are reported, and saved primitives refresh their previews (#968)** — a component row whose primitive binding resolves to nothing is now named in the Messages panel (library, row, table and the missing id) once when it appears, after a mount or a table reload, and its Symbol / Footprint cell shows in the error colour with a warning mark in the library browser. Saving a symbol or footprint in its editor now refreshes the copy an open Component Preview tab had already resolved, instead of drawing the old one until the tab is reopened.
- **Saving a symbol or footprint now updates the components that use it (#832)** — in Personal mode the primitive gets a patch version bump and every component row using it is re-pinned to the new version; rows that could not be updated, and released rows left behind in Team mode, are named in the Messages panel.
- **Grid Properties Apply refuses a zero, negative or non-finite step instead of closing as if applied (#972)** — the previous grid step is kept, the dialog stays open, and the refusal with its reason is written to the Messages panel.
- **License Guard checks the renderer again (#966)** — the "no bare Standard comment-word" rule pointed at the renamed `crates/signex-render/` and passed without scanning anything; it now scans `crates/signex-renderer/` and fails when a configured path has no files.
- **Ctrl+Z / Ctrl+Y follow edit order across net colours and edits (#102, #975)** — undoing no longer reverts net-colour floods before newer edits; colour steps and document edits now undo and redo in the order they were made, per document, so switching tabs never undoes another document's colour; a new edit (including Annotate) ends that document's colour redo branch, and reloading a document drops its colour steps.
- **PCB files no longer merge pads or lose layer types (#104)** — `.snxpcb` now keys each pad to its footprint by uuid, so footprints sharing a reference designator keep their own pads, and the stackup round-trips layer types; format is `snxpcb/1.1` and older files still open. `.snxsch`/`.snxpcb` of any minor version of the current major load, but a file written by a newer minor is read-only in this build: saving it (including Save As) is refused with a message naming both versions, so content this build cannot represent is never silently dropped. A different major is refused naming both versions. A file listing more `layer_types` than layers, or two footprints sharing a uuid on save, is refused. A new library's first file is written atomically. A `.snxpcb` in which two footprints share a uuid is refused on load, naming both. Annotate and Reset Duplicate Designators leave newer-minor (read-only) sheets untouched, still count their designators as taken, and name them in the result notice.
- **Close Project reference walk no longer trusts a `/` inside a verbatim UNC name, and follows projects opened through a mapped drive (#978)** — in a `\\?\UNC\server\share\…` reference only `\` separates the server and share, so `\\?\UNC\trusted/share\x.snxsch` is refused instead of being read as the trusted share (mixed `\\?/` and `//?\` prefixes are refused as devices). A project opened through a mapped drive, DFS or a symlink to a share trusts the share its real path is on, so its own sheets are no longer refused. A `..` chain from a UNC project stays on the project's share because the path normaliser stops it at the share root; the former separate climb check, which the real walk never reached, is gone. Library and dirty paths such as `\\attacker\share\lib.snxlib` are still not canonicalised, and roots, loaded and extra paths now use the same refusal rule as child references. A server or share is compared with ASCII-only case folding, so a Unicode look-alike such as `\\DES<U+212A>TOP\c$` (U+212A KELVIN SIGN, which Windows treats as a different host) is refused instead of being read as the trusted `desktop`, and its identity key no longer merges with the real path's.
- **Page setup memory (#982)** — the PDF export page setup remembered for a document is now forgotten when its tab or project closes, instead of lingering for the whole session.
- **Distributor credentials are stored in the OS keychain (#976)** — DigiKey/Mouser keys and the DigiKey sign-in token now persist in Windows Credential Manager, macOS Keychain or the Linux Secret Service instead of being lost on restart, and a missing keychain is reported with the reason.
- **PDF "Current page" exports the active sheet (#109, #984)** — choosing Current page in Print Preview or PDF export now renders the sheet you are looking at instead of always the first sheet of the project.
- **Library rename leftovers (#804)** — renaming a project library now re-points every duplicate `.snxprj` entry for it in one atomic write (and rolls them all back together), commits the patched project file when project git is on, and says explicitly when the saved project lists the library as a shared path (Save writes the new name; Discard keeps the old one); after a failed rollback the project is only marked dirty once the file has been moved back.
- **Enable Version Control failures are shown, and the tab bar scrolls (#110, #987)** — a failed Enable Version Control now raises an error card and a Messages entry instead of closing silently with auto-commit off. Document tabs scroll sideways when they overflow, long titles are shortened with an ellipsis, and hovering a tab shows its full path, and the strip scrolls the newly active tab into view.
- **Shift+Space rotates clockwise (#962)** — Space and Shift+Space (and the classic R / Shift+R and the two Active Bar rotate buttons) now rotate the selected symbol in opposite directions, mirrored symbols included; before, both turned counter-clockwise.
- **Annotate honours the processing order (#961)** — the Up/Down/Across choice in the Annotate dialog now decides the numbering order, and the dialog legend draws Up as bottom-to-top.
- **New Component says why it cannot run, and new rows show they are unbound (#994, #967)** — File ▸ Library ▸ New Component and Add New ▸ Component used to do nothing when no library was open or the library had no component table; they now write a Messages-panel note asking you to open a library with a component table first, and the dead `new_component` modal state, messages and view are gone. A row created without a symbol or footprint still never mints a `.snxsym` / `.snxfpt` (no file is written without an explicit action); instead it reads "No symbol bound" / "No footprint bound" in the warning colour in the Library Browser grid (the cell is itself the Pick Symbol / Pick Footprint button), the Properties panel and the Component Preview, and the creation is reported once in the Messages panel. Binding a symbol or footprint clears the marker.
- **ERC no longer rebuilds a shared sub-sheet once per path through the hierarchy (#998)** — a sheet reachable through several parents (stacked diamonds, reused channels) was built and connectivity-projected once per path, which doubled with every diamond; one ERC run now builds each sheet's context once and shares it, keeping the ancestor guard for reference cycles. A duplicate-designator clash between two descendant sheets is reported once per run instead of once for every ancestor's check, at the owning sheet's own coordinates. Two copied channel directories that clash identically each keep their finding, because the run-level dedupe keys on the owning sheet's resolved identity rather than on the message text.
- **Close Project no longer opens Windows device names (#999)** — a sheet or library path named `NUL`, `COM1`, `aux` or `LPT9` (with or without an extension, in any folder) is refused before any file access and listed in the Messages panel, instead of being opened as a device.
- **Symbol editor Space rotates counter-clockwise (#980)** — in the symbol editor Space now turns the selected pin or graphic counter-clockwise on screen and Shift+Space clockwise (the Altium convention); before, the two keys were swapped.
- **A failed file check in the case probe is reported (#970)** — when the probe cannot stat a file (deleted mid-probe, permission denied on a share) it no longer silently answers "case-sensitive"; the Messages panel now says the volume's case rule could not be decided, with the reason, and the host default is used.
- **Library drift follow-ups (#1002)** — unmounting one library now drops its drift rows at once instead of leaving "Update from library" live until the recheck returns, a library symbol that cannot be read keeps its previous drift row instead of clearing it, and replacing a symbol on a sheet with no embedded copy still refuses a new pin that would land on a connection. Replace-symbol connectivity is now tested for junctions, no-connects, and wires passing through a pin.
- **Net-colour undo (#1005)** — Ctrl+Z on one sheet now reverts only that sheet's net colours; colours applied on another sheet in between are left alone, and "Clear all net colours" clears the active sheet only.
- **Library symbol updates keep the sheet copy and its version in step (#950)** — "Update from library", Personal-mode auto-apply at open and the Team Library Updates dialog all replace the embedded symbol and stamp the instances' library version together, so an applied update no longer reappears as drift or as a pending update. There is one apply path: a worker reads the library's current symbol and each instance's row fresh (through the library the row itself lives in), and the result lands as one undo step per shared symbol (none for the silent open-time apply). Every instance shares one embedded copy, so an apply covers every placed instance of a selected symbol: unticking an instance of a selected symbol does not exclude it, and the dialog offers one checkbox per shared symbol. At open, a symbol is auto-applied only when every placed instance of it has a readable Personal-mode row that still binds it; if a stale instance is Team-mode its stale instances go to the dialog, and if an unreadable, missing, unmounted, re-bound or Team sibling blocks it, nothing is applied and Messages names that reference. Each instance is decided from its fresh row: a row that binds the symbol is stamped with the version read; a re-bound row, a missing row and an unreadable library each get their own Messages line ("version not stamped") and keep their stamp. If a row moved on after the version the user approved, the whole symbol is held back (no copy, no stamps) with one Messages line per held-back symbol naming what moved (and to which version) and the left-behind instances, and the next scan offers it again; other symbols in the same Apply still apply. A refused update (a connected pin would vanish) leaves both untouched and says why. A completion for a sheet that was closed, reloaded or renamed meanwhile is dropped with a Messages line, and Apply on a sheet that is no longer open keeps its pending record.
- **Move edge cases (#973)** — moving a symbol with no pins or graphics no longer shifts its designator, and a click-click move cancels when the selection changes back to what it was before the pick.
- **A `.snxsch` / `.snxpcb` / `.snxsym` / `.snxfpt` holding `nan` or `inf` is refused at load (#924)** — TOML accepts those values but saving refuses them, so such a file used to open and then never save; the load now fails with an error naming the offending key path.
- **Close Project walk refuses bare verbatim drives and symlinked device/share targets (#1019, #1020)** — a reference spelled `\?\C:` (or `//?/C:`) no longer opens the raw volume device, and a sheet or library path that is a symlink or junction to a device or an untrusted network share is no longer followed.
- **A PCB tab saves atomically and commits like a schematic (#140)** — a PCB tab with unsaved changes now saves atomically and commits to the project's git history like a schematic; a clean PCB is never re-written, and a History restore of a PCB commits the restored file as it is on disk (#140).
- **A History restore commits the restored file, or says it did not (#1025)** — restoring a footprint or symbol from History now commits the restored file to project git as "Restore X". A restore whose file cannot be loaded back (footprint, symbol or schematic) raises an error card that says it was not committed and, for unparseable content, to restore a different revision; the stale editor or engine is dropped so Ctrl+S can no longer overwrite the restored file, and a save of that tab reports a failure instead of logging a write that never happened. A project git commit requested while one for the same file is queued now takes the newest label, and one requested while it is running runs afterwards instead of being dropped (#1025).
- **Page setup memory (#1036)** — a dirty tab closed into the background keeps its remembered page setup, so reopening it restores the setup along with its unsaved edits.
- **Net-colour undo follow-up (#1005, #1028)** — net-colour undo now covers a net's junctions as well as its wires, "Clear All Net Colors" is global again (each sheet's Ctrl+Z restores its own colours), and a closed sheet's colours are dropped.
- **A save with no open editor no longer blames a History restore (#1025)** — saving a symbol or footprint tab that has no open editor now says "no open editor backs <file>; close the tab and reopen it" instead of blaming a History restore (#1025).
- **PDF and SVG export no longer paint bus entries in the bus colour (#1050)** — a bus entry takes the wire colour of the active theme instead of being overwritten by the bus colour.
- **Footprint pads keep every field (#960)** — net, testpoint, lock, per-side mask/paste overrides, tenting, thermal relief, features and hole tolerances now survive saving and reopening a `.snxfpt` instead of resetting, and pads default to paste on with thermal relief, so the stock footprints now get paste openings; footprints made in earlier builds may still have paste off and direct connect, so re-check them (#1073).
- **Component tables (#1056)** — saving a component to a table, a `.snxlib` library, the library database or the library server now refuses a NaN or infinite parameter, naming the row and the parameter, instead of storing a value that could not be read back. The existing data is left untouched.
- **A PCB edit made during a save stays unsaved (#1040)** — a PCB save now carries a board revision, so an edit made while the write runs keeps the tab dirty and queues no commit of the stale file; the next save writes it.
- **Grid snap lands exactly on the grid (#1057)** — the snap used the grid pitch widened from f32 (1.27 became 1.2699999809...), so snapped points were saved 1-4 nm off the grid; snapping, move, align-to-grid and paste offsets now use the exact decimal pitch, so a point 100 steps out on the 1.27 mm grid saves as 127000000 nm.
- **Command palette lists only commands that apply (#1070)** — the palette no longer offers schematic placement commands on the start screen or in the footprint editor, nor commands that are greyed in the menu; global commands (New/Open Project, Preferences, Keyboard Shortcuts, Open Panel) stay, and the list updates when you switch tabs.
- **macOS installer builds again** — the release job now sizes the disk image from the app it holds, so the universal build no longer fails with "No space left on device".
- **A stale library row can no longer overwrite someone else's save (#115, #1072)** — saving or deleting a database-library row now names the revision it was read at (`If-Match`), and the server refuses the write with 409 and a "changed by someone else" message when the row has moved on, instead of silently replacing it. Row `PUT`/`DELETE` now require `If-Match` (an older client gets 428), so client and server must be upgraded together. A Component Preview or Edit Component modal whose save was refused offers **Reload saved version**, which asks before it drops your edits; an unedited preview of a row that another surface just saved reloads itself, while an edited one keeps its draft and is refused on save. The unused advisory-lock endpoints are removed from the library server.
- **Close All Documents keeps dirty drafts (#110)** — Projects panel > Close All Documents now parks dirty symbol, footprint and Component Preview drafts like the tab-menu Close All, instead of discarding them without a prompt.
- **Save All matches case variants by the owning project's volume rule (#1077)** — on project close and app exit, two spellings of one file are matched by the rule of the owning project's volume, not the host family's: differing case variants on a case-insensitive volume are refused (every unsaved marker kept), and on a case-sensitive folder each file is saved separately.
- **Save As onto a path that is still opening no longer loses edits (#1089)** — Save As now refuses a target whose open is in flight, and a late open result for a path a document already owns is dropped with a warning instead of adding a second tab and replacing the engine.
- **History restore waits for pending commits (#1090)** — Restoring an older revision is refused, with a message to try again, while a save's version-control commit is still queued or running, so the restored content is never committed under the save's label.
- **Project-tree menu no longer touches the disk (#1098)** — whether "Enable Version Control..." shows now comes from a `.git` check done off the UI thread with the rest of the project-tree probe, instead of a `stat` on every redraw while the menu is open; it refreshes after Enable Version Control completes.
- **A crashed print-preview render says so (#1101)** — if rendering the PDF preview fails, the error card and the Messages panel now say the render failed and why, instead of claiming the page range has no pages.
- **A stale preview save no longer overwrites a newer edit in a local library (#1108)** — rows of a local (git-backed) library now carry a base, a hash of the row as stored, and saving or deleting a row based on an older copy is refused with the same "changed by someone else" message and **Reload saved version** offer as a database library, instead of silently replacing a browser cell commit. The check runs against the file on disk at write time, so a save from your own surfaces in sequence is never refused.
- **Saves of one file under different spellings can no longer overwrite each other out of order (#1112)** — the write-ordering ledger, the in-flight set and the Save As refusal were keyed by the exact path, so on a case-insensitive volume `Lib.snxsch` and `lib.snxsch` were two files to the ledger: an older snapshot could overwrite a newer one after the dirty marker was cleared, and the older write's completion could clear the newer one's marker. They are now ordered under a case-folded path that does not depend on the project or its volume probe, and an older snapshot is skipped only when the worker thread confirms both spellings are the same file. On a case-sensitive volume two case-twin files are still both written; a skipped snapshot leaves its document modified and the Messages panel names it.
- **Readable flag text at every turn (#1118)** — Global and hierarchical labels rotated 180° (or X-mirrored) no longer show upside-down text, and at 90° the text reads bottom-to-top instead of top-to-bottom. The flag still points where it did. In PDF/SVG export, vertical label text set to right justification also reads bottom-to-top.
- **History panel no longer stalls on slow disks (#1120)** — Switching tabs no longer checks the disk on the UI thread to find the file's repository; that check now runs in the background with the history load.
- **Rotated text lands in the right place in PDF (#1125)** — Vertical labels, fields and notes that are right-, centre- or top-aligned, or that use sub/superscript or overbar markup, are placed along their own baseline in exported PDFs, matching SVG export and the canvas.
- **3D models really go into Git LFS (#1126)** — With "Track binary 3D models via Git LFS" ticked, Enable Version Control and later saves now commit `.step`, `.stp`, `.wrl` and `.iges` files as Git LFS pointers, with the models stored in `.git/lfs/objects`. Before, they were committed whole. A `git push` uploads them if git-lfs is installed, and says so if it is not.
- **Enable Version Control no longer freezes the window (#1128)** — Creating the repository and its first commit now runs in the background. The dialog shows "Enabling…" until it finishes, so a large project or a network drive no longer locks up the app.
- **Rename and Delete wait for a file that is still opening (#1129)** — Renaming or deleting a file in the project tree while it is still opening is refused with a message, instead of leaving a tab on the old path whose next save would recreate the file.
- **Footprints made before #1052 can get their paste openings back (#1073)** — a footprint whose sketch pads were all saved with paste disabled and direct connect now offers, when opened, to enable paste and thermal relief on them (one undo step, nothing saved until you save), and the Properties panel warns about SMD pads that have no paste opening.
- **Unsaved edits survive ERC navigation and child-sheet double-click (#1135)** — Jumping to an ERC marker, or double-clicking a child sheet, on a sheet whose tab was closed with unsaved edits now reopens it with those edits instead of replacing them with the saved file.
- **Save As and Open messages keep their reason (#1137)** — Messages now lead with why a Save As or a late open was refused, then the file name, so a long path no longer cuts the reason off. File > Open of a path that a Save As is still writing is refused instead of showing the old content, a dropped duplicate open brings the owning tab to the front, and the "tab may show previous content" warning appears only when such a tab exists.
- **Pasted symbols no longer print the original's designators (#1138)** — pasting or duplicating a symbol copied its per-placement instance entries, so the copy exported the original's designators whatever its own reference said; the copy now starts with no entries and takes its designator from its own reference.
- **Loose-document BOM no longer depends on the focused tab (#1139)** — when a parent sheet places a child sheet more than once and both are open without a project, the BOM now walks from the top of the loose hierarchy, so the child is counted once per placement whether the parent or the child tab has focus.
- **A failed offset undo no longer slides sketch geometry (#1144)** — When the solver needed its perturbed retry and the first pass that takes the retry's start offset back out failed, it returned the solution with the offset still in it, sliding a partly constrained point (one held at a distance from another) along its circle. The solve now reports `RetryOffsetNotUndone` instead of handing out that state.
- **One document per file, whatever the spelling of its path (#1149)** — opening a file under another spelling of a path that is already open (`lib.snxsch` while `Lib.snxsch` is open, a `./` or `sub/..` path, an NFC / NFD name on a volume that ignores the difference) now brings the existing tab to the front from File > Open, Open Recent, the project tree, child-sheet and Navigator double-clicks, ERC navigation and symbol/footprint opens, instead of opening a second document that could be saved over the first with its edits unmarked. Symlinks are not resolved, so a link and its target still open as two documents.
- **Footprint sketches with many pads solve again (#1153)** — Points no constraint touches (most of every minted pad) are left out of the solve and keep their place, so a QFN-40 or a row of rounded-rect pads with a single dimension no longer runs out of the solver's work budget.
- **Quit and Close Project no longer drop a library that Save All could not create (#1155)** — when a new library in the project could not be created, Save All wrote the project without it and then quit or closed anyway. Now the project stays dirty, the failure is reported, and Signex stays open so you can retry or choose Discard All.
- **A sketch that never converges no longer floods the Messages panel (#685)** — A drag on a sketch that cannot converge, or that keeps running out of its solve budget, now logs that failure once instead of on every pointer tick, even though the residual and the work spent change each time.
- **ERC power-rail shorts and unloaded child sheets (#107)** — `PowerPortShort` now follows connectivity, so a wire joining two differently named power ports (for example +5V and GND) is reported instead of only ports sharing a pin tip, and a sheet symbol whose child file did not load now gets a warning instead of silently skipping its sheet-pin checks.
- **Smoother, exact rubber-band move preview (#1177)** — With snap off the drag preview now steps every few screen pixels instead of recomputing on every cursor event, and with snap on it shows exactly the move that will be made instead of one rounded slightly off the grid. Moving a power port with "Keep wires connected when moving" on no longer falls back to a plain move with a false "could not keep … connected" notice when its net also carries a local net label, or when another port of its value touches a port of a different value.
- **A project rule set from a newer Signex no longer stops the project from opening (#1180)** — a `rules` section this build cannot read (a newer schema or a changed value shape) is kept unchanged on save and not applied; ERC uses this machine's preferences and says so once in the Messages panel. A rules section also re-saves exactly as it was written.
- **Release builds ship the bundled fonts' licenses (#1179)** — the Roboto license is now in the repository with its source and version recorded, and every installer and archive carries the license text of each font compiled into Signex (Roboto and Iosevka) under `licenses/fonts`.
- **Sheets sized `A`–`E`, `USLetter` or `USLegal` print on their own paper again (#1199)** — since #1188 these short names fell back to A4 in the PDF and print preview. They now read as ANSI A–E, Letter and Legal on the canvas, in the Properties picker and in every export. The stored text is not rewritten.
- **New Project creates its own folder and Enable Version Control refuses shared folders (#1205)** - File > New Project now writes `<name>/<name>.snxprj`, `<name>.snxsch` and the managed `.gitignore` into a new folder named after the project (or an empty same-named folder you made), and refuses a non-empty folder, one that already holds a project, or a location inside another project's folder; a failed create removes only what it made. Enable Version Control, and the first save of a project with version control on but no `.git`, refuse a home, Desktop, Documents or Downloads folder (as the platform names them), a filesystem root, and a folder that holds other projects or cannot be fully scanned, with the reason in the Messages panel (a paused auto-commit is announced once per project).
- **Cross-sheet Port bus width check (#1206)** — a Port bus clash between two sheets is no longer missed when another clash sits inside one child, a pair on a single sheet is no longer reported twice, and equally wide Ports that carry different signals are reported as a name conflict instead of a width mismatch.
- **Placement designators follow-ups (#1200)** — a child sheet opened from the project (not by descending) now shows the designators its printed page uses; a placement whose sheet symbol was deleted is no longer shown or edited; on a sheet placed only once a designator edit changes the symbol's own designator instead of adding a per-placement entry; and a longer designator drawn for a placement can be clicked and box-selected across its whole width.
- **Autosave recovery copies are no longer overwritten or skipped (#1208)** - the recovery check now runs for every way a schematic opens (project tree, child sheet, ERC navigation, not only File > Open); autosave writes nothing over an earlier run copy from open until its prompt is answered; the prompt warns when the copy is older than the saved file and that Restore cannot be undone; every Restore failure and a failed superseded-copy removal reach Messages with the reason first; quit now waits for the copy removal that follows a save; the startup sweep reports the entries it could not read or remove; a recovery check that cannot read the copy or its document keeps autosave paused for that document (close and reopen it to check again), and a Restore of a copy that is gone resumes autosave and says so.
- **ERC rule edits no longer undo through the sheet, and Store no longer replaces a newer rule set (#1209)** — Ctrl+Z / Ctrl+Y now follow the window the key was typed in (the ERC dialog steps its rule history and never falls through to the sheet; the main window keeps sheet undo), Store is refused when the project holds a rule set from a newer Signex, a click that changes nothing records no undo step, the dialog's Undo/Redo buttons are really disabled when empty, and a closed project drops its rule history.
- **Add Existing registers an outside library in place (#1213)** — Add Existing now registers a `.snxlib` outside the project in place instead of copying only the one file, stores one from a project subfolder by its real relative path, and mounts it right away so its components show in the Components panel without reopening the project. An outside library is stored by absolute path, so a project that references one is no longer portable to another machine. Whether a library is inside the project, and whether it is already listed, now follows the project volume's case rule, so one library spelled two ways is no longer registered twice.
- **Case probe no longer reads a vanished entry as case-sensitive (#1216)** — when the entry the volume probe borrowed disappears between its two lookups, the answer is now undecided (treated as case-insensitive) instead of case-sensitive, so Save All cannot write two spellings that overwrite each other.
- **Renaming a project is committed as a rename (#1219)** — in a version-controlled project the old `.snxprj` no longer stays behind as an unstaged deletion. One commit, `Rename project <old> → <new>`, removes the old name and adds the new one, so `git status` is clean and the history shows the rename. A failed commit is listed in the Messages panel.
- **Designator edit on a shared sheet (#1224)** — editing a designator on a child sheet that another open project also uses, or whose other project cannot be fully read, now writes that placement's own entry instead of `reference`, so the other project's designator no longer changes with it. While another open project's walk is incomplete, every designator edit takes this entry route, shared sheet or not.
- **Net class styles (#1236)** — The Messages panel names a sheet that could not be parsed right after a short reason, with the full path last, so the 157-byte cut keeps the file name; a failed style resolution is retried once the project's key moves; double-click and the reorder click hit a wire at the width its net class draws it; and a loaded class named `(none)` is still a class when picked in the label's Net Class list. The canvas also compares one revision number per frame instead of the whole style map.
- **Placeholder Active Bar tools (#1249)** — Sheet Entry, Device Sheet Symbol, Reuse Block, Round Rectangle, Graphic, Blanket, Compile Mask, Differential Pair, Bezier, Ellipse, Elliptical Arc and the three Harness tools are now greyed with a "Planned" tooltip (the Harness group button too) instead of silently drawing a rectangle, circle, line or `DIFF_PAIR` note; their shortcuts and palette entries are not offered as runnable, and a keystroke says in the Messages panel that the tool is not built yet.
- **Locked symbols stay put in group moves, Cut and on the canvas (#1250)** — wires ending on a locked symbol's pin no longer drag off it, Cut skips locked symbols in both the copy and the delete, and a padlock is drawn above every locked symbol.
- **Planned menu rows and panels (#1251)** — menu, context-menu and panel entries for features that are not built yet are greyed with a "Planned" tooltip (or say so on the panel). View ▸ Zoom In / Zoom Out, Place ▸ Power Port / Text / No Connect and the canvas context menu's Project Options now work instead of sitting inert, and Design ▸ Generate BOM opens the same BOM dialog as File ▸ Export ▸ Bill of Materials.
- **Paste review follow-ups (#1263)** — a Copy that carries nothing (a sheet symbol or sheet pin alone) now empties the system clipboard instead of leaving an older Signex fragment to be pasted; Cut writes the system clipboard only after the delete went through; the paste and rubber-stamp ghost shows the designators the click will give (Next free or Reset) instead of the copied ones; a paste keeps the anchor of the copy made in this window when the grid changed in between.
- **Paste Array follow-ups (#1264)** — the 100 000-item cap is checked against the copy that is placed, not the one the dialog opened with; the project's designators are read only when Next free is chosen (when the dialog opens on it or when it is picked).
- **Rename Project no longer loses exports or strands libraries (#1322)** — an export into a project whose folder is being renamed is refused with a notice instead of writing into the old folder, and the folder move is refused (naming the library) while any Global, Installed, mounted, browser-tab or in-flight library lies inside the folder.
- **Renaming a project folder no longer races writes or loses recovery copies (#1323)** — while the folder moves, writes into it are refused with a notice to try again, and the autosave recovery copies of its documents, open or closed, move with them.
- **Electrical-grid snap now works (#1246)** — with the Shift+E toggle on, a wire click within 1.27 mm of a pin end, wire end or junction lands exactly on it, with a ring marking the target; with it off, wires snap to the grid as before.
- **Locked symbols stay put (#1250)** — a symbol marked Locked is no longer moved, rotated, mirrored, deleted or edited (its designator included) from the canvas, keyboard, Move Selection dialog, Properties panel or Replace All. An edit that reaches several symbols skips the locked ones and names them in the Messages panel; an edit aimed only at a locked symbol is refused with the reason. Unlock the symbol to change it. The canvas lock indicator and a lock for wires, labels and graphics are still to come.
- **Update PCB from schematic no longer runs ERC (#1247)** — the command was wired to Run ERC, so its shortcut ran a rule check and looked like a board update. Until forward annotation exists it is unavailable, like any other command without an action, instead of running something else.
- **Parameter Set no longer joins nets (#1248)** — the Parameter Set tool placed an ordinary net label (`PARAM=VALUE`) that renamed wires and merged same-text sheets. The tool is now disabled until a non-electrical directive object exists.
- **Flaky footprint solve test (#1320)** — `repeated_failing_solves_reach_the_messages_panel_once` no longer counts other tests' entries in the shared diagnostics ring.
- **Rotate and mirror no longer drift (#1039)** — two mirrors of a selection cancel exactly, an on-grid selection stays on the grid, and a lone label or note turns in place instead of shifting.
- **Library price fixes (#152)** — a price like "0,125" is no longer read as 125, a DigiKey break without a unit price is no longer cached as free, a malformed price-cache entry no longer stops the library opening, and a pricing refresh exchanges the DigiKey token once instead of once per part.
- **Dispatch command findings (#1350)** — Backspace/Delete while drawing a wire now drops the pending vertex instead of deleting the selection (also with nothing selected, and for a polyline's last point), and theme, grid and net-colour changes now refresh undocked schematic and PCB windows instead of only the main canvas.
- **Review cleanups (#1351)** — a window that closes before the OS confirms an undocked-tab or detached-panel open no longer leaves a stale window entry and canvas behind, the dead `commit_ghost_for_pre_placement` is removed, and stale doc comments on the palette, overlay and command-bridge messages are corrected.
- **Grid Properties has one Step field (#1351)** — the grid keeps a single step for both axes, so the dialog no longer offers a Step Y that Apply dropped; the stale command-registry and contract doc comments now describe today's behaviour.
- **Components panel filtering (#1353)** — the panel now lowercases the search text and library name once per library instead of once per row on every redraw, so typing in a large mounted library stays responsive.
- **Library editor findings (#1356)** — the "Add custom parameter" pills now take the name typed beside them, Save keeps a number or measurement still being typed (and Remove drops its pending text), and Open Symbol/Footprint Editor opens the file from the library the reference names.
- **Library browser cleanups (#1357)** — Symbol and Footprint cells now show an 8-character uuid prefix instead of the full 36-character uuid, and the dead preview pane, its width constant, and the hidden Classes sidebar block with its orphaned class-edit messages, state and handlers are removed (the sidebar header now reads Tables) along with the stale browser layout docs. New-library path validation is now shared and also rejects Windows reserved names (including `CON.x`) and trailing dots/spaces, the delete-confirm no longer claims git recovery, and stale browser/editor doc comments are corrected.
- **Review cleanups (#1363, partial)** — the distributor cache encodes MPNs injectively (`%XX` for anything outside `[A-Za-z0-9-._]`, so Windows-reserved characters and `..` inside an MPN now cache correctly), treats a cached file holding a different MPN as a miss, and the distributor HTTP user-agent carries the crate version.
- **Net-level ERC findings and built-in rule ids (#1364)** — a rule that reports a net now carries the net, marks its lowest designator/pin with a distinct net marker that a No-ERC directive on that pin does not hide, and a click selects the whole net and zooms to fit it; built-in rule ids are an explicit per-rule table pinned by a test instead of derived from the label.
- **Core-crate review findings (#1365)** — a `.snxprj` saved with a UTF-8 BOM now opens instead of being treated as an empty legacy marker, and a non-UTF-8 one is refused rather than replaced on the next save; PCB stackup layer ids that are not 0..n now survive save and reload; schematic ERC checks Letter, Legal, Tabloid, B5 and custom sheets against their real size instead of A4.
- **Format `snxpcb/1.2` and project-file guard (#1365)** — the PCB stackup `layer_ids` key bumps the minor version so older builds open such boards read-only, a malformed `layer_ids` list (wrong length or repeated ids) is rejected, and a `.snxprj` that is not JSON (for example merge-conflict markers) is refused instead of opening as an empty project.
- **Handler findings (#1367)** — Editing a rectangle's start X no longer collapses it to zero height when its start sits below its end, an ERC Quick Fix no longer drops a NoConnect on the wrong sheet when the target sheet fails to open (the reason is logged to Messages), and Add Existing registers a sheet or PCB picked from a project subdirectory by its path inside the project instead of its bare file name.
- **Delete File no longer drops the project entry when the disk delete fails (#1368)** — a failed remove (or an existence check that errored) left the sheet or library pruned from the project and the project dirtied while the file was still on disk; the entry now stays and the error notice is shown. Also corrected stale comments and removed the unused parameters of the foreign-library loader stub.
- **Review leftovers (#1371)** — the symbol preview keeps its render cache across frames and draws text boxes, the 3D importer drops its two unused warnings, keys its cache on the canonical path and a glTF's external files and evicts old entries, the library server loses its superseded tables and pages its list routes (the database adapter pages through them), and the chrome catalog draws the real project tree.
- **Library panel search and DigiKey sign-in callback (#1373)** — the left-dock library search box now filters the library list instead of doing nothing, and the DigiKey browser callback no longer hangs or fails on a stray local connection such as a favicon request.
- **Library review cleanups (#1375)** — the DigiKey browser page now says "connected" only after the token exchange succeeds, and a denied authorization is reported as denied instead of a redirect-URI problem; a freshly mounted library now loads its cached prices; the primitive picker no longer allocates per item per frame while filtering. Stale docs (lifecycle filter, update dialog sort, distributor settings, persistence errors, module overviews) now match the code, and the dead `tiny_http` dependency, the unused `ComponentEditorState` alias, the never-read `expanded` flags and the test-only `prepare_mount_read_only` are removed.
- **Review cleanups (#1377)** — PDF fit-to-page now accounts for buses, junctions, notes and child sheets, pin bookmarks are now sorted and skip power symbols, bookmark `/Count` counts all descendants, the netlist listing escapes quotes, backslashes and line breaks in net names, and stale signex-bake docs and warnings no longer promise unshipped versions.
- **Keymap precedence is now explicit (#1379)** — the Keyboard Shortcuts pane marks profile bindings that the fixed F1 and Esc keys override, the chord recorder warns about them, and the Altium profile binds Delete once.
- **Review cleanups (#1381)** — command-palette fuzzy search no longer panics on characters whose lowercase form is longer (e.g. U+0130), designator increment cannot overflow, the panel context treats an empty sheet paper size as A4 everywhere, and stale comments and dead code are removed.
- **History panel refreshes after a save commit (#1382)** — the History panel now reloads when the auto-commit for the file it shows lands, so the new commit appears without switching tabs.
- **Undo a net-class pick (#1220)** — Ctrl+Z after picking a net class in the Properties panel now undoes the pick, in order with schematic edits, instead of undoing the previous schematic edit.
- **ERC markers and B&W PDF (#1333, #1342)** — ERC markers now stay on the canvas when nothing is selected, and a Black & White PDF export keeps a white page instead of filling it black.
- **A pinned datasheet PDF now travels with the library (#1332)** — Saving a row that pins a PDF stores it at `assets/datasheets/<hash>.pdf` beside the library file and, in a git-backed library, commits it together with the row, so the hash resolves after a clone. Nothing is written when you pick the file, only on Save; a file that is not a PDF, or is over 64 MB, is refused on the Datasheet tab and in Messages. A library that keeps no files beside its library file, such as a server library, refuses the Save with a message to use a URL, before any file is written. If the commit fails the row is not saved and stays unsaved in the editor.
- **Power ports placed from the Active Bar draw their own shape (#171)** — a power port placed without a library entry fell back to the 8 x 5.6 mm placeholder box on the canvas, in hit-testing and in the SVG export, and its placement ghost was always a triangle. Ground, bar, arrow, circle and wave ports now lower to their own body, shared by the canvas, hit-test, ghost and SVG export. Signal Ground and Earth have their own shapes, and the Properties panel's Style row reads the same style the body is drawn from. Earth now hangs below its connection point like the other grounds, so switching a port between Earth and a rising style turns it by 180° to keep its orientation.
- **Missing library entries are marked on the canvas (#1409)** — a placed symbol whose library entry is absent now draws a crossed-out box in the no-connect colour labelled with its `lib_id`, and the load-time warning reaches the Messages panel instead of only the log.
- **Sketch geometry no longer produces duplicate vertices or spiky offsets (#1384)** — boolean unions no longer emit repeated vertices, rounded offsets of sharp notches stay within the adjacent edges, and splitting a pad line no longer leaves stale ids in the pad ownership ledger.
- **A placed symbol keeps its library link across save and reload (#1338)** — `library_id`, `row_id` and `library_version` are now written to `.snxsch`, so drift checks and Update from Library still find the row after a reopen. Sheets without linked symbols save byte-identically.
- **An unreadable `global_libraries.toml` is no longer overwritten (#1352)** — adding or promoting a global library when the file could not be read or parsed used to replace it with a one-entry list, losing the others. The file is now left untouched and the error is shown.
- **Designator-standard pick undoes in order (#1421)** — a pick in the Project tab's General section is now an undo step on the same history as description and parameter edits, so Ctrl+Z no longer skips it.
- **Polygon Difference returns the right shape (#1345)** — subtracting an overlapping polygon, or combining polygons wound in opposite directions, returned garbage rings. Difference now returns the correct outline, and a clip fully inside the subject comes back as a hole.
- **Designator standard letters checked against the standard texts (#1428)** — under EN IEC 81346-2 a transistor is now `Q`, an op-amp `T`, an antenna `T` and an inductor `C`; under DIN 40719-2 a display is `P` and a switching regulator `U`. A class a standard does not name (EN IEC 81346-2 comparator, linear regulator, crystal) keeps the library symbol's letters. Sources and page numbers are in `docs/architecture/designator-standards.md`.
- **A failing recovery-copy move keeps a current copy, and owed moves settle on save and quit (#1433)** — when the move of a recovery copy to a renamed document kept failing, autosave stayed blocked at the new name and every edit made meanwhile was lost after a crash; after two failures autosave now writes the current content to the new name, and opening the document also finds a copy still owed a move (the newest is offered, the other kept). A save now removes the obsolete old copy at once even with autosave off or while quitting, and a save no longer races a running move into bringing back an outdated copy.
- **BOM and footprint bake fixes (#1376)** — `include_dnp` alone now lists DNP parts, which count as not fitted; a clockwise silk arc whose start angle is greater than its end (for example CW 90° to 30°) no longer bakes as the 300° complementary arc; tagging every edge of a closed outline bakes it once per layer instead of once per edge, and a pour or board cutout whose clearance or corner radius cannot be evaluated is skipped with a warning instead of being baked at 0; an array's source pad is no longer baked twice, and is baked normally when its array is skipped; and ratio and angle sketch parameters keep their units when baked. Behaviour changes to know about: a part with `in_bom` or `on_board` off is dropped from default exports and, when listed through the not-fitted option, always counts as not fitted and is never purchased, even with an explicit fitted field, and a unitless parameter reads as a plain number, falling back to millimetres where a length is expected.
- **Keyboard shortcuts (#1378)** — Ctrl shortcuts match on every OS, a shortcut that prefixes a longer chord waits for the next key, editing one binding changes only that binding, and backups never overwrite an earlier one. Menus and context menus show a hint only for a command the active profile binds, never an unrelated default.
- **Rotated labels pinned by tests (#1444)** — internal cleanup: the glyph-atlas anchor maths drops its unused rotation term, and the routing of rotated labels to the CPU canvas layer is now unit-tested.
- **Library fixes (#1361)** — a search query cut inside a multi-byte character no longer crashes; looking a part up by distributor URL picks the matching result instead of the last one; a pin function containing `|` is refused at save instead of silently splitting.
- **Text pre-cull counts a lone CR as a line break (#1450)** — the height bound used by the text pre-cull now treats a bare `\r` like `\n`, so CR-only text is no longer under-measured and culled while still visible. Follow-up to #1429.
- **Save All keeps an edited case variant dirty when the save is refused (#1423)** — with two spellings of one file open and only one edited, a refused Save All no longer clears the edited spelling's unsaved marker, so quitting cannot close and lose its edits; the edited spelling is the one written, and a volume whose case-folding could not be probed is now reported in Messages.
- **Numeric boxes keep what you type (#1352)** — pin and graphic fields in the symbol editor and the pre-placement shape width no longer snap away `-` or `1.` while you type, and a clamped value (a pin length typed as `0.5`) is not rewritten mid-keystroke; the typed text ends on Enter, a tab or selection switch, or undo and redo. Enter reverts text that is not a number.
- **Footprint silk arcs draw the right way round (#1424)** — an arc whose start angle is greater than its end angle was drawn on the footprint canvas as the opposite arc. The canvas now follows the same convention as the bake and click-selection, documented on the arc type.
- **Net-class styling keyed on the case fold (#1344)** — the net-class refresh gate and cache key now include the project's key rules, so a probe that changes them re-runs the stitch. Behaviour change from #1420: stitch warnings (unreadable child sheet, duplicate UUIDs, a root that is not assembled, the "declares no root sheet" fallback) no longer reach the Messages panel on every edit; for a project without net classes they appear only on ERC or export, and for a project with net classes also from the net-class styling job.
- **Flaky Messages-panel test (#1452)** — log entries written from several threads at once could be stored out of id order, which made a sketch test fail at random.
- **A sheet from a newer Signex now refuses edits (#1402)** — editing, net colouring and library updates on a read-only sheet are refused before the document changes, so the tab never turns dirty and no edit is stranded until save. The tab shows a "read-only" label and the canvas a badge; Messages names the file once per open.
- **Library server no longer silently overwrites primitives (#1445)** — `POST /symbols`, `/footprints` and `/sims` are create-only (409 on an existing uuid) and replacing goes through `PUT` with an `If-Match` revision, and the client checks `/version` for `primitive_revisions` before any primitive request so an older server is refused before anything is written.
- **ERC lists violations in a stable order (#1364)** — the same design could list duplicate-designator and net-label-conflict violations in a different order from run to run.
- **Sketch fixes (#1386)** — BGA row lettering no longer overflows for very large start rows, and a colinear segment that touches another at one point reports where along it the touch is.
- **Opening a file and Ctrl+S no longer split one macOS file into two documents (#1447)** — a loose file now takes its case rule from a probe of its folder, treated as case-insensitive until that answers, so `lib.snxsch` opened beside `Lib.snxsch` focuses the existing tab; Ctrl+S refuses to overwrite another open spelling of the same file whose contents differ.
- **3D view tile layout (#1371)** — the tile grid's row count no longer collapses when there are more items than columns.
- **Dock tabs and the draw-mode pill (#1379)** — closing or undocking a tab to the left of the active one keeps the same panel active; clicking the Wire/Bus pill while drawing a bus keeps the Bus tool; the Active Bar's chips, toggles and pill follow the theme.
- **Review leftovers (#1470)** — the next designator now clears every open sheet of the project, a failed engine install keeps the open sheet and reports to Messages, startup parses `prefs.json` once, and the menu bar no longer clones the recent-projects list each frame.
- **Courtyard outline merges abutting pads and says what it leaves out (#1473)** — pads are grown by the courtyard slack before the union, so a row of touching pads gives one outline. When the single outline polygon cannot cover every pad, the failure names the pads in the Messages panel. "Recompute footprint courtyard outline from pads" is now reachable from the command palette.
- **Black & White print preview matches the PDF (#1403)** — light lines and text no longer disappear from the preview on light themes, and a selection highlight no longer stays painted after resuming placement.
- **Polygon booleans refuse input they cannot compute correctly (#1448)** — `polygon_op` now returns a typed error for rings that touch along an edge or vertex within the contact tolerance, for near-contact crossings it cannot classify, and for a walk that cannot close, where it used to return a partial or inverted ring that read as a plausible wrong shape; the footprint courtyard outline reports refused pads in the Messages panel when it is recomputed.
- **ERC flags one bus drawn at two widths (#1049)** — separate runs of the same bus on one sheet with different or offset widths (for example `D[0..7]` and `D[0..3]`) are now reported.
- **Active bar "Flip Selected Sheet Symbols Along X / Y" now matches the X / Y keys (#1404)** — the two flip entries were crossed, so Along X flipped top-bottom while the X key and the context-menu Mirror X flip left-right; all three now flip the same way.
- **Toggling theme, grid, snap, the pin matrix or the first-run tour no longer stalls the UI on a disk write (#1486)** — these `prefs.json` writes now run on a background worker, and the newest value always wins when toggles come in quickly.
- **Library updates land after a late mount (#1449)** — when a sheet opens before its library has mounted, the update check now runs again once the library mounts, instead of waiting for the sheet to be reopened. Re-opening a library from the browser after closing it runs the check again too.
- **Power ports now connect with or without a library entry (#1408)** — Active Bar ports are placed with a pin at their anchor, and a port with no entry resolves to the same built-in body in the netlist, ERC and drawing, so old files connect unchanged.
- **Every library commit now carries any untracked datasheet PDF that a row pins (#1471)** — a save that failed twice no longer leaves a pin in HEAD without its PDF after the next save, insert or delete of any row, and a store that finishes after its editor was closed, reopened or saved again no longer acts on the newer save.
- **Saving a `.snxlib` keeps keys this build does not know (#1435)** — a save now edits the file's own text instead of rewriting it, so unknown keys, key order and untouched rows survive; a save that would drop an unknown key is refused, and duplicate class keys are rejected at load.
- **One identity fold for Save All, open and Ctrl+S (#1505)** — a directory whose case rule cannot be probed is now treated as one file by all three, instead of Save All keeping the host default.
- **Edit Component Details is reachable again (#1500)** — select a row in the Library Browser and press Edit Details to open it.
- **Active Bar right-clicks match the drawn buttons (#1495)** — the right-click hit-test now uses the same slot layout the bar is drawn with, so a right-click toward the right end no longer opens a neighbouring button's menu.
- **Project Replace refuses a stale designator hierarchy (#1514)** — a sheet symbol placed, removed or renamed on an open sheet while Replace reads unopened sheets now cancels the replace with a Messages line instead of silently rewriting the new instance.
- **ERC reports name the rules that produced them (#1503)** — exporting while a newer ERC run is still going no longer labels the old results with the new rule set, and Save as profile no longer creates a second profile with the same name.
- **Preferences ERC severities wait for Save (#1504)** — changing a severity or pressing Reset in Preferences no longer writes the preferences file on the spot; Discard & Close drops the edit and Save writes it, like every other Preferences field.
- **Active Bar right-click and dropdown line up with the Wire/Bus pill (#1509)** — while the Wire or Bus tool shows its draw-mode pill, right-clicking a bar button and the dropdown it opens now match the drawn button, also with side panels docked.
- **Saving Preferences keeps ERC severities set in the ERC dialog (#1504)** — a severity changed in the ERC dialog while Preferences was open was reverted when Preferences was saved; Save now applies only the rules changed in Preferences.
- **Preferences footer clears after a matching ERC dialog edit (#1504)** — setting a rule in the ERC dialog to the value already pending in Preferences no longer leaves the unsaved-changes footer up.
- **A recovery copy no longer stays stranded after a project rename (#1510)** — a copy whose move keeps failing is finished at the next start, and a copy saved under a dot-segment or case spelling of the old path is carried too.
- **Escaped pin tags are still committed (#1522)** — a hand-edited datasheet cell that spells the pin kind with a JSON escape no longer has its pinned PDF left out of the library commit.
- **Passive calculator, symbol preview and library server fixes (#1371)** — switching passive-calculator tabs edits the shown part, the symbol preview no longer truncates pin rotations, and library-server health checks are never rate limited.
- **Preferences: an unhandled Distributors message can no longer close the window (#1379)** — the Distributors pane mapped any message it did not expect to Close; it now ignores it. No current control sends such a message, so this is defensive.
- **ERC rule files catch duplicate rule ids (#1364)** — two rules with the same id in an `erc.dsl` are now reported at the second definition (and the file loads no rules until fixed, with a warning), and parse errors underline the whole bad span.
- **Invalid BGA start row is reported (#1386)** — a BGA array whose start row is not a valid row letter now warns at bake time instead of silently numbering from row A.
- **Recovery copies are keyed by the case-folded path (#1540)** — an autosave recovery copy is now filed under the document path folded by its volume's case rule, so `Sheet.snxsch` and `sheet.snxsch` on a case-insensitive volume share one copy and are found whichever way the file is opened, while on a case-sensitive volume they stay separate. Copies filed under an older key (the raw, the as-given or the volume-folded spelling) are still found and moved to the new key; with copies at more than one key only the newest is offered and the others stay on disk, logged. Nothing is deleted or overwritten.
- **Footprint sketch bake no longer keeps stale geometry (#1544)** — when the sketch bakes none of a kind it owns (pours, cutouts, keepouts, mask openings or excludes, paste apertures, V-scores, top or bottom silk, courtyard), that field is cleared instead of keeping the previous bake, so Save cannot write geometry the sketch no longer produces; the clear is reported in the Messages panel. The sketch owns a kind while it holds an entity tagged for it or after it baked that kind (recorded as `baked_kinds` in the sketch, absent from files the sketch never baked), so hand-authored or imported pours and hand-placed silk are kept.
- **Undo and Redo follow the window you type in (#1546)** — Ctrl+Z / Ctrl+Y in an undocked schematic or PCB window step that window's document, instead of whatever tab is active in the main window.
- **Recovery-copy moves at startup are bounded and race-free (#1540)** — the startup pass that finishes pending recovery-copy moves handles at most 256 per start, and clearing a finished move can no longer delete a newer one recorded at the same moment.
- **Edit menu Undo and Redo follow their window (#1550)** — in an undocked schematic or PCB window, the Edit menu's Undo / Redo rows are greyed by that window's document and step it, as Ctrl+Z / Ctrl+Y already do.
- **Sketch dimensions read count and angle parameters correctly (#1384)** — a dimension expression that uses a count or angle parameter (for example `pitch * (pin_count - 1)`) is no longer evaluated as if every parameter were a length.
- **Command palette and net colour fixes (#1351)** — a palette command now follows the same enabled/disabled rule as its shortcut, and an empty or invalid value in a custom net-colour channel keeps the previous value instead of setting it to 0.
- **Array counts and PDF fit-to-page (#1472)** — an array whose count resolves to a length or angle is skipped with a warning instead of baking a wrong number of pads, and PDF fit-to-page now includes drawings and full symbol bodies so they are never clipped.
- **Save All case-variant test no longer flaky on macOS (#1576)** — the test that feeds a case-sensitive probe now only asserts on a volume that really is case-sensitive, since on APFS the two spellings are one file.
- **Arrange follow-ups (#1583)** — the Active Bar Bring To Front and Send To Back now reorder selected drawings too, SVG and PDF export paint Behind drawings under the wires and Above drawings over the symbols, and the canvas context menu's Arrange entry has an icon.
- **Search index survives drop and reopen on Windows (#1593)** — the Tantivy writer is now wound down (merge threads joined) on close and commit retries transient access-denied sharing violations.
- **Library part numbers and tokens (#1363)** — a local-git library now refuses an insert or edit that repeats another part number (blank drafts excepted) or a table name with an illegal character, and a raw database auth token is no longer printed by `Debug`.
- **Preferences saves no longer freeze the window (#1351, #1602)** — Save Preferences, the recent-projects list, custom filter presets and the default rule profile choice now write `prefs.json` in the background, and quitting right after waits for those writes so the last change is not lost. New libraries take their symbol grid size, pin selection and component classes from the in-memory settings, so a library opened right after Save sees the saved values.
- **Recovery dialogs skip read-only libraries (#814)** — a stock library that fails to open no longer offers the git re-init recovery, which would have written to it; a plain read-only explanation is shown instead.
- **Save All and the Exclude prompt no longer lose a case-variant file's edits in a folder whose case rule is unknown (#1447)** — they write the first dirty spelling, keep the other spellings unsaved and name them, and Quit or Close stays open until each spelling is saved.
- **Part numbers are unique in the library server database (#1613)** — a unique index on the trimmed part number makes two writers racing the same new PN end with one created and one conflict, and the migration refuses a database that already holds duplicates without touching any row.
- **Whitespace-only part-number duplicates are caught (#1613)** — a part number that differs from an existing one only by surrounding spaces is now refused as a duplicate, and a password in a database URL no longer shows up in debug output.
- **Library adapter doc (#1363)** — the `library_id` doc comment names the `.snxlib` root key instead of the removed `library.toml`.
- **Library refresh keeps component descriptions (#1375)** — the off-thread library refresh blanked every picker summary description; it now keeps the row description, and the primitive picker, updates dialog and library panel use theme tokens instead of fixed colours.
- **Context-menu shortcut hints follow the keymap (#271)** — the canvas right-click Rotate, Mirror X, Mirror Y and Delete rows now show the shortcut from the active keymap profile instead of fixed text.
- **Legacy kΩ values edit at the right scale (#1606)** — the component editor's Parameters tab seeded a legacy `{4.7, "kΩ"}` as `4.7` labelled with the template unit `ohm`, so committing it, even unchanged, stored 4.7 Ω, 1000 times too small. It now opens as 4.7 with the `k` prefix picked and stores 4700 Ω on a change; an unchanged commit leaves the value alone.
- **No TLS panic, PN uniqueness on the database backend (#1613)** — distributor HTTP clients are built on first use so a TLS failure is an error, and the database adapter refuses a part number another row holds.
- **Newest recovery copy offered (#1644)** — when a document has recovery copies under both its old and current key, the newer one is offered and the other is kept as `.recovery.older` instead of being hidden (#1540).
- **Footprint sketch: a rejected over-constraining edit is listed in the constraints panel (#685)** — the conflicting constraints appear in the Properties DOF section in the theme danger colour, and clicking a row selects its entity; the sketch stays at the last good solve.
- **Keyboard shortcuts (#103)** — The Properties, schematic filter and schematic list panel shortcuts now open a closed panel, close it when it is open and in front, and bring it forward when it is hidden; Next/Previous Tab is pinned by tests to step every tab kind with wrap-around.
- **A bent wire draws both legs at the same weight (#1654)** — the legs had one colour, but the canvas drew an axis-aligned hairline one or two pixels thick depending on its position, so the thinner leg read as black; hairlines now snap to whole pixels.
- **Stock library ships placeable parts (#1653)** — `Stock.snxlib` listed no components, so Place Component and the Components panel Place button had nothing to place from Stock. It now ships 34 rows (R, C, L, D, LED, transistors, connectors, the 74HC parts and more), with the 0805 chip footprint on the two-pin passives and SOIC-8 on the generic IC.
- **Stock diodes and LEDs get a polarised footprint (#1657)** — D, D_Schottky, D_Zener and LED were paired with the symmetric `R0805` land, so a board could be built with them backwards; they now use `Chip-0805-Polarised` (pad 1 = cathode, cathode bar on the silkscreen, courtyard and fab outline), and R, C, L, Ferrite and Fuse use the generic `Chip-0805`.
- **Capturing a footprint filter preset no longer freezes the editor on disk IO (#1639)** — the preset is saved to `prefs.json` on a background writer, ordered against other captures, instead of read and rewritten inside the UI update.
- **Library diffs now show changed alternates and supply listings (#1363)** — an alternate whose status or notes changed (for example Approved to Disqualified), or a distributor listing whose URL or minimum order quantity changed, is reported as a change instead of being invisible to the diff.
- **Footprints with a courtyard tag save again (#1664)** — a footprint whose sketch carried a courtyard-tagged entity failed to write to `.snxfpt` because the marker could not be represented in TOML; it now saves and reloads unchanged.
- **One builder for panel symbol lists (#1381)** — the document-load and refresh paths build the panel's symbol and placed-row lists with the same function, so the two can no longer drift apart.
- **Next designator is unique across the project (#1381)** — a new part is numbered past every designator used on any sheet of its project, closed sheets included (scanned off-thread when the placement starts), and the tab-strip placeholder and the strip share one height constant.
- **Footprint sketch edits no longer freeze on large sketches (#1683)** — the redundancy check now draws on the solve work budget and is skipped with "redundancy check skipped (budget)" when it would not fit, and a failed field bake no longer leaves the baked pads unrecorded.
- **The Linux AppImage starts on glibc 2.35 and newer (#1688)** — the Linux release builds now run on Ubuntu 22.04, so the AppImage, `.deb` and tarball need glibc 2.35 or newer instead of 2.39, and run on Debian 12, Deepin 25 and similar current stable distributions. The release build fails if a newer glibc symbol slips in.
- **Net highlight and stepping follow the sheet placement** — a child sheet placed twice no longer lights up the other placement's copy of a highlighted net, and stepping to a member in another placement switches to it; a failed net-index build retries when you run a command again, closed projects release their index, and a sheet history entry that cannot be opened is dropped.
- **Library cascade and reads:** a symbol or footprint save commits one `.snxlib` write whose message names the rows it actually moved, a cascade that fails on a retryable error is run again by the next Save and one that cannot succeed says what to fix, `read_row` converts only the row asked for, and `snapshot_table` reuses rows it already converted.
- **Opening a stock symbol or footprint edits a copy (#813)** — the read-only stock original is never opened; the first open asks which writable library receives the copy (or creates one), remembers it, and opens the copy under the stock name with a suffix only on a collision.
- **A failed git commit no longer fails a save (#858)** — a library save whose file was written is kept in every write path (rows, tables, classes, symbols, footprints, sims, external edits, renames) even when git then refuses the commit; Messages says "Saved, not committed" once per cause, the library browser shows an uncommitted-changes banner with a Commit now button, and the next successful commit records the pending files.
- **Library edits no longer freeze the window (#893)** — adding, editing, deleting and binding rows, inline cell edits, creating, renaming and deleting tables, the Component Preview Save and the git re-init of a library now run in the background, in the order you made them, and quit or closing the library waits for them. A second quick edit to the same row is applied on top of the first instead of being refused; only a change made by another process or user is still refused.

### Fixed — BOM

- **BOM counts every child sheet, once per placement (#730)** — the BOM was built from the printed pages only, so parts on hierarchical child sheets were missing and a child placed N times counted once. It now walks the same project hierarchy as the netlist and lists N copies of a child's parts (power symbols still excluded). Per-instance designators do not exist yet, so each copy carries the sheet's own designator. A loose document keeps exporting every unowned open tab, as the PDF does.
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
- **The remaining flat root `Message` leaves are namespaced (#162)** — `Esc`, unclaimed keys and the footprint mode shortcut moved under `Message::Input`, the selection-mode cycle and lasso commit under `Message::Selection`, the drawing-field edit under `Message::Edit`, and the history-load result and no-op under `Message::Ui`; the root `Message` is down to 37 variants. No behaviour change.
- **Schematic groundwork for the GPU render path (#199)** — the scene cache shares its scene by reference with a process-wide generation, and the schematic can now be mounted as a backdrop, a GPU shader layer, a second GPU layer for the items of a move-drag and an interactive overlay that draws the rotated text. The mount stays behind an off gate. Internal groundwork, no visible change.
- **Context menus: canvas and Place / Align rows now read their text from the command table (#271)** — Cut, Copy, Paste, Paste Special, Mirror X/Y, Delete, Preferences, Properties and every Place / Align flyout row take their label from the command catalog, so one command reads the same on the menu bar, Active Bar and context menu; the visible wording is unchanged. Rows with no command id (Find Text..., Rotate, project-tree and tab rows, the greyed stubs) keep their literal text.
- **Saving a row in a large component library is several times faster (#416)** — an insert, edit, delete or version bind on a 50k-row library takes about 0.25–0.45 s instead of 1.2–1.5 s (about 40–110 ms instead of 250–350 ms at 10k rows), while the file is still written whole and atomically, committed once per edit, and an edit from a stale copy still refused.
- **Font-derived sizes in view and preferences (#509)** — text sizes, spacings and paddings in `app/view/**` and `preferences/**` now come from `TypeScale`/`Spacing`; no visual change.
- **Font-derived sizes in the dock panels (#509)** — text sizes, spacings and paddings in `panels/**` now come from `TypeScale`/`Spacing`; no visual change.
- **Font-derived sizes in library and remaining views (#509)** — text sizes, spacings and paddings in `library/**`, `active_bar/**`, `dock/**`, `find_replace/**`, `menu_bar/**` and the remaining modals now come from `TypeScale`/`Spacing`; no visual change.
- **A document's unsaved state has one owner (#661)** — the editor flag, the workspace dirty set and the tab's own flag are no longer three copies written from about 60 places: `DocumentState::is_document_dirty` answers for every document kind, the tab dot, the title-bar count, the quit and close gates, Save All and the project tree all read it, and `mark_document_dirty` and `mark_document_clean` are the only writers. A symbol or footprint draft now counts as unsaved the moment it is edited, not after the next panel refresh. See `docs/DIRTY_STATE.md`.
- **Changelog entries are per-PR fragments (#748)** — a PR adds `changelog.d/<number>.<section>.md` instead of editing `CHANGELOG.md`, so merges no longer make every other open PR conflict on it; `scripts/changelog_assemble.sh` folds the fragments into `## [Unreleased]` (or opens the `## [X.Y.Z]` section with `--version`/`--date`) at release time, and CI rejects a non-release PR that edits `CHANGELOG.md`.
- **Why `.snxprj` is JSON (#384)** — new `docs/architecture/snxprj-format.md` records the history (JSON persistence landed after the v0.9 TOML+TSV cutover, with no recorded reason), the constraints that hold today, and the open gaps (unknown keys are dropped on save, no format token). The README no longer describes `.snxprj` as TOML+TSV, and the `format/mod.rs` and `ProjectData` docs now state the reason and link the new page.
- **Release gated on tests (#113)** — the release workflow now runs `cargo test --workspace` before building binaries, and the library server gains tests for the bind policy and rate limit, plus signex-types TSV field-corruption tests. A float cell in a `.snxsch`/`.snxpcb` table that is empty, `NaN` or infinite is now reported as a parse error instead of loading as `0` or a non-finite value, and saving a document that holds a non-finite float now fails with an error naming the block, row and field instead of writing a file that cannot be reopened.
- **Idle cursor moves no longer run drag handling (#111)** — a mouse move with no drag in flight only records the pointer position (plus a cheap panel-state repair), and BOM preview sorting allocates one key per row instead of two per comparison. A PDF pan in progress also counts as a layout drag for this guard, so its cursor moves keep reaching the pan handler.
- **Opening from the project tree and History restore no longer freeze the window (#859)** — double-clicking a schematic, PCB, symbol or footprint in the project tree, and Restore in the History panel, now read and parse the file on a background thread. A result that arrives after you closed the tab, all documents or the project is dropped instead of reopening it. A second click on the same file while it loads opens one tab. Failures still reach the Messages panel with the same text. While a restore runs, saving that document is refused with a Messages line, also after its tab was closed, until the restore's worker reports. Save All reports it as that file's failure. A restore cancelled that way still lands in a clean tab reopened meanwhile, and a tab with unsaved edits keeps them with a Messages line saying a save would overwrite the restore. Edits made meanwhile are kept instead of being replaced. A save of unchanged content does not count as an edit. A finished restore never switches tabs or touches the tab you are working in. A read that lands on a sheet whose tab-less engine holds unsaved edits keeps those edits and attaches the tab to that engine. Annotate and Reset Duplicate Designators leave an unopened sheet unchanged, with a Messages line, while a read or restore of it is running. Delete from disk is refused while a restore of the file is running. Rename of the file, or of its project folder, is refused while a restore of it is running. A Restore clicked while that file is still being saved is refused too, because the save's "Save X" commit would otherwise record the restored bytes instead of the ones you saved.
- **Renaming and deleting from the project tree no longer freeze the window (#859)** — renaming a sheet, PCB, primitive or library, and deleting a file or library, now move or remove the file on a background thread, and open tabs, engines and project entries follow once it finishes. While one is running, saving, restoring or reopening that file, starting a second rename or delete of it, and edits to the library being renamed are refused with a Messages line naming the cause (other libraries stay editable); a refused library edit also says so on screen; project-wide Annotate, Reset Duplicate Designators and project Replace are refused as a whole, on screen, while a rename or delete holds one of their sheets or pages or moves a file in the project folder, and a Replace already reading the sheets is cancelled rather than writing a moved or deleted sheet back; a held quit says so on screen; edits made meanwhile travel with the renamed document. A project or tab closed while it runs is handled by what is open when it finishes, and a library rename the app cannot follow is moved back off the UI thread. Failures still reach the Messages panel with the same text.
- **Search boxes no longer write `prefs.json` on every keystroke.** The component filter and library browser searches are saved once typing pauses, off the UI thread, and quit waits for the write.
- **Shortcuts follow the menu's enabled state (#907)** — a shortcut for a command the menu shows greyed out now does nothing, so Ctrl+S with no document open no longer runs a save. Undo and Redo are now enabled in the symbol and footprint editors, where they already worked from the keyboard, and Save is enabled whenever the project file has unsaved changes.
- **Theme-aware hover and primary-button colours (#114)** — dialogs migrated to the new shared styles show hover and pressed feedback on light themes too, and primary buttons use a darkened shade of the theme accent with white text that meets 4.5:1 on every theme (on the default Signex theme, Save All / Apply / OK turn dark orange instead of blue).
- **Annotate, BOM, command palette and Run ERC no longer do heavy work per frame or on the UI thread (#111)** — the Annotate preview is cached and reloaded off-thread whenever the open documents or the project page list change, the BOM row order and the command palette ranking are computed when their inputs change instead of every frame, and Run ERC reads child sheets and runs the rules in a background task (the ERC panel shows "Running…", and unreadable sheets or rule-file errors appear in the Messages panel). A newer run supersedes an older one, and Clear ERC cancels one in flight. While the Annotate list is reloading, lock-all and the Annotate buttons wait instead of acting on a stale list; a failed reload says so and is retried; Enter in the command palette runs the row you highlighted.
- **Saving a schematic no longer freezes the window (#859)** — Ctrl+S and Save As encode the sheet, then write it on a background thread; the unsaved marker clears only if the sheet is unchanged since the snapshot, a failed write keeps it and appears in the Messages panel, and quitting, renaming a sheet and restoring from History wait for a write in progress so an older write never lands over newer content.
- **Zone holes: the tolerant ear clipper does far less work on a long thin strip of cut-outs (#1006)** — it now asks the vertex grid for the cells the ear triangle crosses instead of the cells under its bounding box, reaches the corners of a long thin ear first, and tries the vertex that blocked the previous ear before searching; the triangulation is unchanged. An 8000-hole 1000:1 strip clips about three times faster, though the cost on such a strip still grows faster than linearly.
- **Schematic edits refresh less of the render cache (#171)** — the cache now re-copies only the collections a change touched instead of the whole sheet, annotate and sheet-reference rewrites pass the narrow set, and the PCB software-render path reuses the cached scene on pan and zoom instead of re-tessellating the board.
- **Export and New Project file writes no longer run on the UI thread (#859)** — PDF, netlist (including Export anyway) and BOM exports hand their bytes to a background crash-safe write, and New Project creates its directory and files in the background and opens the project when they are done; a failed write is reported to the user once it completes.
- **Annotate order has one source (#990)** — the part grouping and the numbering pass read the processing order from the same place, so multi-unit parts always pair up in the order the dialog shows.
- **The project tree no longer touches the disk on every refresh (#111)** — the `(missing)` marks and each library's symbol and footprint lists come from a cache that a background thread refills when a project or library changes and on the existing 2 s external-edit check, so files deleted or added outside Signex still show up within a couple of seconds and the window never waits on a slow drive.
- **Faster zone cut-outs (#971)** — joining many holes into a copper zone is now near-linear (a 100x100 grid of cut-outs bridges in ~60 ms instead of ~1.9 s), and odd or non-finite zone geometry no longer panics.
- **Text-size and spacing scale (#509)** — dialogs share one type scale (caption/body/label/title/heading) and spacing steps instead of hard-coded sizes; library close, document options, distributor settings, STEP attach and the footprint pad/align/move dialogs are first, with some gaps shifting by a pixel or two to the shared steps.
- **Footprint sketch drags no longer freeze the editor while the solver runs (#1047)** — a drag tick moves the geometry at once and solves it on a background task; a result is applied only if no newer edit has happened since, and ticks that arrive meanwhile are coalesced into one follow-up solve. A drag is now one undo step, not one per cursor tick. During a continuous drag the pads, silk and constraint colours do not follow the cursor until it pauses for longer than one solve, because every result that arrives mid-drag is already stale. A save, an undo or redo, a footprint switch, a mode switch or any other message except the drag ticks, their solve result and cursor tracking first finishes the owed solve, so nothing unsolved is ever written or kept in undo history. Other sketch edits still solve inline.
- **Bus labels are read strictly (#1048)** — bus labels are read strictly. `D[0 .. 7]`, `D[0..7]A` and other text the bus syntax does not allow are now refused instead of being half-understood; ERC reports each one, anywhere along the bus, as a bus label that is not a valid bus name. The bus width check compares the number of signals rather than the numeric bounds, and two names that carry different signals are reported as a name conflict. Net connectivity is unchanged: a bus-name label on a wire still names its net, and ERC now warns about it (#1048).
- **Dragging a bus carries its entries (#1050)** — bus entries sitting on a dragged bus move with it, and with wire keeping on the wires ending on their far side follow, all in one undo step.
- **A net with several names uses one deterministic name (#1043)** — Power beats Global beats Hierarchical beats a local label, ties go to the full name (including the sheet path) that sorts first whatever the order of child sheets in the file, and every other name stays an alias. Exported net names can change when an existing project is reopened: Power now beats Global, ties resolve on the full name, and hierarchical names are qualified by the winning sheet. Custom ERC rules that match `net.name` may need to use `net.names` to keep matching an alias. See docs/NET_NAMING.md.
- **Symbol / footprint saves and the sheets of Save All no longer freeze the window (#859)** — saving or saving as a symbol or footprint library, and the schematic and library writes of the close-project and quit Save All, now run on a background thread; the project file (`.snxprj`) and Component Preview rows are still written inline. A draft edited while it saves stays marked unsaved, a document edited during Save All is asked about again before the project closes or Signex quits, a second save of a file still being written runs once that write finishes, a failure still reaches the Messages panel and the Save Failed card, and quitting waits for the writes.
- **"Current page" export and print preview print the copy of a placed-twice child you descended into (#1133)** — descending through a sheet symbol (double-click, context menu, Navigator, Up Hierarchy) now records which placement the tab shows, so the page carries that placement's designators instead of the smallest-uuid guess; a sheet opened straight from disk keeps the guess. The canvas itself still shows each part's base designator.
- **PDF pages match the canvas (#1146)** — With "Title block" on (the default), exported PDFs and the print preview now draw the sheet frame, zone markers and title block, and lay out the whole sheet on the page so parts keep their place against the frame. Untick "Title block" for the previous content-fitted layout.
- **User guide refreshed (#1158, refs #322)** — `docs/USER_GUIDE.md` now covers the bundled Stock library and the Components panel sections, and adds an end-to-end walkthrough from a new project to a saved, annotated, ERC-checked sheet. It states that placing a library part is not yet reachable from a menu or button.
- **Library parts can be placed from the UI (#1160)** — the Place Component picker and a **Place** button on each Components panel row now put the selected part on the active schematic as one undo step, or show a **Place Failed** notice when no schematic is open.
- **Panning the schematic no longer rebuilds the drawn scene every frame (#171)** — the content layer keeps the scene it built and replays it through the live camera, so a pan inside the built margin skips the snapshot-to-scene rebuild; a zoom step, a theme change or any document edit still rebuilds once.
- **Custom width and height are applied on Enter, and custom edges under 10 mm are refused (#1196)** — typing in the Width or Height box keeps your text as a draft and writes one undoable edit when you press Enter, so typing `400` gives 400 mm instead of being raised to 10 mm on the first key. A stored `Custom` size with an edge between 0 and 10 mm, which an earlier build accepted, is now treated as unrecognised: `Custom 5x100` is drawn and printed as A4 landscape, the Messages panel reports it when the sheet opens, and the stored text is left unchanged. The old `A`..`E`, `USLetter` and `USLegal` aliases printing as A4 landscape shipped earlier in #1188.
- **The title block is a compact table (#1201)** — the built-in title block on the canvas and in exported PDFs is now a 110 × 28 mm table of labelled cells (Title; Doc No, Rev, Sheet, Date; Company, File; Drawn, Checked, Approved) instead of a 180 × 40 mm box of loose text, and a long value shrinks to fit its cell.
- **Rename Project renames the project folder too (#1221)** — when the folder holding the project is named after it (`Foo/Foo.snxprj`, as New Project creates), renaming to `Bar` leaves `Bar/Bar.snxprj`. Open tabs, unsaved-change markers, the Open Recent list and the version-control root follow the move. The folder is moved off the UI thread, and the rename is refused if a folder with the new name already exists or a save, an open or a version-control commit in the project is still running; if the folder cannot be moved the project is left as it was. A project whose folder has another name, or that mounts project libraries, keeps its folder and only the `.snxprj` is renamed.
- **Library server refuses row writes from older clients (#1230)** — `signex-library-server` now rejects component-row creates and updates that do not declare the current row schema, so an older signex build can no longer silently drop row fields it does not know. Upgrade signex clients together with the server.
- **Selecting and dragging many items no longer rescans the whole sheet per item or per frame (#1334, #1335)** — the selection overlay builds the item bounds once per draw instead of once per selected wire, label or drawing. A move-drag now draws the items it leaves alone once, in a retained layer, and each frame draws only the moving items (and, with "keep wires", the wires it stretches) on top, so drawing a frame costs the size of the selection rather than the size of the sheet. The layer is drawn again on a zoom, a pan, a theme change, an edit, or when the set of stretched wires changes. A pan or auto-pan during a drag therefore still costs a full redraw per frame. With "keep wires", each new snap step still recomputes the stretched wires over the sheet. **Intended change:** while dragging, a moved item always draws above every unmoved one, even where it normally sits under an overlapping item; it returns to its place when dropped.
- **Panel cleanups (#1354)** — fixed stale and garbled panel doc comments, removed four unused form-row helpers and orphan section banners, dropped the duplicate Components arm in the panel router, and replaced the developer-facing Library stub text with a user hint.
- **Review cleanups (#1360)** — no user-visible change: stale canvas, preferences and arc-span docs corrected, and the unused grid state, ERC marker uuid and identity text-escape stubs removed.
- **PCB canvas reuses the scene dashed-line helper and applies fit-to-board in one step (#1360)** — the duplicate `draw_dashed_line` and the redundant `pending_fit` copy are gone; behaviour is unchanged.
- **Format and engine cleanups (#1364)** — the dead `net` column is gone from the `.snxsch` wire table (no version bump, old column not read), junction reconciliation uses a per-command spatial index instead of scanning every wire per junction, and the unused `auto_net_name` is removed.
- **Doc fixes in project navigation and net selection (#1368)** — finished the truncated module docs, noted that a failed delete leaves the file's tabs closed, and re-wrapped the `expand_to_net` doc with its connectivity limit.
- **GPU scene uploads are dirty-gated and viewport-culled (#1371)** — `scene_shader` now uploads only the categories that changed and only items near the view (with a re-cull on pan/zoom), `cull_items` is a linear filter, and `debug_pass` is behind the `debug-pass` feature.
- **Font-derived sizes in preferences, find/replace, status bar, tab bar and active bar (#1379)** — text sizes and paddings now come from `TypeScale`/`Spacing`; no visual change.
- **Editing a large schematic no longer re-clones every symbol's strings into the panel context on each edit (#1383, partial)** — the library-symbol and placed-symbol lists are reused while the render snapshot is unchanged and rebuilt when its revision moves; the project-tree rebuild and designator refresh per edit are still open.
- **Sketch solver uses an analytic sparse Jacobian (#1386)** — each constraint kind now supplies its own partial derivatives over only the variables it touches instead of two residual passes per variable, and the unused public geometry modules (convex clip, fixed-point, half-edge, hull, arc offset, polylabel, simplify) are removed.
- **Stale docs, labeler globs and script headers brought back in line with the tree (#1387)** — the PR labeler now matches the directories that replaced `schematic_runtime.rs`, `menu_bar.rs`, `active_bar.rs` and `preferences.rs` and covers `panels/`, `keymap/` and `signex-net`; the keyboard-shortcut, rendering-rules, codebase-map, UI-workflow, CONTRIBUTING and ADR-0001 docs point at code that exists (native format codec is in `signex-types`, not `signex-engine`); the README no longer claims a mounted schematic GPU path or KiCad netlist export; the lint-lockstep and discarded-task script headers quote the current lint state; the unreferenced `scripts/sync-version.sh` is removed.
- **No synchronous netlist rebuild on the UI thread (#1420)** — stopped rebuilding the project netlist synchronously on the UI thread after every connectivity edit; nothing read that cache.
- **Undo history eviction is O(1) (#1427)** — the engine keeps its undo steps in a deque, so dropping the oldest step at the 100-step cap no longer shifts the whole list.
- **Off-screen schematic text is skipped before shaping (#1429)** — labels that cannot reach the viewport are no longer shaped each pan or zoom frame, so zoomed-in sheets with many labels redraw faster; what is drawn is unchanged.
- **Faster schematic click hit-testing (#1430)** — picking a wire or bus under the cursor no longer searches the sheet by uuid for every item, so clicks on large sheets stay responsive.
- **Menu bar and icons stop redoing per-frame work (#1380)** — the active keymap is shared by `Arc` instead of deep-cloned into the menu context on every view, tinted icon handles are built once per icon and theme instead of on every call. Behaviour is unchanged.
- **Icon cache keyed on the asset path (#1455)** — the tinted icon handle cache now keys on each icon's path instead of the address of its embedded bytes, so identical SVG assets can no longer alias a cache entry, and the leftover no-op ambiguity guard in the active bar is gone. Behaviour is unchanged.
- **Faster sketch solving (#1385)** — dimension expressions are parsed once per solve and entities are looked up by index, instead of re-parsing and scanning on every solver step.
- **Library screens stay responsive (#1374, #1355)** — after a library edit the component list is re-read in the background instead of freezing the window, and the Place Component picker and the Library Browser grid no longer re-filter and re-sort on every redraw.
- **Startup and panel refresh no longer block the UI on disk or foreign-tool probing (#1470)** — the Navigator's sheet hierarchy is assembled off the UI thread (the previous hierarchy shows until the new one lands, and a superseded result is dropped); the global and bundled stock libraries mount in the background instead of before the first frame; the Components panel's leftover install-directory probe for another tool's symbol libraries is gone; sheet-to-project ownership is answered from a cached index that rebuilds only when projects or open sheets' child references change; and the footprint constraint count reads each constraint's entities directly instead of formatting debug text per constraint.
- **Primitive picker windows its rows (#1474)** — the Pick Symbol / Footprint / Sim list now builds only the rows in view plus overscan, so very large libraries no longer stall the UI.
- **Bake passes share one expression-eval module (#1479)** — the cutout, pour, v-score, body3d, pad and array bakers no longer carry private copies of the parameter-context and `=`-prefix evaluation helpers. No behaviour change.
- **Lower undo memory (#111)** — the schematic undo history shares unchanged parts of the sheet between steps instead of copying the whole sheet for every edit.
- **Library modals share themed chrome (#1484)** — The close button and Cancel / primary footer buttons of the Library picker, recovery, updates and close-library dialogs now come from one theme-token set instead of per-dialog hard-coded colours, so they follow the active theme.
- **Faster PCB redraws when zoomed in (#111)** — the PCB canvas skips shapes outside the visible area.
- **Print preview opens faster (#111)** — only the page on screen is drawn; other pages render when you go to them. PDF export no longer blocks the window while it builds the file.
- **Library backend docs state when a save is create-only (#1492)** — the `save_*_versioned` docs now say the default save is only create-only when the plain `save_*` is: a shared backend must make it create-only, as the database backend does, while a local single-writer library overwrites.
- **Faster one-row library saves (#1522)** — saving one row of a large .snxlib no longer converts every row to look for pinned datasheets; a one-row edit at 50,000 rows takes about a third less time.
- **The library browser opens large libraries fast (#1355)** — the component grid now builds only the rows on screen, so scrolling and filtering a 50,000-row library no longer stalls the window; rows are a fixed 24 px tall.
- **Staged datasheet PDFs no longer block the UI on Save All, quit or close library (#1555)** — Save All, quit and close-library now write a row's staged PDFs on a worker and continue (or stop and report) when the write finishes.
- **Faster connectivity and ERC on large sheets (#1366)** — building nets and checking for dangling wires no longer compares every wire with every other item; a 5,000-wire sheet goes from seconds to tens of milliseconds.
- **Components panel stays fast with large libraries (#1353)** — search no longer re-processes every row on every frame, and each library shows at most 200 rows with a "refine search" line for the rest.
- **Navigator rebuilds: row clicks and failures (#1381)** — double-clicking a Navigator row while the hierarchy is rebuilding no longer opens a sheet with a placement taken from the previous hierarchy, and a failed rebuild is retried up to three times (then reported in Messages) instead of leaving the Navigator stale until the project changes.
- **Stray 0.72 text-size literals removed (#1229)** — the label flag geometry and the screen-constant designator text now use the shared `MM_PER_EM` constant instead of their own copy of 0.72; no visible change.
- **Projects tree rows are addressed by key (#1618)** — tree actions (open, rename, remove, reveal, close and the rest) now carry a stable row key resolved against the current tree, so a reordered or rebuilt tree can no longer retarget them.
- **Batched edits snapshot the sheet once (#1366)** — a batch of N commands (paste, find/replace) now takes one undo snapshot instead of N+1 deep copies of the sheet; undo and redo restore exactly the same states.
- **Library: saving a symbol writes the library file and commits once, however many rows use it (#1362)** — the cascade that re-pins bound rows now goes through one batch update instead of one write and one git commit per row; library history gets one commit per symbol save. On a database library the rows are still updated one by one, in order, and the cascade stops at the first failure and reports it.
- **Database tokens saved in plain text now warn once, and can come from the environment (#1363)** — saving a library whose database `auth` is a hand-written token logs one warning per library per session in the Messages panel; setting `auth` to `${env:SIGNEX_NAME}` reads the token from that environment variable when the library connects. Only names starting with `SIGNEX_` (upper-case letters, digits and `_`) are read, so a shared `.snxlib` cannot make Signex send another secret to its server; any other name is an error. The unused per-file TSV table functions (`read_table`, `write_table`, `append_row`, `update_row`, `delete_row`) are removed from `signex-library`.
- **Every new id is a UUID v7 (#1703)** — symbols, wires, sheets and every other new object now get a time-ordered id that carries its creation time; existing ids are never rewritten, and short ids shown in the library and in cascade commit messages now come from the id's random tail, so rows created together no longer share one.
- **Context-menu rows read the command catalog (#271)** — the canvas, Place/Align/Arrange and project-tree menus take each command row's label and shortcut from the same catalog as the menu bar, so they can no longer drift; the canvas "Find Text..." row now reads "Find", and the Open Child Sheet and Properties hints follow your keymap (Enter and, in the Altium profile, F11 no longer show). Open Child Sheet, Create Sheet From Sheet Symbol, Sync Sheet Pins, Change Component and Project Options are now commands in the palette.

### Removed

- **Removed three Appearance settings that did nothing (#635)** — Preferences ▸ Appearance no longer offers Power Port Style, Global/Hier Label Style or Multisheet Style; they were saved but never affected the canvas. Power port style is still chosen per symbol in the Properties panel.
- **Margin and Zones boxes removed from Page Options (#1196)** — the Vertical and Horizontal boxes changed panel state only and nothing read them, so they never affected the drawn or printed sheet. The frame's zone markers are unchanged.
- **Dead component-selection state removed (#1470)** — the leftover `selected_component` placement path, the unreachable Components-panel filter message and its persisted `component_filter` setting are gone; an old `component_filter` key in `prefs.json` is simply ignored.

### Security

- **Library server hardening (#115)** — the server compares the bearer token in constant time, the database client refuses to send a token over cleartext HTTP to a non-loopback host (redirect hops included) and rejects an unresolved `@signex-keychain:` sigil, and distributor URL host checks are dot-anchored so look-alike domains no longer match. TLS must be terminated by a reverse proxy.
- **Library server refuses plaintext on a non-loopback bind (#115)** — `signex-library-server` now exits at startup unless `--allow-insecure-bind` (or `SIGNEX_ALLOW_INSECURE_BIND=1`/`true`/`yes`) is given, since it has no TLS and must sit behind a TLS-terminating proxy. `SIGNEX_TRUSTED_PROXIES` lets the rate limiter key by the real client behind a trusted reverse proxy rather than sharing one bucket; when it is set, `SIGNEX_TRUSTED_PROXY_HEADER` (`x-forwarded-for` or `forwarded`) is required and names the single header the proxy overwrites or appends to. The other header is ignored, so a client cannot forge hops through it. IPv4-mapped peers (`::ffff:10.0.0.1`) match IPv4 proxies and hops may carry a port. The `@signex-keychain:<name>` library `auth` sigil stays refused (OS keychain resolution is tracked in #976).

### Fixed — release

- **The release workflow's test step runs like CI** — release.yml ran `cargo test --workspace`, which runs a test binary's tests as threads of one process; the wgpu canvas tests crashed (SIGSEGV) that way and stopped the v1.0.0-rc.1 release before any build. It now runs `cargo nextest run --workspace --profile ci` plus doctests, exactly as ci.yml's `test · workspace` does.

[Full changelog](https://github.com/alplabai/signex/compare/v0.15.0...v1.0.0-rc.2)

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
