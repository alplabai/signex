# Synthetic datasheet fixtures

These PDFs are **synthetic test fixtures**, not real datasheets. They are
generated programmatically by `tests/ai_stub.rs` using `pdf-writer` so we can
exercise the heuristic pinout extractor (`signex_library::ai_stub`) without
shipping any proprietary content.

| File | Provenance | Purpose |
|------|------------|---------|
| `lm317.pdf` | Synthetic — written by `tests/ai_stub.rs::generate_lm317_pdf` | Includes a textual pin table with three rows (1 ADJ, 2 OUT, 3 IN) so the heuristic can find pins with `confidence ≥ 0.7`. |
| `garbage.pdf` | Synthetic — written by `tests/ai_stub.rs::generate_garbage_pdf` | Empty/decorative content with no recognisable pin table; heuristic must report `confidence < 0.3` and an empty pin list. |

The fixtures are regenerated automatically the first time the test runs
against a missing file (via `ensure_fixtures()`), so they can be removed and
recreated by simply re-running `cargo test -p signex-library --features ai-stub`.

**Do not place real datasheets here** — they are usually copyrighted and we
explicitly avoid shipping any proprietary content with the repository.
