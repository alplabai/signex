# UI Iteration Workflow

This is how UI / chrome work gets done in Signex without burning round-trips on visual guesswork. Adopted after the v0.8 chrome batch took 6+ revisions to land tab styling.

## Three rules

1. **Spec the change before touching code.** Drop an annotated screenshot or a short bullet list with concrete numbers (corner radius in px, border colour as hex, padding, font weight, hover/active fills). Words like "rounded a bit" / "match Altium" / "more accent" are imprecise — every interpretation costs a build cycle.

2. **Iterate inside the chrome catalog, not the full app.** `cargo run -p chrome-catalog` mounts every chrome variant side-by-side (tabs in 5 states × 7 themes, modal cards, project tree rows, …) in a small dev-built binary that rebuilds in seconds. The full app takes ~90 s on a release rebuild + a project to open + panels to arrange.

3. **Use dev builds for chrome work.** Release builds are required for canvas perf testing on the AMD 880M, but chrome iteration doesn't render canvases. `cargo run -p chrome-catalog` (~10–15 s incremental) is the right loop for tab / panel / modal / button styling.

## When to launch the full app

- After the catalog signs off and you want to verify the chrome lands correctly inside the real layout (panels, undock, modals interacting with each other).
- When the change involves canvas, panels with live data, or anything that needs an open KiCad project.
- Final smoke pass before commit.

## Adding a new variant to the catalog

`crates/chrome-catalog/src/main.rs` is structured as one `section(...)` per widget family. To add a new variant:

1. Write a small helper like `tab(...)` or `tree_row(...)` that returns an `Element<'_, Message>` for one variant.
2. Wrap a row of those variants in a `section("Title", tokens, body)` call.
3. Push the section into the `view` `column!`.

Theme switching is global — just pull from the `tokens: &ThemeTokens` argument and the right colour shows up automatically when the user clicks a theme pill.

## When the catalog disagrees with the app

The catalog is canonical for the widget appearance. If the same widget looks wrong in the full app but right in the catalog, the bug is in the surrounding container / padding / layout in the app — not the widget itself. Fix it at the call site.

## No commits during iteration

The catalog is a sandbox. Iterating on a widget should stay in the working tree until the design is signed off — then a single commit lands the final version. Committing every visual tweak pollutes branch history with WIP styling and defeats the rapid-feedback purpose of the catalog.
