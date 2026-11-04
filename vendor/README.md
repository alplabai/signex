# Vendored crates

This directory holds our own retargeted copies of two third-party crates.
They are referenced from the workspace root `Cargo.toml` by path.

## Provenance

| Crate | Upstream version | Source | Upstream repository |
|-------|------------------|--------|---------------------|
| `iced_aw` | 0.13.1 | crates.io | https://github.com/iced-rs/iced_aw |
| `iced_fonts` | 0.3.0 | crates.io | https://github.com/Redhawk18/iced_fonts |

Both were taken from the published crates.io packages, not from git.

## Why vendored

The upstream releases are pinned to iced 0.14. Signex builds against the
`alplabai/iced` fork at 0.15.0-dev, whose widget, overlay and layout APIs differ.
The crates are vendored so they can depend on the fork's `iced_core` and
`iced_widget` and carry the API port.

## Modifications

- Dependencies (`iced_core`, `iced_widget`) retargeted from iced 0.14 to the
  `alplabai/iced` fork, version 0.15.0-dev.
- `iced_aw`: source ported to the fork's API (overlay layout computed at creation,
  `flex::resolve` on widget trees, and similar changes).
- Examples, tests directories and benches removed.
- Repository tooling removed: `.github/`, `.cargo/`, `.vscode/`, `.envrc`,
  `flake.nix`, `flake.lock`, `.pre-commit-config.yaml`, `tarpaulin.toml`,
  `.gitignore`, `Cargo.lock`, `Cargo.toml.orig`, `.cargo_vcs_info.json`.
- Line endings normalized to LF.
- Compiler warnings fixed in `iced_aw`: unused variables and imports, never-read
  overlay fields, unused `Axis::Horizontal` in `menu/flex.rs` (the menu-local flex
  only lays out vertically). Unused manifest keys (`wasm_js`, `advanced`) removed.

## Licences

Both crates are MIT licensed. Each keeps its upstream `LICENSE` file
(`iced_aw/LICENSE`, `iced_fonts/LICENSE`); these must be retained. Bundled font
files in `iced_fonts/fonts` carry their own upstream licences.
