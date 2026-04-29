## Summary

Brief description of what this PR does and why.

## Changes

- 

## License compliance (required — issue #62)

The main signex repo is Apache-2.0 clean. Fill in this block; CI
checks for it.

```
Source basis:        [my own work | Signex's prior code | published format specs | other (specify)]
LLM-assisted:        [yes/no — if yes, list which models]
KiCad source consulted: [yes/no — if yes, the PR belongs in signex-kicad-import, not here]
```

If "KiCad source consulted: yes," route the contribution to the
[signex-kicad-import](https://github.com/alplabai/signex-kicad-import)
GPL-3.0 companion repo instead of this one. See
[docs/LICENSING.md](../docs/LICENSING.md) for the rationale.

## Crates affected

- [ ] signex-types
- [ ] signex-engine
- [ ] signex-render
- [ ] signex-widgets
- [ ] signex-erc / signex-erc-dsl
- [ ] signex-output
- [ ] signex-app

## Checklist

- [ ] `cargo build --workspace` compiles
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace -- -D warnings` is clean
- [ ] `cargo deny check licenses` passes (no GPL transitive deps)
- [ ] License compliance block above is filled in
- [ ] New code has tests where appropriate
- [ ] Milestone is set on this PR
