# Renderer Phase Notes

This folder is the canonical execution and evidence tracker for the clean-room schematic renderer rewrite.

## Scope boundary (schematic only)

- In scope: schematic renderer work for Phase 0 through Phase 6.
- Deferred: PCB 2D runtime renderer work.
- Deferred: PCB 3D runtime renderer work.
- Deferral rule: PCB runtime work starts only after Phase 6 exit criteria are complete.

## Phase index (0-6)

| Phase | Theme | Status | Issue | Checklist |
| --- | --- | --- | --- | --- |
| 0 | Foundation | done | [issues/phase-0-foundation-issue.md](issues/phase-0-foundation-issue.md) | [checklists/phase-0-foundation-checklist.md](checklists/phase-0-foundation-checklist.md) |
| 1 | Line and Circle Pipelines | done | [issues/phase-1-line-circle-issue.md](issues/phase-1-line-circle-issue.md) | [checklists/phase-1-line-circle-checklist.md](checklists/phase-1-line-circle-checklist.md) |
| 2 | Arc Pipeline | done | [issues/phase-2-arc-pipeline-issue.md](issues/phase-2-arc-pipeline-issue.md) | [checklists/phase-2-arc-pipeline-checklist.md](checklists/phase-2-arc-pipeline-checklist.md) |
| 3 | Polygon and Text Integration | done | [issues/phase-3-polygon-text-issue.md](issues/phase-3-polygon-text-issue.md) | [checklists/phase-3-polygon-text-checklist.md](checklists/phase-3-polygon-text-checklist.md) |
| 4 | Text Pipeline Completion | done | [issues/phase-4-text-pipeline-issue.md](issues/phase-4-text-pipeline-issue.md) | [checklists/phase-4-text-pipeline-checklist.md](checklists/phase-4-text-pipeline-checklist.md) |
| 5 | Grid and Overlay System | done | [issues/phase-5-grid-overlay-issue.md](issues/phase-5-grid-overlay-issue.md) | [checklists/phase-5-grid-overlay-checklist.md](checklists/phase-5-grid-overlay-checklist.md) |
| 6 | Integration and Hardening | done | [issues/phase-6-integration-hardening-issue.md](issues/phase-6-integration-hardening-issue.md) | [checklists/phase-6-integration-hardening-checklist.md](checklists/phase-6-integration-hardening-checklist.md) |

## Deferred milestone planning index

| Milestone | Theme | Status | Issue | Checklist |
| --- | --- | --- | --- | --- |
| B | PCB 2D preparation (Sprint B) | done | [issues/milestone-b-pcb2d-preparation-issue.md](issues/milestone-b-pcb2d-preparation-issue.md) | [checklists/milestone-b-pcb2d-preparation-checklist.md](checklists/milestone-b-pcb2d-preparation-checklist.md) |
| C (prep) | PCB 3D and model import preparation | done | [issues/milestone-c-pcb3d-model-import-issue.md](issues/milestone-c-pcb3d-model-import-issue.md) | [checklists/milestone-c-pcb3d-model-import-checklist.md](checklists/milestone-c-pcb3d-model-import-checklist.md) |
| C (exec) | PCB 3D runtime execution | done | [issues/milestone-c-pcb3d-runtime-execution-issue.md](issues/milestone-c-pcb3d-runtime-execution-issue.md) | [checklists/milestone-c-pcb3d-runtime-execution-checklist.md](checklists/milestone-c-pcb3d-runtime-execution-checklist.md) |
| D | `signex-model-import` pipeline preparation | not_started | [issues/milestone-d-model-import-issue.md](issues/milestone-d-model-import-issue.md) | [checklists/milestone-d-model-import-checklist.md](checklists/milestone-d-model-import-checklist.md) |
| E | `signex-3d-model-importer` → `signex-renderer` runtime integration | not_started | [issues/milestone-e-renderer-importer-integration-issue.md](issues/milestone-e-renderer-importer-integration-issue.md) | [checklists/milestone-e-renderer-importer-integration-checklist.md](checklists/milestone-e-renderer-importer-integration-checklist.md) |
| F | Schematic runtime cutover (`signex-render` → `signex-renderer`) | not_started | [issues/milestone-f-schematic-runtime-cutover-issue.md](issues/milestone-f-schematic-runtime-cutover-issue.md) | [checklists/milestone-f-schematic-runtime-cutover-checklist.md](checklists/milestone-f-schematic-runtime-cutover-checklist.md) |

## Numbering and naming rules

Use these conventions to keep phase tracking deterministic and audit-friendly.

1. Each phase has exactly one issue file: `issues/phase-N-*-issue.md`.
2. Each phase has exactly one checklist file: `checklists/phase-N-*-checklist.md`.
3. Implementation task logs use numbered task IDs: `logs/phase-N-task-01-*.md` to `logs/phase-N-task-04-*.md`.
4. Supplemental validation notes that are not part of task 01-04 use: `logs/phase-N-smoke-*.md` with metadata `Task ID: SMOKE`.
5. If a file does not match these patterns, rename or reclassify it before phase closure.
6. Deferred milestone planning files use: `issues/milestone-b-*-issue.md`, `checklists/milestone-b-*-checklist.md`, and `logs/milestone-b-task-01-*.md` onward.

## Folder layout

- [templates/phase-note-template.md](templates/phase-note-template.md): reusable template for each phase task note
- [templates/clean-room-module-header-template.md](templates/clean-room-module-header-template.md): clean-room header template
- [templates/commit-derivation-checklist.md](templates/commit-derivation-checklist.md): commit-level derivation checklist
- [issues](issues): phase execution issues
- [checklists](checklists): compact phase execution checklists
- [logs](logs): implementation and smoke evidence notes

## Required evidence format

Each completed task note must include:

- Source: public standard, document, or internal decision source
- Derivation: mapping, formula, or implementation reasoning
- Rationale: why this approach was selected
- Clean-room check: confirmation that no GPL-licensed source was consulted
- Verification: tests, smoke output, benchmark, or build evidence

## Usage

1. Create a new note under [logs](logs) from [templates/phase-note-template.md](templates/phase-note-template.md).
2. Fill all required evidence fields.
3. Link the note from the corresponding phase issue and checklist entries.
4. Keep entries concise and reviewable.
