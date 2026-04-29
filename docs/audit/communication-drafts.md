# Communication drafts — issue #62

Phase 7 of the Apache-clean remediation. Drafts of the public messages
to apply manually after the cutover lands. Apply each via the listed
channel; the autonomous remediation run can't push from this repo.

---

## 1. Reply to issue #62 (post on the GitHub issue itself)

```markdown
@sethhillbrand — quick update on issue #62.

We've completed the Apache-clean remediation. The main signex repo no
longer contains KiCad-derived code; KiCad I/O lives in a separate
GPL-3.0-or-later companion tool at https://github.com/alplabai/signex-kicad-import.

Specifically:

- `kicad-parser` and `kicad-writer` crates moved to the companion repo
  under GPL-3.0-or-later (no longer in the main workspace).
- `signex-output/src/netlist/kicad_sexpr.rs` (KiCad netlist exporter)
  moved out of the main repo.
- KiCad-flavoured types in `signex-types` removed and replaced with
  Signex-curated designs:
  - `PinElectricalType` (12-variant, KiCad-shaped) → `PinDirection`
    (14 variants — adds `GroundReference`, `Differential`, `Clock` as
    Signex-original; rationale in `crates/signex-types/docs/pin-design.md`).
  - `PinShape` → `PinShapeStyle` (7 variants vs your 9; drops
    per-direction-low modifiers since polarity is now on `PinDirection`).
  - `LayerId(u8)` numeric constants (pre-KiCad-7 numbering) → semantic
    `SignexLayer` enum.
  - KiCad markup parser (`~{}` / `^{}` / `_{}` curly-brace syntax) →
    Markdown-extension `parse_signex_markup` (`**bold**`, `*italic*`,
    `~~strike~~`, `^sup^`, `~sub~`, `_~overbar~_`, `[label](url)`).
  - `kicad_auto_net_name_from_pins` returning `Net-(<r>-Pad<p>)` →
    `auto_net_name` returning `unnamed-<sheet>:<ref>:<pin>`. The KiCad
    format string is gone from the main repo.
- Two CI gates added to keep it that way:
  - `.github/workflows/license-guard.yml` fails any push/PR
    introducing KiCad-derived identifiers in `crates/`.
  - `deny.toml` + `cargo deny check licenses` rejects GPL transitive
    deps from the main workspace's dependency closure.
- v0.7.0 / v0.7.1 / v0.8.0 release notes flagged with the licensing
  notice pointing here.
- v0.9.0 ships the first Apache-clean main binary alongside
  `signex-kicad-import` v0.1.0 for users with existing KiCad projects.

The audit trail lives at:
- `docs/audit/kicad-derivation.md` — file-by-file audit
- `.claude/PRPs/issue-62-licensing-remediation.md` — strategy doc
- `.claude/PRPs/issue-62-execution-plan.md` — engineering plan +
  decision log
- `docs/LICENSING.md` — public licensing doc

Happy to take feedback on whether this resolves the concern. Will
close #62 once you confirm.

Thanks for raising it cleanly — the two-repo structure ends up
healthier than the original "Apache-2.0 everywhere" claim.

— Caner
```

**Posting:** Comment on https://github.com/alplabai/signex/issues/62
once v0.9.0 ships and the companion repo is public.

---

## 2. signex.dev website update (separate website repo at `../signex-website`)

The current homepage tagline is along the lines of "KiCad-compatible
schematic and PCB editor." Replace with framing that emphasises native
formats + optional KiCad migration.

**Suggested replacement copy** (drop into the hero section):

```markdown
# Signex

Open-source EDA tooling built in Rust with GPU-accelerated rendering and
an Altium Designer-quality UI. Native, line-diffable file formats.
Available as Apache-2.0 Community Edition or subscription Pro Edition.

## Migrating from KiCad?

Signex reads and writes its own native `.snxsch` / `.snxpcb` formats.
For users with existing KiCad projects, the optional
[signex-kicad-import](https://github.com/alplabai/signex-kicad-import)
companion tool (GPL-3.0-or-later, distributed independently) converts
`.kicad_pro` projects to Signex format one-way. Run it once; open the
result in Signex from then on.
```

