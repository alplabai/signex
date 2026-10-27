# Library link of a placed symbol in `.snxsch` (#1338, #1331)

A symbol placed from a library row remembers which row it came from and the
row version it is pinned to. The link is what the Library Updates flow and
drift detection compare against the library's current row, so losing it on
save silently turns both off for that symbol.

## Stored fields

| Field | On disk (`[extras.symbols.<uuid>]`) |
|---|---|
| `Symbol.library_id` | `library_id = "<uuid>"` |
| `Symbol.library_version` | `library_version = "1.0.2"` |
| `Symbol.row_id` | `row_id = "<uuid>"` |

Each key is written only when set (`library_id` / `row_id` not `None`,
`library_version` not empty). A symbol with no link adds nothing to the file,
so a file with no linked symbols is byte-identical to what a build without the
keys wrote. `SymbolExtras::is_default` counts the three fields, otherwise a
symbol whose only extras are the link would be elided on save. A file without
the keys loads with `library_id = None`, `row_id = None` and an empty version,
as before.

## Format decision

No format-token bump: the token stays `snxsch/1`. `[extras.symbols.<uuid>]`
keeps unknown keys, so a build that predates the fields keeps the link when it
opens and re-saves the file (the same placement as the font families in
`snxsch-font-family.md`). The fields are declared in alphabetical key order
because an older build re-emits unknown keys sorted; this makes its re-save of
our file byte-identical when the symbol carries no other unknown extras keys.
With other unknown keys present the key order can differ, but the link is
still kept. A hand-edited `library_id` or `row_id` that is not a UUID makes the
sheet fail to load, the same as a malformed `pin_uuids` entry.

`crates/signex-types/tests/fixtures/library_link/` holds a file this build
writes and the same file after trunk at `e5fc9378` (no link keys) parsed and
re-saved it; `format/library_link_tests.rs` asserts the link survives and the
bytes match. The tests' `regenerate_link_fixtures` explains how to rebuild them.
