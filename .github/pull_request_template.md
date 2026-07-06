## Summary

Brief description of what this PR does and why.

## Changes

-

## License compliance (required — issue #62)

The main signex repo is Apache-2.0 clean. Fill in this block; CI
(`PR-description self-declaration`) checks for it.

```
Source basis:        [my own work | Signex's prior code | published format specs | other (specify)]
LLM-assisted:        [yes/no — if yes, list which models]
KiCad source consulted: [yes/no — if yes, the PR belongs in signex-kicad-import, not here]
```

If "KiCad source consulted: yes," route the contribution to the
[signex-kicad-import](https://github.com/alplabai/signex-kicad-import)
GPL-3.0 companion repo instead of this one. See
[docs/LICENSING.md](../docs/LICENSING.md) for the rationale.

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
- [ ] License-compliance block above is filled in (**self-declaration**)

Advisory (surfaced by CI, not blocking — please still keep them clean):

- [ ] `cargo fmt --all` applied
- [ ] `cargo clippy --workspace` reviewed
- [ ] New code has tests where appropriate
- [ ] Milestone is set on this PR