**Apply by:** editing the website's relevant page (homepage hero +
licensing page), commit, push to the website's hosting (typically a
Hostinger SSH alias). Deploy.

---

## 3. GitHub Discussions sticky / pinned post

Title: **"v0.9 ships Apache-clean — KiCad migration via companion tool"**

Body:

```markdown
Hi everyone — heads-up about the v0.9 release.

Per [issue #62](https://github.com/alplabai/signex/issues/62), the main
signex repo is now Apache-2.0 clean. KiCad import / export moves to a
separate optional companion tool at
https://github.com/alplabai/signex-kicad-import (GPL-3.0-or-later).

**TL;DR for existing users:**

1. v0.7.x and v0.8.0 binaries are flagged superseded — please install
   v0.9.0 or later for the Apache-clean binary.
2. If you've been opening `.kicad_sch` files directly, the new flow is:
   download `signex-kicad-import` from
   https://github.com/alplabai/signex-kicad-import/releases, run it
   once against your `.kicad_pro`, then open the resulting `.snxprj`
   in Signex. Conversion is one-way and keeps your originals intact.
3. Native `.snxsch` / `.snxpcb` formats are now the canonical Signex
   formats — TOML envelope + TSV bulk blocks, line-diffable in git,
   ~5× smaller than JSON.

Background and rationale are in [docs/LICENSING.md](https://github.com/alplabai/signex/blob/dev/docs/LICENSING.md).
Questions / migration friction reports / general discussion: drop them
in this thread.

Thanks again to Seth Hillbrand of the KiCad project for raising the
licensing issue cleanly — the two-repo structure is a healthier
outcome than what we had before.
```

**Apply by:** sign in to GitHub Discussions for `alplabai/signex`,
create a new "Announcements" discussion, paste, pin.

---

## 4. Discord announcement (if there's a Signex Discord)

Short version (Discord posts read shorter than GitHub):

```
📣 Signex v0.9 ships Apache-clean

Per issue #62, the main repo is now Apache-2.0 clean. KiCad I/O
moves to an optional companion tool: signex-kicad-import (GPL-3.0).

What this means:
• v0.7/v0.8 binaries flagged superseded — please install v0.9+.
• Migrating from KiCad? Download signex-kicad-import, run once,
  open the resulting .snxprj in Signex.
• Native .snxsch / .snxpcb formats are the canonical Signex
  formats — line-diffable, ~5× smaller than JSON.

Details: https://github.com/alplabai/signex/blob/dev/docs/LICENSING.md
Companion: https://github.com/alplabai/signex-kicad-import
```

**Apply by:** post in the announcements channel; pin if appropriate.

---

## 5. Email to signex.dev mailing list / newsletter (if one exists)

Skip if there's no list. Otherwise mirror the GitHub Discussions post
above with a slightly more formal tone. Keep under 300 words.

---

## Apply order

1. Ship v0.9.0 to main (tag `v0.9.0`, run `release.yml`).
2. Tag v0.1.0 on the companion repo (`signex-kicad-import`); run its
   release workflow to produce binaries.
3. Edit v0.7.0 / v0.7.1 / v0.8.0 release bodies on GitHub UI to add
   the licensing notice (see `docs/audit/release-notes-remediation-v07-v08.md`).
4. Post the issue #62 reply (above).
5. Update signex.dev hero + licensing page; deploy.
6. Pin the GitHub Discussions announcement.
7. Post the Discord announcement (if applicable).
8. Optionally, the mailing list note.
9. Wait for Seth's confirmation reply on issue #62 → close the issue.

Total time: about 1 hour of manual application once the v0.9 release
is in flight.
