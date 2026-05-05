# Phase Note

## Metadata

- Phase: Milestone F (Execution)
- Task ID: 02
- Task name: app compatibility bridge for schematic runtime contracts
- Owner: renderer-team
- Date: 2026-05-05
- Status: done

## Scope

Introduce a schematic runtime bridge module in `signex-app` and migrate all
app source modules to use that bridge instead of directly calling
`signex_render::schematic`.

## Implementation summary

- Added bridge module:
  - `crates/signex-app/src/schematic_runtime.rs`
- Exposed bridge at crate root:
  - `crates/signex-app/src/lib.rs` (`pub mod schematic_runtime;`)
- Migrated direct callsites:
  - Replaced `signex_render::schematic::...` with
    `crate::schematic_runtime::...` across app modules.

Post-migration verification command:

```text
rg -n "signex_render::schematic::" crates/signex-app/src
```

Result:

- Remaining direct references: only inside the bridge module.
- No direct `signex_render::schematic` references in other app source modules.

Migrated module set (13 files):

- `crates/signex-app/src/canvas/mod.rs`
- `crates/signex-app/src/panels/mod.rs`
- `crates/signex-app/src/app/bootstrap.rs`
- `crates/signex-app/src/app/state.rs`
- `crates/signex-app/src/app/load_gateway.rs`
- `crates/signex-app/src/app/mutation_gateway.rs`
- `crates/signex-app/src/app/dispatch/mod.rs`
- `crates/signex-app/src/app/dispatch/text_edit.rs`
- `crates/signex-app/src/app/handlers/canvas.rs`
- `crates/signex-app/src/app/handlers/erc.rs`
- `crates/signex-app/src/app/handlers/selection_workflow.rs`
- `crates/signex-app/src/app/handlers/dock/property_editor.rs`
- `crates/signex-app/src/app/view/dialogs.rs`

## Cutover impact

- Direct legacy runtime usage is now centralized and controllable via one module.
- Future Task 03/04 swaps can target bridge internals first, minimizing
  multi-file churn and regression risk.

## Clean-room evidence

- Source: Milestone F Task 02 scope from issue/checklist.
- Derivation: centralized bridge pattern used to isolate legacy runtime surface.
- Rationale: prepare low-risk incremental replacement of bridge internals with
  `signex-renderer` APIs while preserving app behavior.
- Clean-room check: No GPL-licensed source consulted.
- Verification: callsite scan confirms only bridge module retains direct
  `signex_render::schematic` imports.

## Artifacts

- PR/commit: pending
- Test output: pending (covered in Task 07 validation pass)
- Screenshot/benchmark: n/a

## Exit checklist

- [x] Implementation completed
- [x] Source and derivation documented
- [x] Clean-room check confirmed
- [x] Verification artifact added
- [x] Linked from issue/checklist
