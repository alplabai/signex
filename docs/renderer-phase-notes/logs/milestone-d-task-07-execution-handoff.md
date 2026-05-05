# Phase Note

## Metadata

- Phase: Milestone D (Preparation)
- Task ID: 07
- Task name: execution sprint handoff and readiness gate
- Owner: renderer-team
- Date: 2026-05-05
- Status: done

## Scope

Finalize the Milestone D preparation package, define the execution sprint task
ordering, verify all prerequisite decisions are locked, and document the DoR/DoD
gates for the Milestone D execution sprint.

## Execution sprint task ordering

The recommended implementation order for Milestone D execution is:

### Sprint E1 — VRML/WRL → GLB (first vertical slice)

Rationale: VRML is pre-tessellated, the parser is straightforward, and this
delivers end-to-end pipeline coverage with the lowest engineering risk.

| Seq | Task |
|-----|------|
| E1-01 | Scaffold `crates/signex-model-import/` with Cargo.toml, `lib.rs`, `error.rs` |
| E1-02 | Implement VRML lexer + parser (Tier 0 synthetic fixture passing) |
| E1-03 | Implement `IndexedFaceSet` → triangle mesh extraction |
| E1-04 | Implement GLB serializer (`glb/writer.rs`) |
| E1-05 | Implement coordinate/unit normalization (coord.rs) |
| E1-06 | Wire `import_model()` for VRML: lexer → parser → normalize → GLB writer |
| E1-07 | Add cache layer (cache.rs, cache hit/miss tests) |
| E1-08 | Integration test: Tier 1 VRML fixture round-trips cleanly |
| E1-09 | Connect `signex-renderer` ingest to `import_model()` output (optional — may land in separate PR) |

### Sprint E2 — GLTF → GLB wrapping

| Seq | Task |
|-----|------|
| E2-01 | Implement GLTF wrapper (gltf/wrap.rs, Task 04 contract) |
| E2-02 | Embed external `.bin` buffers |
| E2-03 | Embed PNG/JPEG textures (with TextureMissing warning) |
| E2-04 | Integration tests: Tier 3 and Tier 4 GLTF fixtures |

### Sprint E3 — STEP/STP (Phase A: flat faces only)

| Seq | Task |
|-----|------|
| E3-01 | Implement P21 tokenizer + entity map builder |
| E3-02 | Implement plane-face tessellation (fan triangulation of ADVANCED_FACE + PLANE) |
| E3-03 | Wire STEP path through normalize → GLB writer |
| E3-04 | Integration tests: Tier 1 STEP synthetic fixture |
| E3-05 | Document subprocess delegation fallback contract (subprocess.rs interface) |

## Definition of Ready (DoR) for Milestone D execution sprint

Before any E1 task begins, all of the following must be true:

- [x] Scope freeze (Task 01): locked. No new source formats added without scope amendment.
- [x] STEP parser contract (Task 02): P21 parsing strategy and error model documented.
- [x] VRML parser contract (Task 03): VRML97 parsing strategy and error model documented.
- [x] GLTF wrapping contract (Task 04): GLB container packing algorithm documented.
- [x] GLB normalization contract (Task 05): coordinate, unit, and mesh normalization rules locked.
- [x] Crate scaffold design (Task 06): module layout, public API, and test harness plan documented.
- [x] `signex-renderer` integration contract (Milestone C): `GlbSource` API stable, `ingest_runtime_glb` accepts bytes and path.
- [ ] `crates/signex-model-import/` directory created and registered in workspace `Cargo.toml`.
- [ ] Milestone D execution issue created in project tracker (GitHub issue or equivalent).

## Definition of Done (DoD) for Milestone D execution sprint

Sprint is complete when:

- [ ] `cargo test -p signex-model-import` passes (unit + integration tests, all fixture tiers defined in Task 06).
- [ ] `cargo test -p signex-renderer` still passes (no regression).
- [ ] `cargo clippy -p signex-model-import -- -D warnings` clean.
- [ ] VRML Tier 0 and Tier 1 fixtures produce valid GLB accepted by `ingest_runtime_glb`.
- [ ] Cache hit test: second call with same source returns cached GLB without re-converting.
- [ ] `ImportMetadata` fields are stable (round-trip: write metadata → read from GLB asset.extras → compare).
- [ ] All `ModelImportError` variants have at least one test that triggers them.

## Benchmark plan

Following the Milestone C benchmark smoke pattern, Milestone D execution should
add benchmark smoke tests in `tests/import_benchmark_smoke.rs`:

| Fixture tier | Assertion |
|--------------|-----------|
| Tier S (synthetic single triangle, VRML) | `import_model()` completes in < 100 ms on dev hardware |
| Tier M (synthetic 2-body component, VRML, ~200 faces) | completes in < 500 ms |
| Cache hit (any tier) | second call is > 10× faster than first |

Times are wall-clock assertions documented as expected ranges, not hard `assert!`
gates. Use `std::time::Instant` and log the duration; fail the test only if
duration exceeds a generous 5× multiplier over the documented expectation.

## Risk register

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| STEP curved surfaces exceed Phase A scope | High | Document as deferred; emit `UnsupportedGeometry` warning with grey fallback mesh |
| KiCad WRL uses VRML extensions not in ISO 14772 | Low | Survey KiCad library samples; document any non-standard fields found |
| Cache directory permissions on Windows | Medium | Add `CacheFailed` error path; fall back to temp dir |
| glTF requires-extension fields on community models | Medium | Log `UnsupportedGltfExtension` warning; continue without extension |

## Clean-room evidence

- Source: Milestone C contracts, Tasks 01–06 of this milestone.
- Derivation: execution ordering follows risk-minimization principle (VRML first,
  STEP Phase A limited scope, GLTF pure packaging).
- Rationale: delivering a working VRML → GLB pipeline in Sprint E1 unblocks
  `signex-renderer` 3D viewer integration without waiting for STEP tessellation.
- Clean-room check: No GPL-licensed source consulted.
- Verification: Milestone D issue Task 07 marked done; checklist updated.

## Artifacts

- PR/commit: pending
- Test output: documentation-only task
- Screenshot/benchmark: n/a

## Exit checklist

- [x] Implementation completed
- [x] Source and derivation documented
- [x] Clean-room check confirmed
- [x] Verification artifact added
- [x] Linked from issue/checklist
