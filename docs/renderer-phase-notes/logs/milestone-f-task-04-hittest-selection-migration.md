# Phase Note

## Metadata

- Phase: Milestone F (Execution)
- Task ID: 04
- Task name: hit-test and selection workflow migration
- Owner: renderer-team
- Date: 2026-05-06
- Status: done

## Scope

Close the hit-test/selection migration slice by confirming all app selection
entry points route through the app-local schematic runtime bridge and by adding
explicit parity tests for core selection behavior.

## Implementation summary

- Confirmed app-side hit-test callsites route through
  `crate::schematic_runtime::hit_test` bridge APIs.
- Added focused parity tests in
  `crates/signex-app/src/schematic_runtime.rs` for:
  - wire segment tolerance hit-testing
  - `Inside` vs `Touching` rectangle selection behavior
  - polygon selection anchored item capture (wire + label)

## Verification

Commands:

```text
rg -n "crate::schematic_runtime::hit_test::" crates/signex-app/src/app crates/signex-app/src/canvas/mod.rs
cargo test -p signex-app schematic_runtime::tests
```

Result:

- Hit-test callsites in canvas/handlers/dispatch resolve to
  `crate::schematic_runtime::hit_test::*`.
- `cargo test -p signex-app schematic_runtime::tests`: pass
  (`3 passed; 0 failed`).

## Clean-room evidence

- Source: app contract and Milestone F issue requirements.
- Derivation: runtime bridge behavior validated from app-local APIs and tests.
- Clean-room check: no legacy GPL schematic runtime source used in this slice.

## Exit checklist

- [x] Hit-test callsites moved to app runtime bridge
- [x] Selection mode parity checks added and passing
- [x] Evidence log added
