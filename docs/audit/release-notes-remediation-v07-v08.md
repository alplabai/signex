# Release-note remediation drafts — v0.7.0 / v0.7.1 / v0.8.0

Phase 6 of the issue-62 Apache-clean remediation. The signex
v0.7.0 and v0.8.0 binaries shipped to GitHub Releases under
Apache-2.0 contained KiCad-derived code (`crates/kicad-parser`,
`crates/kicad-writer`, `crates/signex-output/src/netlist/kicad_sexpr.rs`)
that should have been released under KiCad's reciprocal terms. Per
issue [#62](https://github.com/alplabai/signex/issues/62), the fix
is to flag those releases retroactively with the licensing notice
below.

**Apply manually** by editing each release's body in the GitHub UI
(or via `gh release edit`). The signex repo can't push from the
autonomous remediation run.

---

## v0.7.0 / v0.7.1 — addendum to release notes

Insert at the **top** of the existing release body:

```markdown
> ⚠️ **Licensing notice (added 2026-04-29)**
>
> This release contained KiCad-derived code (the `kicad-parser` and
> `kicad-writer` crates, plus the KiCad netlist exporter in
> `signex-output`) shipped under Apache-2.0 in error. Those parts of
> the codebase derive from KiCad's GPL-3.0 source and should have
> been released under KiCad's reciprocal terms.
>
> From v0.9.0 onwards, KiCad I/O is moved to a separate
> GPL-3.0-licensed companion tool: [signex-kicad-import](https://github.com/alplabai/signex-kicad-import).
> The main signex repository is Apache-2.0 clean and contains no
> KiCad-derived code.
>
> See issue [#62](https://github.com/alplabai/signex/issues/62) for
> the audit, the remediation plan, and the cutover commits.
>
> **What this means for you:**
> - This release **remains available** for historical use, but
>   please prefer v0.9.0 (or later) for new installations.
> - If you have existing KiCad project files, install the
>   companion tool [signex-kicad-import](https://github.com/alplabai/signex-kicad-import/releases)
>   alongside Signex Community to convert them to the native
>   `.snxsch` / `.snxpcb` formats.
> - Apache consumers of Signex Community Edition (anyone embedding
>   or linking against it) get a clean Apache codebase from v0.9.0
>   forward — no GPL aggregation in their build closure.
```

---

## v0.8.0 — addendum to release notes

Insert at the **top** of the existing release body (same wording, version-stamped):

```markdown
> ⚠️ **Licensing notice (added 2026-04-29)**
>
> This release contained KiCad-derived code (`kicad-parser`,
> `kicad-writer`, KiCad netlist exporter) shipped under Apache-2.0
> in error. Those parts derive from KiCad's GPL-3.0 source and
> should have been released under KiCad's reciprocal terms.
>
> From v0.9.0 onwards, KiCad I/O lives in a separate GPL-3.0
> companion tool: [signex-kicad-import](https://github.com/alplabai/signex-kicad-import).
> The main signex repository is Apache-2.0 clean.
>
> See issue [#62](https://github.com/alplabai/signex/issues/62) for
> context. This release stays available for historical use; prefer
> v0.9.0 (or later) for new installations. Existing KiCad project
> files convert via the companion tool.
```

---

## v0.9.0 — release notes (when it ships)

The v0.9.0 release notes will document the cutover. Suggested top-of-body block:

```markdown
## Apache-2.0 clean cutover (issue #62)

This release is the first one with no KiCad-derived code in the main
binary. KiCad I/O moves to the optional [signex-kicad-import](https://github.com/alplabai/signex-kicad-import)
companion tool (GPL-3.0-or-later), shipped independently.

### What changed

- **New native formats:** Signex now reads and writes its own native
  `.snxsch` (schematic) and `.snxpcb` (PCB) formats, both built on
  the TOML envelope + TSV bulk-block pattern shared with `.snxlib` /
  `.snxsym` / `.snxfpt`. Files are line-diffable in git, ~5× smaller
  than the equivalent JSON, and editable by hand if needed.
- **No more direct .kicad_sch / .kicad_pcb opening** in the main
  binary. To migrate existing KiCad projects, install the optional
  companion tool [signex-kicad-import](https://github.com/alplabai/signex-kicad-import)
  and run it against your `.kicad_pro` once — it produces sibling
  `.snxsch` / `.snxpcb` / `.snxprj` files that Signex Community
  reads natively.
- **Pin types redesigned.** The `PinElectricalType` enum (12
  variants, KiCad-shaped) is replaced by `PinDirection` (14
  variants) with Signex-original additions for `GroundReference`,
  `Differential`, and `Clock`. See `crates/signex-types/docs/pin-design.md`
  for the rationale.
- **Layer model redesigned.** The numeric `LayerId(u8)` constants
  (`F_CU=0`, `B_CU=31`, ...) are replaced by the semantic
  `SignexLayer` enum.
- **Markup language replaced.** The KiCad curly-brace markup
  (`~{X}`, `^{X}`, `_{X}`) is replaced by a Markdown-extension
  format: `**bold**`, `*italic*`, `~~strike~~`, `^superscript^`,
  `~subscript~`, `_~overbar~_` (Signex extension for active-low
  signal naming), `[label](url)` for links.

### Migration

If you have existing KiCad projects:

1. Download the [signex-kicad-import](https://github.com/alplabai/signex-kicad-import/releases)
   companion tool for your platform.
2. Run it against your `.kicad_pro`:
   ```
   signex-kicad-import path/to/project.kicad_pro
   ```
3. Open the produced `.snxprj` in Signex.

The companion tool keeps your original `.kicad_sch` / `.kicad_pcb`
files in place; conversion is one-way (KiCad → Signex) and does not
modify the originals.

### Why two repos?

Signex Community is Apache-2.0. KiCad's source is GPL-3.0, and
file-format implementations derived from it are subject to KiCad's
reciprocal terms. To preserve a clean Apache codebase for Signex,
KiCad I/O ships as an optional GPL-3.0 companion. Apache consumers
of Signex Community see no GPL aggregation in their build closure.

Thanks to [@sethhillbrand](https://github.com/sethhillbrand) for
raising the licensing issue cleanly and giving us a chance to fix
the code rather than scrub it.
```

---

## Release-table flags (suggested layout for the README's release table)

If the README has a release table, add a "Status" column flagging
v0.7.x and v0.8.0 as superseded:

| Version | Date | Status | Notes |
|---|---|---|---|
| v0.7.0 | 2026-04-22 | ⚠️ superseded | Contained KiCad-derived code; install v0.9+ for Apache-clean binary. |
| v0.7.1 | 2026-04-24 | ⚠️ superseded | macOS Apple-Silicon ad-hoc codesign hotfix; still contains KiCad-derived code. |
| v0.8.0 | 2026-04-26 | ⚠️ superseded | Output generation; still contains KiCad-derived code. |
| v0.9.0 | (this release) | ✅ Apache-clean | First Apache-2.0-clean binary. Bundles the library subsystem (Library Browser + SCH Library editor + Component Editor) that was in flight on `feature/v0.9-snxlib-as-file` plus the Apache-clean cutover. KiCad I/O via optional companion. |

(Note: v0.9.0 combines two streams — the library subsystem work was
paused mid-flight for the issue-62 licensing remediation, then resumed
on top of the Apache-clean foundation. Both ship in this release.)

---

## Process notes for whoever applies these (Caner)

- The licensing notice goes at the **top** of each release body
  before the existing changelog, not at the bottom.
- GitHub Releases supports markdown — copy/paste should render the
  blockquote and admonition emoji as expected.
- After the v0.9.0 release ships, issue [#62](https://github.com/alplabai/signex/issues/62)
  can be closed with a final comment linking the v0.9.0 tag and
  the cutover commits.
- The signex.dev website + Discord sticky also need the matching
  framing update — see Phase 7 communication drafts in
  `docs/audit/communication-drafts.md`.
