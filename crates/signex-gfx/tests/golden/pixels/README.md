# Pixel goldens

`<scene>.png` files (128x128 RGBA8) are the reference frames for
`tests/pixel_goldens.rs`: `line_circle`, `arc`, `polygon`, `text`, `grid`.

## How they are compared

- **Mesa lavapipe/llvmpipe** (Linux CI `test · workspace`): strict. Max
  per-channel diff <= 64 and mean absolute error <= 1.5 (both on 0-255).
- **Any other adapter** (WARP on Windows, Metal on macOS, a real GPU): coarse
  invariants only — frame not all black, the scene's dominant colours present.
  Those rasterizers differ at edges, so exact references would be flaky.
- A missing reference file skips the strict compare with a loud `SKIP` line.
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

## Generating via CI

While references are missing, the Linux `test · workspace` job sets
`SIGNEX_GOLDEN_CANDIDATES` and the test writes each rendered lavapipe frame to
that directory as `<scene>.png` (the source tree is untouched). The job uploads
it as the `pixel-golden-candidates` artifact. Download it from the PR run,
review the images, and commit them into `tests/golden/pixels/`.

Text uses only `tests/fonts/*.ttf`, never system fonts.
