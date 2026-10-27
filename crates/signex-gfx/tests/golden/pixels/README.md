# Pixel goldens

`<scene>.png` files (128x128 RGBA8) are the reference frames for
`tests/pixel_goldens.rs`: `line_circle`, `arc`, `polygon`, `text`, `grid`.

## How they are compared

- **Mesa lavapipe/llvmpipe** (Linux CI `test · workspace`): strict. Max
  per-channel diff <= 64 and mean absolute error <= 1.5 (both on 0-255).
- **Any other adapter** (WARP on Windows, Metal on macOS, a real GPU): coarse
  invariants only — frame not all black, the scene's dominant colours present.
  Those rasterizers differ at edges, so exact references would be flaky.
- The references are committed. CI sets `SIGNEX_REQUIRE_LAVAPIPE=1`, which fails
  closed: a non-lavapipe adapter or a missing reference fails the job.
- Locally (flag unset) a missing reference skips the strict compare with a loud
  `SKIP` line.
- No adapter at all fails the test.

## Regenerating

References must be produced on lavapipe; the test refuses to write them from
any other adapter.

```text
SIGNEX_UPDATE_GOLDENS=1 cargo test -p signex-gfx --test pixel_goldens -- --nocapture
```

On Linux: `sudo apt-get install mesa-vulkan-drivers`, and if a hardware GPU is
also present, `VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.x86_64.json`
(path varies by distro) so lavapipe is the adapter wgpu picks. Review the PNGs
before committing; regenerate only for an intended rendering change.

## Adding or changing a scene via CI

1. Add the scene (or change it) and push. The Linux `test · workspace` job
   renders on lavapipe, writes each missing reference to
   `target/golden-candidates/<scene>.png` (source tree untouched), then fails.
2. The job uploads that directory as the `pixel-golden-candidates` artifact
   (also on failure). Download it from the run and review the images.
3. Commit them into `tests/golden/pixels/` and push. For a changed scene whose
   reference drifted, delete the old PNG first to get a fresh candidate (or
   regenerate locally as above), review, commit.

Text uses only `tests/fonts/*.ttf`, never system fonts.
