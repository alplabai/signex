# UI audit harness

`tests/audit.rs` opens every audited UI surface (one per mockup row) in a
fresh app, renders it headlessly in Graphite Light and Dark, and runs the
row's checks through `Signex::update`. It is `#[ignore]`d: it needs a wgpu
adapter, and skips with a note when there is none.

## Running

```text
AUDIT_DIR=out cargo test -p signex-app --test audit -- --ignored --nocapture
```

- `AUDIT_DIR`: output directory (default `<temp>/signex-audit`). It gets
  `<id>-<light|dark>.png` and `audit.jsonl`, one line per row and theme:
  `{id, theme, opened, checks: [{name, ok, err}], error}`. The file is
  appended to, so clear it between runs.
- `AUDIT_ONLY`: comma list of row ids (`m42,m75`) or group names (`pcb`).
- `AUDIT_THEMES`: `light`, `dark` or both (default both).

On the build pool, run the same command as a pool job. A node with no GPU
needs a software Vulkan driver (lavapipe) or the run skips.

## Adding a row

1. Find the row's group in `surfaces/` (`global`, `project`, `sch_a`,
   `sch_b`, `annot_valid`, `pcb`, `output_prefs`, `lib`, `new_features`);
   its header comment lists the rows still to add.
2. Add a `Surface` to that file's `rows()`: `id` is the mockup id (`"m42"`,
   `"m122b"`), `open` drives a fresh app there with `fixture.rs` helpers and
   `Signex::update`, `window` picks the captured window (`Auto` takes a
   detached dialog when one opened), `checks` are named
   `fn(&mut Signex) -> Result<(), String>`.
3. Remove the row from the header comment. Checks shared by several groups
   go in `surfaces/checks.rs`.

A further state of a row is an entry in `surfaces/states.rs` with the id
`<row>-<state>`, written as `<row>-<state>-<theme>.png`. Run them all with
`AUDIT_ONLY=states`. Its `open` fails when the state was not reached.

`render.rs`, `fixture.rs` and `surface.rs` are shared with
`graphite_screens.rs`, which renders a fixed list of these rows by name.
