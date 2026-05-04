# Renderer Phase Notes

This folder tracks clean-room implementation evidence and execution status for the schematic-first renderer rewrite.

## Folder layout

- templates/phase-note-template.md: reusable note template for each implementation task
- issues/phase-0-foundation-issue.md: Phase 0 execution issue in checklist format
- checklists/phase-0-foundation-checklist.md: compact execution checklist
- logs/: per-task notes created from the template

## Required evidence format

Each completed task must include the following fields:

- Source: standard section or public specification
- Derivation: direct mapping, formula, or design decision
- Rationale: why the approach/value was chosen
- Clean-room check: confirmation that no GPL-licensed source was consulted
- Verification: test name, screenshot id, benchmark id, or build output

## Usage

1. Create a new note under logs/ from templates/phase-note-template.md.
2. Fill all required evidence fields.
3. Link the note from the corresponding issue/checklist item.
4. Keep entries concise and audit-friendly.
