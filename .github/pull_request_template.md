## Summary

Brief description of what this PR does and why.

## Changes

-

## License compliance (issue #62)

- [ ] This is original work, or derived only from sources whose licence I read
      and confirmed Apache-2.0-compatible.

**Derived from something? Name the source and its licence:** _(one line, or `n/a`)_

A port is a derivative work — rewriting a project in Rust does not reset its
licence. GPL/copyleft, **any** Creative Commons licence (CC BY included), and any
"non-commercial" / "no resale" term are all incompatible here; Signex Pro is sold
from this source. Full list:
[CONTRIBUTING.md](../CONTRIBUTING.md#license-compliance-for-contributions).
Unsure about a source? [Ask in an issue](https://github.com/alplabai/signex/issues/new) —
that's cheaper than finding out at review. If it *is* license-gated, add a line
`License-gated sources: yes` and CI routes it to the GPL-3.0 companion repo
[signex-kicad-import](https://github.com/alplabai/signex-kicad-import).

## Labels

`area:` labels are applied automatically from the changed paths. Please add:

- a **`type:`** label (feature / bug / refactor / docs / ci / chore …)
- a **`priority:`** label if it's more or less than routine
- **`data-loss`**, **`regression`**, or **`breaking-change`** if they apply

See [`.github/labels.yml`](labels.yml) for the full taxonomy.

## Crates affected

- [ ] signex-types
- [ ] signex-engine
- [ ] signex-library / signex-library-server
- [ ] signex-sketch
- [ ] signex-bake
- [ ] signex-erc / signex-erc-dsl
- [ ] signex-bom
- [ ] signex-output
- [ ] signex-renderer / signex-gfx
- [ ] signex-3d-model-importer
- [ ] signex-widgets / chrome-catalog
- [ ] signex-app

## Checklist

Hard CI gates (must pass to merge):

- [ ] `cargo check --workspace` compiles (**Check**)
- [ ] `cargo test --workspace` passes (**Test**)
- [ ] `cargo deny check licenses` clean — no GPL transitive deps (**License audit**)
- [ ] License-compliance question above is answered (**self-declaration**)

Advisory (surfaced by CI, not blocking — please still keep them clean):

- [ ] `cargo fmt --all` applied
- [ ] `cargo clippy --workspace` reviewed
- [ ] New code has tests where appropriate
- [ ] Milestone is set on this PR
